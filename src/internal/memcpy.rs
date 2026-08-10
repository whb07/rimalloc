//! Optimized x86-64 memcpy copied from faststrings commit `39e2381`.

use core::arch::x86_64::*;

/// High-performance memcpy with automatic dispatch.
///
/// Dispatch (AVX2 baseline):
/// - `0..32`: GPR / SSE overlapping ladder (avoids AVX entry on tiny copies)
/// - `32..128`: AVX2 overlapping YMM (matches glibc VEC=32 size classes)
/// - exact `128` / `256` / `512` / `1024`: branch-free leaves, checked inside
///   their size band so neighbors do not pay unrelated peels
/// - otherwise medium, then NT above [`NT_THRESHOLD`]
///
/// # Safety
///
/// - `dest` and `src` must be valid for reads/writes of `n` bytes
/// - The memory regions must not overlap
/// - AVX2 must be supported if the AVX2 path is taken
#[inline(always)]
pub unsafe fn optimized_memcpy_unified(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    // SAFETY: callers provide valid non-overlapping ranges; unaligned SIMD accesses stay within those ranges.
    unsafe {
        // Size-class dispatch into narrow leaves so each band only pays its own
        // compares and does not jump into a mega-medium dispatcher.
        if n < 32 {
            return optimized_memcpy_sse_tiny(dest, src, n);
        }
        if n < 128 {
            return optimized_memcpy_avx2_small(dest, src, n);
        }
        if n < 256 {
            if n == 128 {
                return optimized_memcpy_avx2_exact_128(dest, src);
            }
            return optimized_memcpy_avx2_overlap_129_255(dest, src, n);
        }
        if n < 512 {
            if n == 256 {
                return optimized_memcpy_avx2_exact_256(dest, src);
            }
            return optimized_memcpy_avx2_blocks_257_511(dest, src, n);
        }
        if n < 1024 {
            if n == 512 {
                return optimized_memcpy_avx2_exact_512(dest, src);
            }
            return optimized_memcpy_avx2_blocks_513_1023(dest, src, n);
        }
        if n == 1024 {
            return optimized_memcpy_avx2_exact_1024(dest, src);
        }

        if n < NT_THRESHOLD {
            optimized_memcpy_avx2_large(dest, src, n)
        } else {
            optimized_memcpy_avx2_nt(dest, src, n)
        }
    }
}

// =============================================================================
// TINY PATH: GPR / SSE2 (0-31 bytes)
// =============================================================================

#[inline(always)]
unsafe fn optimized_memcpy_sse_tiny(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    // SAFETY: callers provide valid non-overlapping ranges; unaligned SIMD accesses stay within those ranges.
    unsafe {
        // n < 32 guaranteed by dispatch
        // SAFETY: Unaligned loads/stores are valid for any alignment; caller
        // guarantees `src`/`dest` are valid for `n` bytes and non-overlapping.

        if n >= 16 {
            // 16-31 bytes: 2 × 16-byte loads/stores (overlapping)
            let v0 = _mm_loadu_si128(src as *const __m128i);
            let v1 = _mm_loadu_si128(src.add(n - 16) as *const __m128i);
            _mm_storeu_si128(dest as *mut __m128i, v0);
            _mm_storeu_si128(dest.add(n - 16) as *mut __m128i, v1);
            return dest;
        }

        if n >= 8 {
            // 8-15 bytes: 2 × 8-byte loads/stores (overlapping)
            let a = core::ptr::read_unaligned(src as *const u64);
            let b = core::ptr::read_unaligned(src.add(n - 8) as *const u64);
            core::ptr::write_unaligned(dest as *mut u64, a);
            core::ptr::write_unaligned(dest.add(n - 8) as *mut u64, b);
            return dest;
        }

        if n >= 4 {
            // 4-7 bytes: 2 × 4-byte loads/stores (overlapping)
            let a = core::ptr::read_unaligned(src as *const u32);
            let b = core::ptr::read_unaligned(src.add(n - 4) as *const u32);
            core::ptr::write_unaligned(dest as *mut u32, a);
            core::ptr::write_unaligned(dest.add(n - 4) as *mut u32, b);
            return dest;
        }

        if n >= 2 {
            // 2-3 bytes: 2 × 2-byte loads/stores (overlapping)
            let a = core::ptr::read_unaligned(src as *const u16);
            let b = core::ptr::read_unaligned(src.add(n - 2) as *const u16);
            core::ptr::write_unaligned(dest as *mut u16, a);
            core::ptr::write_unaligned(dest.add(n - 2) as *mut u16, b);
            return dest;
        }

        if n == 1 {
            *dest = *src;
        }

        dest
    }
}

