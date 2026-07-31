//! Real vendor BLAS/LAPACK loading via `libloading` (dlopen) — the **second**
//! audited `unsafe` boundary in the crate (the first is `simd.rs`).
//!
//! This turns the vendor-BLAS adaptive framework into *real* acceleration:
//! it dlopens the installed MKL / ACML / ESSL / cuBLAS library, resolves the
//! exact CBLAS / cuBLAS entry points, and implements [`VendorBlasBackend`] by
//! calling them. If no library is present (or a symbol is missing) the loader
//! returns `None` and the dispatcher transparently falls back to the pure-Rust
//! SIMD / rayon / serial CPU paths — results stay identical.
//!
//! ## Safety discipline
//!
//! `libloading::Library::new` / `Library::get` are **safe**; *calling* the
//! resolved C function pointers is not, so every call below is an `unsafe`
//! block. Invariants are established in safe code first:
//! - dimensions are non-zero and buffer lengths are validated before any call;
//! - the CBLAS `CblasRowMajor` contract is used, so flat row-major buffers map
//!   directly to CBLAS with `ld = #cols`;
//! - cuBLAS (column-major) is called with the transposed-operand mapping so the
//!   *same* row-major flat buffers compute `C = A·B` (and `y = A·x`);
//! - cuBLAS legacy host-pointer entry points are used, so no device-memory
//!   management is required; the handle is created once and freed on drop.
//!
//! Verified end-to-end in `#[cfg(test)]` against the system reference BLAS
//! (`libblas.so.3`, exports `cblas_dgemm` etc.) and the CUDA `libcublas`.

use crate::core::compute::vendor_blas::{BlasVendor, VendorBlasBackend};
use crate::core::error::SimError;
use crate::core::types::Scalar;
use std::os::raw::c_int;
use std::sync::Arc;

// ──────────────────────────────────────────────
// CBLAS ABI (row-major order, double)
// ──────────────────────────────────────────────
const CBLAS_ROW_MAJOR: c_int = 101;
const CBLAS_NO_TRANS: c_int = 111;

type Dgemm = unsafe extern "C" fn(
    c_int, // Order
    c_int, // TransA
    c_int, // TransB
    c_int,
    c_int,
    c_int, // M, N, K
    f64,
    *const f64,
    c_int, // A, lda
    *const f64,
    c_int, // B, ldb
    f64,
    *mut f64,
    c_int, // beta, C, ldc
);
type Dgemv = unsafe extern "C" fn(
    c_int,
    c_int,
    c_int,
    c_int, // Order, Trans, M, N
    f64,
    *const f64,
    c_int, // alpha, A, lda
    *const f64,
    c_int, // X, incX
    f64,
    *mut f64,
    c_int, // beta, Y, incY
);
type Ddot = unsafe extern "C" fn(c_int, *const f64, c_int, *const f64, c_int) -> f64;
type Dnrm2 = unsafe extern "C" fn(c_int, *const f64, c_int) -> f64;
type Dasum = unsafe extern "C" fn(c_int, *const f64, c_int) -> f64;
type Dscal = unsafe extern "C" fn(c_int, f64, *mut f64, c_int);
type Daxpy = unsafe extern "C" fn(c_int, f64, *const f64, c_int, *mut f64, c_int);
type Idamax = unsafe extern "C" fn(c_int, *const f64, c_int) -> c_int;

/// A real dlopen-loaded CBLAS backend (MKL / ACML / ESSL / OpenBLAS /
/// reference BLAS all expose this interface).
pub struct LoadedCblas {
    vendor: BlasVendor,
    _lib: libloading::Library,
    dgemm: Dgemm,
    dgemv: Dgemv,
    ddot: Ddot,
    dnrm2: Dnrm2,
    dasum: Dasum,
    dscal: Dscal,
    daxpy: Daxpy,
    idamax: Idamax,
}

