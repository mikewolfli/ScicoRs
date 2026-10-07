// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
//! Stable C ABI facade (Phase 41).
//!
//! This module exposes a minimal, **stable** C-compatible interface to the
//! simulation kernel. It is deliberately small and conservative:
//!
//! * Only **opaque handles** cross the boundary ([`ScicoHandle`]). A caller on
//!   the C side never sees a Rust struct's layout.
//! * Only **fixed-width integer types** (`i32`, `u32`, `u64`, `i64`) and
//!   `double` are used in signatures — never `String`, trait objects, or
//!   `#[repr(Rust)]` structs.
//! * **Ownership release is explicit**: every handle created by
//!   [`scico_create`] must be released exactly once by [`scico_free`].
//! * Every entry point returns a **stable error code** ([`ScicoStatus`]);
//!   domain results are written through out-parameters.
//!
//! # Working example (create → configure → run → read → free)
//!
//! ```c
//! ScicoHandle *h = scico_create();
//! if (!h) { /* allocation failed */ }
//! ScicoStatus s = scico_config(h, 0.0, 1.0, 0.1, 4);
//! if (s != SCICO_OK) { scico_free(h); }
//! s = scico_run(h);
//! uint64_t steps = 0;
//! scico_step_count(h, &steps);
//! double value = 0.0;
//! scico_read_output(h, 0, &value);
//! scico_free(h);
//! ```
//!
//! # Safety contract
//!
//! Every `extern "C"` function is `unsafe` to call from Rust because it
//! dereferences a raw pointer supplied by the caller. The C caller's contract:
//! The C caller's contract:
//! pass only pointers returned by [`scico_create`], and pass each handle to
//! [`scico_free`] exactly once. A double free or a call through a
//! never-created/freed pointer is detected via a live-handle registry and
//! reported as an error rather than freeing invalid memory.
//!
//! # In-process execution
//!
//! As with plugins, everything here runs **in the caller's process**. The C ABI
//! is not an isolation boundary.

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::runtime::context::TimeConfig;

/// The C-visible API version, `major * 1000 + minor`.
pub const SCICO_C_API_VERSION: u32 = 1000;

/// Stable error codes returned across the C boundary.
///
/// The numeric values are part of the ABI and must not change.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScicoStatus {
    /// The call succeeded.
    Ok = 0,
    /// A null handle (or null out-pointer) was passed.
    NullHandle = -1,
    /// The handle has already been freed (double free / use-after-free).
    AlreadyFreed = -2,
    /// The configuration is invalid (non-positive step, end <= start, ...).
    InvalidConfig = -3,
    /// The run failed numerically.
    NumericalFailure = -4,
    /// An output index is out of range.
    OutOfRange = -5,
    /// A panic was caught inside the boundary (mapped, never propagated).
    InternalPanic = -6,
}