/// AVX2 overlapping copy for `32 <= n < 128` (glibc-style VEC=32 classes).
#[target_feature(enable = "avx2")]
unsafe fn optimized_memcpy_avx2_small(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    // SAFETY: callers provide valid non-overlapping ranges; unaligned SIMD accesses stay within those ranges.
    unsafe {
        // SAFETY: Unaligned AVX loads/stores are valid for any alignment; caller
        // guarantees `src`/`dest` are valid for `n` bytes and non-overlapping.
        if n < 64 {
            // 32-63: two overlapping YMM (exact 32 stores the same vector twice).
            let v0 = _mm256_loadu_si256(src as *const __m256i);
            let v1 = _mm256_loadu_si256(src.add(n - 32) as *const __m256i);
            _mm256_storeu_si256(dest as *mut __m256i, v0);
            _mm256_storeu_si256(dest.add(n - 32) as *mut __m256i, v1);
            return dest;
        }

        if n == 64 {
            // Exact 64: two sequential YMM (no overlap needed).
            let h0 = _mm256_loadu_si256(src as *const __m256i);
            let h1 = _mm256_loadu_si256(src.add(32) as *const __m256i);
            _mm256_storeu_si256(dest as *mut __m256i, h0);
            _mm256_storeu_si256(dest.add(32) as *mut __m256i, h1);
            return dest;
        }

        // 65-127: glibc `last_4x_vec` shape — always 4×YMM overlapping head/tail.
        // Branch-free body; graduated 3×YMM for 65-96 lost to this at size 96.
        let h0 = _mm256_loadu_si256(src as *const __m256i);
        let h1 = _mm256_loadu_si256(src.add(32) as *const __m256i);
        let t0 = _mm256_loadu_si256(src.add(n - 64) as *const __m256i);
        let t1 = _mm256_loadu_si256(src.add(n - 32) as *const __m256i);
        _mm256_storeu_si256(dest as *mut __m256i, h0);
        _mm256_storeu_si256(dest.add(32) as *mut __m256i, h1);
        _mm256_storeu_si256(dest.add(n - 64) as *mut __m256i, t0);
        _mm256_storeu_si256(dest.add(n - 32) as *mut __m256i, t1);
        dest
    }
}

#[target_feature(enable = "avx2")]
unsafe fn copy_128_avx2(d: *mut u8, s: *const u8) {
    // SAFETY: callers provide valid non-overlapping ranges; unaligned SIMD accesses stay within those ranges.
    unsafe {
        let v0 = _mm256_loadu_si256(s as *const __m256i);
        let v1 = _mm256_loadu_si256(s.add(32) as *const __m256i);
        let v2 = _mm256_loadu_si256(s.add(64) as *const __m256i);
        let v3 = _mm256_loadu_si256(s.add(96) as *const __m256i);
        _mm256_storeu_si256(d as *mut __m256i, v0);
        _mm256_storeu_si256(d.add(32) as *mut __m256i, v1);
        _mm256_storeu_si256(d.add(64) as *mut __m256i, v2);
        _mm256_storeu_si256(d.add(96) as *mut __m256i, v3);
    }
}

#[target_feature(enable = "avx2")]
unsafe fn copy_64_avx2(d: *mut u8, s: *const u8) {
    // SAFETY: callers provide valid non-overlapping ranges; unaligned SIMD accesses stay within those ranges.
    unsafe {
        let v0 = _mm256_loadu_si256(s as *const __m256i);
        let v1 = _mm256_loadu_si256(s.add(32) as *const __m256i);
        _mm256_storeu_si256(d as *mut __m256i, v0);
        _mm256_storeu_si256(d.add(32) as *mut __m256i, v1);
    }
}

