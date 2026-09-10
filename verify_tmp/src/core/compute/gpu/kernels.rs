//! Compute pipelines and typed launch helpers for the SCIcoRS WGSL kernels.
//!
//! Pipelines are compiled once per device in [`GpuKernels::new`] and reused for
//! every dispatch, so a hot-path call only allocates buffers and records one
//! compute pass. Each kernel is bound to the storage precision the device
//! supports (see [`super::device::GpuPrecision`]).
//!
//! # Buffer/binding contract
//!
//! Every kernel uses the same four-binding shape:
//!
//! | binding | resource | purpose |
//! |---------|----------|---------|
//! | 0 | storage (read) | first input |
//! | 1 | storage (read / read-write) | second input, or output when unused |
//! | 2 | storage (read-write) | output |
//! | 3 | uniform `vec4<u32>` | sizes and op tags |
//!
//! Control values are `u32` only, so the control path itself is exact. Numeric
//! payloads are widened to `f64` and split into two `u32` halves when they must
//! travel through the uniform (scalars and AXPY's `alpha`).

use std::mem::size_of;

use crate::core::error::SimError;
use crate::core::types::Scalar;

use super::device::{GpuContext, GpuPrecision};
use super::pool::{BufferPool, BufferRoleSpec, PooledBuffer};

/// WGSL source for the 32-bit kernels (universal fallback).
const PRECISION32_WGSL: &str = include_str!("precision32.wgsl");
/// WGSL source for the 64-bit kernels (requires `Features::SHADER_F64`).
const PRECISION64_WGSL: &str = include_str!("precision64.wgsl");

/// Workgroup size of the binary/scalar kernels — must match the WGSL.
const ELEMENT_WORKGROUP: u32 = 64;
/// Workgroup size of the reduction kernels — must match the WGSL.
const REDUCE_WORKGROUP: u32 = 256;
/// Elements per thread in reduction stage 1 — must match `REDUCE_STRIDE`.
const REDUCE_STRIDE: u32 = 64;
/// Edge of the square matrix-multiply workgroup — must match `TILE` in the
/// WGSL tiled GEMM.
const MATMUL_TILE: u32 = 16;

/// Operation selector shared by the element-wise and reduction kernels.
///
/// The discriminants are part of the WGSL ABI: `binary_main` reads
/// `0 = add, 1 = sub, 2 = mul`, `reduce_partial` reads `0 = dot, 1 = sum`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    /// `a + b`
    Add,
    /// `a - b`
    Sub,
    /// `a * b` (Hadamard product)
    Mul,
    /// `|a|` (unary; the second operand is ignored)
    Abs,
    /// `Σ aᵢ·bᵢ` (reduction)
    Dot,
    /// `Σ aᵢ` (reduction)
    Sum,
    /// `Σ |aᵢ|` (reduction)
    Asum,
    /// `maxᵢ |aᵢ|` (reduction)
    MaxAbs,
}

impl BinaryOp {
    /// WGSL kernel tag.
    ///
    /// Shared across the element-wise and reduction kernels:
    /// `0 = add/dot`, `1 = sub/sum`, `2 = mul/asum`, `3 = abs/max-abs`.
    fn tag(self) -> u32 {
        match self {
            BinaryOp::Add | BinaryOp::Dot => 0,
            BinaryOp::Sub | BinaryOp::Sum => 1,
            BinaryOp::Mul | BinaryOp::Asum => 2,
            BinaryOp::Abs | BinaryOp::MaxAbs => 3,
        }
    }

    /// Name used in error messages.
    pub fn name(self) -> &'static str {
        match self {
            BinaryOp::Add => "add",
            BinaryOp::Sub => "sub",
            BinaryOp::Mul => "mul",
            BinaryOp::Abs => "abs",
            BinaryOp::Dot => "dot",
            BinaryOp::Sum => "sum",
            BinaryOp::Asum => "asum",
            BinaryOp::MaxAbs => "max_abs",
        }
    }

    /// Whether this op is a global reduction (a different kernel entry point).
    fn is_reduction(self) -> bool {
        matches!(
            self,
            BinaryOp::Dot | BinaryOp::Sum | BinaryOp::Asum | BinaryOp::MaxAbs
        )
    }
}

