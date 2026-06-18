//! Hot-path throughput micro-benchmarks for mechanical_mimalloc.
//!
//! These isolate the allocator's malloc/free fast paths from surrounding noise
//! (no `Vec` growth, no size generation, no shuffles inside the timed region).
//! The pointer pool is a fixed-capacity `Vec` allocated via the *system*
//! allocator once at setup, so the only allocator traffic during measurement is
//! the allocator under test.
//!
//! Run with: `cargo bench --features bench --bench hotpath`
//! Compare against system: `cargo bench --features bench,compare --bench hotpath`

// The GlobalAlloc shim calls unsafe extern "C" fns; bodies are unsafe by intent.
#![allow(unsafe_op_in_unsafe_fn)]

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
#[cfg(feature = "compare")]
use std::alloc::System;
use std::alloc::{GlobalAlloc, Layout};
use std::hint::black_box as hint_black_box;
use std::time::Duration;

/// `GlobalAlloc` shim over mechanical_mimalloc's C-ABI entry points.
struct MechanicalMiMalloc;

unsafe impl GlobalAlloc for MechanicalMiMalloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.align() <= 16 {
            mechanical_mimalloc::raw::alloc::mi_malloc(layout.size()).cast()
        } else {
            mechanical_mimalloc::raw::alloc_aligned::mi_malloc_aligned(
                layout.size(),
                layout.align(),
            )
            .cast()
        }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        mechanical_mimalloc::raw::free::mi_free(ptr.cast());
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if layout.align() <= 16 {
            mechanical_mimalloc::raw::alloc::mi_realloc(ptr.cast(), new_size).cast()
        } else {
            mechanical_mimalloc::raw::alloc_aligned::mi_realloc_aligned(
                ptr.cast(),
                new_size,
                layout.align(),
            )
            .cast()
        }
    }
}

static MECH: MechanicalMiMalloc = MechanicalMiMalloc;
#[cfg(feature = "compare")]
static SYS: System = System;

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

/// Fixed-capacity pointer pool, grown once via the system allocator so it never
/// perturbs the allocator under test. Slots hold `(ptr, layout)`; an empty slot
/// is `(null, Layout::new::<u8>())`.
struct PtrPool {
    slots: Vec<(*mut u8, Layout)>,
}

impl PtrPool {
    fn new() -> Self {
        let mut slots = Vec::with_capacity(POOL_SIZE);
        slots.resize(POOL_SIZE, (std::ptr::null_mut(), Layout::new::<u8>()));
        Self { slots }
    }

    #[inline(always)]
    fn slot(&mut self, i: usize) -> &mut (*mut u8, Layout) {
        &mut self.slots[i]
    }
}

fn fill<A: GlobalAlloc>(alloc: &A, size: usize, pool: &mut PtrPool) {
    let layout = Layout::from_size_align(size, 8).unwrap();
    for i in 0..POOL_SIZE {
        let p = unsafe { alloc.alloc(layout) };
        *pool.slot(i) = (p, layout);
    }
}

// ---------------------------------------------------------------------------
// Benchmark 1: paired alloc-then-free (the realistic hot path).
// Allocates POOL_SIZE blocks then frees them all. Canonical throughput number:
// total alloc+free ops per second.
// ---------------------------------------------------------------------------

fn paired_alloc_free<A: GlobalAlloc>(alloc: &A, size: usize, pool: &mut PtrPool) -> usize {
    let layout = Layout::from_size_align(size, 8).unwrap();
    for i in 0..POOL_SIZE {
        let p = unsafe { alloc.alloc(layout) };
        *pool.slot(i) = (p, layout);
    }
    for i in 0..POOL_SIZE {
        let (p, l) = *pool.slot(i);
        unsafe { alloc.dealloc(p, l) };
    }
    POOL_SIZE
}