#[target_feature(enable = "avx2")]
unsafe fn copy_256_avx2(d: *mut u8, s: *const u8) {
    // SAFETY: callers provide valid non-overlapping ranges; unaligned SIMD accesses stay within those ranges.
    unsafe {
        let v0 = _mm256_loadu_si256(s as *const __m256i);
        let v1 = _mm256_loadu_si256(s.add(32) as *const __m256i);
        let v2 = _mm256_loadu_si256(s.add(64) as *const __m256i);
        let v3 = _mm256_loadu_si256(s.add(96) as *const __m256i);
        let v4 = _mm256_loadu_si256(s.add(128) as *const __m256i);
        let v5 = _mm256_loadu_si256(s.add(160) as *const __m256i);
        let v6 = _mm256_loadu_si256(s.add(192) as *const __m256i);
        let v7 = _mm256_loadu_si256(s.add(224) as *const __m256i);
        _mm256_storeu_si256(d as *mut __m256i, v0);
        _mm256_storeu_si256(d.add(32) as *mut __m256i, v1);
        _mm256_storeu_si256(d.add(64) as *mut __m256i, v2);
        _mm256_storeu_si256(d.add(96) as *mut __m256i, v3);
        _mm256_storeu_si256(d.add(128) as *mut __m256i, v4);
        _mm256_storeu_si256(d.add(160) as *mut __m256i, v5);
        _mm256_storeu_si256(d.add(192) as *mut __m256i, v6);
        _mm256_storeu_si256(d.add(224) as *mut __m256i, v7);
    }
}

/// Copy `rem` bytes with overlapping AVX2 vectors sized to the remainder.
///
/// For `rem < 32`, overlaps into the previous 32 bytes of `d`/`s`. Callers must
/// have already copied at least 32 bytes immediately before `d`/`s`.
#[target_feature(enable = "avx2")]
unsafe fn copy_remainder_avx2(d: *mut u8, s: *const u8, rem: usize) {
    // SAFETY: callers provide valid non-overlapping ranges; unaligned SIMD accesses stay within those ranges.
    unsafe {
        if rem == 0 {
            return;
        }

        // SAFETY: Unaligned AVX loads/stores are valid for any alignment; caller
        // guarantees `s`/`d` are valid for `rem` bytes (and `rem < 32` may read/write
        // 32 - rem bytes before those pointers from a prior bulk copy).
        if rem < 32 {
            let v = _mm256_loadu_si256(s.add(rem).sub(32) as *const __m256i);
            _mm256_storeu_si256(d.add(rem).sub(32) as *mut __m256i, v);
            return;
        }

        if rem <= 64 {
            if rem == 64 {
                copy_64_avx2(d, s);
                return;
            }
            let v0 = _mm256_loadu_si256(s as *const __m256i);
            let v1 = _mm256_loadu_si256(s.add(rem - 32) as *const __m256i);
            _mm256_storeu_si256(d as *mut __m256i, v0);
            _mm256_storeu_si256(d.add(rem - 32) as *mut __m256i, v1);
            return;
        }

        if rem <= 96 {
            copy_64_avx2(d, s);
            let t = _mm256_loadu_si256(s.add(rem - 32) as *const __m256i);
            _mm256_storeu_si256(d.add(rem - 32) as *mut __m256i, t);
            return;
        }

        if rem <= 128 {
            if rem == 128 {
                copy_128_avx2(d, s);
                return;
            }
            copy_64_avx2(d, s);
            copy_64_avx2(d.add(rem - 64), s.add(rem - 64));
            return;
        }

        if rem <= 192 {
            copy_128_avx2(d, s);
            copy_64_avx2(d.add(rem - 64), s.add(rem - 64));
            return;
        }

        if rem < 256 {
            // 193-255: overlapping 128B head/tail.
            copy_128_avx2(d, s);
            copy_128_avx2(d.add(rem - 128), s.add(rem - 128));
            return;
        }

        // rem == 256
        copy_256_avx2(d, s);
    }
}

// =============================================================================
// AVX PATHS
// =============================================================================

// Switch to non-temporal stores once a single copy would thrash a typical
// post-2020 L3 *slice* (Zen 2/3 CCD ≈ 16–32 MiB shared by 4–8 cores).
// On this class of CPU (e.g. 5950X: 2×32 MiB L3), ~3/4 of one CCD's L3
// (~24 MiB) is the streaming crossover; below that, cached stores win for
// hot reused destinations. Compile-time constant — no runtime LLC probe.
const NT_THRESHOLD: usize = 24 * 1024 * 1024;

/// Branch-free 128B AVX2 leaf (4×YMM load-all / store-all).
#[target_feature(enable = "avx2")]
unsafe fn optimized_memcpy_avx2_exact_128(dest: *mut u8, src: *const u8) -> *mut u8 {
    // SAFETY: callers provide valid non-overlapping ranges; unaligned SIMD accesses stay within those ranges.
    unsafe {
        let a0 = _mm256_loadu_si256(src as *const __m256i);
        let a1 = _mm256_loadu_si256(src.add(32) as *const __m256i);
        let a2 = _mm256_loadu_si256(src.add(64) as *const __m256i);
        let a3 = _mm256_loadu_si256(src.add(96) as *const __m256i);
        _mm256_storeu_si256(dest as *mut __m256i, a0);
        _mm256_storeu_si256(dest.add(32) as *mut __m256i, a1);
        _mm256_storeu_si256(dest.add(64) as *mut __m256i, a2);
        _mm256_storeu_si256(dest.add(96) as *mut __m256i, a3);
        dest
    }
}