/// A compiled, code-typed pipeline plus the element type it was built for.
struct Kernel {
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

/// All compute pipelines for one device, plus its buffer pool.
pub struct GpuKernels {
    precision: GpuPrecision,
    pool: BufferPool,
    matmul: Kernel,
    binary: Kernel,
    scale: Kernel,
    axpy: Kernel,
    reduce_partial: Kernel,
    reduce_finalize: Kernel,
    transpose: Kernel,
}

impl GpuKernels {
    /// Compile every kernel for `ctx` at the device's supported precision.
    ///
    /// Shader compilation happens on the device, so a WGSL error surfaces here
    /// (via [`GpuContext::submit_and_wait`] during the device's async error
    /// scope) rather than at a later dispatch.
    pub fn new(ctx: &GpuContext) -> Result<Self, SimError> {
        let precision = ctx.precision();
        let source = match precision {
            GpuPrecision::F32 => PRECISION32_WGSL,
            GpuPrecision::F64 => PRECISION64_WGSL,
        };
        let device = ctx.device();
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scico-rs kernels"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });

        let matmul = build_kernel(
            device,
            &module,
            "matmul_main",
            &matmul_bind_group_layout(device),
            "scico matmul",
        );
        let binary = build_kernel(
            device,
            &module,
            "binary_main",
            &binary_bind_group_layout(device),
            "scico binary",
        );
        let scale = build_kernel(
            device,
            &module,
            "scale_main",
            &scale_bind_group_layout(device),
            "scico scale",
        );
        let axpy = build_kernel(
            device,
            &module,
            "axpy_main",
            &axpy_bind_group_layout(device),
            "scico axpy",
        );
        let reduce_partial = build_kernel(
            device,
            &module,
            "reduce_partial",
            &reduce_bind_group_layout(device),
            "scico reduce partial",
        );
        let reduce_finalize = build_kernel(
            device,
            &module,
            "reduce_finalize",
            &reduce_bind_group_layout(device),
            "scico reduce finalize",
        );
        let transpose = build_kernel(
            device,
            &module,
            "transpose_main",
            &transpose_bind_group_layout(device),
            "scico transpose",
        );

        Ok(Self {
            precision,
            pool: BufferPool::new(device, precision),
            matmul,
            binary,
            scale,
            axpy,
            reduce_partial,
            reduce_finalize,
            transpose,
        })
    }

    /// The buffer pool backing this device's launches.
    pub fn pool(&self) -> &BufferPool {
        &self.pool
    }

    /// Number of buffers currently parked for reuse.
    pub fn pooled_buffers(&self) -> usize {
        self.pool.pooled_count()
    }

    /// The storage precision these kernels were compiled for.
    pub fn precision(&self) -> GpuPrecision {
        self.precision
    }

    /// `C(m×n) = A(m×k) · B(k×n)` on the device. All buffers are row-major.
    pub fn mat_mul(
        &self,
        ctx: &GpuContext,
        a: &[Scalar],
        b: &[Scalar],
        m: usize,
        k: usize,
        n: usize,
    ) -> Result<Vec<Scalar>, SimError> {
        if m == 0 || k == 0 || n == 0 {
            return Ok(Vec::new());
        }
        let params = [
            u32_from(m, "mat_mul m")?,
            u32_from(k, "mat_mul k")?,
            u32_from(n, "mat_mul n")?,
            0,
        ];
        self.element_launch(
            ctx,
            &self.matmul,
            params,
            (Some(a), Some(b)),
            m * n,
            dispatch_matmul(m, n),
        )
    }

    /// Element-wise binary op on equal-length vectors.
    pub fn binary(
        &self,
        ctx: &GpuContext,
        op: BinaryOp,
        a: &[Scalar],
        b: &[Scalar],
    ) -> Result<Vec<Scalar>, SimError> {
        debug_assert!(!op.is_reduction(), "binary() called with a reduction op");
        let params = [u32_from(a.len(), "binary len")?, op.tag(), 0, 0];
        self.element_launch(
            ctx,
            &self.binary,
            params,
            (Some(a), Some(b)),
            a.len(),
            dispatch_elements(a.len()),
        )
    }

