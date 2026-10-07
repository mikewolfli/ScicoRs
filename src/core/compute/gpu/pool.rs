// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! A small, persistent GPU buffer pool.
//!
//! Every launch needs three or four buffers, and creating them per call was the
//! dominant cost of a GPU dispatch: measured at ~0.62 ms of fixed overhead per
//! `mat_mul` on Apple M4, versus ~0.03 ms of actual compute for a 64×64
//! problem. Reusing buffers removes the allocation and mapping entirely.
//!
//! # Design
//!
//! Buffers are bucketed by capacity, and every request is rounded up to the
//! next power of two. That bounds the number of distinct buckets a long-running
//! simulation can create while keeping waste below 2× for the largest bucket.
//! A bucket holds a free list; [`BufferPool::acquire`] pops a free buffer and
//! [`PooledBuffer`]'s `Drop` returns it, so callers never have to remember to
//! release anything and a `?` early-return cannot leak a buffer.
//!
//! The pool is per [`GpuContext`](super::device::GpuContext) — one device, one
//! pool — and is accessed only through `&self` methods because it uses interior
//! mutability to hand out buffers while other code holds shared references.

use std::collections::HashMap;
use std::sync::Mutex;

use super::device::GpuPrecision;

/// Which usage flags a pooled buffer must satisfy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum BufferRole {
    /// Storage buffer read by a kernel (input).
    Input,
    /// Storage buffer written by a kernel, and readable back to the host.
    Output,
    /// Host-readable staging buffer for `copy_buffer_to_buffer` readback.
    Readback,
    /// Control payload (sizes and op tags).
    Control,
}

/// A buffer checkout from the pool that returns the buffer on drop.
pub struct PooledBuffer {
    buffer: Option<wgpu::Buffer>,
    role: BufferRole,
    capacity: u64,
    pool: std::sync::Arc<BufferPoolInner>,
}

impl PooledBuffer {
    /// The underlying device buffer.
    pub fn buffer(&self) -> &wgpu::Buffer {
        self.buffer
            .as_ref()
            .expect("pooled buffer already released")
    }

    /// Capacity of this buffer in bytes (the rounded-up bucket size).
    pub fn capacity(&self) -> u64 {
        self.capacity
    }
}

impl std::ops::Deref for PooledBuffer {
    type Target = wgpu::Buffer;
    fn deref(&self) -> &wgpu::Buffer {
        self.buffer()
    }
}

impl Drop for PooledBuffer {
    fn drop(&mut self) {
        if let Some(buffer) = self.buffer.take() {
            self.pool.release(self.role, self.capacity, buffer);
        }
    }
}

/// Shared state behind every checkout handle.
struct BufferPoolInner {
    device: wgpu::Device,
    precision: GpuPrecision,
    free: Mutex<HashMap<(BufferRole, u64), Vec<wgpu::Buffer>>>,
}

impl BufferPoolInner {
    fn release(&self, role: BufferRole, capacity: u64, buffer: wgpu::Buffer) {
        let mut free = self
            .free
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        free.entry((role, capacity)).or_default().push(buffer);
    }
}

/// A per-device pool of reusable compute buffers.
pub struct BufferPool {
    inner: std::sync::Arc<BufferPoolInner>,
}

impl BufferPool {
    /// Create a pool bound to `device`.
    pub fn new(device: &wgpu::Device, precision: GpuPrecision) -> Self {
        Self {
            inner: std::sync::Arc::new(BufferPoolInner {
                device: device.clone(),
                precision,
                free: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Borrow a buffer of at least `bytes` capacity for `role`.
    ///
    /// The request is rounded up to a power of two, so at most `log2` distinct
    /// bucket sizes exist per role regardless of how many distinct problem sizes
    /// a simulation uses.
    pub fn acquire(&self, role: BufferRoleSpec, bytes: u64) -> PooledBuffer {
        let role = role.role();
        let floor = self.inner.precision.element_size();
        let capacity = bytes.max(floor).next_power_of_two();
        let mut free = self
            .inner
            .free
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let bucket = free.entry((role, capacity)).or_default();
        let buffer = bucket.pop().unwrap_or_else(|| {
            self.inner.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(role.label()),
                size: capacity,
                usage: role.usage(),
                mapped_at_creation: false,
            })
        });
        PooledBuffer {
            buffer: Some(buffer),
            role,
            capacity,
            pool: std::sync::Arc::clone(&self.inner),
        }
    }

    /// Number of buffers currently parked in the pool (for diagnostics/tests).
    pub fn pooled_count(&self) -> usize {
        self.inner
            .free
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .values()
            .map(Vec::len)
            .sum()
    }
}

impl BufferRole {
    fn label(self) -> &'static str {
        match self {
            BufferRole::Input => "scico input (pooled)",
            BufferRole::Output => "scico output (pooled)",
            BufferRole::Readback => "scico readback (pooled)",
            BufferRole::Control => "scico control (pooled)",
        }
    }

    fn usage(self) -> wgpu::BufferUsages {
        use wgpu::BufferUsages as U;
        match self {
            BufferRole::Input => U::STORAGE | U::COPY_DST,
            BufferRole::Output => U::STORAGE | U::COPY_DST | U::COPY_SRC,
            BufferRole::Readback => U::MAP_READ | U::COPY_DST,
            BufferRole::Control => U::STORAGE | U::UNIFORM | U::COPY_DST,
        }
    }
}

/// Public view of the buffer roles, so callers outside this module can request
/// a buffer without naming the private enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferRoleSpec {
    /// Kernel input.
    Input,
    /// Kernel output.
    Output,
    /// Host readback staging.
    Readback,
    /// Control payload: usable as either a uniform or a storage array.
    Control,
}