/// Branch-free 256B AVX2 leaf (8×YMM load-all / store-all).
/// Always uses unaligned ops (glibc VMOVU style); on Zen3 aligned addresses
/// take the same fast path through `vmovups`.
#[target_feature(enable = "avx2")]
unsafe fn optimized_memcpy_avx2_exact_256(dest: *mut u8, src: *const u8) -> *mut u8 {
    // SAFETY: callers provide valid non-overlapping ranges; unaligned SIMD accesses stay within those ranges.
    unsafe {
        let a0 = _mm256_loadu_si256(src as *const __m256i);
        let a1 = _mm256_loadu_si256(src.add(32) as *const __m256i);
        let a2 = _mm256_loadu_si256(src.add(64) as *const __m256i);
        let a3 = _mm256_loadu_si256(src.add(96) as *const __m256i);
        let a4 = _mm256_loadu_si256(src.add(128) as *const __m256i);
        let a5 = _mm256_loadu_si256(src.add(160) as *const __m256i);
        let a6 = _mm256_loadu_si256(src.add(192) as *const __m256i);
        let a7 = _mm256_loadu_si256(src.add(224) as *const __m256i);
        _mm256_storeu_si256(dest as *mut __m256i, a0);
        _mm256_storeu_si256(dest.add(32) as *mut __m256i, a1);
        _mm256_storeu_si256(dest.add(64) as *mut __m256i, a2);
        _mm256_storeu_si256(dest.add(96) as *mut __m256i, a3);
        _mm256_storeu_si256(dest.add(128) as *mut __m256i, a4);
        _mm256_storeu_si256(dest.add(160) as *mut __m256i, a5);
        _mm256_storeu_si256(dest.add(192) as *mut __m256i, a6);
        _mm256_storeu_si256(dest.add(224) as *mut __m256i, a7);
        dest
    }
}

/// Branch-free 512B AVX2 leaf: two sequential 256B blocks (register-reuse).
#[target_feature(enable = "avx2")]
unsafe fn optimized_memcpy_avx2_exact_512(dest: *mut u8, src: *const u8) -> *mut u8 {
    // SAFETY: callers provide valid non-overlapping ranges; unaligned SIMD accesses stay within those ranges.
    unsafe {
        // Inline both blocks (no helper calls) — same shape as exact_256, twice.
        let a0 = _mm256_loadu_si256(src as *const __m256i);
        let a1 = _mm256_loadu_si256(src.add(32) as *const __m256i);
        let a2 = _mm256_loadu_si256(src.add(64) as *const __m256i);
        let a3 = _mm256_loadu_si256(src.add(96) as *const __m256i);
        let a4 = _mm256_loadu_si256(src.add(128) as *const __m256i);
        let a5 = _mm256_loadu_si256(src.add(160) as *const __m256i);
        let a6 = _mm256_loadu_si256(src.add(192) as *const __m256i);
        let a7 = _mm256_loadu_si256(src.add(224) as *const __m256i);
        _mm256_storeu_si256(dest as *mut __m256i, a0);
        _mm256_storeu_si256(dest.add(32) as *mut __m256i, a1);
        _mm256_storeu_si256(dest.add(64) as *mut __m256i, a2);
        _mm256_storeu_si256(dest.add(96) as *mut __m256i, a3);
        _mm256_storeu_si256(dest.add(128) as *mut __m256i, a4);
        _mm256_storeu_si256(dest.add(160) as *mut __m256i, a5);
        _mm256_storeu_si256(dest.add(192) as *mut __m256i, a6);
        _mm256_storeu_si256(dest.add(224) as *mut __m256i, a7);

        let b0 = _mm256_loadu_si256(src.add(256) as *const __m256i);
        let b1 = _mm256_loadu_si256(src.add(288) as *const __m256i);
        let b2 = _mm256_loadu_si256(src.add(320) as *const __m256i);
        let b3 = _mm256_loadu_si256(src.add(352) as *const __m256i);
        let b4 = _mm256_loadu_si256(src.add(384) as *const __m256i);
        let b5 = _mm256_loadu_si256(src.add(416) as *const __m256i);
        let b6 = _mm256_loadu_si256(src.add(448) as *const __m256i);
        let b7 = _mm256_loadu_si256(src.add(480) as *const __m256i);
        _mm256_storeu_si256(dest.add(256) as *mut __m256i, b0);
        _mm256_storeu_si256(dest.add(288) as *mut __m256i, b1);
        _mm256_storeu_si256(dest.add(320) as *mut __m256i, b2);
        _mm256_storeu_si256(dest.add(352) as *mut __m256i, b3);
        _mm256_storeu_si256(dest.add(384) as *mut __m256i, b4);
        _mm256_storeu_si256(dest.add(416) as *mut __m256i, b5);
        _mm256_storeu_si256(dest.add(448) as *mut __m256i, b6);
        _mm256_storeu_si256(dest.add(480) as *mut __m256i, b7);
        dest
    }
}