    /// `out = scale * a` on the device.
    ///
    /// The `f64` scale is encoded for the active kernel set: the `f32` kernels
    /// take it bit-cast into one `u32` slot (matching their `f32` storage), the
    /// `f64` kernels take its two `u32` halves.
    pub fn scale(
        &self,
        ctx: &GpuContext,
        scale: Scalar,
        a: &[Scalar],
    ) -> Result<Vec<Scalar>, SimError> {
        let encoded = self.encode_scalar(scale);
        let params = [u32_from(a.len(), "scale len")?, encoded.0, encoded.1, 0];
        // `scale_main` declares three bindings (input, output, uniform), so it
        // launches through the dedicated three-binding path.
        self.element_launch3(
            ctx,
            &self.scale,
            params,
            a,
            a.len(),
            dispatch_elements(a.len()),
        )
    }

    /// `y = alpha * x + y` on the device, returned as a fresh vector.
    pub fn axpy(
        &self,
        ctx: &GpuContext,
        alpha: Scalar,
        x: &[Scalar],
        y: &[Scalar],
    ) -> Result<Vec<Scalar>, SimError> {
        let encoded = self.encode_scalar(alpha);
        let params = [u32_from(x.len(), "axpy len")?, encoded.0, encoded.1, 0];
        // `axpy_main` declares a distinct bind group shape (x, y, out, control).
        let device = ctx.device();
        let buf_x = self.upload(ctx, x)?;
        let buf_y = self.upload(ctx, y)?;
        let buf_out = self.output_buffer(y.len() as u64 * self.precision.element_size());
        let buf_ctrl = self.control_buffer(ctx, &params)?;
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scico axpy"),
            layout: &self.axpy.bind_group_layout,
            entries: &[
                bind_entry(0, &buf_x),
                bind_entry(1, &buf_y),
                bind_entry(2, &buf_out),
                bind_entry(3, &buf_ctrl),
            ],
        });
        let mut encoder = device.create_command_encoder(&encoder_descriptor());
        record_compute(
            &mut encoder,
            &self.axpy.pipeline,
            &bind_group,
            dispatch_elements(y.len()),
        );
        let readback = self.readback_buffer(y.len() as u64 * self.precision.element_size());
        let byte_len = (y.len() as u64) * self.precision.element_size();
        encoder.copy_buffer_to_buffer(&buf_out, 0, &readback, 0, byte_len);
        ctx.submit_and_wait(encoder)?;
        download(ctx, &readback, y.len(), self.precision)
    }

    /// Shared launch path for kernels with three bindings
    /// (input, output, control).
    fn element_launch3(
        &self,
        ctx: &GpuContext,
        kernel: &Kernel,
        params: [u32; 4],
        input: &[Scalar],
        out_len: usize,
        workgroups: (u32, u32, u32),
    ) -> Result<Vec<Scalar>, SimError> {
        let device = ctx.device();
        let buf_in = self.upload(ctx, input)?;
        let buf_out = self.output_buffer(out_len as u64 * self.precision.element_size());
        let buf_ctrl = self.control_buffer(ctx, &params)?;
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scico scalar"),
            layout: &kernel.bind_group_layout,
            entries: &[
                bind_entry(0, &buf_in),
                bind_entry(1, &buf_out),
                bind_entry(2, &buf_ctrl),
            ],
        });
        let mut encoder = device.create_command_encoder(&encoder_descriptor());
        record_compute(&mut encoder, &kernel.pipeline, &bind_group, workgroups);
        let readback = self.readback_buffer(out_len as u64 * self.precision.element_size());
        let byte_len = (out_len as u64) * self.precision.element_size();
        encoder.copy_buffer_to_buffer(&buf_out, 0, &readback, 0, byte_len);
        ctx.submit_and_wait(encoder)?;
        download(ctx, &readback, out_len, self.precision)
    }

    /// Encode an `f64` scalar for the WGSL uniform.
    ///
    /// Returns `(slot_y, slot_z)`. The `f32` kernel set reads only the first
    /// slot (a bit-cast `f32`); the `f64` set recombines both halves.
    fn encode_scalar(&self, value: Scalar) -> (u32, u32) {
        match self.precision {
            GpuPrecision::F32 => ((value as f32).to_bits(), 0),
            GpuPrecision::F64 => split_f64(value),
        }
    }

    /// Global reduction (`Dot` / `Sum`) via two-stage device reduction.
    ///
    /// Stage 1 emits one partial per workgroup; stage 2 collapses the partials.
    /// The final value is read back from element 0 of the same buffer.
    pub fn reduce(
        &self,
        ctx: &GpuContext,
        op: BinaryOp,
        a: &[Scalar],
        b: &[Scalar],
    ) -> Result<Scalar, SimError> {
        debug_assert!(op.is_reduction(), "reduce() called with a binary op");
        let len = a.len();
        let device = ctx.device();
        let partials = partial_count(len)?;

        let buf_a = self.upload(ctx, a)?;
        let buf_b = self.upload(ctx, b)?;
        // Partials live in `out[0..partials]`; stage 2 collapses them in place,
        // leaving the scalar in `out[0]`.
        let out_len = partials.max(1);
        let buf_out = self.output_buffer(out_len as u64 * ctx.precision().element_size());

        let mut encoder = device.create_command_encoder(&encoder_descriptor());
        // Stage 1.
        let params = [
            u32_from(len, "reduce len")?,
            op.tag(),
            u32_from(partials, "reduce partials")?,
            0,
        ];
        let buf_ctrl = self.control_buffer(ctx, &params)?;
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scico reduce partial"),
            layout: &self.reduce_partial.bind_group_layout,
            entries: &[
                bind_entry(0, &buf_a),
                bind_entry(1, &buf_b),
                bind_entry(2, &buf_out),
                bind_entry(3, &buf_ctrl),
            ],
        });
        record_compute(
            &mut encoder,
            &self.reduce_partial.pipeline,
            &bind_group,
            (partials as u32, 1, 1),
        );

        // Stage 2 — collapses `out[0..partials]` into `out[0]`. The op tag must
        // be propagated so the finalize folds with the same monoid as stage 1
        // (a max-abs reduction cannot be finalized by summing its partials).
        if partials > 1 {
            let params2 = [
                u32_from(len, "reduce finalize len")?,
                op.tag(),
                u32_from(partials, "reduce finalize")?,
                0,
            ];
            let buf_ctrl2 = self.control_buffer(ctx, &params2)?;
            let bind_group2 = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("scico reduce finalize"),
                layout: &self.reduce_finalize.bind_group_layout,
                entries: &[
                    bind_entry(0, &buf_a),
                    bind_entry(1, &buf_b),
                    bind_entry(2, &buf_out),
                    bind_entry(3, &buf_ctrl2),
                ],
            });
            record_compute(
                &mut encoder,
                &self.reduce_finalize.pipeline,
                &bind_group2,
                (1, 1, 1),
            );
        }

        let readback = self.readback_buffer(ctx.precision().element_size());
        encoder.copy_buffer_to_buffer(&buf_out, 0, &readback, 0, self.precision.element_size());
        ctx.submit_and_wait(encoder)?;

        let values = download(ctx, &readback, 1, self.precision)?;
        Ok(values.first().copied().unwrap_or(0.0))
    }

    /// Shared launch path for the element-wise kernels: upload inputs, bind the
    /// output buffer, dispatch, and read the result back.
    fn element_launch(
        &self,
        ctx: &GpuContext,
        kernel: &Kernel,
        params: [u32; 4],
        inputs: (Option<&[Scalar]>, Option<&[Scalar]>),
        out_len: usize,
        workgroups: (u32, u32, u32),
    ) -> Result<Vec<Scalar>, SimError> {
        let device = ctx.device();
        let empty: [Scalar; 0] = [];
        let buf_first = self.upload(ctx, inputs.0.unwrap_or(&empty))?;
        let buf_second = self.upload(ctx, inputs.1.unwrap_or(&empty))?;
        let buf_out = self.output_buffer(out_len as u64 * self.precision.element_size());
        let buf_ctrl = self.control_buffer(ctx, &params)?;

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scico elementwise"),
            layout: &kernel.bind_group_layout,
            entries: &[
                bind_entry(0, &buf_first),
                bind_entry(1, &buf_second),
                bind_entry(2, &buf_out),
                bind_entry(3, &buf_ctrl),
            ],
        });

        let mut encoder = device.create_command_encoder(&encoder_descriptor());
        record_compute(&mut encoder, &kernel.pipeline, &bind_group, workgroups);

        let readback = self.readback_buffer(out_len as u64 * self.precision.element_size());
        let byte_len = (out_len as u64) * self.precision.element_size();
        encoder.copy_buffer_to_buffer(&buf_out, 0, &readback, 0, byte_len);
        ctx.submit_and_wait(encoder)?;

        download(ctx, &readback, out_len, self.precision)
    }

    /// Out-of-place transpose of an `rows × cols` row-major matrix.
    ///
    /// Returns the `cols × rows` row-major result. The launch reuses the
    /// three-binding `(in, out, control)` shape of the scalar kernel, so the
    /// device-side work is a single gather pass with no host-side permutation.
    pub fn transpose(
        &self,
        ctx: &GpuContext,
        input: &[Scalar],
        rows: usize,
        cols: usize,
    ) -> Result<Vec<Scalar>, SimError> {
        if rows == 0 || cols == 0 {
            return Ok(Vec::new());
        }
        let len = rows
            .checked_mul(cols)
            .ok_or_else(|| SimError::numerical("wgpu transpose: dimensions overflow usize"))?;
        if input.len() != len {
            return Err(SimError::numerical(format!(
                "wgpu transpose: input len={} but {rows}x{cols}={len}",
                input.len()
            )));
        }
        let params = [
            u32_from(rows, "transpose rows")?,
            u32_from(cols, "transpose cols")?,
            0,
            0,
        ];
        self.element_launch3(
            ctx,
            &self.transpose,
            params,
            input,
            len,
            dispatch_elements(len),
        )
    }

    /// Check out a read-only input buffer and fill it with `data`.
    ///
    /// The values are narrowed to `f32` first when the device runs the `f32`
    /// kernel set; the buffer is then written through a staging slice so no
    /// extra allocation is needed.
    fn upload(&self, ctx: &GpuContext, data: &[Scalar]) -> Result<PooledBuffer, SimError> {
        let bytes = encode_elements(self.precision, data);
        let buf = self.pool.acquire(BufferRoleSpec::Input, bytes.len() as u64);
        if !bytes.is_empty() {
            ctx.queue().write_buffer(buf.buffer(), 0, &bytes);
        }
        Ok(buf)
    }

    /// Check out a writable output buffer of at least `bytes` capacity.
    fn output_buffer(&self, bytes: u64) -> PooledBuffer {
        self.pool.acquire(BufferRoleSpec::Output, bytes)
    }

    /// Check out a host-readable staging buffer of at least `bytes` capacity.
    fn readback_buffer(&self, bytes: u64) -> PooledBuffer {
        self.pool.acquire(BufferRoleSpec::Readback, bytes)
    }

    /// Check out a control buffer and fill it with the four `u32` slots.
    ///
    /// Four `u32` slots are exactly 16 bytes, satisfying the `vec4` uniform
    /// stride rule, and `vec4<u32>` is layout-compatible with
    /// `array<u32, 4>` so the same bytes serve both the uniform-declaring
    /// kernels (matrix/binary/reduce) and the storage-array ones (scale/axpy).
    fn control_buffer(
        &self,
        ctx: &GpuContext,
        params: &[u32; 4],
    ) -> Result<PooledBuffer, SimError> {
        let bytes = as_bytes(params);
        let buf = self
            .pool
            .acquire(BufferRoleSpec::Control, bytes.len() as u64);
        ctx.queue().write_buffer(buf.buffer(), 0, bytes);
        Ok(buf)
    }
}