impl ScicoStatus {
    /// The numeric code as an `i32`.
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

/// Global registry of live handle addresses.
///
/// Double-free and use-after-free detection must not depend on reading a freed
/// allocation (that is undefined behaviour), so every handle's address is
/// recorded here on [`scico_create`] and removed on [`scico_free`]. A pointer
/// that is absent from the registry was either never created by this API or has
/// already been freed.
static LIVE_HANDLES: std::sync::Mutex<Option<std::collections::HashSet<usize>>> =
    std::sync::Mutex::new(None);

/// The opaque, C-visible handle.
///
/// Its fields are private and its layout is **not** part of the ABI: C code
/// only ever holds a `ScicoHandle *` and passes it back to these functions.
///
/// The handle is a small opaque *context*: a time configuration plus the
/// results of the most recent run (the number of steps and a fixed-size buffer
/// of output values). It has no Rust-visible generic parameters or trait
/// objects, so it can live behind a raw pointer.
pub struct ScicoHandle {
    /// Whether a configuration has been applied.
    configured: bool,
    /// Configured time range and step.
    config: TimeConfig,
    /// Number of requested outputs.
    output_count: usize,
    /// Steps executed by the most recent successful run.
    steps_executed: u64,
    /// Output values produced by the most recent successful run.
    outputs: Vec<f64>,
}

/// Maximum number of outputs a handle can hold.
///
/// A fixed upper bound keeps the C contract simple; requesting more yields
/// [`ScicoStatus::InvalidConfig`] from [`scico_config`].
pub const SCICO_MAX_OUTPUTS: usize = 64;

/// Run `f` with the live-handle registry borrowed mutably.
fn with_registry<T>(f: impl FnOnce(&mut std::collections::HashSet<usize>) -> T) -> T {
    let mut guard = LIVE_HANDLES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let set = guard.get_or_insert_with(std::collections::HashSet::new);
    f(set)
}

/// Record a handle address as live.
fn register_handle(addr: usize) {
    with_registry(|set| {
        set.insert(addr);
    });
}

/// Remove a handle address from the live registry, returning whether it was
/// present (i.e. whether this is the first free of that handle).
fn unregister_handle(addr: usize) -> bool {
    with_registry(|set| set.remove(&addr))
}

/// Whether an address is currently a live handle.
fn is_live(addr: usize) -> bool {
    with_registry(|set| set.contains(&addr))
}

/// Create a new simulation context.
///
/// Returns a heap-allocated opaque handle, or null if allocation fails. The
/// caller owns the handle and must release it with [`scico_free`].
#[unsafe(no_mangle)]
pub extern "C" fn scico_create() -> *mut ScicoHandle {
    let handle = Box::new(ScicoHandle {
        configured: false,
        config: TimeConfig::default(),
        output_count: 0,
        steps_executed: 0,
        outputs: Vec::new(),
    });
    let raw = Box::into_raw(handle);
    register_handle(raw as usize);
    raw
}

/// Release a handle created by [`scico_create`].
///
/// Passing a null pointer is a no-op (and not an error). Passing a pointer that
/// has already been freed is detected through the live-handle registry and
/// reported as [`ScicoStatus::AlreadyFreed`] without freeing again.
///
/// # Safety
///
/// `handle` must be null or a pointer previously returned by [`scico_create`]
/// that has not been freed. Freeing a handle twice is detected and reported, but
/// the caller must not otherwise reuse the pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scico_free(handle: *mut ScicoHandle) -> i32 {
    if handle.is_null() {
        return ScicoStatus::Ok.as_i32();
    }
    let status = catch_unwind(AssertUnwindSafe(|| {
        // Liveness is decided entirely by the registry, so no freed memory is
        // ever read here.
        if !unregister_handle(handle as usize) {
            return ScicoStatus::AlreadyFreed;
        }
        // SAFETY: the address was registered by `scico_create` and has just been
        // removed from the registry, so this is the unique owner and the pointer
        // is still a valid `Box<ScicoHandle>`.
        unsafe {
            drop(Box::from_raw(handle));
        }
        ScicoStatus::Ok
    }));
    status.unwrap_or(ScicoStatus::InternalPanic).as_i32()
}

/// Configure a handle with a time range, step and number of outputs.
///
/// Returns [`ScicoStatus::InvalidConfig`] when `end_time <= start_time`,
/// `step <= 0`, or `output_count` exceeds [`SCICO_MAX_OUTPUTS`].
///
/// # Safety
///
/// `handle` must be null or a pointer previously returned by [`scico_create`]
/// that has not been freed. A null or freed handle is reported as an error and
/// never dereferenced.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scico_config(
    handle: *mut ScicoHandle,
    start_time: f64,
    end_time: f64,
    step: f64,
    output_count: u32,
) -> i32 {
    // SAFETY: `handle` is null or came from `scico_create` (the C caller's
    // contract); `with_handle` validates the magic before use.
    unsafe {
        with_handle(handle, |h| {
            if !(start_time.is_finite() && end_time.is_finite() && step.is_finite()) {
                return Err(ScicoStatus::InvalidConfig);
            }
            if end_time <= start_time || step <= 0.0 {
                return Err(ScicoStatus::InvalidConfig);
            }
            if output_count as usize > SCICO_MAX_OUTPUTS {
                return Err(ScicoStatus::InvalidConfig);
            }
            h.config = TimeConfig {
                start_time,
                end_time,
                max_step: step,
                min_step: step.clamp(f64::MIN_POSITIVE, 1e-9),
                initial_step: step,
            };
            h.output_count = output_count as usize;
            h.outputs = vec![0.0; h.output_count];
            h.steps_executed = 0;
            h.configured = true;
            Ok(())
        })
    }
}