/// Branch-free 1024B AVX2 leaf: four sequential 256B blocks.
#[target_feature(enable = "avx2")]
unsafe fn optimized_memcpy_avx2_exact_1024(dest: *mut u8, src: *const u8) -> *mut u8 {
    // SAFETY: callers provide valid non-overlapping ranges; unaligned SIMD accesses stay within those ranges.
    unsafe {
        copy_256_avx2(dest, src);
        copy_256_avx2(dest.add(256), src.add(256));
        copy_256_avx2(dest.add(512), src.add(512));
        copy_256_avx2(dest.add(768), src.add(768));
        dest
    }
}

/// Overlapping AVX2 copy for `129 <= n <= 255`.
#[target_feature(enable = "avx2")]
unsafe fn optimized_memcpy_avx2_overlap_129_255(
    dest: *mut u8,
    src: *const u8,
    n: usize,
) -> *mut u8 {
    // SAFETY: callers provide valid non-overlapping ranges; unaligned SIMD accesses stay within those ranges.
    unsafe {
        // SAFETY: Unaligned AVX loads/stores are valid for any alignment; caller
        // guarantees `src`/`dest` are valid for `n` bytes and non-overlapping.
        if n <= 192 {
            // 129–192: 128B head + 64B overlapping tail (6×YMM, not 8).
            let v0 = _mm256_loadu_si256(src as *const __m256i);
            let v1 = _mm256_loadu_si256(src.add(32) as *const __m256i);
            let v2 = _mm256_loadu_si256(src.add(64) as *const __m256i);
            let v3 = _mm256_loadu_si256(src.add(96) as *const __m256i);
            let t0 = _mm256_loadu_si256(src.add(n - 64) as *const __m256i);
            let t1 = _mm256_loadu_si256(src.add(n - 32) as *const __m256i);
            _mm256_storeu_si256(dest as *mut __m256i, v0);
            _mm256_storeu_si256(dest.add(32) as *mut __m256i, v1);
            _mm256_storeu_si256(dest.add(64) as *mut __m256i, v2);
            _mm256_storeu_si256(dest.add(96) as *mut __m256i, v3);
            _mm256_storeu_si256(dest.add(n - 64) as *mut __m256i, t0);
            _mm256_storeu_si256(dest.add(n - 32) as *mut __m256i, t1);
            return dest;
        }

        // 193–255: 128B head + 128B overlapping tail.
        let v0 = _mm256_loadu_si256(src as *const __m256i);
        let v1 = _mm256_loadu_si256(src.add(32) as *const __m256i);
        let v2 = _mm256_loadu_si256(src.add(64) as *const __m256i);
        let v3 = _mm256_loadu_si256(src.add(96) as *const __m256i);
        let t0 = _mm256_loadu_si256(src.add(n - 128) as *const __m256i);
        let t1 = _mm256_loadu_si256(src.add(n - 96) as *const __m256i);
        let t2 = _mm256_loadu_si256(src.add(n - 64) as *const __m256i);
        let t3 = _mm256_loadu_si256(src.add(n - 32) as *const __m256i);
        _mm256_storeu_si256(dest as *mut __m256i, v0);
        _mm256_storeu_si256(dest.add(32) as *mut __m256i, v1);
        _mm256_storeu_si256(dest.add(64) as *mut __m256i, v2);
        _mm256_storeu_si256(dest.add(96) as *mut __m256i, v3);
        _mm256_storeu_si256(dest.add(n - 128) as *mut __m256i, t0);
        _mm256_storeu_si256(dest.add(n - 96) as *mut __m256i, t1);
        _mm256_storeu_si256(dest.add(n - 64) as *mut __m256i, t2);
        _mm256_storeu_si256(dest.add(n - 32) as *mut __m256i, t3);
        dest
    }
}

