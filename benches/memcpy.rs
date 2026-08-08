//! Compare allocator `_mi_memcpy` against glibc `memcpy` for small sizes.
//!
//! Run with:
//! `cargo bench --features bench --bench memcpy`
//!
//! Quick iteration:
//!   MIMALLOC_MEMCPY_SAMPLE_SIZE=20
//!   MIMALLOC_MEMCPY_MEASUREMENT_SECS=1
//!   MIMALLOC_MEMCPY_WARMUP_SECS=1

#![allow(unsafe_op_in_unsafe_fn)]

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::env;
use std::time::Duration;

const SIZES: &[usize] = &[8, 16, 64, 128, 256];
const COPIES_PER_ITER: u64 = 4_096;

fn env_usize(name: &str, default: usize, min: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
        .max(min)
}

fn env_secs(name: &str, default: u64) -> Duration {
    Duration::from_secs(
        env::var(name)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
            .max(1),
    )
}

fn alloc_buffers(size: usize) -> (Vec<u8>, Vec<u8>) {
    let mut src = vec![0u8; size];
    let dst = vec![0u8; size];
    for (i, byte) in src.iter_mut().enumerate() {
        *byte = (i as u8).wrapping_mul(31).wrapping_add(7);
    }
    (src, dst)
}

fn bench_memcpy(c: &mut Criterion) {
    let mut group = c.benchmark_group("memcpy");
    group.sample_size(env_usize("MIMALLOC_MEMCPY_SAMPLE_SIZE", 50, 10));
    group.measurement_time(env_secs("MIMALLOC_MEMCPY_MEASUREMENT_SECS", 2));
    group.warm_up_time(env_secs("MIMALLOC_MEMCPY_WARMUP_SECS", 1));

    for &size in SIZES {
        {
            let (src, mut mi_dst) = alloc_buffers(size);
            let mut glibc_dst = vec![0u8; size];
            unsafe {
                mechanical_mimalloc::__bench::_mi_memcpy(
                    mi_dst.as_mut_ptr().cast(),
                    src.as_ptr().cast(),
                    size,
                );
                let _ = libc::memcpy(glibc_dst.as_mut_ptr().cast(), src.as_ptr().cast(), size);
            }
            assert_eq!(mi_dst, glibc_dst);
            assert_eq!(mi_dst, src);
        }

        group.throughput(Throughput::Bytes(size as u64 * COPIES_PER_ITER));

        group.bench_with_input(BenchmarkId::new("mi_memcpy", size), &size, |b, &size| {
            let (src, mut dst) = alloc_buffers(size);
            b.iter(|| unsafe {
                let n = black_box(size);
                let src_ptr = black_box(src.as_ptr()).cast();
                let dst_ptr = black_box(dst.as_mut_ptr()).cast();
                for _ in 0..COPIES_PER_ITER {
                    mechanical_mimalloc::__bench::_mi_memcpy(dst_ptr, src_ptr, n);
                }
                black_box(dst.as_ptr());
            });
        });

        group.bench_with_input(BenchmarkId::new("glibc_memcpy", size), &size, |b, &size| {
            let (src, mut dst) = alloc_buffers(size);
            b.iter(|| unsafe {
                let n = black_box(size);
                let src_ptr = black_box(src.as_ptr()).cast();
                let dst_ptr = black_box(dst.as_mut_ptr()).cast();
                for _ in 0..COPIES_PER_ITER {
                    // Force a real libc call; Rust's copy_* helpers can lower to
                    // inline code for small constant sizes.
                    let _ = libc::memcpy(dst_ptr, src_ptr, n);
                }
                black_box(dst.as_ptr());
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_memcpy);
criterion_main!(benches);