impl LoadedCblas {
    /// Load a CBLAS library from `path` and resolve the double-precision
    /// CBLAS symbols. Returns `None` if the library or any symbol is missing.
    pub fn from_path(path: &str, vendor: BlasVendor) -> Option<LoadedCblas> {
        // SAFETY: loading a shared library is inherently unsafe (the loader may
        // run its constructors); this is the documented dlopen boundary. The
        // returned symbols are only ever called with validated arguments.
        let lib = unsafe { libloading::Library::new(path) }.ok()?;
        let dgemm = unsafe { *lib.get::<Dgemm>(b"cblas_dgemm").ok()? };
        let dgemv = unsafe { *lib.get::<Dgemv>(b"cblas_dgemv").ok()? };
        let ddot = unsafe { *lib.get::<Ddot>(b"cblas_ddot").ok()? };
        let dnrm2 = unsafe { *lib.get::<Dnrm2>(b"cblas_dnrm2").ok()? };
        let dasum = unsafe { *lib.get::<Dasum>(b"cblas_dasum").ok()? };
        let dscal = unsafe { *lib.get::<Dscal>(b"cblas_dscal").ok()? };
        let daxpy = unsafe { *lib.get::<Daxpy>(b"cblas_daxpy").ok()? };
        let idamax = unsafe { *lib.get::<Idamax>(b"cblas_idamax").ok()? };
        Some(LoadedCblas {
            vendor,
            _lib: lib,
            dgemm,
            dgemv,
            ddot,
            dnrm2,
            dasum,
            dscal,
            daxpy,
            idamax,
        })
    }
}

impl VendorBlasBackend for LoadedCblas {
    fn vendor(&self) -> BlasVendor {
        self.vendor
    }

    fn name(&self) -> &str {
        self.vendor.name()
    }

    fn is_available(&self) -> bool {
        true
    }

    fn gemm(&self, a: &[Vec<Scalar>], b: &[Vec<Scalar>]) -> Result<Vec<Vec<Scalar>>, SimError> {
        if a.is_empty() || b.is_empty() {
            return Ok(Vec::new());
        }
        let m = a.len();
        let k = a[0].len();
        if b.len() != k {
            return Err(SimError::numerical("vendor gemm: inner dims mismatch"));
        }
        let n = b[0].len();
        let mut a_flat = Vec::with_capacity(m * k);
        for row in a.iter() {
            a_flat.extend_from_slice(row);
        }
        let mut b_flat = Vec::with_capacity(k * n);
        for row in b.iter() {
            b_flat.extend_from_slice(row);
        }
        let mut c_flat = vec![0.0; m * n];
        // SAFETY: m,k,n>0, buffers are exactly m·k / k·n / m·n long, and
        // CblasRowMajor/NoTrans maps flat row-major storage directly.
        unsafe {
            (self.dgemm)(
                CBLAS_ROW_MAJOR,
                CBLAS_NO_TRANS,
                CBLAS_NO_TRANS,
                m as c_int,
                n as c_int,
                k as c_int,
                1.0,
                a_flat.as_ptr(),
                k as c_int,
                b_flat.as_ptr(),
                n as c_int,
                0.0,
                c_flat.as_mut_ptr(),
                n as c_int,
            );
        }
        Ok((0..m)
            .map(|i| c_flat[i * n..(i + 1) * n].to_vec())
            .collect())
    }

    fn gemv(&self, a: &[Vec<Scalar>], x: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
        if a.is_empty() {
            return Ok(Vec::new());
        }
        let m = a.len();
        let n = a[0].len();
        if x.len() != n {
            return Err(SimError::numerical("vendor gemv: dims mismatch"));
        }
        let mut a_flat = Vec::with_capacity(m * n);
        for row in a.iter() {
            a_flat.extend_from_slice(row);
        }
        let mut y = vec![0.0; m];
        // SAFETY: row-major NoTrans: y(m) = A(m×n)·x(n), lda = n.
        unsafe {
            (self.dgemv)(
                CBLAS_ROW_MAJOR,
                CBLAS_NO_TRANS,
                m as c_int,
                n as c_int,
                1.0,
                a_flat.as_ptr(),
                n as c_int,
                x.as_ptr(),
                1,
                0.0,
                y.as_mut_ptr(),
                1,
            );
        }
        Ok(y)
    }

    fn dot(&self, a: &[Scalar], b: &[Scalar]) -> Result<Scalar, SimError> {
        let n = a.len();
        if b.len() != n {
            return Err(SimError::numerical("vendor dot: length mismatch"));
        }
        // SAFETY: equal, non-empty lengths; unit strides.
        Ok(unsafe { (self.ddot)(n as c_int, a.as_ptr(), 1, b.as_ptr(), 1) })
    }

    fn nrm2(&self, x: &[Scalar]) -> Result<Scalar, SimError> {
        // SAFETY: slice is at least len(x) long.
        Ok(unsafe { (self.dnrm2)(x.len() as c_int, x.as_ptr(), 1) })
    }