/// Run the configured simulation.
///
/// Real behaviour: the handle integrates a deterministic model over the
/// configured time range at the configured step and fills the output buffer.
/// The model is a first-order relaxation `y' = -y` (the canonical stable linear
/// system), evaluated with explicit Euler at the configured step, so the outputs
/// are genuine computed numbers and the step count reflects the real range.
///
/// Returns [`ScicoStatus::InvalidConfig`] if the handle was not configured, or
/// [`ScicoStatus::NumericalFailure`] if a non-finite value is produced.
///
/// # Safety
///
/// `handle` must be null or a pointer previously returned by [`scico_create`]
/// that has not been freed. A null or freed handle is reported as an error and
/// never dereferenced.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scico_run(handle: *mut ScicoHandle) -> i32 {
    // SAFETY: `handle` is null or came from `scico_create` (the C caller's
    // contract); `with_handle` validates the magic before use.
    unsafe {
        with_handle(handle, |h| {
            if !h.configured {
                return Err(ScicoStatus::InvalidConfig);
            }
            let step = h.config.initial_step;
            let mut t = h.config.start_time;
            let mut y = 1.0_f64;
            let mut steps = 0_u64;
            let mut samples: Vec<f64> = Vec::with_capacity(h.output_count);
            while t < h.config.end_time {
                // Explicit Euler for y' = -y.
                y += step * (-y);
                t += step;
                steps += 1;
                if !y.is_finite() || !t.is_finite() {
                    return Err(ScicoStatus::NumericalFailure);
                }
                if samples.len() < h.output_count {
                    samples.push(y);
                }
            }
            // Pad the output buffer if the range produced fewer steps than outputs.
            while samples.len() < h.output_count {
                samples.push(y);
            }
            h.steps_executed = steps;
            h.outputs = samples;
            Ok(())
        })
    }
}

/// Read the number of steps executed by the most recent run into `out_steps`.
///
/// Returns [`ScicoStatus::NullHandle`] if `out_steps` is null.
///
/// # Safety
///
/// `handle` must be null or a live handle from [`scico_create`]; `out_steps`
/// must be null or point to a writable `u64` owned by the caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scico_step_count(handle: *mut ScicoHandle, out_steps: *mut u64) -> i32 {
    if out_steps.is_null() {
        return ScicoStatus::NullHandle.as_i32();
    }
    // SAFETY: `handle` is null or came from `scico_create` (the C caller's
    // contract); `with_handle` validates the magic before use.
    unsafe {
        with_handle(handle, |h| {
            // SAFETY: `out_steps` is non-null (checked above) and points to a
            // caller-owned `u64`.
            *out_steps = h.steps_executed;
            Ok(())
        })
    }
}

/// Read output `index` into `out_value`.
///
/// Returns [`ScicoStatus::OutOfRange`] when `index >= output_count`.
///
/// # Safety
///
/// `handle` must be null or a live handle from [`scico_create`]; `out_value`
/// must be null or point to a writable `f64` owned by the caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scico_read_output(
    handle: *mut ScicoHandle,
    index: u32,
    out_value: *mut f64,
) -> i32 {
    if out_value.is_null() {
        return ScicoStatus::NullHandle.as_i32();
    }
    // SAFETY: `handle` is null or came from `scico_create` (the C caller's
    // contract); `with_handle` validates the magic before use.
    unsafe {
        with_handle(handle, |h| {
            let idx = index as usize;
            let value = h.outputs.get(idx).ok_or(ScicoStatus::OutOfRange)?;
            // SAFETY: `out_value` is non-null and points to a caller-owned `f64`.
            *out_value = *value;
            Ok(())
        })
    }
}

/// Read the number of configured outputs into `out_count`.
///
/// # Safety
///
/// `handle` must be null or a live handle from [`scico_create`]; `out_count`
/// must be null or point to a writable `u32` owned by the caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn scico_output_count(handle: *mut ScicoHandle, out_count: *mut u32) -> i32 {
    if out_count.is_null() {
        return ScicoStatus::NullHandle.as_i32();
    }
    // SAFETY: `handle` is null or came from `scico_create` (the C caller's
    // contract); `with_handle` validates the magic before use.
    unsafe {
        with_handle(handle, |h| {
            // SAFETY: `out_count` is non-null and points to a caller-owned `u32`.
            *out_count = h.output_count as u32;
            Ok(())
        })
    }
}

/// The stable numeric error code for a given [`ScicoStatus`] value, for callers
/// that only have the integer. Unknown codes return [`ScicoStatus::NullHandle`]
/// as a defensive default.
#[unsafe(no_mangle)]
pub extern "C" fn scico_status_from_code(code: i32) -> i32 {
    match code {
        c if c == ScicoStatus::Ok.as_i32() => ScicoStatus::Ok.as_i32(),
        c if c == ScicoStatus::NullHandle.as_i32() => ScicoStatus::NullHandle.as_i32(),
        c if c == ScicoStatus::AlreadyFreed.as_i32() => ScicoStatus::AlreadyFreed.as_i32(),
        c if c == ScicoStatus::InvalidConfig.as_i32() => ScicoStatus::InvalidConfig.as_i32(),
        c if c == ScicoStatus::NumericalFailure.as_i32() => ScicoStatus::NumericalFailure.as_i32(),
        c if c == ScicoStatus::OutOfRange.as_i32() => ScicoStatus::OutOfRange.as_i32(),
        c if c == ScicoStatus::InternalPanic.as_i32() => ScicoStatus::InternalPanic.as_i32(),
        _ => ScicoStatus::NullHandle.as_i32(),
    }
}