/// `257 <= n <= 511`: one 256B block + sized remainder; upper band overlaps.
#[target_feature(enable = "avx2")]
unsafe fn optimized_memcpy_avx2_blocks_257_511(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    // SAFETY: callers provide valid non-overlapping ranges; unaligned SIMD accesses stay within those ranges.
    unsafe {
        // SAFETY: caller guarantees valid non-overlapping `n` bytes; remainder may
        // overlap into the 256B head for rem < 32.
        if n <= 384 {
            copy_256_avx2(dest, src);
            copy_remainder_avx2(dest.add(256), src.add(256), n - 256);
            return dest;
        }

        // 385–511: overlapping 256B head/tail (same traffic as exact 512).
        copy_256_avx2(dest, src);
        copy_256_avx2(dest.add(n - 256), src.add(n - 256));
        dest
    }
}

/// `513 <= n <= 1023`: straight 256B blocks + sized remainder (no while).
#[target_feature(enable = "avx2")]
unsafe fn optimized_memcpy_avx2_blocks_513_1023(
    dest: *mut u8,
    src: *const u8,
    n: usize,
) -> *mut u8 {
    // SAFETY: callers provide valid non-overlapping ranges; unaligned SIMD accesses stay within those ranges.
    unsafe {
        // SAFETY: caller guarantees valid non-overlapping `n` bytes.
        copy_256_avx2(dest, src);
        copy_256_avx2(dest.add(256), src.add(256));
        if n <= 768 {
            copy_remainder_avx2(dest.add(512), src.add(512), n - 512);
            return dest;
        }

        // 769–1023: third block + remainder (exact 1024 peeled upstream).
        copy_256_avx2(dest.add(512), src.add(512));
        copy_remainder_avx2(dest.add(768), src.add(768), n - 768);
        dest
    }
}

/// `n >= 1025`, below NT: dest-aligned main loop + sized remainder.
#[target_feature(enable = "avx2")]
unsafe fn optimized_memcpy_avx2_large(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    // SAFETY: callers provide valid non-overlapping ranges; unaligned SIMD accesses stay within those ranges.
    unsafe {
        // SAFETY: Unaligned AVX loads/stores are valid for any alignment; caller
        // guarantees `src`/`dest` are valid for `n` bytes and non-overlapping.
        let mut d = dest;
        let mut s = src;
        let mut rem = n;

        // Keep 1025–1280 off the align prologue: four unaligned blocks + rem.
        if n <= 1280 {
            while rem >= 256 {
                copy_256_avx2(d, s);
                d = d.add(256);
                s = s.add(256);
                rem -= 256;
            }
            copy_remainder_avx2(d, s, rem);
            return dest;
        }

        let misalign = (d as usize) & 31;
        if misalign != 0 {
            let advance = 32 - misalign;
            let first_v = _mm256_loadu_si256(s as *const __m256i);
            _mm256_storeu_si256(d as *mut __m256i, first_v);
            d = d.add(advance);
            s = s.add(advance);
            rem -= advance;
        }

        // Prefetch pays off only once the copy clearly spills L2. At 256KiB the
        // src+dst working set already equals a 512KiB L2 and prefetch just adds
        // contention, so gate above it; smaller copies keep the plain loop.
        if n > 512 * 1024 {
            while rem >= 256 {
                // ~3 iterations ahead into L1 (T0). Prefetches never fault, so
                // running past the buffer end on final iterations is harmless.
                _mm_prefetch(s.add(768) as *const i8, _MM_HINT_T0);
                _mm_prefetch(s.add(832) as *const i8, _MM_HINT_T0);
                _mm_prefetch(s.add(896) as *const i8, _MM_HINT_T0);
                _mm_prefetch(s.add(960) as *const i8, _MM_HINT_T0);

                let a0 = _mm256_loadu_si256(s as *const __m256i);
                let a1 = _mm256_loadu_si256(s.add(32) as *const __m256i);
                let a2 = _mm256_loadu_si256(s.add(64) as *const __m256i);
                let a3 = _mm256_loadu_si256(s.add(96) as *const __m256i);
                let a4 = _mm256_loadu_si256(s.add(128) as *const __m256i);
                let a5 = _mm256_loadu_si256(s.add(160) as *const __m256i);
                let a6 = _mm256_loadu_si256(s.add(192) as *const __m256i);
                let a7 = _mm256_loadu_si256(s.add(224) as *const __m256i);

                // SAFETY: Aligned stores require 32-byte alignment; `d` is aligned
                // by the prologue and advances in 32-byte multiples.
                _mm256_store_si256(d as *mut __m256i, a0);
                _mm256_store_si256(d.add(32) as *mut __m256i, a1);
                _mm256_store_si256(d.add(64) as *mut __m256i, a2);
                _mm256_store_si256(d.add(96) as *mut __m256i, a3);
                _mm256_store_si256(d.add(128) as *mut __m256i, a4);
                _mm256_store_si256(d.add(160) as *mut __m256i, a5);
                _mm256_store_si256(d.add(192) as *mut __m256i, a6);
                _mm256_store_si256(d.add(224) as *mut __m256i, a7);

                d = d.add(256);
                s = s.add(256);
                rem -= 256;
            }

            copy_remainder_avx2(d, s, rem);
            return dest;
        }

        while rem >= 256 {
            let a0 = _mm256_loadu_si256(s as *const __m256i);
            let a1 = _mm256_loadu_si256(s.add(32) as *const __m256i);
            let a2 = _mm256_loadu_si256(s.add(64) as *const __m256i);
            let a3 = _mm256_loadu_si256(s.add(96) as *const __m256i);
            let a4 = _mm256_loadu_si256(s.add(128) as *const __m256i);
            let a5 = _mm256_loadu_si256(s.add(160) as *const __m256i);
            let a6 = _mm256_loadu_si256(s.add(192) as *const __m256i);
            let a7 = _mm256_loadu_si256(s.add(224) as *const __m256i);

            // SAFETY: Aligned stores require 32-byte alignment; `d` is aligned by
            // the prologue and advances in 32-byte multiples.
            _mm256_store_si256(d as *mut __m256i, a0);
            _mm256_store_si256(d.add(32) as *mut __m256i, a1);
            _mm256_store_si256(d.add(64) as *mut __m256i, a2);
            _mm256_store_si256(d.add(96) as *mut __m256i, a3);
            _mm256_store_si256(d.add(128) as *mut __m256i, a4);
            _mm256_store_si256(d.add(160) as *mut __m256i, a5);
            _mm256_store_si256(d.add(192) as *mut __m256i, a6);
            _mm256_store_si256(d.add(224) as *mut __m256i, a7);

            d = d.add(256);
            s = s.add(256);
            rem -= 256;
        }

        copy_remainder_avx2(d, s, rem);
        dest
    }
}