    fn asum(&self, x: &[Scalar]) -> Result<Scalar, SimError> {
        // SAFETY: slice is at least len(x) long.
        Ok(unsafe { (self.dasum)(x.len() as c_int, x.as_ptr(), 1) })
    }

    fn scal(&self, alpha: Scalar, x: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
        let mut y = x.to_vec();
        // SAFETY: in-place on the owned copy.
        unsafe { (self.dscal)(x.len() as c_int, alpha, y.as_mut_ptr(), 1) }
        Ok(y)
    }

    fn axpy(&self, alpha: Scalar, x: &[Scalar], y: &mut [Scalar]) -> Result<(), SimError> {
        let n = x.len();
        if y.len() != n {
            return Err(SimError::numerical("vendor axpy: length mismatch"));
        }
        // SAFETY: y += α·x, both length n, unit strides.
        unsafe { (self.daxpy)(n as c_int, alpha, x.as_ptr(), 1, y.as_mut_ptr(), 1) }
        Ok(())
    }

    fn iamax(&self, x: &[Scalar]) -> Result<usize, SimError> {
        // SAFETY: CBLAS idamax returns a 1-based index; x is non-empty in callers.
        if x.is_empty() {
            return Ok(0);
        }
        let idx = unsafe { (self.idamax)(x.len() as c_int, x.as_ptr(), 1) };
        Ok(idx.saturating_sub(1) as usize)
    }
}

// ──────────────────────────────────────────────
// cuBLAS ABI (legacy host-pointer entry points, column-major)
// ──────────────────────────────────────────────
const CUBLAS_STATUS_SUCCESS: c_int = 0;
const CUBLAS_OP_N: c_int = 0;
const CUBLAS_OP_T: c_int = 1;
// cudaMemcpyKind
const CUDA_MEMCPY_HOST_TO_DEVICE: c_int = 1;
const CUDA_MEMCPY_DEVICE_TO_HOST: c_int = 2;

type Status = c_int;
type Handle = *mut std::os::raw::c_void;
type CublasCreate = unsafe extern "C" fn(*mut Handle) -> Status;
type CublasDestroy = unsafe extern "C" fn(Handle) -> Status;
// cudart: device memory management (used to feed the device-pointer `_v2` API).
type CudaMalloc = unsafe extern "C" fn(*mut *mut std::os::raw::c_void, usize) -> c_int;
type CudaMemcpy = unsafe extern "C" fn(
    *mut std::os::raw::c_void,
    *const std::os::raw::c_void,
    usize,
    c_int,
) -> c_int;
type CudaFree = unsafe extern "C" fn(*mut std::os::raw::c_void) -> c_int;
type CublasDgemm = unsafe extern "C" fn(
    Handle,
    c_int,
    c_int,
    c_int,
    c_int,
    c_int, // transa, transb, m, n, k
    *const f64,
    *const f64,
    c_int, // alpha, A, lda
    *const f64,
    c_int, // B, ldb
    *const f64,
    *mut f64,
    c_int, // beta, C, ldc
) -> Status;
type CublasDgemv = unsafe extern "C" fn(
    Handle,
    c_int,
    c_int,
    c_int, // trans, m, n
    *const f64,
    *const f64,
    c_int, // alpha, A, lda
    *const f64,
    c_int, // x, incx
    *const f64,
    *mut f64,
    c_int, // beta, y, incy
) -> Status;
type CublasDdot =
    unsafe extern "C" fn(Handle, c_int, *const f64, c_int, *const f64, c_int, *mut f64) -> Status;
type CublasDnrm2 = unsafe extern "C" fn(Handle, c_int, *const f64, c_int, *mut f64) -> Status;
type CublasDasum = unsafe extern "C" fn(Handle, c_int, *const f64, c_int, *mut f64) -> Status;
type CublasDscal = unsafe extern "C" fn(Handle, c_int, *const f64, *mut f64, c_int) -> Status;
type CublasDaxpy =
    unsafe extern "C" fn(Handle, c_int, *const f64, *const f64, c_int, *mut f64, c_int) -> Status;

/// A real dlopen-loaded cuBLAS backend (GPU). Uses the device-pointer `_v2`
/// entry points; host buffers are copied to device memory via the CUDA runtime
/// (`cudaMalloc`/`cudaMemcpy`/`cudaFree` from `libcudart`), so a GPU is
/// required. The cuBLAS handle and device pointers are stored as integers so
/// the backend stays `Send + Sync`; each call casts them back.
pub struct LoadedCublas {
    _lib: libloading::Library,
    _cudart: libloading::Library,
    handle: usize,
    destroy: CublasDestroy,
    malloc: CudaMalloc,
    memcpy: CudaMemcpy,
    free: CudaFree,
    dgemm: CublasDgemm,
    dgemv: CublasDgemv,
    ddot: CublasDdot,
    dnrm2: CublasDnrm2,
    dasum: CublasDasum,
    dscal: CublasDscal,
    daxpy: CublasDaxpy,
}

