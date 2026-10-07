// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT
// SCIcoRS GPU kernels — WGSL compute shaders (float32 storage path).
//
// Included via `include_str!` from `gpu/kernel.rs`. **Important portability
// constraint:** Metal (and many other adapters) do not expose the
// `SHADER_F64` feature, so WGSL `f64` storage buffers are *not* universally
// available — verified on Apple M4 where `Features::SHADER_F64 == false`.
// Every numeric payload here is therefore `f32`, which is computable on every
// WebGPU-capable adapter. The `f64` variant of the same kernels lives in
// `precision64.wgsl` and is only compiled when the adapter advertises
// `SHADER_F64`.
//
// Layout: flat row-major, matching `core::compute::matrix`'s dense storage.
// All control values (sizes, op tags) travel as `u32` in one `vec4<u32>`
// uniform so no numeric precision is lost on the control path.

// ===========================================================================
// Dense matrix multiply: C(m×n) = A(m×k) · B(k×n), all row-major.
// One invocation computes one element of C.
// ===========================================================================
@group(0) @binding(0) var<storage, read> matmul_a: array<f32>;
@group(0) @binding(1) var<storage, read> matmul_b: array<f32>;
@group(0) @binding(2) var<storage, read_write> matmul_c: array<f32>;
// [m, k, n, 0]
@group(0) @binding(3) var<uniform> matmul_dims: vec4<u32>;

// Tile edge for the shared-memory GEMM. A TILE×TILE workgroup loads a
// TILE×TILE block of A and of B into workgroup memory, then each thread
// accumulates its output element from that block. This cuts global-memory
// traffic by a factor of TILE, which is what makes the GPU kernel competitive
// with the SIMD CPU kernel instead of being memory-bound.
const TILE: u32 = 16u;

var<workgroup> tile_a: array<f32, 256>;
var<workgroup> tile_b: array<f32, 256>;

@compute @workgroup_size(16, 16, 1)
fn matmul_main(
    @builtin(global_invocation_id) gid: vec3<u32>,
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
) {
    let m = matmul_dims.x;
    let k = matmul_dims.y;
    let n = matmul_dims.z;
    let row = gid.y;
    let col = gid.x;

    var acc: f32 = 0.0;
    let tiles = (k + TILE - 1u) / TILE;
    for (var t: u32 = 0u; t < tiles; t = t + 1u) {
        // Cooperative load: each thread fills one element of each tile from a
        // flattened index over the TILE×TILE block.
        let flat = lid.y * TILE + lid.x;
        let a_col = t * TILE + (flat % TILE);
        let a_row = wid.y * TILE + (flat / TILE);
        if (a_row < m && a_col < k) {
            tile_a[flat] = matmul_a[a_row * k + a_col];
        } else {
            tile_a[flat] = 0.0;
        }
        let b_row = t * TILE + (flat / TILE);
        let b_col = wid.x * TILE + (flat % TILE);
        if (b_row < k && b_col < n) {
            tile_b[flat] = matmul_b[b_row * n + b_col];
        } else {
            tile_b[flat] = 0.0;
        }
        workgroupBarrier();

        for (var i: u32 = 0u; i < TILE; i = i + 1u) {
            acc = acc + tile_a[lid.y * TILE + i] * tile_b[i * TILE + lid.x];
        }
        workgroupBarrier();
    }

    if (row < m && col < n) {
        matmul_c[row * n + col] = acc;
    }
}

// ===========================================================================
// Element-wise binary ops: out[i] = a[i] <op> b[i] for equal-length vectors.
// `op`: 0 = add, 1 = sub, 2 = mul (Hadamard), 3 = abs(a[i]).
// Op 3 ignores `bin_b`, which lets the same pipeline serve the unary map.
// ===========================================================================
@group(0) @binding(0) var<storage, read> bin_a: array<f32>;
@group(0) @binding(1) var<storage, read> bin_b: array<f32>;
@group(0) @binding(2) var<storage, read_write> bin_out: array<f32>;
// [len, op, 0, 0]
@group(0) @binding(3) var<uniform> bin_dims: vec4<u32>;

@compute @workgroup_size(64, 1, 1)
fn binary_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= bin_dims.x) {
        return;
    }
    let op = bin_dims.y;
    if (op == 0u) {
        bin_out[i] = bin_a[i] + bin_b[i];
    } else if (op == 1u) {
        bin_out[i] = bin_a[i] - bin_b[i];
    } else if (op == 2u) {
        bin_out[i] = bin_a[i] * bin_b[i];
    } else {
        bin_out[i] = abs(bin_a[i]);
    }
}

// ===========================================================================
// Scalar-vector multiply: out[i] = scale * a[i].
// The f64 scale is narrowed to f32 and bit-cast into a u32 uniform slot, which
// matches the f32 precision of the storage buffers this kernel set targets.
// ===========================================================================
@group(0) @binding(0) var<storage, read> scale_a: array<f32>;
@group(0) @binding(1) var<storage, read_write> scale_out: array<f32>;
// [len, scale_bits, 0, 0] — the f32 scale bit-cast into a u32 slot.
@group(0) @binding(2) var<storage, read> scale_dims: array<u32>;

@compute @workgroup_size(64, 1, 1)
fn scale_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= scale_dims[0]) {
        return;
    }
    let s = bitcast<f32>(scale_dims[1]);
    scale_out[i] = s * scale_a[i];
}