// ──────────────────────────────────────────────
// Pipeline construction helpers
// ──────────────────────────────────────────────

fn build_kernel(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    entry_point: &str,
    layout: &wgpu::BindGroupLayout,
    label: &str,
) -> Kernel {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(label),
        layout: Some(&pipeline_layout),
        module,
        entry_point: Some(entry_point),
        compilation_options: Default::default(),
        cache: None,
    });
    Kernel {
        pipeline,
        bind_group_layout: layout.clone(),
    }
}

/// Bind group layout for the matrix-multiply kernel (A, B, C, uniform).
fn matmul_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    four_binding_layout(device, "scico matmul layout")
}

/// Bind group layout for the element-wise kernels (a, b, out, uniform).
fn binary_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    four_binding_layout(device, "scico binary layout")
}

/// Bind group layout for the transpose kernel: one read-only input, one
/// read-write output and the control buffer.
fn transpose_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("scico transpose layout"),
        entries: &[
            storage_entry(0, true),
            storage_entry(1, false),
            storage_entry(2, true),
        ],
    })
}

/// Bind group layout for the scalar-vector kernel: one read-only input, one
/// read-write output and the control buffer.
fn scale_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("scico scale layout"),
        entries: &[
            storage_entry(0, true),
            storage_entry(1, false),
            storage_entry(2, true),
        ],
    })
}