#[target_feature(enable = "avx2")]
unsafe fn optimized_memcpy_avx2_nt(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    // SAFETY: callers provide valid non-overlapping ranges; unaligned SIMD accesses stay within those ranges.
    unsafe {
        let mut d = dest;
        let mut s = src;
        let mut rem = n;

        // Alignment prologue (NT stores require 32-byte alignment)
        let misalign = (d as usize) & 31;
        if misalign != 0 {
            let advance = 32 - misalign;
            // SAFETY: Unaligned load/store are valid for any alignment; caller
            // guarantees `src`/`dest` are valid for `n` bytes.
            let v = _mm256_loadu_si256(s as *const __m256i);
            _mm256_storeu_si256(d as *mut __m256i, v);
            d = d.add(advance);
            s = s.add(advance);
            rem -= advance;
        }

        // Main loop: non-temporal stores (bypass cache) + source prefetch.
        // 256B/iter matches the cached large path so we stay bandwidth-limited.
        while rem >= 256 {
            // Prefetch source ahead; dest is intentionally not cached (NT).
            _mm_prefetch(s.add(768) as *const i8, _MM_HINT_T0);
            _mm_prefetch(s.add(832) as *const i8, _MM_HINT_T0);
            _mm_prefetch(s.add(896) as *const i8, _MM_HINT_T0);
            _mm_prefetch(s.add(960) as *const i8, _MM_HINT_T0);

            // SAFETY: Unaligned loads are valid for any alignment; caller guarantees
            // `src` is readable for the loop span.
            let a0 = _mm256_loadu_si256(s as *const __m256i);
            let a1 = _mm256_loadu_si256(s.add(32) as *const __m256i);
            let a2 = _mm256_loadu_si256(s.add(64) as *const __m256i);
            let a3 = _mm256_loadu_si256(s.add(96) as *const __m256i);
            let a4 = _mm256_loadu_si256(s.add(128) as *const __m256i);
            let a5 = _mm256_loadu_si256(s.add(160) as *const __m256i);
            let a6 = _mm256_loadu_si256(s.add(192) as *const __m256i);
            let a7 = _mm256_loadu_si256(s.add(224) as *const __m256i);

            // SAFETY: Non-temporal stores require 32-byte alignment; `d` is aligned
            // by the prologue and advances in 32-byte multiples.
            _mm256_stream_si256(d as *mut __m256i, a0);
            _mm256_stream_si256(d.add(32) as *mut __m256i, a1);
            _mm256_stream_si256(d.add(64) as *mut __m256i, a2);
            _mm256_stream_si256(d.add(96) as *mut __m256i, a3);
            _mm256_stream_si256(d.add(128) as *mut __m256i, a4);
            _mm256_stream_si256(d.add(160) as *mut __m256i, a5);
            _mm256_stream_si256(d.add(192) as *mut __m256i, a6);
            _mm256_stream_si256(d.add(224) as *mut __m256i, a7);

            d = d.add(256);
            s = s.add(256);
            rem -= 256;
        }

        // Tail with regular stores; rem < 256 and prologue already wrote >= 32 when
        // rem < 32 after the loop (or rem == 0).
        if rem > 0 {
            if rem < 32 {
                // Overlap into the last NT/prologue block.
                let v = _mm256_loadu_si256(s.add(rem).sub(32) as *const __m256i);
                _mm256_storeu_si256(d.add(rem).sub(32) as *mut __m256i, v);
            } else {
                copy_remainder_avx2(d, s, rem);
            }
        }

        // REQUIRED: fence ensures NT stores are visible before function returns
        // SAFETY: SFENCE orders prior non-temporal stores; required for correctness
        // before returning to callers that may observe the memory.
        _mm_sfence();

        dest
    }
}