// ===========================================================================
// AXPY: y[i] = alpha * x[i] + y[i]  (alpha as f64 bit-halves).
// ===========================================================================
@group(0) @binding(0) var<storage, read> axpy_x: array<f32>;
@group(0) @binding(1) var<storage, read> axpy_y: array<f32>;
@group(0) @binding(2) var<storage, read_write> axpy_out: array<f32>;
// [len, alpha_bits, 0, 0] — the f32 alpha bit-cast into a u32 slot.
@group(0) @binding(3) var<storage, read> axpy_dims: array<u32>;

@compute @workgroup_size(64, 1, 1)
fn axpy_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= axpy_dims[0]) {
        return;
    }
    let alpha = bitcast<f32>(axpy_dims[1]);
    axpy_out[i] = alpha * axpy_x[i] + axpy_y[i];
}

// ===========================================================================
// Global reductions: dot(a,b) / sum(a) / sum(|a|) / max(|a|). Each workgroup
// reduces its slice into a partial, then `reduce_finalize` collapses the
// partials to a single value. Workgroup size stays a power of two for the tree
// reduction.
//
// `op`: 0 = dot, 1 = sum, 2 = asum, 3 = max-abs. Ops 2 and 3 are seeded with the
// identity of their monoid (0 and 0 respectively) so empty slices are safe.
// ===========================================================================
const REDUCE_WG: u32 = 256u;
const REDUCE_STRIDE: u32 = 64u;

@group(0) @binding(0) var<storage, read> red_a: array<f32>;
@group(0) @binding(1) var<storage, read> red_b: array<f32>;
@group(0) @binding(2) var<storage, read_write> red_out: array<f32>;
// [len, op, num_partials, 0]
@group(0) @binding(3) var<uniform> red_dims: vec4<u32>;

var<workgroup> red_scratch: array<f32, 256>;

// Stage 1: one workgroup per contiguous slice of REDUCE_WG * REDUCE_STRIDE.
@compute @workgroup_size(256, 1, 1)
fn reduce_partial(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
) {
    let len = red_dims.x;
    let op = red_dims.y;
    let base = wid.x * REDUCE_WG * REDUCE_STRIDE;
    var acc: f32 = 0.0;
    // Strided access keeps consecutive threads on consecutive addresses.
    for (var s: u32 = 0u; s < REDUCE_STRIDE; s = s + 1u) {
        let i = base + s * REDUCE_WG + lid.x;
        if (i < len) {
            if (op == 0u) {
                acc = acc + red_a[i] * red_b[i];
            } else if (op == 1u) {
                acc = acc + red_a[i];
            } else if (op == 2u) {
                acc = acc + abs(red_a[i]);
            } else {
                // max-abs: the monoid identity 0 makes an all-zero (or empty)
                // slice reduce to 0, mirroring the CPU `fold(0.0, f64::max)`.
                acc = max(acc, abs(red_a[i]));
            }
        }
    }
    red_scratch[lid.x] = acc;
    workgroupBarrier();
    var stride: u32 = REDUCE_WG / 2u;
    loop {
        if (stride == 0u) {
            break;
        }
        if (lid.x < stride) {
            if (op == 3u) {
                red_scratch[lid.x] = max(red_scratch[lid.x], red_scratch[lid.x + stride]);
            } else {
                red_scratch[lid.x] = red_scratch[lid.x] + red_scratch[lid.x + stride];
            }
        }
        workgroupBarrier();
        stride = stride / 2u;
    }
    if (lid.x == 0u) {
        red_out[wid.x] = red_scratch[0];
    }
}

// Stage 2: single workgroup collapses the per-workgroup partials in place.
@compute @workgroup_size(256, 1, 1)
fn reduce_finalize(@builtin(local_invocation_id) lid: vec3<u32>) {
    let n = red_dims.z;
    let op = red_dims.y;
    var acc: f32 = 0.0;
    for (var i: u32 = lid.x; i < n; i = i + REDUCE_WG) {
        if (op == 3u) {
            acc = max(acc, red_out[i]);
        } else {
            acc = acc + red_out[i];
        }
    }
    red_scratch[lid.x] = acc;
    workgroupBarrier();
    var stride: u32 = REDUCE_WG / 2u;
    loop {
        if (stride == 0u) {
            break;
        }
        if (lid.x < stride) {
            if (op == 3u) {
                red_scratch[lid.x] = max(red_scratch[lid.x], red_scratch[lid.x + stride]);
            } else {
                red_scratch[lid.x] = red_scratch[lid.x] + red_scratch[lid.x + stride];
            }
        }
        workgroupBarrier();
        stride = stride / 2u;
    }
    if (lid.x == 0u) {
        red_out[0] = red_scratch[0];
    }
}

// ===========================================================================
// Out-of-place transpose: out[col * rows + row] = in[row * cols + col].
// A dense transpose is pure memory movement; doing it on-device avoids a
// host round-trip through the row-major `Vec<Vec<Scalar>>` layout.
// ===========================================================================
@group(0) @binding(0) var<storage, read> transpose_in: array<f32>;
@group(0) @binding(1) var<storage, read_write> transpose_out: array<f32>;
// [rows, cols, 0, 0]
@group(0) @binding(2) var<storage, read> transpose_dims: array<u32>;

@compute @workgroup_size(64, 1, 1)
fn transpose_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let rows = transpose_dims[0];
    let cols = transpose_dims[1];
    let i = gid.x;
    if (i >= rows * cols) {
        return;
    }
    let row = i / cols;
    let col = i % cols;
    transpose_out[col * rows + row] = transpose_in[i];
}