/// Bind group layout for AXPY: `(x, y, out, control)`.
fn axpy_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("scico axpy layout"),
        entries: &[
            storage_entry(0, true),
            storage_entry(1, true),
            storage_entry(2, false),
            storage_entry(3, true),
        ],
    })
}

/// Bind group layout shared by both reduction stages.
fn reduce_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    four_binding_layout(device, "scico reduce layout")
}

/// The common four-binding layout: `(read, read, read-write, uniform)`.
///
/// Used by every kernel that declares two storage inputs plus an output and a
/// uniform — matrix multiply, the element-wise binary op and the reductions.
fn four_binding_layout(device: &wgpu::Device, label: &str) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[
            storage_entry(0, true),
            storage_entry(1, true),
            storage_entry(2, false),
            uniform_entry(3),
        ],
    })
}

fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

// ──────────────────────────────────────────────
// Buffer helpers
// ──────────────────────────────────────────────

fn encoder_descriptor() -> wgpu::CommandEncoderDescriptor<'static> {
    wgpu::CommandEncoderDescriptor {
        label: Some("scico compute encoder"),
    }
}

/// Create a storage buffer sized for `len` elements, zero-initialised.
/// Encode host `f64` values into the byte layout of the active storage type.
fn encode_elements(precision: GpuPrecision, data: &[Scalar]) -> Vec<u8> {
    match precision {
        GpuPrecision::F64 => as_bytes(data).to_vec(),
        GpuPrecision::F32 => {
            let narrowed: Vec<f32> = data.iter().map(|&v| v as f32).collect();
            as_bytes(&narrowed).to_vec()
        }
    }
}

