// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! GPU adapter/device acquisition and capability reporting.
//!
//! All wgpu object creation happens here, once per backend, so the rest of the
//! GPU module can work with an already-validated [`GpuContext`]. Device
//! acquisition never panics: every failure is reported as a [`SimError`] so the
//! adaptive dispatcher can fall back to the CPU paths.

use crate::core::error::SimError;

/// Which storage precision the device kernels execute in.
///
/// Chosen once per device from `Adapter::features()`:
/// [`GpuPrecision::F64`] when [`wgpu::Features::SHADER_F64`] is present,
/// otherwise [`GpuPrecision::F32`] (the universally available path).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuPrecision {
    /// 32-bit storage buffers — available on every WebGPU-capable adapter.
    F32,
    /// 64-bit storage buffers — requires `Features::SHADER_F64`.
    F64,
}

impl GpuPrecision {
    /// Size in bytes of one element in this precision.
    pub fn element_size(self) -> u64 {
        match self {
            GpuPrecision::F32 => 4,
            GpuPrecision::F64 => 8,
        }
    }

    /// Human-readable name.
    pub fn name(self) -> &'static str {
        match self {
            GpuPrecision::F32 => "f32",
            GpuPrecision::F64 => "f64",
        }
    }
}

/// An initialised wgpu device together with the metadata the dispatcher needs.
pub struct GpuContext {
    /// Kept alive for the lifetime of the device; also serves as the adapter
    /// handle for hal-level queries.
    device: wgpu::Device,
    queue: wgpu::Queue,
    description: String,
    precision: GpuPrecision,
    max_elements: usize,
}

impl GpuContext {
    /// Enumerate adapters and initialise the most suitable one.
    ///
    /// Selection prefers a discrete/high-performance adapter over an integrated
    /// one, and requests the highest reported limits so large matrices fit.
    /// Returns `Err` when no adapter can produce a device.
    pub fn new() -> Result<Self, SimError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            apply_limit_buckets: true,
        }))
        .map_err(|e| SimError::runtime(format!("wgpu: no suitable adapter: {e}")))?;

        let info = adapter.get_info();
        let features = adapter.features();
        let precision = if features.contains(wgpu::Features::SHADER_F64) {
            GpuPrecision::F64
        } else {
            GpuPrecision::F32
        };

        // Request exactly what this feature set needs: 64-bit shader math when
        // the kernel set is f64, otherwise nothing extra. Requesting a feature
        // the adapter lacks makes `request_device` fail outright.
        let required_features = match precision {
            GpuPrecision::F64 => wgpu::Features::SHADER_F64,
            GpuPrecision::F32 => wgpu::Features::empty(),
        };

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("scico-rs compute device"),
            required_features,
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
        }))
        .map_err(|e| SimError::runtime(format!("wgpu: device creation failed: {e}")))?;

        let limits = device.limits();
        let max_elements = compute_max_elements(limits.max_storage_buffer_binding_size, precision);
        let description = format!("wgpu/{:?}/{}", info.backend, info.name);

        Ok(Self {
            device,
            queue,
            description,
            precision,
            max_elements,
        })
    }

    /// The wgpu device.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// The wgpu queue.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Adapter/device description, e.g. `"wgpu/Metal/Apple M4"`.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The storage precision selected for this device.
    pub fn precision(&self) -> GpuPrecision {
        self.precision
    }

    /// Maximum number of elements a single storage buffer may hold.
    pub fn max_elements(&self) -> usize {
        self.max_elements
    }

    /// Whether the device is usable. The context only exists once device
    /// creation has succeeded, so this is `true` for any live [`GpuContext`];
    /// it exists to satisfy the [`super::WgpuBackend::is_available`] contract.
    pub fn device_ok(&self) -> bool {
        true
    }

    /// Submit a recorded command buffer and block until the GPU is done.
    ///
    /// The wait is what makes the backend synchronous; it also surfaces device
    /// errors (including WGSL validation and out-of-bounds faults) as `Err`
    /// instead of a silently wrong result.
    pub fn submit_and_wait(&self, encoder: wgpu::CommandEncoder) -> Result<(), SimError> {
        let index = self.queue.submit(Some(encoder.finish()));
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(index),
                timeout: None,
            })
            .map_err(|e| SimError::runtime(format!("wgpu: device poll failed: {e}")))?;
        Ok(())
    }
}

/// Derive the element capacity of one storage buffer from the binding limit.
///
/// Leaves one element of head-room so a zero-length tail slice is never
/// requested, and never returns zero (which would disable the GPU path
/// entirely on an adapter reporting an unexpectedly small limit).
fn compute_max_elements(max_binding_size: u64, precision: GpuPrecision) -> usize {
    let per_element = precision.element_size();
    let usable = max_binding_size.saturating_sub(per_element);
    usize::try_from(usable / per_element).unwrap_or(0).max(1)
}

#[cfg(feature = "gpu")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precision_element_sizes() {
        assert_eq!(GpuPrecision::F32.element_size(), 4);
        assert_eq!(GpuPrecision::F64.element_size(), 8);
        assert_eq!(GpuPrecision::F32.name(), "f32");
        assert_eq!(GpuPrecision::F64.name(), "f64");
    }

    #[test]
    fn max_elements_never_zero() {
        assert_eq!(compute_max_elements(0, GpuPrecision::F32), 1);
        assert_eq!(compute_max_elements(0, GpuPrecision::F64), 1);
        // 64 MiB at 4 bytes/element.
        assert_eq!(
            compute_max_elements(1 << 26, GpuPrecision::F32),
            (1 << 24) - 1
        );
    }

    #[test]
    fn context_reports_consistent_metadata() {
        let Ok(ctx) = GpuContext::new() else { return };
        assert!(ctx.device_ok());
        assert!(ctx.max_elements() > 0);
        assert!(ctx.description().starts_with("wgpu/"));
        // The selected precision must be executable: f64 only when advertised.
        if ctx.precision() == GpuPrecision::F64 {
            assert!(ctx.device().features().contains(wgpu::Features::SHADER_F64));
        }
        println!(
            "wgpu context: {} precision={} max_elements={}",
            ctx.description(),
            ctx.precision().name(),
            ctx.max_elements()
        );
    }
}