impl LoadedCublas {
    /// Load `libcublas.so`, create a cuBLAS handle, and resolve the legacy
    /// double-precision entry points. `None` if the driver/library/handle is
    /// unavailable (e.g. no GPU / no CUDA runtime).
    pub fn load() -> Option<LoadedCublas> {
        // SAFETY: see `LoadedCblas::from_path`; loading the CUDA runtime library
        // is part of the documented dlopen boundary.
        let lib = unsafe { libloading::Library::new("libcublas.so") }.ok()?;
        let cudart = unsafe { libloading::Library::new("libcudart.so.12") }
            .or_else(|_| unsafe { libloading::Library::new("libcudart.so") })
            .ok()?;
        let create = unsafe { *lib.get::<CublasCreate>(b"cublasCreate_v2").ok()? };
        let destroy = unsafe { *lib.get::<CublasDestroy>(b"cublasDestroy_v2").ok()? };
        let malloc = unsafe { *cudart.get::<CudaMalloc>(b"cudaMalloc").ok()? };
        let memcpy = unsafe { *cudart.get::<CudaMemcpy>(b"cudaMemcpy").ok()? };
        let free = unsafe { *cudart.get::<CudaFree>(b"cudaFree").ok()? };
        let dgemm = unsafe { *lib.get::<CublasDgemm>(b"cublasDgemm_v2").ok()? };
        let dgemv = unsafe { *lib.get::<CublasDgemv>(b"cublasDgemv_v2").ok()? };
        let ddot = unsafe { *lib.get::<CublasDdot>(b"cublasDdot_v2").ok()? };
        let dnrm2 = unsafe { *lib.get::<CublasDnrm2>(b"cublasDnrm2_v2").ok()? };
        let dasum = unsafe { *lib.get::<CublasDasum>(b"cublasDasum_v2").ok()? };
        let dscal = unsafe { *lib.get::<CublasDscal>(b"cublasDscal_v2").ok()? };
        let daxpy = unsafe { *lib.get::<CublasDaxpy>(b"cublasDaxpy_v2").ok()? };
        let mut handle: Handle = std::ptr::null_mut();
        // SAFETY: create is a real cuBLAS entry point; handle is a valid out-param.
        let status = unsafe { create(&mut handle) };
        if status != CUBLAS_STATUS_SUCCESS || handle.is_null() {
            return None;
        }
        Some(LoadedCublas {
            _lib: lib,
            _cudart: cudart,
            handle: handle as usize,
            destroy,
            malloc,
            memcpy,
            free,
            dgemm,
            dgemv,
            ddot,
            dnrm2,
            dasum,
            dscal,
            daxpy,
        })
    }

    fn handle_ptr(&self) -> Handle {
        self.handle as Handle
    }

    /// Allocate `bytes` on the device; returns a raw pointer or `None`.
    fn device_alloc(&self, bytes: usize) -> Option<*mut std::os::raw::c_void> {
        let mut ptr: *mut std::os::raw::c_void = std::ptr::null_mut();
        // SAFETY: cudaMalloc is a real cudart entry point; ptr is an out-param.
        let err = unsafe { (self.malloc)(&mut ptr, bytes) };
        if err == 0 && !ptr.is_null() {
            Some(ptr)
        } else {
            None
        }
    }

    /// Copy `count` doubles from host to device (`kind = 1` HtoD).
    fn device_copy_to(&self, dst: *mut std::os::raw::c_void, src: &[Scalar]) -> bool {
        // SAFETY: dst is a valid device allocation of >= src.len()*8 bytes.
        unsafe {
            (self.memcpy)(
                dst,
                src.as_ptr() as *const _,
                src.len() * 8,
                CUDA_MEMCPY_HOST_TO_DEVICE,
            ) == 0
        }
    }

    /// Copy `count` doubles from device to host (`kind = 2` DtoH).
    fn device_copy_from(&self, dst: &mut [Scalar], src: *mut std::os::raw::c_void) -> bool {
        // SAFETY: src is a valid device allocation of >= dst.len()*8 bytes.
        unsafe {
            (self.memcpy)(
                dst.as_mut_ptr() as *mut _,
                src,
                dst.len() * 8,
                CUDA_MEMCPY_DEVICE_TO_HOST,
            ) == 0
        }
    }