/// Map `buffer` and copy `len` elements into a host vector.
fn download(
    ctx: &GpuContext,
    buffer: &wgpu::Buffer,
    len: usize,
    precision: GpuPrecision,
) -> Result<Vec<Scalar>, SimError> {
    let device = ctx.device();
    // The mapped range must match the precision the buffer was written in, not
    // the host `f64` width: on an `f32` device the buffer holds 4 bytes per
    // element, so slicing `len * 8` would run past the end of the allocation.
    let byte_len = (len.max(1) as u64) * precision.element_size();
    let slice = buffer.slice(..byte_len);

    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = tx.send(result);
    });
    device
        .poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })
        .map_err(|e| SimError::runtime(format!("wgpu: poll while mapping failed: {e}")))?;
    rx.recv()
        .map_err(|e| SimError::runtime(format!("wgpu: map callback dropped: {e}")))?
        .map_err(|e| SimError::runtime(format!("wgpu: buffer map failed: {e}")))?;

    let view = slice
        .get_mapped_range()
        .map_err(|e| SimError::runtime(format!("wgpu: mapped range unavailable: {e}")))?;
    let out = decode_elements(precision, &view, len);
    drop(view);
    buffer.unmap();
    Ok(out)
}

/// Decode a mapped device buffer into host `f64` values.
fn decode_elements(precision: GpuPrecision, bytes: &[u8], len: usize) -> Vec<Scalar> {
    match precision {
        GpuPrecision::F64 => {
            let src = from_bytes(bytes, len);
            src.to_vec()
        }
        GpuPrecision::F32 => {
            let src = from_bytes::<f32>(bytes, len);
            src.iter().map(|&v| v as Scalar).collect()
        }
    }
}

fn bind_entry(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
}

/// Record one compute pass into `encoder`.
fn record_compute(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::ComputePipeline,
    bind_group: &wgpu::BindGroup,
    workgroups: (u32, u32, u32),
) {
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("scico compute pass"),
        timestamp_writes: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    pass.dispatch_workgroups(workgroups.0, workgroups.1, workgroups.2);
}

// ──────────────────────────────────────────────
// Dispatch arithmetic
// ──────────────────────────────────────────────

/// Workgroup grid for a 1-D element-wise kernel.
fn dispatch_elements(len: usize) -> (u32, u32, u32) {
    let groups = len.div_ceil(ELEMENT_WORKGROUP as usize).max(1);
    (groups as u32, 1, 1)
}

/// Workgroup grid for the tiled matrix-multiply kernel (one thread per entry).
fn dispatch_matmul(m: usize, n: usize) -> (u32, u32, u32) {
    let tile = MATMUL_TILE as usize;
    (n.div_ceil(tile) as u32, m.div_ceil(tile) as u32, 1)
}

