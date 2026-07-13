//! Hot-path throughput micro-benchmarks for mechanical_mimalloc.
//!
//! These isolate the allocator's malloc/free fast paths from surrounding noise
//! (no `Vec` growth, no size generation, no shuffles inside the timed region).
//! The pointer pool is fixed-capacity scratch allocated via the *system*
//! allocator once at setup, so the only allocator traffic during measurement is
//! the allocator under test.
//!
//! Run the default Rust library build with:
//! `cargo bench --features bench --bench hotpath`
//!
//! Run the C ABI/interposition hot-path build with:
//! `cargo bench --features "bench interpose" --bench hotpath`
//!
//! Compare against system with:
//! `cargo bench --features "bench compare" --bench hotpath`
//!
//! Criterion IDs do not include Cargo feature sets. Use a clean
//! `CARGO_TARGET_DIR` or clear `target/criterion` when comparing default and
//! interposition builds.
//!
//! Quick iteration controls:
//!   MIMALLOC_HOTPATH_SAMPLE_SIZE=10
//!   MIMALLOC_HOTPATH_MEASUREMENT_SECS=1
//!   MIMALLOC_HOTPATH_WARMUP_SECS=1
//!   MIMALLOC_HOTPATH_NRESAMPLES=1000

// The GlobalAlloc shim calls unsafe extern "C" fns; bodies are unsafe by intent.
#![allow(unsafe_op_in_unsafe_fn)]

mod support;

use criterion::measurement::WallTime;
use criterion::{
    Bencher, BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main,
};
use std::alloc::{GlobalAlloc, Layout};
use std::env;
use std::hint::black_box as hint_black_box;
use std::time::Duration;
#[cfg(feature = "compare")]
use support::allocators::SYSTEM_ALLOCATOR;
use support::allocators::{PUBLIC_MIMALLOC, RAW_MIMALLOC};
use support::no_alloc::{
    CountingSystem, assert_no_harness_allocations_enabled, harness_allocation_check_enabled,
};
use support::pool::PointerPool;
use support::remote_fixture::RemoteFreeFixture;

#[global_allocator]
static BENCH_HARNESS_ALLOCATOR: CountingSystem = CountingSystem;

/// Maximum live allocations held in a pool. Large enough to defeat per-page
/// free-list caching dominating the measurement, small enough to keep the
/// pointer array in L1/L2.
const POOL_SIZE: usize = 4096;

/// Size classes exercising distinct mimalloc bins:
///   16   — smallest small-page bin (tightest fast path)
///   64   — common small object
///   256  — mid small-page
///   1024 — boundary of MI_SMALL_SIZE_MAX (small-page path limit)
///   4096 — large page (different page-queue path)
///   32768 — direct/large allocation (OS-backed)
const HOT_SIZES: &[usize] = &[16, 64, 256, 1024, 4096, 32768];
const HOT_ALIGNED_CASES: &[(usize, usize)] = &[(64, 64), (256, 64)];

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

fn hotpath_sample_size(default: usize) -> usize {
    env_usize("MIMALLOC_HOTPATH_SAMPLE_SIZE", default, 10)
}

fn hotpath_nresamples(default: usize) -> usize {
    env_usize("MIMALLOC_HOTPATH_NRESAMPLES", default, 1)
}

fn hotpath_measurement_time(default_secs: u64) -> Duration {
    env_secs("MIMALLOC_HOTPATH_MEASUREMENT_SECS", default_secs)
}

fn hotpath_warmup_time() -> Duration {
    env_secs("MIMALLOC_HOTPATH_WARMUP_SECS", 3)
}

fn iter_timed<R>(
    bencher: &mut Bencher<'_, WallTime>,
    verify_harness: bool,
    mut timed_body: impl FnMut() -> R,
) {
    if verify_harness {
        bencher.iter(|| black_box(assert_no_harness_allocations_enabled(&mut timed_body)));
    } else {
        bencher.iter(|| black_box(timed_body()));
    }
}

fn fill<A: GlobalAlloc>(alloc: &A, size: usize, pool: &mut PointerPool) {
    let layout = Layout::from_size_align(size, 8).unwrap();
    for i in 0..POOL_SIZE {
        let p = unsafe { alloc.alloc(layout) };
        assert!(!p.is_null(), "hot-path benchmark setup allocation failed");
        *pool.slot(i) = (p, layout);
    }
}

// ---------------------------------------------------------------------------
// Benchmark 1: paired alloc-then-free (the realistic hot path).
// Allocates POOL_SIZE blocks then frees them all. Canonical throughput number:
// total alloc+free ops per second.
// ---------------------------------------------------------------------------