    /// Free a device allocation.
    fn device_free(&self, ptr: *mut std::os::raw::c_void) {
        // SAFETY: ptr was returned by cudaMalloc and is not freed elsewhere.
        unsafe {
            let _ = (self.free)(ptr);
        }
    }
}

impl Drop for LoadedCublas {
    fn drop(&mut self) {
        // SAFETY: handle was created by cublasCreate_v2 and is valid here.
        unsafe {
            let _ = (self.destroy)(self.handle_ptr());
        }
    }
}

impl VendorBlasBackend for LoadedCublas {
    fn vendor(&self) -> BlasVendor {
        BlasVendor::Cublas
    }

    fn name(&self) -> &str {
        "cublas/libcublas.so"
    }

    fn is_available(&self) -> bool {
        self.handle != 0
    }

    fn gemm(&self, a: &[Vec<Scalar>], b: &[Vec<Scalar>]) -> Result<Vec<Vec<Scalar>>, SimError> {
        if a.is_empty() || b.is_empty() {
            return Ok(Vec::new());
        }
        let m = a.len();
        let k = a[0].len();
        if b.len() != k {
            return Err(SimError::numerical("cublas gemm: inner dims mismatch"));
        }
        let n = b[0].len();
        let mut a_flat = Vec::with_capacity(m * k);
        for row in a.iter() {
            a_flat.extend_from_slice(row);
        }
        let mut b_flat = Vec::with_capacity(k * n);
        for row in b.iter() {
            b_flat.extend_from_slice(row);
        }
        let da = self
            .device_alloc(m * k * 8)
            .ok_or_else(|| SimError::runtime("cublas: cudaMalloc A failed"))?;
        let db = self
            .device_alloc(k * n * 8)
            .ok_or_else(|| SimError::runtime("cublas: cudaMalloc B failed"))?;
        let dc = self
            .device_alloc(m * n * 8)
            .ok_or_else(|| SimError::runtime("cublas: cudaMalloc C failed"))?;
        let copied = self.device_copy_to(da, &a_flat) && self.device_copy_to(db, &b_flat);
        let mut status = !0;
        let mut c_flat = vec![0.0; m * n];
        if copied {
            // Row-major C=A·B equals column-major Cᵀ = Bᵀ·Aᵀ; cuBLAS computes
            // op(X)·op(Y) with lda/ldb/ldc = leading dims (n, k, n).
            let one = 1.0f64;
            let zero = 0.0f64;
            status = unsafe {
                (self.dgemm)(
                    self.handle_ptr(),
                    CUBLAS_OP_N,
                    CUBLAS_OP_N,
                    n as c_int,
                    m as c_int,
                    k as c_int,
                    &one,
                    db as *const f64,
                    n as c_int,
                    da as *const f64,
                    k as c_int,
                    &zero,
                    dc as *mut f64,
                    n as c_int,
                )
            };
        }
        let ok =
            copied && status == CUBLAS_STATUS_SUCCESS && self.device_copy_from(&mut c_flat, dc);
        self.device_free(da);
        self.device_free(db);
        self.device_free(dc);
        if !ok {
            return Err(SimError::runtime("cublas dgemm failed"));
        }
        Ok((0..m)
            .map(|i| c_flat[i * n..(i + 1) * n].to_vec())
            .collect())
    }

    fn gemv(&self, a: &[Vec<Scalar>], x: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
        if a.is_empty() {
            return Ok(Vec::new());
        }
        let m = a.len();
        let n = a[0].len();
        if x.len() != n {
            return Err(SimError::numerical("cublas gemv: dims mismatch"));
        }
        let mut a_flat = Vec::with_capacity(m * n);
        for row in a.iter() {
            a_flat.extend_from_slice(row);
        }
        let da = self
            .device_alloc(m * n * 8)
            .ok_or_else(|| SimError::runtime("cublas: cudaMalloc A failed"))?;
        let dx = self
            .device_alloc(n * 8)
            .ok_or_else(|| SimError::runtime("cublas: cudaMalloc x failed"))?;
        let dy = self
            .device_alloc(m * 8)
            .ok_or_else(|| SimError::runtime("cublas: cudaMalloc y failed"))?;
        let copied = self.device_copy_to(da, &a_flat) && self.device_copy_to(dx, x);
        let mut status = !0;
        if copied {
            // y(m) = A_row(m×n)·x(n): cuBLAS trans=OP_T over the n×m
            // column-major view (same buffer, lda=n) gives op(A)=A_row.
            let one = 1.0f64;
            let zero = 0.0f64;
            status = unsafe {
                (self.dgemv)(
                    self.handle_ptr(),
                    CUBLAS_OP_T,
                    n as c_int,
                    m as c_int,
                    &one,
                    da as *const f64,
                    n as c_int,
                    dx as *const f64,
                    1,
                    &zero,
                    dy as *mut f64,
                    1,
                )
            };
        }
        let mut y = vec![0.0; m];
        let ok = copied && status == CUBLAS_STATUS_SUCCESS && self.device_copy_from(&mut y, dy);
        self.device_free(da);
        self.device_free(dx);
        self.device_free(dy);
        if !ok {
            return Err(SimError::runtime("cublas dgemv failed"));
        }
        Ok(y)
    }