/// Number of partials emitted by reduction stage 1 for `len` elements.
fn partial_count(len: usize) -> Result<usize, SimError> {
    let per_workgroup = (REDUCE_WORKGROUP * REDUCE_STRIDE) as usize;
    let groups = len.div_ceil(per_workgroup).max(1);
    if groups > u32::MAX as usize {
        return Err(SimError::numerical(format!(
            "wgpu reduce: {len} elements needs {groups} workgroups, exceeding the u32 grid limit"
        )));
    }
    Ok(groups)
}

/// Convert a length to `u32`, reporting the overflow instead of truncating.
fn u32_from(value: usize, what: &str) -> Result<u32, SimError> {
    u32::try_from(value).map_err(|_| {
        SimError::numerical(format!("wgpu {what}: {value} exceeds the u32 kernel limit"))
    })
}

/// Split an `f64` into little-endian low/high `u32` halves for the uniform.
fn split_f64(value: Scalar) -> (u32, u32) {
    let bits = value.to_bits();
    (bits as u32, (bits >> 32) as u32)
}

// ──────────────────────────────────────────────
// f64 <-> bytes (no external bytemuck dependency)
// ──────────────────────────────────────────────

fn as_bytes<T>(values: &[T]) -> &[u8] {
    // SAFETY: `T` is plain-old-data with no padding for every type used here
    // (`f64`, `u32`, `[u32; 4]`), and the returned slice borrows the same memory
    // for the same lifetime, so it cannot outlive the data it points to.
    unsafe {
        std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values))
    }
}

fn from_bytes<T>(bytes: &[u8], len: usize) -> &[T] {
    // SAFETY: the callers pass a mapped buffer that is at least
    // `len * size_of::<T>()` bytes long (see `readback_buffer`), and the
    // returned slice borrows the mapping so it cannot outlive it.
    unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<T>(), len) }
}

/// Readback buffers are sized in `f64` units; the f32 view is half as wide but
/// never longer than the allocation.
const _: () = assert!(size_of::<Scalar>() >= size_of::<f32>());

#[cfg(feature = "gpu")]
#[cfg(test)]
mod tests {
    use super::*;

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

    fn context() -> Option<GpuContext> {
        GpuContext::new().ok()
    }

    #[test]
    fn split_and_reassemble_f64() {
        for v in [0.0, 1.0, -1.0, 2.5, -0.75, 1e-300, 1.234_567_890_123_45e5] {
            let (lo, hi) = split_f64(v);
            let bits = (hi as u64) << 32 | lo as u64;
            assert_eq!(Scalar::from_bits(bits), v, "round-trip failed for {v:e}");
        }
    }

    #[test]
    fn dispatch_arithmetic_is_exact_at_boundaries() {
        assert_eq!(dispatch_elements(0), (1, 1, 1));
        assert_eq!(dispatch_elements(64), (1, 1, 1));
        assert_eq!(dispatch_elements(65), (2, 1, 1));
        // One thread per output element, 16×16 per workgroup.
        assert_eq!(dispatch_matmul(16, 16), (1, 1, 1));
        assert_eq!(dispatch_matmul(16, 17), (2, 1, 1));
        assert_eq!(dispatch_matmul(17, 16), (1, 2, 1));
        assert_eq!(partial_count(1).unwrap(), 1);
        assert_eq!(partial_count(16_384).unwrap(), 1);
        assert_eq!(partial_count(16_385).unwrap(), 2);
    }

    #[test]
    fn binary_op_tags_match_wgsl_abi() {
        assert_eq!(BinaryOp::Add.tag(), 0);
        assert_eq!(BinaryOp::Sub.tag(), 1);
        assert_eq!(BinaryOp::Mul.tag(), 2);
        assert_eq!(BinaryOp::Abs.tag(), 3);
        assert_eq!(BinaryOp::Dot.tag(), 0);
        assert_eq!(BinaryOp::Sum.tag(), 1);
        assert_eq!(BinaryOp::Asum.tag(), 2);
        assert_eq!(BinaryOp::MaxAbs.tag(), 3);
        assert!(BinaryOp::Dot.is_reduction());
        assert!(BinaryOp::Sum.is_reduction());
        assert!(BinaryOp::Asum.is_reduction());
        assert!(BinaryOp::MaxAbs.is_reduction());
        assert!(!BinaryOp::Add.is_reduction());
        assert!(!BinaryOp::Abs.is_reduction());
        assert_eq!(BinaryOp::Add.name(), "add");
        assert_eq!(BinaryOp::MaxAbs.name(), "max_abs");
    }