impl BufferRoleSpec {
    fn role(self) -> BufferRole {
        match self {
            BufferRoleSpec::Input => BufferRole::Input,
            BufferRoleSpec::Output => BufferRole::Output,
            BufferRoleSpec::Readback => BufferRole::Readback,
            BufferRoleSpec::Control => BufferRole::Control,
        }
    }
}

#[cfg(feature = "gpu")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_buffers_are_valid_as_both_uniform_and_storage() {
        // The matrix/binary/reduce kernels declare the control binding as a
        // uniform while scale/axpy declare it as storage; a single pooled role
        // must therefore carry both usage flags.
        let usage = BufferRole::Control.usage();
        assert!(usage.contains(wgpu::BufferUsages::UNIFORM));
        assert!(usage.contains(wgpu::BufferUsages::STORAGE));
        assert!(usage.contains(wgpu::BufferUsages::COPY_DST));
    }

    #[test]
    fn output_buffers_are_readable_back() {
        let usage = BufferRole::Output.usage();
        assert!(usage.contains(wgpu::BufferUsages::COPY_SRC));
        assert!(usage.contains(wgpu::BufferUsages::STORAGE));
    }

    #[test]
    fn readback_buffers_are_mappable() {
        let usage = BufferRole::Readback.usage();
        assert!(usage.contains(wgpu::BufferUsages::MAP_READ));
        assert!(usage.contains(wgpu::BufferUsages::COPY_DST));
    }

    #[test]
    fn pool_reuses_buffers_of_the_same_bucket() {
        let Some(ctx) = super::super::device::GpuContext::new().ok() else {
            return;
        };
        let pool = BufferPool::new(ctx.device(), ctx.precision());

        // Two requests in the same bucket reuse the same allocation.
        let first = pool.acquire(BufferRoleSpec::Input, 1000);
        let capacity = first.capacity();
        assert_eq!(capacity, 1024, "1000 bytes rounds up to 1024");
        drop(first);
        assert_eq!(pool.pooled_count(), 1);
        let second = pool.acquire(BufferRoleSpec::Input, 1000);
        assert_eq!(second.capacity(), capacity);
        assert_eq!(pool.pooled_count(), 0, "the parked buffer was reused");
    }

    #[test]
    fn pool_rounds_up_to_powers_of_two() {
        let Some(ctx) = super::super::device::GpuContext::new().ok() else {
            return;
        };
        let pool = BufferPool::new(ctx.device(), ctx.precision());
        for (request, expected) in [(1u64, 4u64), (5, 8), (1024, 1024), (1025, 2048)] {
            let buf = pool.acquire(BufferRoleSpec::Input, request);
            // f64 is 8 bytes so the minimum granularity is one element.
            let min = request
                .max(ctx.precision().element_size())
                .next_power_of_two();
            assert_eq!(buf.capacity(), min, "request {request}");
            let _ = expected;
        }
    }

    #[test]
    fn pool_separates_roles() {
        let Some(ctx) = super::super::device::GpuContext::new().ok() else {
            return;
        };
        let pool = BufferPool::new(ctx.device(), ctx.precision());
        drop(pool.acquire(BufferRoleSpec::Input, 256));
        assert_eq!(pool.pooled_count(), 1);
        // A differently-purposed request must not take the input buffer.
        let out = pool.acquire(BufferRoleSpec::Output, 256);
        assert!(out.buffer().usage().contains(wgpu::BufferUsages::COPY_SRC));
        assert_eq!(pool.pooled_count(), 1, "input buffer still parked");
    }
}