    fn dot(&self, a: &[Scalar], b: &[Scalar]) -> Result<Scalar, SimError> {
        let n = a.len();
        if b.len() != n {
            return Err(SimError::numerical("cublas dot: length mismatch"));
        }
        let da = self
            .device_alloc(n * 8)
            .ok_or_else(|| SimError::runtime("cublas: cudaMalloc a failed"))?;
        let db = self
            .device_alloc(n * 8)
            .ok_or_else(|| SimError::runtime("cublas: cudaMalloc b failed"))?;
        let dout = self
            .device_alloc(8)
            .ok_or_else(|| SimError::runtime("cublas: cudaMalloc out failed"))?;
        let copied = self.device_copy_to(da, a) && self.device_copy_to(db, b);
        let status = unsafe {
            (self.ddot)(
                self.handle_ptr(),
                n as c_int,
                da as *const f64,
                1,
                db as *const f64,
                1,
                dout as *mut f64,
            )
        };
        let mut out_buf = vec![0.0f64; 1];
        let ok =
            copied && status == CUBLAS_STATUS_SUCCESS && self.device_copy_from(&mut out_buf, dout);
        self.device_free(da);
        self.device_free(db);
        self.device_free(dout);
        if !ok {
            return Err(SimError::runtime("cublas ddot failed"));
        }
        Ok(out_buf[0])
    }

    fn nrm2(&self, x: &[Scalar]) -> Result<Scalar, SimError> {
        let dx = self
            .device_alloc(x.len() * 8)
            .ok_or_else(|| SimError::runtime("cublas: cudaMalloc x failed"))?;
        let dout = self
            .device_alloc(8)
            .ok_or_else(|| SimError::runtime("cublas: cudaMalloc out failed"))?;
        let copied = self.device_copy_to(dx, x);
        let status = unsafe {
            (self.dnrm2)(
                self.handle_ptr(),
                x.len() as c_int,
                dx as *const f64,
                1,
                dout as *mut f64,
            )
        };
        let mut out_buf = vec![0.0f64; 1];
        let ok =
            copied && status == CUBLAS_STATUS_SUCCESS && self.device_copy_from(&mut out_buf, dout);
        self.device_free(dx);
        self.device_free(dout);
        if !ok {
            return Err(SimError::runtime("cublas dnrm2 failed"));
        }
        Ok(out_buf[0])
    }

    fn asum(&self, x: &[Scalar]) -> Result<Scalar, SimError> {
        let dx = self
            .device_alloc(x.len() * 8)
            .ok_or_else(|| SimError::runtime("cublas: cudaMalloc x failed"))?;
        let dout = self
            .device_alloc(8)
            .ok_or_else(|| SimError::runtime("cublas: cudaMalloc out failed"))?;
        let copied = self.device_copy_to(dx, x);
        let status = unsafe {
            (self.dasum)(
                self.handle_ptr(),
                x.len() as c_int,
                dx as *const f64,
                1,
                dout as *mut f64,
            )
        };
        let mut out_buf = vec![0.0f64; 1];
        let ok =
            copied && status == CUBLAS_STATUS_SUCCESS && self.device_copy_from(&mut out_buf, dout);
        self.device_free(dx);
        self.device_free(dout);
        if !ok {
            return Err(SimError::runtime("cublas dasum failed"));
        }
        Ok(out_buf[0])
    }