fn paired_alloc_free<A: GlobalAlloc>(alloc: &A, size: usize, pool: &mut PointerPool) -> usize {
    let layout = Layout::from_size_align(size, 8).unwrap();
    paired_alloc_free_layout(alloc, layout, pool)
}

fn paired_alloc_free_layout<A: GlobalAlloc>(
    alloc: &A,
    layout: Layout,
    pool: &mut PointerPool,
) -> usize {
    for i in 0..POOL_SIZE {
        let p = unsafe { alloc.alloc(layout) };
        *pool.slot(i) = (p, layout);
    }
    for i in 0..POOL_SIZE {
        let (p, l) = *pool.slot(i);
        unsafe { alloc.dealloc(p, l) };
        *pool.slot(i) = (std::ptr::null_mut(), l);
    }
    POOL_SIZE
}

fn bench_paired_alloc_free(c: &mut Criterion) {
    let verify_harness = harness_allocation_check_enabled();
    let mut group = c.benchmark_group("hotpath/paired_alloc_free");
    group.sample_size(hotpath_sample_size(120));
    group.measurement_time(hotpath_measurement_time(8));
    group.warm_up_time(hotpath_warmup_time());
    group.noise_threshold(0.02);
    group.nresamples(hotpath_nresamples(200_000));

    for &size in HOT_SIZES {
        group.throughput(Throughput::Elements(POOL_SIZE as u64));
        let mut public_pool = PointerPool::new(POOL_SIZE);
        let mut raw_pool = PointerPool::new(POOL_SIZE);

        #[cfg(feature = "compare")]
        group.bench_with_input(BenchmarkId::new("system", size), &size, |b, &size| {
            let mut pool = PointerPool::new(POOL_SIZE);
            iter_timed(b, verify_harness, || {
                paired_alloc_free(&SYSTEM_ALLOCATOR, size, &mut pool)
            });
        });

        group.bench_with_input(
            BenchmarkId::new("mechanical_mimalloc_public", size),
            &size,
            |b, &size| {
                iter_timed(b, verify_harness, || {
                    paired_alloc_free(&PUBLIC_MIMALLOC, size, &mut public_pool)
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("mechanical_mimalloc_raw", size),
            &size,
            |b, &size| {
                iter_timed(b, verify_harness, || {
                    paired_alloc_free(&RAW_MIMALLOC, size, &mut raw_pool)
                });
            },
        );
    }
    group.finish();
}

fn bench_aligned_paired_alloc_free(c: &mut Criterion) {
    let verify_harness = harness_allocation_check_enabled();
    let mut group = c.benchmark_group("hotpath/aligned_paired_alloc_free");
    group.sample_size(hotpath_sample_size(120));
    group.measurement_time(hotpath_measurement_time(8));
    group.warm_up_time(hotpath_warmup_time());
    group.noise_threshold(0.02);
    group.nresamples(hotpath_nresamples(200_000));

    for &(size, align) in HOT_ALIGNED_CASES {
        group.throughput(Throughput::Elements(POOL_SIZE as u64));
        let layout = Layout::from_size_align(size, align).unwrap();
        let mut public_pool = PointerPool::new(POOL_SIZE);
        let mut raw_pool = PointerPool::new(POOL_SIZE);
        let case = format!("{size}/align{align}");

        #[cfg(feature = "compare")]
        group.bench_with_input(BenchmarkId::new("system", &case), &layout, |b, &layout| {
            let mut pool = PointerPool::new(POOL_SIZE);
            iter_timed(b, verify_harness, || {
                paired_alloc_free_layout(&SYSTEM_ALLOCATOR, layout, &mut pool)
            });
        });

        group.bench_with_input(
            BenchmarkId::new("mechanical_mimalloc_public", &case),
            &layout,
            |b, &layout| {
                iter_timed(b, verify_harness, || {
                    paired_alloc_free_layout(&PUBLIC_MIMALLOC, layout, &mut public_pool)
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("mechanical_mimalloc_raw", case),
            &layout,
            |b, &layout| {
                iter_timed(b, verify_harness, || {
                    paired_alloc_free_layout(&RAW_MIMALLOC, layout, &mut raw_pool)
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark 2: free-only throughput.
// Setup (untimed) fills a fresh pool; the timed routine frees it. The pool ends
// empty so nothing leaks. Isolates the free fast path.
// ---------------------------------------------------------------------------

fn free_routine<A: GlobalAlloc>(alloc: &A, pool: &mut PointerPool) -> usize {
    for i in 0..POOL_SIZE {
        let (p, l) = *pool.slot(i);
        unsafe { alloc.dealloc(p, l) };
        *pool.slot(i) = (std::ptr::null_mut(), l);
    }
    POOL_SIZE
}

fn iter_free_only<A: GlobalAlloc>(
    bencher: &mut Bencher<'_, WallTime>,
    verify_harness: bool,
    allocator: &A,
    size: usize,
) {
    if verify_harness {
        bencher.iter_batched_ref(
            || {
                let mut pool = PointerPool::new(POOL_SIZE);
                fill(allocator, size, &mut pool);
                pool
            },
            |pool| {
                black_box(assert_no_harness_allocations_enabled(|| {
                    free_routine(allocator, pool)
                }))
            },
            criterion::BatchSize::SmallInput,
        );
    } else {
        bencher.iter_batched_ref(
            || {
                let mut pool = PointerPool::new(POOL_SIZE);
                fill(allocator, size, &mut pool);
                pool
            },
            |pool| black_box(free_routine(allocator, pool)),
            criterion::BatchSize::SmallInput,
        );
    }
}

fn bench_free_only(c: &mut Criterion) {
    let verify_harness = harness_allocation_check_enabled();
    let mut group = c.benchmark_group("hotpath/free_only");
    group.sample_size(hotpath_sample_size(120));
    group.measurement_time(hotpath_measurement_time(8));
    group.warm_up_time(hotpath_warmup_time());
    group.noise_threshold(0.02);
    group.nresamples(hotpath_nresamples(200_000));

    for &size in HOT_SIZES {
        group.throughput(Throughput::Elements(POOL_SIZE as u64));

        #[cfg(feature = "compare")]
        group.bench_with_input(BenchmarkId::new("system", size), &size, |b, &size| {
            iter_free_only(b, verify_harness, &SYSTEM_ALLOCATOR, size);
        });

        group.bench_with_input(
            BenchmarkId::new("mechanical_mimalloc_public", size),
            &size,
            |b, &size| {
                iter_free_only(b, verify_harness, &PUBLIC_MIMALLOC, size);
            },
        );
        group.bench_with_input(
            BenchmarkId::new("mechanical_mimalloc_raw", size),
            &size,
            |b, &size| {
                iter_free_only(b, verify_harness, &RAW_MIMALLOC, size);
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark 3: remote free throughput.
// Setup (untimed) allocates the pool on a live owner thread and hands the
// pointers to the benchmark thread. The timed region only frees the pool,
// exercising the remote/free-from-other-thread path without including thread
// spawn or channel synchronization in the measured duration.
// ---------------------------------------------------------------------------

fn bench_remote_free(c: &mut Criterion) {
    let verify_harness = harness_allocation_check_enabled();
    let mut group = c.benchmark_group("hotpath/remote_free");
    group.sample_size(hotpath_sample_size(40));
    group.measurement_time(hotpath_measurement_time(5));
    group.warm_up_time(hotpath_warmup_time());
    group.noise_threshold(0.03);
    group.nresamples(hotpath_nresamples(100_000));

    for &size in HOT_SIZES {
        group.throughput(Throughput::Elements(POOL_SIZE as u64));

        #[cfg(feature = "compare")]
        {
            let layout = Layout::from_size_align(size, 8).unwrap();
            let mut fixture =
                RemoteFreeFixture::new(&SYSTEM_ALLOCATOR, layout, POOL_SIZE, verify_harness);
            group.bench_with_input(BenchmarkId::new("system", size), &size, |b, _| {
                b.iter_custom(|iterations| fixture.measure_remote_free(iterations))
            });
        }

        let layout = Layout::from_size_align(size, 8).unwrap();
        let mut public_fixture =
            RemoteFreeFixture::new(&PUBLIC_MIMALLOC, layout, POOL_SIZE, verify_harness);
        group.bench_with_input(
            BenchmarkId::new("mechanical_mimalloc_public", size),
            &size,
            |b, _| b.iter_custom(|iterations| public_fixture.measure_remote_free(iterations)),
        );
        let mut raw_fixture =
            RemoteFreeFixture::new(&RAW_MIMALLOC, layout, POOL_SIZE, verify_harness);
        group.bench_with_input(
            BenchmarkId::new("mechanical_mimalloc_raw", size),
            &size,
            |b, _| b.iter_custom(|iterations| raw_fixture.measure_remote_free(iterations)),
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark 4: LIFO stack pattern (alloc, free-in-reverse).
// Exercises the per-page free-list push/pop cache most favourably. This is the
// pattern mimalloc's fast path is optimised for.
// ---------------------------------------------------------------------------

fn lifo_stack<A: GlobalAlloc>(
    alloc: &A,
    size: usize,
    depth: usize,
    pool: &mut PointerPool,
) -> usize {
    let layout = Layout::from_size_align(size, 8).unwrap();
    let mut ops = 0usize;
    for i in 0..depth {
        *pool.slot(i) = (unsafe { alloc.alloc(layout) }, layout);
        ops += 1;
    }
    for i in (0..depth).rev() {
        unsafe { alloc.dealloc(pool.slot(i).0, layout) };
        *pool.slot(i) = (std::ptr::null_mut(), layout);
        ops += 1;
    }
    hint_black_box(ops)
}

fn bench_lifo_stack(c: &mut Criterion) {
    let verify_harness = harness_allocation_check_enabled();
    let mut group = c.benchmark_group("hotpath/lifo_stack");
    group.sample_size(hotpath_sample_size(150));
    group.measurement_time(hotpath_measurement_time(6));
    group.warm_up_time(hotpath_warmup_time());
    group.noise_threshold(0.02);
    group.nresamples(hotpath_nresamples(200_000));

    // Depth chosen to stay within a single small page's block count for the
    // small sizes (so we measure the per-page free-list, not page-queue churn).
    let cases: &[(usize, usize)] = &[(16, 512), (64, 256), (256, 128), (1024, 64)];

    for &(size, depth) in cases {
        group.throughput(Throughput::Elements((depth * 2) as u64));
        let mut public_pool = PointerPool::new(POOL_SIZE);
        let mut raw_pool = PointerPool::new(POOL_SIZE);

        #[cfg(feature = "compare")]
        group.bench_with_input(
            BenchmarkId::new("system", format!("{size}/{depth}")),
            &(size, depth),
            |b, &(s, d)| {
                let mut pool = PointerPool::new(POOL_SIZE);
                iter_timed(b, verify_harness, || {
                    lifo_stack(&SYSTEM_ALLOCATOR, s, d, &mut pool)
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("mechanical_mimalloc_public", format!("{size}/{depth}")),
            &(size, depth),
            |b, &(s, d)| {
                iter_timed(b, verify_harness, || {
                    lifo_stack(&PUBLIC_MIMALLOC, s, d, &mut public_pool)
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("mechanical_mimalloc_raw", format!("{size}/{depth}")),
            &(size, depth),
            |b, &(s, d)| {
                iter_timed(b, verify_harness, || {
                    lifo_stack(&RAW_MIMALLOC, s, d, &mut raw_pool)
                });
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark 5: single-thread reuse (the tightest possible loop).
// Alloc one, free one, repeat. Measures raw per-op latency of the fast path
// rather than batch throughput.
// ---------------------------------------------------------------------------

fn single_reuse<A: GlobalAlloc>(alloc: &A, size: usize, ops: usize) -> usize {
    let layout = Layout::from_size_align(size, 8).unwrap();
    let mut p = unsafe { alloc.alloc(layout) };
    for _ in 0..ops {
        unsafe { alloc.dealloc(p, layout) };
        p = unsafe { alloc.alloc(layout) };
    }
    unsafe { alloc.dealloc(p, layout) };
    hint_black_box(ops)
}

fn bench_single_reuse(c: &mut Criterion) {
    let verify_harness = harness_allocation_check_enabled();
    let mut group = c.benchmark_group("hotpath/single_reuse");
    group.sample_size(hotpath_sample_size(300));
    group.measurement_time(hotpath_measurement_time(6));
    group.warm_up_time(hotpath_warmup_time());
    group.noise_threshold(0.02);
    group.nresamples(hotpath_nresamples(300_000));

    const OPS: usize = 50_000;
    for &size in &[16usize, 64, 256, 1024] {
        group.throughput(Throughput::Elements(OPS as u64));

        #[cfg(feature = "compare")]
        group.bench_with_input(BenchmarkId::new("system", size), &size, |b, &size| {
            iter_timed(b, verify_harness, || {
                single_reuse(&SYSTEM_ALLOCATOR, size, OPS)
            });
        });

        group.bench_with_input(
            BenchmarkId::new("mechanical_mimalloc_public", size),
            &size,
            |b, &size| {
                iter_timed(b, verify_harness, || {
                    single_reuse(&PUBLIC_MIMALLOC, size, OPS)
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("mechanical_mimalloc_raw", size),
            &size,
            |b, &size| {
                iter_timed(b, verify_harness, || single_reuse(&RAW_MIMALLOC, size, OPS));
            },
        );
    }
    group.finish();
}

criterion_group!(g_paired, bench_paired_alloc_free);
criterion_group!(g_aligned_paired, bench_aligned_paired_alloc_free);
criterion_group!(g_free_only, bench_free_only);
criterion_group!(g_remote_free, bench_remote_free);
criterion_group!(g_lifo, bench_lifo_stack);
criterion_group!(g_reuse, bench_single_reuse);

criterion_main!(
    g_paired,
    g_aligned_paired,
    g_free_only,
    g_remote_free,
    g_lifo,
    g_reuse
);