/// Return the C ABI version, so a caller can check compatibility up front.
#[unsafe(no_mangle)]
pub extern "C" fn scico_api_version() -> u32 {
    SCICO_C_API_VERSION
}

/// Shared helper: check a handle is live, then run `f`, mapping panics to
/// [`ScicoStatus::InternalPanic`] and never unwinding across the boundary.
///
/// # Safety
///
/// `handle` must be null or a pointer previously returned by [`scico_create`]
/// that has not been through [`scico_free`] (a freed pointer is detected and
/// rejected).
unsafe fn with_handle<F>(handle: *mut ScicoHandle, f: F) -> i32
where
    F: FnOnce(&mut ScicoHandle) -> Result<(), ScicoStatus>,
{
    if handle.is_null() {
        return ScicoStatus::NullHandle.as_i32();
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        // Liveness is decided by the registry, never by reading the (possibly
        // freed) allocation.
        if !is_live(handle as usize) {
            return ScicoStatus::AlreadyFreed;
        }
        // SAFETY: the address is registered as live, so it is a valid
        // `Box<ScicoHandle>` created by `scico_create`.
        let h = unsafe { &mut *handle };
        match f(h) {
            Ok(()) => ScicoStatus::Ok,
            Err(status) => status,
        }
    }));
    result.unwrap_or(ScicoStatus::InternalPanic).as_i32()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_version_is_stable() {
        assert_eq!(scico_api_version(), 1000);
        assert_eq!(SCICO_C_API_VERSION, 1000);
    }

    #[test]
    fn status_codes_are_stable() {
        assert_eq!(ScicoStatus::Ok.as_i32(), 0);
        assert_eq!(ScicoStatus::NullHandle.as_i32(), -1);
        assert_eq!(ScicoStatus::AlreadyFreed.as_i32(), -2);
        assert_eq!(ScicoStatus::InvalidConfig.as_i32(), -3);
        assert_eq!(ScicoStatus::NumericalFailure.as_i32(), -4);
        assert_eq!(ScicoStatus::OutOfRange.as_i32(), -5);
        assert_eq!(ScicoStatus::InternalPanic.as_i32(), -6);
        assert_eq!(
            scico_status_from_code(ScicoStatus::InvalidConfig.as_i32()),
            ScicoStatus::InvalidConfig.as_i32()
        );
        // Unknown codes map defensively.
        assert_eq!(
            scico_status_from_code(999),
            ScicoStatus::NullHandle.as_i32()
        );
    }

    #[test]
    fn create_config_run_read_free_lifecycle() {
        let handle = scico_create();
        assert!(!handle.is_null());

        // Config: 0 -> 1 s, step 0.1, four outputs.
        let cfg = unsafe { scico_config(handle, 0.0, 1.0, 0.1, 4) };
        assert_eq!(cfg, ScicoStatus::Ok.as_i32());

        let mut count: u32 = 0;
        assert_eq!(
            unsafe { scico_output_count(handle, &mut count) },
            ScicoStatus::Ok.as_i32()
        );
        assert_eq!(count, 4);

        let run = unsafe { scico_run(handle) };
        assert_eq!(run, ScicoStatus::Ok.as_i32());

        let mut steps: u64 = 0;
        assert_eq!(
            unsafe { scico_step_count(handle, &mut steps) },
            ScicoStatus::Ok.as_i32()
        );
        // 0 -> 1 s at a 0.1 s step is 10 steps; accumulated floating-point
        // rounding means the loop may need one extra step to actually cross the
        // end time, so accept 10 or 11.
        assert!(
            steps == 10 || steps == 11,
            "expected 10 or 11 steps, got {}",
            steps
        );

        let mut v0: f64 = 0.0;
        assert_eq!(
            unsafe { scico_read_output(handle, 0, &mut v0) },
            ScicoStatus::Ok.as_i32()
        );
        // First Euler step of y' = -y from y=1: y1 = 1 + 0.1*(-1) = 0.9.
        assert!((v0 - 0.9).abs() < 1e-12, "expected 0.9, got {}", v0);

        let mut v3: f64 = 0.0;
        assert_eq!(
            unsafe { scico_read_output(handle, 3, &mut v3) },
            ScicoStatus::Ok.as_i32()
        );
        assert!(v3.is_finite() && v3 > 0.0 && v3 < v0);

        let free = unsafe { scico_free(handle) };
        assert_eq!(free, ScicoStatus::Ok.as_i32());
    }

    #[test]
    fn null_handle_is_rejected_without_crashing() {
        assert_eq!(
            unsafe { scico_config(std::ptr::null_mut(), 0.0, 1.0, 0.1, 1) },
            ScicoStatus::NullHandle.as_i32()
        );
        assert_eq!(
            unsafe { scico_run(std::ptr::null_mut()) },
            ScicoStatus::NullHandle.as_i32()
        );
        let mut steps: u64 = 0;
        assert_eq!(
            unsafe { scico_step_count(std::ptr::null_mut(), &mut steps) },
            ScicoStatus::NullHandle.as_i32()
        );
        // Freeing null is a no-op success.
        assert_eq!(unsafe { scico_free(std::ptr::null_mut()) }, 0);
    }

    #[test]
    fn null_out_pointer_is_rejected() {
        let handle = scico_create();
        assert_eq!(
            unsafe { scico_step_count(handle, std::ptr::null_mut()) },
            ScicoStatus::NullHandle.as_i32()
        );
        assert_eq!(
            unsafe { scico_read_output(handle, 0, std::ptr::null_mut()) },
            ScicoStatus::NullHandle.as_i32()
        );
        assert_eq!(
            unsafe { scico_output_count(handle, std::ptr::null_mut()) },
            ScicoStatus::NullHandle.as_i32()
        );
        unsafe { scico_free(handle) };
    }

    #[test]
    fn error_config_is_rejected() {
        let handle = scico_create();
        // end <= start.
        assert_eq!(
            unsafe { scico_config(handle, 1.0, 0.5, 0.1, 1) },
            ScicoStatus::InvalidConfig.as_i32()
        );
        // non-positive step.
        assert_eq!(
            unsafe { scico_config(handle, 0.0, 1.0, 0.0, 1) },
            ScicoStatus::InvalidConfig.as_i32()
        );
        // too many outputs.
        assert_eq!(
            unsafe { scico_config(handle, 0.0, 1.0, 0.1, 10_000) },
            ScicoStatus::InvalidConfig.as_i32()
        );
        // Running before a successful config fails.
        assert_eq!(
            unsafe { scico_run(handle) },
            ScicoStatus::InvalidConfig.as_i32()
        );
        unsafe { scico_free(handle) };
    }

    #[test]
    fn read_out_of_range_is_reported() {
        let handle = scico_create();
        unsafe { scico_config(handle, 0.0, 0.5, 0.1, 2) };
        unsafe { scico_run(handle) };
        let mut v: f64 = 0.0;
        assert_eq!(
            unsafe { scico_read_output(handle, 5, &mut v) },
            ScicoStatus::OutOfRange.as_i32()
        );
        unsafe { scico_free(handle) };
    }

    #[test]
    fn double_free_is_detected() {
        let handle = scico_create();
        assert_eq!(unsafe { scico_free(handle) }, ScicoStatus::Ok.as_i32());
        // Second free through the retained (now dangling) pointer must be
        // detected via the freed magic rather than freeing twice.
        assert_eq!(
            unsafe { scico_free(handle) },
            ScicoStatus::AlreadyFreed.as_i32()
        );
    }

    #[test]
    fn use_after_free_is_detected() {
        let handle = scico_create();
        unsafe { scico_config(handle, 0.0, 1.0, 0.1, 1) };
        unsafe { scico_free(handle) };
        // Any call through the dangling pointer reports AlreadyFreed.
        assert_eq!(
            unsafe { scico_run(handle) },
            ScicoStatus::AlreadyFreed.as_i32()
        );
        let mut steps: u64 = 0;
        assert_eq!(
            unsafe { scico_step_count(handle, &mut steps) },
            ScicoStatus::AlreadyFreed.as_i32()
        );
    }

    #[test]
    fn zero_output_config_runs_but_reads_nothing() {
        let handle = scico_create();
        assert_eq!(
            unsafe { scico_config(handle, 0.0, 0.3, 0.1, 0) },
            ScicoStatus::Ok.as_i32()
        );
        assert_eq!(unsafe { scico_run(handle) }, ScicoStatus::Ok.as_i32());
        let mut v: f64 = 0.0;
        assert_eq!(
            unsafe { scico_read_output(handle, 0, &mut v) },
            ScicoStatus::OutOfRange.as_i32()
        );
        unsafe { scico_free(handle) };
    }
}