    #[test]
    fn kernels_run_reductions_and_transpose_on_device() {
        let Some(ctx) = context() else { return };
        let kernels = GpuKernels::new(&ctx).unwrap();
        let a = rand_vec(5000, 0xC3);
        let asum = kernels.reduce(&ctx, BinaryOp::Asum, &a, &a).unwrap();
        let max_abs = kernels.reduce(&ctx, BinaryOp::MaxAbs, &a, &a).unwrap();
        let cpu_asum: Scalar = a.iter().map(|v| v.abs()).sum();
        let cpu_max: Scalar = a.iter().map(|v| v.abs()).fold(0.0, Scalar::max);
        let tol = 1e-3 * cpu_asum.abs().max(1.0);
        assert!((asum - cpu_asum).abs() <= tol, "asum {asum} vs {cpu_asum}");
        let tol_max = 1e-3 * cpu_max.abs().max(1.0);
        assert!(
            (max_abs - cpu_max).abs() <= tol_max,
            "max_abs {max_abs} vs {cpu_max}"
        );

        // Transpose a 3×5 into 5×3 and compare with the CPU permutation.
        let (rows, cols) = (3usize, 5usize);
        let m = rand_vec(rows * cols, 0xD4);
        let gpu_t = kernels.transpose(&ctx, &m, rows, cols).unwrap();
        assert_eq!(gpu_t.len(), rows * cols);
        for r in 0..rows {
            for c in 0..cols {
                let expected = m[r * cols + c];
                let got = gpu_t[c * rows + r];
                assert!(
                    (got - expected).abs() < 1e-6,
                    "({r},{c}) {got} vs {expected}"
                );
            }
        }
    }

    #[test]
    fn u32_conversion_reports_overflow() {
        assert_eq!(u32_from(7, "x").unwrap(), 7);
        assert!(u32_from(usize::MAX, "x").is_err());
    }

    #[test]
    fn kernels_build_and_run_scale_on_device() {
        let Some(ctx) = context() else { return };
        let kernels = GpuKernels::new(&ctx).unwrap();
        let a = rand_vec(1000, 0x99);
        let out = kernels.scale(&ctx, 3.0, &a).unwrap();
        assert_eq!(out.len(), a.len());
        for (i, (o, v)) in out.iter().zip(&a).enumerate() {
            let expected = 3.0 * v;
            // The device may use f32 storage; compare at that precision.
            let tol = 1e-3 * expected.abs().max(1.0);
            assert!((o - expected).abs() <= tol, "index {i}: {o} vs {expected}");
        }
    }

    #[test]
    fn kernels_mat_mul_on_device() {
        let Some(ctx) = context() else { return };
        let kernels = GpuKernels::new(&ctx).unwrap();
        let (m, k, n) = (5, 7, 3);
        let a = rand_vec(m * k, 0xA1);
        let b = rand_vec(k * n, 0xB2);
        let cpu_a: Vec<Vec<Scalar>> = a.chunks(k).map(|r| r.to_vec()).collect();
        let cpu_b: Vec<Vec<Scalar>> = b.chunks(n).map(|r| r.to_vec()).collect();
        let cpu = crate::core::compute::matrix::mat_mul(&cpu_a, &cpu_b).unwrap();
        let gpu = kernels.mat_mul(&ctx, &a, &b, m, k, n).unwrap();
        assert_eq!(gpu.len(), m * n);
        for i in 0..m {
            for j in 0..n {
                let expected = cpu[i][j];
                let got = gpu[i * n + j];
                assert!(
                    (expected - got).abs() <= 1e-3 * expected.abs().max(1.0),
                    "[{i},{j}] {expected} vs {got}"
                );
            }
        }
    }

    #[test]
    fn kernels_reduce_on_device() {
        let Some(ctx) = context() else { return };
        let kernels = GpuKernels::new(&ctx).unwrap();
        let a = rand_vec(9_000, 0xC3);
        let got = kernels.reduce(&ctx, BinaryOp::Sum, &a, &a).unwrap();
        let expected: Scalar = a.iter().sum();
        assert!((got - expected).abs() <= 1e-3 * expected.abs().max(1.0));
    }
}
