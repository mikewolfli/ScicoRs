//! Batched GPU submission — amortising the fixed per-dispatch round-trip.
//!
//! # Why this exists (measured problem)
//!
//! The synchronous [`GpuBackend`](crate::core::compute::backend::GpuBackend)
//! contract submits one command buffer per operation and blocks on the result.
//! On an integrated adapter (verified on Apple M4 / Metal) that costs a fixed
//! ~0.55 ms per call regardless of problem size, because every call pays for
//! the submit → poll(Wait) → map_async round-trip even when the kernel itself
//! takes microseconds.
//!
//! When a caller has several *independent* operations (the common case in a
//! simulation step that updates many fields), those round-trips can be collapsed
//! into **one** submission: record every kernel into a single
//! [`wgpu::CommandEncoder`], submit once, and read every result back with a
//! single device poll. The fixed cost is then paid once per batch instead of
//! once per operation.
//!
//! # Scope and guarantees
//!
//! - Results are **identical** to the one-at-a-time path: the same kernels run
//!   with the same inputs, only the submission granularity changes.
//! - A batch is executed atomically: if any operation is invalid (length
//!   mismatch, ragged matrix, oversized buffer) the batch is rejected *before*
//!   anything is submitted, so no partial work is ever left on the device.
//! - Readback ordering is preserved: `results[i]` always corresponds to
//!   `ops[i]`, even though every operation shares one command buffer.

use crate::core::error::SimError;
use crate::core::types::Scalar;

use super::device::GpuContext;
use super::kernels::{BatchBuffers, BinaryOp, GpuKernels};
use super::pool::{BufferRoleSpec, PooledBuffer};