fn bench_paired_alloc_free(c: &mut Criterion) {
    let mut group = c.benchmark_group("hotpath/paired_alloc_free");
    group.sample_size(120);
    group.measurement_time(Duration::from_secs(8));
    group.noise_threshold(0.02);
    group.nresamples(200_000);

    for &size in HOT_SIZES {
        group.throughput(Throughput::Elements(POOL_SIZE as u64));
        let mut pool = PtrPool::new();

        #[cfg(feature = "compare")]
        group.bench_with_input(BenchmarkId::new("system", size), &size, |b, &size| {
            b.iter(|| black_box(paired_alloc_free(&SYS, size, &mut pool)))
        });

        group.bench_with_input(
            BenchmarkId::new("mechanical_mimalloc", size),
            &size,
            |b, &size| b.iter(|| black_box(paired_alloc_free(&MECH, size, &mut pool))),
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark 2: free-only throughput.
// Setup (untimed) fills a fresh pool; the timed routine frees it. The pool ends
// empty so nothing leaks. Isolates the free fast path.
// ---------------------------------------------------------------------------

fn free_routine<A: GlobalAlloc>(alloc: &A, pool: &mut PtrPool) -> usize {
    for i in 0..POOL_SIZE {
        let (p, l) = *pool.slot(i);
        unsafe { alloc.dealloc(p, l) };
    }
    POOL_SIZE
}

fn bench_free_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("hotpath/free_only");
    group.sample_size(120);
    group.measurement_time(Duration::from_secs(8));
    group.noise_threshold(0.02);
    group.nresamples(200_000);

    for &size in HOT_SIZES {
        group.throughput(Throughput::Elements(POOL_SIZE as u64));

        #[cfg(feature = "compare")]
        group.bench_with_input(BenchmarkId::new("system", size), &size, |b, &size| {
            b.iter_batched_ref(
                || {
                    let mut p = PtrPool::new();
                    fill(&SYS, size, &mut p);
                    p
                },
                |pool| black_box(free_routine(&SYS, pool)),
                criterion::BatchSize::SmallInput,
            )
        });

        group.bench_with_input(
            BenchmarkId::new("mechanical_mimalloc", size),
            &size,
            |b, &size| {
                b.iter_batched_ref(
                    || {
                        let mut p = PtrPool::new();
                        fill(&MECH, size, &mut p);
                        p
                    },
                    |pool| black_box(free_routine(&MECH, pool)),
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark 4: LIFO stack pattern (alloc, free-in-reverse).
// Exercises the per-page free-list push/pop cache most favourably. This is the
// pattern mimalloc's fast path is optimised for.
// ---------------------------------------------------------------------------

fn lifo_stack<A: GlobalAlloc>(alloc: &A, size: usize, depth: usize) -> usize {
    let layout = Layout::from_size_align(size, 8).unwrap();
    // On-stack ring to avoid allocator traffic; depth must be <= POOL_SIZE.
    let mut stack = vec![std::ptr::null_mut::<u8>(); depth];
    let mut ops = 0usize;
    for slot in stack.iter_mut() {
        *slot = unsafe { alloc.alloc(layout) };
        ops += 1;
    }
    for slot in stack.into_iter().rev() {
        unsafe { alloc.dealloc(slot, layout) };
        ops += 1;
    }
    hint_black_box(ops)
}

fn bench_lifo_stack(c: &mut Criterion) {
    let mut group = c.benchmark_group("hotpath/lifo_stack");
    group.sample_size(150);
    group.measurement_time(Duration::from_secs(6));
    group.noise_threshold(0.02);
    group.nresamples(200_000);

    // Depth chosen to stay within a single small page's block count for the
    // small sizes (so we measure the per-page free-list, not page-queue churn).
    let cases: &[(usize, usize)] = &[(16, 512), (64, 256), (256, 128), (1024, 64)];

    for &(size, depth) in cases {
        group.throughput(Throughput::Elements((depth * 2) as u64));

        #[cfg(feature = "compare")]
        group.bench_with_input(
            BenchmarkId::new("system", format!("{size}/{depth}")),
            &(size, depth),
            |b, &(s, d)| b.iter(|| black_box(lifo_stack(&SYS, s, d))),
        );

        group.bench_with_input(
            BenchmarkId::new("mechanical_mimalloc", format!("{size}/{depth}")),
            &(size, depth),
            |b, &(s, d)| b.iter(|| black_box(lifo_stack(&MECH, s, d))),
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
    let mut group = c.benchmark_group("hotpath/single_reuse");
    group.sample_size(300);
    group.measurement_time(Duration::from_secs(6));
    group.noise_threshold(0.02);
    group.nresamples(300_000);

    const OPS: usize = 50_000;
    for &size in &[16usize, 64, 256, 1024] {
        group.throughput(Throughput::Elements(OPS as u64));

        #[cfg(feature = "compare")]
        group.bench_with_input(BenchmarkId::new("system", size), &size, |b, &size| {
            b.iter(|| black_box(single_reuse(&SYS, size, OPS)))
        });

        group.bench_with_input(
            BenchmarkId::new("mechanical_mimalloc", size),
            &size,
            |b, &size| b.iter(|| black_box(single_reuse(&MECH, size, OPS))),
        );
    }
    group.finish();
}

criterion_group!(g_paired, bench_paired_alloc_free);
criterion_group!(g_free_only, bench_free_only);
criterion_group!(g_lifo, bench_lifo_stack);
criterion_group!(g_reuse, bench_single_reuse);

criterion_main!(g_paired, g_free_only, g_lifo, g_reuse);