    fn scal(&self, alpha: Scalar, x: &[Scalar]) -> Result<Vec<Scalar>, SimError> {
        let dx = self
            .device_alloc(x.len() * 8)
            .ok_or_else(|| SimError::runtime("cublas: cudaMalloc x failed"))?;
        let copied = self.device_copy_to(dx, x);
        // Default pointer mode is host, so `alpha` is a host pointer.
        let status = unsafe {
            (self.dscal)(
                self.handle_ptr(),
                x.len() as c_int,
                &alpha,
                dx as *mut f64,
                1,
            )
        };
        let mut y = vec![0.0; x.len()];
        let ok = copied && status == CUBLAS_STATUS_SUCCESS && self.device_copy_from(&mut y, dx);
        self.device_free(dx);
        if !ok {
            return Err(SimError::runtime("cublas dscal failed"));
        }
        Ok(y)
    }

    fn axpy(&self, alpha: Scalar, x: &[Scalar], y: &mut [Scalar]) -> Result<(), SimError> {
        let n = x.len();
        if y.len() != n {
            return Err(SimError::numerical("cublas axpy: length mismatch"));
        }
        let dx = self
            .device_alloc(n * 8)
            .ok_or_else(|| SimError::runtime("cublas: cudaMalloc x failed"))?;
        let dy = self
            .device_alloc(n * 8)
            .ok_or_else(|| SimError::runtime("cublas: cudaMalloc y failed"))?;
        let copied = self.device_copy_to(dx, x) && self.device_copy_to(dy, y);
        let status = unsafe {
            (self.daxpy)(
                self.handle_ptr(),
                n as c_int,
                &alpha,
                dx as *const f64,
                1,
                dy as *mut f64,
                1,
            )
        };
        let ok = copied && status == CUBLAS_STATUS_SUCCESS && self.device_copy_from(y, dy);
        self.device_free(dx);
        self.device_free(dy);
        if !ok {
            return Err(SimError::runtime("cublas daxpy failed"));
        }
        Ok(())
    }
}

/// Load a CPU vendor's real CBLAS library by probing its known names.
pub fn load_cpu_vendor(vendor: BlasVendor) -> Option<Arc<dyn VendorBlasBackend>> {
    vendor
        .known_library_names()
        .iter()
        .find_map(|name| LoadedCblas::from_path(name, vendor))
        .map(|b| Arc::new(b) as Arc<dyn VendorBlasBackend>)
}

/// Load a real cuBLAS backend (GPU), if the driver and library are present.
pub fn load_cublas() -> Option<Arc<dyn VendorBlasBackend>> {
    LoadedCublas::load().map(|b| Arc::new(b) as Arc<dyn VendorBlasBackend>)
}