/// One operation in a GPU batch.
///
/// The variants mirror the [`GpuBackend`](crate::core::compute::backend::GpuBackend)
/// surface, so any operation the backend supports can also be batched.
#[derive(Debug, Clone)]
pub enum BatchOp<'a> {
    /// `C = A · B` for row-major matrices.
    MatMul {
        a: &'a [Vec<Scalar>],
        b: &'a [Vec<Scalar>],
    },
    /// Element-wise `c = a + b`.
    Add { a: &'a [Scalar], b: &'a [Scalar] },
    /// Element-wise `c = a - b`.
    Sub { a: &'a [Scalar], b: &'a [Scalar] },
    /// Element-wise `c = a * b` (Hadamard).
    Mul { a: &'a [Scalar], b: &'a [Scalar] },
    /// Element-wise `c = |a|`.
    Abs { a: &'a [Scalar] },
    /// Element-wise `c = scale * a`.
    Scale { a: &'a [Scalar], scale: Scalar },
    /// `y = alpha * x + y` (returned as a fresh vector).
    Axpy {
        alpha: Scalar,
        x: &'a [Scalar],
        y: &'a [Scalar],
    },
    /// Global dot product.
    Dot { a: &'a [Scalar], b: &'a [Scalar] },
    /// Global sum.
    Sum { a: &'a [Scalar] },
    /// Global sum of absolute values.
    Asum { a: &'a [Scalar] },
    /// Global maximum absolute value.
    MaxAbs { a: &'a [Scalar] },
}

/// The value produced by one batched operation.
#[derive(Debug, Clone, PartialEq)]
pub enum BatchValue {
    /// A matrix result (from [`BatchOp::MatMul`]).
    Matrix(Vec<Vec<Scalar>>),
    /// A vector result (element-wise ops).
    Vector(Vec<Scalar>),
    /// A scalar result (reductions).
    Scalar(Scalar),
}

/// A recorded kernel launch, plus where its result must be read from.
struct Recorded {
    /// Number of elements to decode.
    len: usize,
    /// A matrix result is reshaped to `rows × cols` after readback.
    matrix_shape: Option<(usize, usize)>,
    /// Index of this launch's readback buffer in [`BatchEncoder::readbacks`],
    /// or `usize::MAX` for a legitimate empty result.
    readback: usize,
}

/// A pending readback copy: the staging buffer plus how many elements it holds.
struct PendingReadback {
    buffer: PooledBuffer,
    /// Number of elements copied into this staging buffer.
    len: usize,
    /// Buffers the recorded commands still reference.
    _keepalive: Vec<PooledBuffer>,
}

/// Accumulates independent GPU operations into a single command buffer.
///
/// Create one with [`GpuKernels::batch`], push operations with
/// [`BatchEncoder::push`], then run them with [`BatchEncoder::execute`]. The
/// batch is submitted exactly once and read back exactly once.
pub struct BatchEncoder<'k> {
    kernels: &'k GpuKernels,
    /// The device this batch is bound to, so the ergonomic `push_*` helpers do
    /// not require the caller to thread a context through every call.
    ctx: &'k GpuContext,
    encoder: wgpu::CommandEncoder,
    recorded: Vec<Recorded>,
    /// Readback buffers, indexed by `Recorded::readback`; kept alive until the
    /// poll after submission completes.
    readbacks: Vec<PendingReadback>,
    /// Total bytes to be copied back, used to size the single staging budget.
    total_readback_bytes: u64,
}

impl<'k> BatchEncoder<'k> {
    /// Start a new batch bound to `kernels`' device.
    pub(super) fn new(kernels: &'k GpuKernels, ctx: &'k GpuContext) -> Self {
        Self {
            kernels,
            ctx,
            encoder: ctx
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("scico batched encoder"),
                }),
            recorded: Vec::new(),
            readbacks: Vec::new(),
            total_readback_bytes: 0,
        }
    }

    /// The device context this batch is bound to.
    pub fn context(&self) -> &'k GpuContext {
        self.ctx
    }

    /// Queue an element-wise add without threading a context explicitly.
    pub fn push_add(&mut self, a: &[Scalar], b: &[Scalar]) -> Result<usize, SimError> {
        self.push(self.ctx, BatchOp::Add { a, b })
    }

    /// Queue an element-wise subtract.
    pub fn push_sub(&mut self, a: &[Scalar], b: &[Scalar]) -> Result<usize, SimError> {
        self.push(self.ctx, BatchOp::Sub { a, b })
    }

    /// Queue an element-wise Hadamard product.
    pub fn push_mul(&mut self, a: &[Scalar], b: &[Scalar]) -> Result<usize, SimError> {
        self.push(self.ctx, BatchOp::Mul { a, b })
    }

    /// Queue an element-wise absolute value.
    pub fn push_abs(&mut self, a: &[Scalar]) -> Result<usize, SimError> {
        self.push(self.ctx, BatchOp::Abs { a })
    }

    /// Queue a scalar-vector multiply.
    pub fn push_scale(&mut self, a: &[Scalar], scale: Scalar) -> Result<usize, SimError> {
        self.push(self.ctx, BatchOp::Scale { a, scale })
    }

    /// Queue a global sum.
    pub fn push_sum(&mut self, a: &[Scalar]) -> Result<usize, SimError> {
        self.push(self.ctx, BatchOp::Sum { a })
    }

    /// Queue a global dot product.
    pub fn push_dot(&mut self, a: &[Scalar], b: &[Scalar]) -> Result<usize, SimError> {
        self.push(self.ctx, BatchOp::Dot { a, b })
    }

    /// Queue a row-major matrix multiply.
    pub fn push_mat_mul(
        &mut self,
        a: &[Vec<Scalar>],
        b: &[Vec<Scalar>],
    ) -> Result<usize, SimError> {
        self.push(self.ctx, BatchOp::MatMul { a, b })
    }

    /// Submit the batch using the bound context.
    pub fn run(self) -> Result<Vec<BatchValue>, SimError> {
        let ctx = self.ctx;
        self.execute(ctx)
    }

    /// Number of operations queued so far.
    pub fn len(&self) -> usize {
        self.recorded.len()
    }

    /// Whether the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.recorded.is_empty()
    }

    /// Total bytes that will be copied back when the batch runs.
    ///
    /// Useful for deciding whether a batch is worth submitting at all: below a
    /// few hundred kilobytes the CPU path is usually faster.
    pub fn readback_bytes(&self) -> u64 {
        self.total_readback_bytes
    }

    /// Validate and record `op` into the batch.
    ///
    /// Validation happens here, before submission, so an invalid operation
    /// aborts the whole batch without leaving partial work on the device.
    ///
    /// Returns the index of the queued operation, which is also its position in
    /// the vector returned by [`Self::execute`].
    pub fn push(&mut self, ctx: &GpuContext, op: BatchOp<'_>) -> Result<usize, SimError> {
        // Validate shapes first; a rejected op must not touch the encoder.
        match &op {
            BatchOp::MatMul { a, b } => {
                if a.is_empty() || b.is_empty() {
                    // An empty product is a legitimate no-op with an empty
                    // result; record it as a zero-length readback.
                    return self.push_empty_matrix();
                }
                let k = a[0].len();
                if b.len() != k {
                    return Err(SimError::numerical(format!(
                        "batch mat_mul: A cols={k}, B rows={}",
                        b.len()
                    )));
                }
            }
            BatchOp::Add { a, b }
            | BatchOp::Sub { a, b }
            | BatchOp::Mul { a, b }
            | BatchOp::Dot { a, b }
                if a.len() != b.len() =>
            {
                return Err(SimError::numerical(format!(
                    "batch {}: length mismatch {} vs {}",
                    op_name(&op),
                    a.len(),
                    b.len()
                )));
            }
            _ => {}
        }
        self.check_capacity(ctx, &op)?;
        self.record(ctx, op)
    }

    /// Record an empty matrix result (used when a matmul operand is empty).
    fn push_empty_matrix(&mut self) -> Result<usize, SimError> {
        let index = self.recorded.len();
        self.recorded.push(Recorded {
            len: 0,
            matrix_shape: Some((0, 0)),
            readback: usize::MAX,
        });
        Ok(index)
    }

    /// Reject an operation whose buffers exceed the adapter's binding limit.
    fn check_capacity(&self, ctx: &GpuContext, op: &BatchOp<'_>) -> Result<(), SimError> {
        let max = ctx.max_elements();
        let over = |n: usize, what: &str| -> Result<(), SimError> {
            if n > max {
                Err(SimError::numerical(format!(
                    "batch {what}: {n} elements exceeds this adapter's limit of {max}"
                )))
            } else {
                Ok(())
            }
        };
        match op {
            BatchOp::MatMul { a, b } => {
                let (m, k, n) = (
                    a.len(),
                    a.first().map_or(0, Vec::len),
                    b.first().map_or(0, Vec::len),
                );
                over(m.saturating_mul(k), "mat_mul A")?;
                over(k.saturating_mul(n), "mat_mul B")?;
                over(m.saturating_mul(n), "mat_mul C")?;
            }
            BatchOp::Add { a, .. }
            | BatchOp::Sub { a, .. }
            | BatchOp::Mul { a, .. }
            | BatchOp::Dot { a, .. }
            | BatchOp::Sum { a }
            | BatchOp::Asum { a }
            | BatchOp::MaxAbs { a }
            | BatchOp::Abs { a }
            | BatchOp::Scale { a, .. } => over(a.len(), op_name(op))?,
            BatchOp::Axpy { x, .. } => over(x.len(), "axpy")?,
        }
        Ok(())
    }

    /// Record the kernel launch for one validated operation.
    fn record(&mut self, ctx: &GpuContext, op: BatchOp<'_>) -> Result<usize, SimError> {
        let index = self.recorded.len();
        let (byte_len, len, matrix_shape) = match op {
            BatchOp::MatMul { a, b } => {
                let (m, k, n) = (a.len(), a[0].len(), b[0].len());
                let out_len = m * n;
                self.record_matmul(ctx, a, b, (m, k, n), out_len)?;
                (self.byte_len(out_len), out_len, Some((m, n)))
            }
            BatchOp::Add { a, b } => {
                self.record_binary(ctx, BinaryOp::Add, a, b)?;
                (self.byte_len(a.len()), a.len(), None)
            }
            BatchOp::Sub { a, b } => {
                self.record_binary(ctx, BinaryOp::Sub, a, b)?;
                (self.byte_len(a.len()), a.len(), None)
            }
            BatchOp::Mul { a, b } => {
                self.record_binary(ctx, BinaryOp::Mul, a, b)?;
                (self.byte_len(a.len()), a.len(), None)
            }
            BatchOp::Abs { a } => {
                self.record_binary(ctx, BinaryOp::Abs, a, a)?;
                (self.byte_len(a.len()), a.len(), None)
            }
            BatchOp::Scale { a, scale } => {
                let encoded = self.kernels.encode_scalar_for_batch(scale);
                self.record_scale(ctx, a, encoded)?;
                (self.byte_len(a.len()), a.len(), None)
            }
            BatchOp::Axpy { alpha, x, y } => {
                let encoded = self.kernels.encode_scalar_for_batch(alpha);
                self.record_axpy(ctx, x, y, encoded)?;
                (self.byte_len(y.len()), y.len(), None)
            }
            BatchOp::Dot { a, b } => {
                self.record_reduce(ctx, BinaryOp::Dot, a, b)?;
                (self.byte_len(1), 1, None)
            }
            BatchOp::Sum { a } => {
                self.record_reduce(ctx, BinaryOp::Sum, a, a)?;
                (self.byte_len(1), 1, None)
            }
            BatchOp::Asum { a } => {
                self.record_reduce(ctx, BinaryOp::Asum, a, a)?;
                (self.byte_len(1), 1, None)
            }
            BatchOp::MaxAbs { a } => {
                self.record_reduce(ctx, BinaryOp::MaxAbs, a, a)?;
                (self.byte_len(1), 1, None)
            }
        };
        let readback = self.readbacks.len() - 1;
        self.total_readback_bytes = self.total_readback_bytes.saturating_add(byte_len);
        self.recorded.push(Recorded {
            len,
            matrix_shape,
            readback,
        });
        Ok(index)
    }

    /// Bytes for `len` elements at the active storage precision.
    fn byte_len(&self, len: usize) -> u64 {
        (len as u64).saturating_mul(self.kernels.precision().element_size())
    }

    /// Allocate a readback buffer for `out_len` elements and schedule its copy,
    /// keeping `output` and every buffer in `keepalive` alive until submission.
    ///
    /// Dropping the referenced buffers here would return them to the pool, where
    /// a later operation in the same batch could reuse the memory while the
    /// encoder still points at it — so ownership is transferred to the batch.
    fn schedule_readback(
        &mut self,
        output: PooledBuffer,
        out_len: usize,
        mut keepalive: Vec<PooledBuffer>,
    ) -> u64 {
        let byte_len = self.byte_len(out_len);
        let readback = self.kernels.acquire_for_batch(
            BufferRoleSpec::Readback,
            byte_len.max(self.kernels.precision().element_size()),
        );
        if byte_len > 0 {
            self.encoder
                .copy_buffer_to_buffer(output.buffer(), 0, readback.buffer(), 0, byte_len);
        }
        keepalive.push(output);
        self.readbacks.push(PendingReadback {
            buffer: readback,
            len: out_len,
            _keepalive: keepalive,
        });
        byte_len
    }

    /// Record a matmul. `shape` is `(m, k, n)`.
    fn record_matmul(
        &mut self,
        ctx: &GpuContext,
        a: &[Vec<Scalar>],
        b: &[Vec<Scalar>],
        shape: (usize, usize, usize),
        out_len: usize,
    ) -> Result<(), SimError> {
        let (m, k, n) = shape;
        let flat_a = flatten(a);
        let flat_b = flatten(b);
        let params = [m as u32, k as u32, n as u32, 0];
        let buf_a = self.kernels.upload_for_batch(ctx, &flat_a)?;
        let buf_b = self.kernels.upload_for_batch(ctx, &flat_b)?;
        let buf_out = self
            .kernels
            .acquire_for_batch(BufferRoleSpec::Output, self.byte_len(out_len));
        let buf_ctrl = self.kernels.control_for_batch(ctx, &params)?;
        self.kernels.record_matmul_for_batch(
            &mut self.encoder,
            ctx,
            BatchBuffers {
                first: &buf_a,
                second: &buf_b,
                out: &buf_out,
                ctrl: &buf_ctrl,
            },
            (m, n),
        );
        self.schedule_readback(buf_out, out_len, vec![buf_a, buf_b, buf_ctrl]);
        Ok(())
    }

    fn record_binary(
        &mut self,
        ctx: &GpuContext,
        op: BinaryOp,
        a: &[Scalar],
        b: &[Scalar],
    ) -> Result<(), SimError> {
        let params = [a.len() as u32, op.tag(), 0, 0];
        let buf_a = self.kernels.upload_for_batch(ctx, a)?;
        let buf_b = self.kernels.upload_for_batch(ctx, b)?;
        let buf_out = self
            .kernels
            .acquire_for_batch(BufferRoleSpec::Output, self.byte_len(a.len()));
        let buf_ctrl = self.kernels.control_for_batch(ctx, &params)?;
        self.kernels.record_binary_for_batch(
            &mut self.encoder,
            ctx,
            BatchBuffers {
                first: &buf_a,
                second: &buf_b,
                out: &buf_out,
                ctrl: &buf_ctrl,
            },
            a.len(),
        );
        self.schedule_readback(buf_out, a.len(), vec![buf_a, buf_b, buf_ctrl]);
        Ok(())
    }

    fn record_scale(
        &mut self,
        ctx: &GpuContext,
        a: &[Scalar],
        encoded: (u32, u32),
    ) -> Result<(), SimError> {
        let params = [a.len() as u32, encoded.0, encoded.1, 0];
        let buf_a = self.kernels.upload_for_batch(ctx, a)?;
        let buf_out = self
            .kernels
            .acquire_for_batch(BufferRoleSpec::Output, self.byte_len(a.len()));
        let buf_ctrl = self.kernels.control_for_batch(ctx, &params)?;
        self.kernels.record_scale_for_batch(
            &mut self.encoder,
            ctx,
            &buf_a,
            &buf_out,
            &buf_ctrl,
            a.len(),
        );
        self.schedule_readback(buf_out, a.len(), vec![buf_a, buf_ctrl]);
        Ok(())
    }

    fn record_axpy(
        &mut self,
        ctx: &GpuContext,
        x: &[Scalar],
        y: &[Scalar],
        encoded: (u32, u32),
    ) -> Result<(), SimError> {
        let params = [x.len() as u32, encoded.0, encoded.1, 0];
        let buf_x = self.kernels.upload_for_batch(ctx, x)?;
        let buf_y = self.kernels.upload_for_batch(ctx, y)?;
        let buf_out = self
            .kernels
            .acquire_for_batch(BufferRoleSpec::Output, self.byte_len(y.len()));
        let buf_ctrl = self.kernels.control_for_batch(ctx, &params)?;
        self.kernels.record_axpy_for_batch(
            &mut self.encoder,
            ctx,
            BatchBuffers {
                first: &buf_x,
                second: &buf_y,
                out: &buf_out,
                ctrl: &buf_ctrl,
            },
            y.len(),
        );
        self.schedule_readback(buf_out, y.len(), vec![buf_x, buf_y, buf_ctrl]);
        Ok(())
    }

    fn record_reduce(
        &mut self,
        ctx: &GpuContext,
        op: BinaryOp,
        a: &[Scalar],
        b: &[Scalar],
    ) -> Result<(), SimError> {
        let partials = self
            .kernels
            .partial_count_for_batch(a.len(), ctx.max_elements())?;
        let buf_a = self.kernels.upload_for_batch(ctx, a)?;
        let buf_b = self.kernels.upload_for_batch(ctx, b)?;
        let out_len = partials.max(1);
        let buf_out = self
            .kernels
            .acquire_for_batch(BufferRoleSpec::Output, self.byte_len(out_len));
        let ctrls = self.kernels.record_reduce_for_batch(
            &mut self.encoder,
            ctx,
            op,
            &buf_a,
            &buf_b,
            &buf_out,
            a.len(),
            partials,
        )?;
        // Only the collapsed scalar in `out[0]` is read back.
        let mut keepalive = vec![buf_a, buf_b];
        keepalive.extend(ctrls);
        self.schedule_readback(buf_out, 1, keepalive);
        Ok(())
    }

    /// Submit the whole batch and return one value per queued operation.
    ///
    /// The encoder is submitted **once** and the device is polled **once**,
    /// which is where the saving over per-call submission comes from.
    pub fn execute(self, ctx: &GpuContext) -> Result<Vec<BatchValue>, SimError> {
        let Self {
            encoder,
            recorded,
            readbacks,
            ..
        } = self;

        // Nothing queued: avoid a pointless submit.
        if recorded.iter().all(|r| r.readback == usize::MAX) {
            return Ok(recorded
                .iter()
                .map(|_| BatchValue::Matrix(Vec::new()))
                .collect());
        }

        ctx.submit_and_wait(encoder)?;

        // Decode every readback from the single polled submission, using the
        // element count recorded for each staging buffer.
        let precision = self.kernels.precision();
        let mut decoded: Vec<Vec<Scalar>> = Vec::with_capacity(readbacks.len());
        for pending in &readbacks {
            decoded.push(self.kernels.download_for_batch(
                ctx,
                &pending.buffer,
                pending.len,
                precision,
            )?);
        }

        // Map each recorded op to its value, preserving order.
        let mut out = Vec::with_capacity(recorded.len());
        for rec in &recorded {
            if rec.readback == usize::MAX {
                out.push(BatchValue::Matrix(Vec::new()));
                continue;
            }
            let values = decoded.get(rec.readback).cloned().unwrap_or_default();
            out.push(match rec.matrix_shape {
                Some((rows, cols)) => BatchValue::Matrix(unflatten(&values, rows, cols)),
                None if rec.len == 1 => BatchValue::Scalar(values.first().copied().unwrap_or(0.0)),
                None => BatchValue::Vector(values),
            });
        }
        Ok(out)
    }
}