use core::ffi::c_void;

/// # Safety
/// `dst..dst + n` and `src..src + n` must be valid non-overlapping byte ranges.
#[inline]
pub unsafe fn _mi_memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    // SAFETY: callers provide valid non-overlapping ranges; unaligned SIMD accesses stay within those ranges.
    unsafe { optimized_memcpy_unified(dst.cast::<u8>(), src.cast::<u8>(), n).cast::<c_void>() }
}

/// # Safety
/// `dst..dst + n` and `src..src + n` must be valid non-overlapping byte ranges.
#[inline]
pub unsafe fn _mi_memcpy_aligned(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
    // SAFETY: callers provide valid non-overlapping ranges; unaligned SIMD accesses stay within those ranges.
    unsafe { optimized_memcpy_unified(dst.cast::<u8>(), src.cast::<u8>(), n).cast::<c_void>() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memcpy_copies_every_size_through_1024_without_overwriting() {
        let mut src = [0u8; 1025];
        let mut dst = [0u8; 1025];
        for (index, byte) in src.iter_mut().enumerate() {
            *byte = (index % 251) as u8;
        }

        for n in 0..=1024 {
            dst.fill(0);
            // SAFETY: the arrays provide disjoint valid ranges for every tested length.
            unsafe {
                optimized_memcpy_unified(dst.as_mut_ptr(), src.as_ptr(), n);
            }
            assert_eq!(&dst[..n], &src[..n], "copy failed at size {n}");
            assert_eq!(dst[n], 0, "copy overwrote byte {n}");
        }
    }

    #[test]
    fn memcpy_handles_alignment_and_dispatch_boundaries() {
        let mut src = [0u8; 9216];
        let mut dst = [0u8; 9216];
        for (index, byte) in src.iter_mut().enumerate() {
            *byte = (index % 251) as u8;
        }

        for src_offset in 0..32 {
            for dst_offset in 0..32 {
                for n in [
                    0, 1, 7, 8, 15, 16, 23, 24, 31, 32, 63, 64, 65, 87, 88, 99, 100, 127, 128, 129,
                    159, 160, 175, 176, 223, 224, 255, 256, 257, 511, 512, 513, 1023, 1024, 1025,
                    2048, 4096, 8192,
                ] {
                    dst.fill(0);
                    // SAFETY: offsets and lengths stay within separate source and destination arrays.
                    unsafe {
                        optimized_memcpy_unified(
                            dst.as_mut_ptr().add(dst_offset),
                            src.as_ptr().add(src_offset),
                            n,
                        );
                    }
                    assert_eq!(
                        &dst[dst_offset..dst_offset + n],
                        &src[src_offset..src_offset + n],
                        "copy failed at size {n}, src offset {src_offset}, dst offset {dst_offset}",
                    );
                    assert_eq!(
                        dst[dst_offset + n],
                        0,
                        "copy overwrote size {n}, src offset {src_offset}, dst offset {dst_offset}",
                    );
                }
            }
        }
    }
}