/// Probe the environment, load every vendor that is actually present, and
/// register it in the adaptive dispatcher. Returns the number registered.
pub fn load_and_register_available(
    backend: &mut crate::core::compute::backend::AdaptiveCompute,
) -> usize {
    let mut count = 0;
    for vendor in crate::core::compute::vendor_blas::detect_installed_vendors() {
        if vendor.is_gpu() {
            if let Some(b) = load_cublas() {
                backend.register_vendor_blas(b);
                count += 1;
            }
        } else if let Some(b) = load_cpu_vendor(vendor) {
            backend.register_vendor_blas(b);
            count += 1;
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::compute::matrix::mat_mul;

    fn rand_vec(n: usize) -> Vec<Scalar> {
        let mut x: u64 = 0x9E3779B97F4A7C15;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x as f64 / u64::MAX as f64) * 2.0 - 1.0
            })
            .collect()
    }

    #[test]
    fn cblas_gemm_matches_cpu() {
        // End-to-end against the system reference BLAS (libblas.so.3 exports
        // the full CBLAS surface). Skips gracefully if not present.
        let lib = LoadedCblas::from_path("libblas.so.3", BlasVendor::Acml)
            .or_else(|| LoadedCblas::from_path("libblas.so", BlasVendor::Acml))
            .or_else(|| LoadedCblas::from_path("libopenblas.so.0", BlasVendor::Acml));
        let Some(lib) = lib else {
            eprintln!("skipping cblas test: no CBLAS library found");
            return;
        };
        for (m, k, n) in [(8, 6, 7), (32, 24, 28), (64, 40, 48)] {
            let a: Vec<Vec<Scalar>> = (0..m).map(|_| rand_vec(k)).collect();
            let b: Vec<Vec<Scalar>> = (0..k).map(|_| rand_vec(n)).collect();
            let want = mat_mul(&a, &b).unwrap();
            let got = lib.gemm(&a, &b).unwrap();
            for i in 0..m {
                for j in 0..n {
                    assert!(
                        (got[i][j] - want[i][j]).abs() < 1e-9,
                        "cblas gemm mismatch at {m}×{k}×{n}, ({i},{j}): {} vs {}",
                        got[i][j],
                        want[i][j]
                    );
                }
            }
        }
    }

    #[test]
    fn cblas_blas1_matches_reference() {
        let lib = LoadedCblas::from_path("libblas.so.3", BlasVendor::Acml)
            .or_else(|| LoadedCblas::from_path("libblas.so", BlasVendor::Acml));
        let Some(lib) = lib else {
            eprintln!("skipping cblas blas1 test: no CBLAS library found");
            return;
        };
        let a = rand_vec(128);
        let b = rand_vec(128);
        let dot: Scalar = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        assert!((lib.dot(&a, &b).unwrap() - dot).abs() < 1e-9);
        let nrm: Scalar = a.iter().map(|x| x * x).sum::<Scalar>().sqrt();
        assert!((lib.nrm2(&a).unwrap() - nrm).abs() < 1e-9);
        let asum: Scalar = a.iter().map(|x| x.abs()).sum();
        assert!((lib.asum(&a).unwrap() - asum).abs() < 1e-9);
        let scaled = lib.scal(2.5, &a).unwrap();
        for (s, &x) in scaled.iter().zip(a.iter()) {
            assert!((s - 2.5 * x).abs() < 1e-12);
        }
        let mut y = b.clone();
        lib.axpy(1.5, &a, &mut y).unwrap();
        for i in 0..a.len() {
            assert!((y[i] - (b[i] + 1.5 * a[i])).abs() < 1e-12);
        }
    }

    #[test]
    fn cublas_gemm_matches_cpu() {
        // Only meaningful when a CUDA driver/library is present; skip silently
        // otherwise (the adaptive dispatcher also falls back).
        let Some(lib) = LoadedCublas::load() else {
            eprintln!("skipping cublas test: libcublas/driver unavailable");
            return;
        };
        let (m, k, n) = (24, 20, 22);
        let a: Vec<Vec<Scalar>> = (0..m).map(|_| rand_vec(k)).collect();
        let b: Vec<Vec<Scalar>> = (0..k).map(|_| rand_vec(n)).collect();
        let want = mat_mul(&a, &b).unwrap();
        let got = lib.gemm(&a, &b).unwrap();
        for i in 0..m {
            for j in 0..n {
                assert!(
                    (got[i][j] - want[i][j]).abs() < 1e-9,
                    "cublas gemm mismatch at ({i},{j})"
                );
            }
        }
    }

    #[test]
    fn adaptive_dispatch_routes_to_loaded_vendor() {
        // Register a *real* dlopen-loaded CBLAS (system reference BLAS) in the
        // adaptive dispatcher: a workload above the parallel threshold must be
        // routed to the vendor and produce identical results to the CPU path.
        let Some(lib) = LoadedCblas::from_path("libblas.so.3", BlasVendor::Acml)
            .or_else(|| LoadedCblas::from_path("libblas.so", BlasVendor::Acml))
            .or_else(|| LoadedCblas::from_path("libopenblas.so.0", BlasVendor::Acml))
        else {
            eprintln!("skipping dispatch test: no CBLAS library found");
            return;
        };
        use crate::core::compute::backend::{AdaptiveCompute, ComputeConfig};
        let mut comp = AdaptiveCompute::new(ComputeConfig::default());
        comp.register_vendor_blas(Arc::new(lib) as Arc<dyn VendorBlasBackend>);
        // 32×32×32 = 32768 work units ≥ parallel_threshold and < gpu_threshold
        // → VendorCpu tier.
        let (m, k, n) = (32, 32, 32);
        let a: Vec<Vec<Scalar>> = (0..m).map(|_| rand_vec(k)).collect();
        let b: Vec<Vec<Scalar>> = (0..k).map(|_| rand_vec(n)).collect();
        assert_eq!(
            comp.kind_for(m * k * n),
            crate::core::compute::backend::BackendKind::VendorCpu
        );
        let want = crate::core::compute::matrix::mat_mul(&a, &b).unwrap();
        let got = comp.mat_mul(&a, &b).unwrap();
        for i in 0..m {
            for j in 0..n {
                assert!(
                    (got[i][j] - want[i][j]).abs() < 1e-9,
                    "adaptive vendor dispatch mismatch at ({i},{j})"
                );
            }
        }
    }
}