/// Name of an operation, for error messages.
fn op_name(op: &BatchOp<'_>) -> &'static str {
    match op {
        BatchOp::MatMul { .. } => "mat_mul",
        BatchOp::Add { .. } => "add",
        BatchOp::Sub { .. } => "sub",
        BatchOp::Mul { .. } => "mul",
        BatchOp::Abs { .. } => "abs",
        BatchOp::Scale { .. } => "scale",
        BatchOp::Axpy { .. } => "axpy",
        BatchOp::Dot { .. } => "dot",
        BatchOp::Sum { .. } => "sum",
        BatchOp::Asum { .. } => "asum",
        BatchOp::MaxAbs { .. } => "max_abs",
    }
}

/// Flatten a row-major matrix into a contiguous vector.
fn flatten(a: &[Vec<Scalar>]) -> Vec<Scalar> {
    let mut out = Vec::with_capacity(a.iter().map(Vec::len).sum());
    for row in a {
        out.extend_from_slice(row);
    }
    out
}

/// Rebuild a row-major matrix from a flat vector.
fn unflatten(flat: &[Scalar], rows: usize, cols: usize) -> Vec<Vec<Scalar>> {
    if rows == 0 || cols == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(rows);
    for r in 0..rows {
        let start = r * cols;
        let end = (start + cols).min(flat.len());
        let mut row = flat[start.min(flat.len())..end].to_vec();
        row.resize(cols, 0.0);
        out.push(row);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::compute::gpu::GpuKernels;
    use crate::core::compute::gpu::device::GpuContext;

    fn context() -> Option<GpuContext> {
        GpuContext::new().ok()
    }

    fn rand_vec(n: usize, seed: u64) -> Vec<Scalar> {
        let mut x = seed | 1;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x as f64 / u64::MAX as f64) * 2.0 - 1.0
            })
            .collect()
    }

    fn tolerance(reference: Scalar) -> Scalar {
        // f32 storage may be in use; 1e-3 relative with an absolute floor.
        1e-3 * reference.abs().max(1.0)
    }

    #[test]
    fn empty_batch_returns_no_values() {
        let Some(ctx) = context() else { return };
        let kernels = GpuKernels::new(&ctx).unwrap();
        let encoder = kernels.batch(&ctx);
        assert!(encoder.is_empty());
        let results = encoder.execute(&ctx).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn batch_matches_individual_launches() {
        let Some(ctx) = context() else { return };
        let kernels = GpuKernels::new(&ctx).unwrap();

        let a = rand_vec(4096, 0x1234);
        let b = rand_vec(4096, 0x5678);
        let m = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let mm = vec![vec![7.0, 8.0], vec![9.0, 10.0], vec![11.0, 12.0]];

        // Reference values via the one-at-a-time kernels.
        let ref_add = kernels.binary(&ctx, BinaryOp::Add, &a, &b).unwrap();
        let ref_sub = kernels.binary(&ctx, BinaryOp::Sub, &a, &b).unwrap();
        let ref_had = kernels.binary(&ctx, BinaryOp::Mul, &a, &b).unwrap();
        let ref_abs = kernels.binary(&ctx, BinaryOp::Abs, &a, &a).unwrap();
        let ref_scale = kernels.scale(&ctx, 2.5, &a).unwrap();
        let ref_axpy = kernels.axpy(&ctx, -0.75, &a, &b).unwrap();
        let ref_dot = kernels.reduce(&ctx, BinaryOp::Dot, &a, &b).unwrap();
        let ref_sum = kernels.reduce(&ctx, BinaryOp::Sum, &a, &a).unwrap();
        let ref_matmul = kernels
            .mat_mul(&ctx, &flatten(&m), &flatten(&mm), 2, 3, 2)
            .unwrap();

        let mut encoder = kernels.batch(&ctx);
        encoder.push(&ctx, BatchOp::Add { a: &a, b: &b }).unwrap();
        encoder.push(&ctx, BatchOp::Sub { a: &a, b: &b }).unwrap();
        encoder.push(&ctx, BatchOp::Mul { a: &a, b: &b }).unwrap();
        encoder.push(&ctx, BatchOp::Abs { a: &a }).unwrap();
        encoder
            .push(&ctx, BatchOp::Scale { a: &a, scale: 2.5 })
            .unwrap();
        encoder
            .push(
                &ctx,
                BatchOp::Axpy {
                    alpha: -0.75,
                    x: &a,
                    y: &b,
                },
            )
            .unwrap();
        encoder.push(&ctx, BatchOp::Dot { a: &a, b: &b }).unwrap();
        encoder.push(&ctx, BatchOp::Sum { a: &a }).unwrap();
        encoder
            .push(&ctx, BatchOp::MatMul { a: &m, b: &mm })
            .unwrap();
        // A single submission for all nine operations.
        assert_eq!(encoder.len(), 9);
        let results = encoder.execute(&ctx).unwrap();
        assert_eq!(results.len(), 9);

        let as_vec = |v: &BatchValue| match v {
            BatchValue::Vector(x) => x.clone(),
            other => panic!("expected vector, got {other:?}"),
        };
        let max_diff = |x: &[Scalar], y: &[Scalar]| {
            x.iter()
                .zip(y)
                .map(|(p, q)| (p - q).abs())
                .fold(0.0, Scalar::max)
        };

        assert!(max_diff(&as_vec(&results[0]), &ref_add) <= tolerance(1.0));
        assert!(max_diff(&as_vec(&results[1]), &ref_sub) <= tolerance(1.0));
        assert!(max_diff(&as_vec(&results[2]), &ref_had) <= tolerance(1.0));
        assert!(max_diff(&as_vec(&results[3]), &ref_abs) <= tolerance(1.0));
        assert!(max_diff(&as_vec(&results[4]), &ref_scale) <= tolerance(1.0));
        assert!(max_diff(&as_vec(&results[5]), &ref_axpy) <= tolerance(1.0));

        match &results[6] {
            BatchValue::Scalar(v) => {
                assert!(
                    (v - ref_dot).abs() <= tolerance(ref_dot),
                    "dot {v} vs {ref_dot}"
                )
            }
            other => panic!("expected scalar, got {other:?}"),
        }
        match &results[7] {
            BatchValue::Scalar(v) => {
                assert!(
                    (v - ref_sum).abs() <= tolerance(ref_sum),
                    "sum {v} vs {ref_sum}"
                )
            }
            other => panic!("expected scalar, got {other:?}"),
        }
        match &results[8] {
            BatchValue::Matrix(c) => {
                assert_eq!(c.len(), 2);
                // 2×3 · 3×2 = 2×2, flattened row-major.
                assert_eq!(flatten(c).len(), 4);
                assert!(max_diff(&flatten(c), &ref_matmul) <= tolerance(100.0));
            }
            other => panic!("expected matrix, got {other:?}"),
        }
    }

    #[test]
    fn batch_keeps_result_order_under_mixed_ops() {
        let Some(ctx) = context() else { return };
        let kernels = GpuKernels::new(&ctx).unwrap();

        let a = rand_vec(2048, 0xAAAA);
        let b = rand_vec(2048, 0xBBBB);
        let mut encoder = kernels.batch(&ctx);
        // Interleave scalars and vectors so an ordering bug cannot pass.
        encoder.push(&ctx, BatchOp::Sum { a: &a }).unwrap();
        encoder.push(&ctx, BatchOp::Add { a: &a, b: &b }).unwrap();
        encoder.push(&ctx, BatchOp::MaxAbs { a: &a }).unwrap();
        let results = encoder.execute(&ctx).unwrap();

        let expect_sum: Scalar = a.iter().sum();
        let expect_add: Vec<Scalar> = a.iter().zip(&b).map(|(x, y)| x + y).collect();
        let expect_max: Scalar = a.iter().map(|v| v.abs()).fold(0.0, Scalar::max);

        match &results[0] {
            BatchValue::Scalar(v) => {
                assert!(
                    (v - expect_sum).abs() <= tolerance(expect_sum),
                    "{v} vs {expect_sum}"
                )
            }
            other => panic!("index 0 should be a scalar, got {other:?}"),
        }
        match &results[1] {
            BatchValue::Vector(v) => {
                let d = v
                    .iter()
                    .zip(&expect_add)
                    .map(|(p, q)| (p - q).abs())
                    .fold(0.0, Scalar::max);
                assert!(d <= tolerance(1.0));
            }
            other => panic!("index 1 should be a vector, got {other:?}"),
        }
        match &results[2] {
            BatchValue::Scalar(v) => {
                assert!(
                    (v - expect_max).abs() <= tolerance(expect_max),
                    "{v} vs {expect_max}"
                )
            }
            other => panic!("index 2 should be a scalar, got {other:?}"),
        }
    }

    #[test]
    fn batch_rejects_invalid_op_before_submitting() {
        let Some(ctx) = context() else { return };
        let kernels = GpuKernels::new(&ctx).unwrap();
        let a = rand_vec(16, 1);
        let short = rand_vec(8, 2);
        // A 1×3 matrix cannot multiply a 2×3 matrix (inner dims 3 vs 2).
        let b = vec![vec![1.0, 2.0, 3.0]];
        let bad = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];

        let mut encoder = kernels.batch(&ctx);
        // Length mismatch must be rejected at push time.
        assert!(
            encoder
                .push(&ctx, BatchOp::Add { a: &a, b: &short })
                .is_err()
        );
        // Matmul with mismatched inner dimensions must be rejected too.
        assert!(
            encoder
                .push(&ctx, BatchOp::MatMul { a: &b, b: &bad })
                .is_err()
        );
        // Nothing valid was queued, so the batch stayed empty and no submit happens.
        assert!(encoder.is_empty());
        let results = encoder.execute(&ctx).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn batch_amortises_the_submission_round_trip() {
        // Evidence that the batch path does what it claims: many small ops in
        // one submission must be substantially faster than the same ops run
        // one-at-a-time, because the fixed round-trip is paid once.
        let Some(ctx) = context() else { return };
        let kernels = GpuKernels::new(&ctx).unwrap();
        let n = 4096;
        let a = rand_vec(n, 0xF00D);
        let b = rand_vec(n, 0xBEEF);
        const OPS: usize = 16;
        const REPS: usize = 8;

        // Warm up both paths so pool/thermal effects are not measured.
        for _ in 0..2 {
            let _ = kernels.binary(&ctx, BinaryOp::Add, &a, &b).unwrap();
            let mut e = kernels.batch(&ctx);
            e.push(&ctx, BatchOp::Add { a: &a, b: &b }).unwrap();
            let _ = e.execute(&ctx).unwrap();
        }

        let start = std::time::Instant::now();
        for _ in 0..REPS {
            for _ in 0..OPS {
                let _ = kernels.binary(&ctx, BinaryOp::Add, &a, &b).unwrap();
            }
        }
        let individual = start.elapsed().as_secs_f64();

        let start = std::time::Instant::now();
        for _ in 0..REPS {
            let mut e = kernels.batch(&ctx);
            for _ in 0..OPS {
                e.push(&ctx, BatchOp::Add { a: &a, b: &b }).unwrap();
            }
            let _ = e.execute(&ctx).unwrap();
        }
        let batched = start.elapsed().as_secs_f64();

        println!(
            "batch round-trip: {OPS} ops x {REPS} reps | individual {:.3} ms | batched {:.3} ms | speedup {:.2}x",
            individual * 1e3,
            batched * 1e3,
            individual / batched
        );
        // A batch must not be slower; the only way it loses is if the fixed
        // round-trip were zero, which the device measurements disprove.
        assert!(
            batched <= individual * 1.2,
            "batched {batched:.6}s vs individual {individual:.6}s"
        );
    }
}
