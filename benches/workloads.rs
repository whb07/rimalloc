// The GlobalAlloc shim calls unsafe extern "C" fns; bodies are unsafe by intent.
#![allow(unsafe_op_in_unsafe_fn)]

//! Allocator workload benchmarks for mechanical_mimalloc.
//!
//! Mirrors the workload shape from yet-another-mimalloc-rs, but drives the
//! allocator through the crate's C-ABI (`mi_malloc`/`mi_free`) via a `GlobalAlloc`
//! shim, since this crate does not expose a native Rust `GlobalAlloc` impl.
//!
//! Run with: `cargo bench --features bench`
//! Compare against system: add `--features bench,compare`

use criterion::{
    AxisScale, BenchmarkId, Criterion, PlotConfiguration, Throughput, black_box, criterion_group,
    criterion_main,
};
use rand::SeedableRng;
use rand::prelude::*;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
#[cfg(feature = "compare")]
use std::alloc::System;
use std::alloc::{GlobalAlloc, Layout};
use std::time::Duration;

/// Deterministic seed for reproducible benchmarks.
const BENCH_SEED: u64 = 0xDEADBEEF_CAFEBABE;

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
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if layout.align() <= 16 {
            mechanical_mimalloc::raw::alloc::mi_calloc(1, layout.size()).cast()
        } else {
            // Fall back to alloc + memset for over-aligned zeroed requests.
            let p = self.alloc(layout);
            if !p.is_null() {
                std::ptr::write_bytes(p, 0, layout.size());
            }
            p
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

static MECH_MIMALLOC: MechanicalMiMalloc = MechanicalMiMalloc;

#[cfg(feature = "compare")]
static SYSTEM: System = System;

#[inline]
fn logical_cpus() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

// ---------------------------------------------------------------------------
// Workload definitions
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
enum Workload {
    RandomSmall,
    FixedSize(usize),
    Increasing,
    HighFragmentation,
}

impl Workload {
    fn name(&self) -> String {
        match self {
            Workload::RandomSmall => "random_8-1024".to_string(),
            Workload::FixedSize(s) => format!("fixed_{s}"),
            Workload::Increasing => "increasing".to_string(),
            Workload::HighFragmentation => "high_frag".to_string(),
        }
    }

    fn generate_sizes(&self, count: usize, seed: u64) -> Vec<usize> {
        let mut rng = StdRng::seed_from_u64(seed);
        match self {
            Workload::RandomSmall => (0..count).map(|_| rng.gen_range(8..=1024)).collect(),
            Workload::FixedSize(size) => vec![*size; count],
            Workload::Increasing => (3..=10)
                .flat_map(|i| vec![1usize << i; count / 8])
                .collect(),
            Workload::HighFragmentation => (0..count)
                .map(|i| if i % 2 == 0 { 64 } else { 32 })
                .collect(),
        }
    }
}

/// Wrapper to make a raw pointer `Send`.
struct SendPtr(*mut u8);
unsafe impl Send for SendPtr {}

// ---------------------------------------------------------------------------
// Single-threaded alloc/free loop
// ---------------------------------------------------------------------------

fn bench_single_thread_workload<A: GlobalAlloc + Sync>(allocator: &A, sizes: &[usize]) -> usize {
    let mut allocations = Vec::with_capacity(sizes.len());
    for &size in sizes {
        let layout = Layout::from_size_align(size, 8).unwrap();
        let ptr = unsafe { allocator.alloc(layout) };
        if !ptr.is_null() {
            allocations.push((SendPtr(ptr), layout));
        }
    }
    for (ptr, layout) in allocations.drain(..) {
        unsafe { allocator.dealloc(ptr.0, layout) };
    }
    sizes.len()
}

fn run_single_thread(c: &mut Criterion, workload: Workload) {
    let mut group = c.benchmark_group(format!("single_thread/{}", workload.name()));
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));
    group.sample_size(200);
    group.measurement_time(Duration::from_secs(12));
    group.noise_threshold(0.025);
    group.nresamples(200_000);

    let sizes = workload.generate_sizes(10000, BENCH_SEED);

    #[cfg(feature = "compare")]
    group.bench_function("system", |b| {
        b.iter(|| black_box(bench_single_thread_workload(&SYSTEM, &sizes)))
    });

    group.bench_function("mechanical_mimalloc", |b| {
        b.iter(|| black_box(bench_single_thread_workload(&MECH_MIMALLOC, &sizes)))
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Multi-threaded alloc/free loop (contention)
// ---------------------------------------------------------------------------

fn run_multi_thread_workload<A: GlobalAlloc + Sync + Send>(
    allocator: &'static A,
    workload: Workload,
    threads: usize,
) {
    let handles: Vec<_> = (0..threads)
        .map(|tid| {
            let wl = workload;
            std::thread::spawn(move || {
                let mut rng = StdRng::seed_from_u64(BENCH_SEED.wrapping_add(tid as u64));
                let mut allocations = Vec::new();
                for _ in 0..5000 {
                    let size = match wl {
                        Workload::RandomSmall => rng.gen_range(8..=1024),
                        Workload::FixedSize(s) => s,
                        Workload::Increasing => rng.gen_range(8..=1024),
                        Workload::HighFragmentation => {
                            if rng.gen_bool(0.5) {
                                64
                            } else {
                                32
                            }
                        }
                    };
                    let layout = Layout::from_size_align(size, 8).unwrap();
                    let ptr = unsafe { allocator.alloc(layout) };
                    if !ptr.is_null() {
                        allocations.push((SendPtr(ptr), layout));
                    }
                }
                for (ptr, layout) in allocations {
                    unsafe { allocator.dealloc(ptr.0, layout) };
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
}

fn run_multi_thread(c: &mut Criterion, workload: Workload) {
    let mut group = c.benchmark_group(format!("multi_thread/{}", workload.name()));
    group.plot_config(PlotConfiguration::default().summary_scale(AxisScale::Logarithmic));
    group.sample_size(80);
    group.measurement_time(Duration::from_secs(15));
    group.noise_threshold(0.025);
    group.nresamples(100_000);

    let num_cpus = logical_cpus();
    for num_threads in [2, 4, 8, 16].into_iter().filter(|&t| t <= num_cpus * 2) {
        #[cfg(feature = "compare")]
        group.bench_with_input(
            BenchmarkId::new("system", num_threads),
            &num_threads,
            |b, &threads| b.iter(|| run_multi_thread_workload(&SYSTEM, workload, threads)),
        );

        group.bench_with_input(
            BenchmarkId::new("mechanical_mimalloc", num_threads),
            &num_threads,
            |b, &threads| b.iter(|| run_multi_thread_workload(&MECH_MIMALLOC, workload, threads)),
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Fragmentation stress (random free order)
// ---------------------------------------------------------------------------

fn run_fragmentation<A: GlobalAlloc + Sync>(allocator: &A) -> usize {
    let object_count = 100_000;
    let mut rng = StdRng::seed_from_u64(BENCH_SEED);
    let mut allocations = Vec::with_capacity(object_count);
    for _ in 0..object_count {
        let size = rng.gen_range(24..=256);
        let layout = Layout::from_size_align(size, 8).unwrap();
        let ptr = unsafe { allocator.alloc(layout) };
        if !ptr.is_null() {
            allocations.push((ptr, layout));
        }
    }
    allocations.shuffle(&mut rng);
    for (ptr, layout) in allocations.drain(..) {
        unsafe { allocator.dealloc(ptr, layout) };
    }
    object_count
}

fn bench_fragmentation(c: &mut Criterion) {
    let mut group = c.benchmark_group("fragmentation");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(12));
    group.noise_threshold(0.025);
    group.nresamples(150_000);
    group.throughput(Throughput::Elements(100_000));

    #[cfg(feature = "compare")]
    group.bench_function("system/random_free_order", |b| {
        b.iter(|| black_box(run_fragmentation(&SYSTEM)))
    });

    group.bench_function("mechanical_mimalloc/random_free_order", |b| {
        b.iter(|| black_box(run_fragmentation(&MECH_MIMALLOC)))
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Lifecycle churn: repeated alloc/grow-via-realloc/free
// ---------------------------------------------------------------------------

fn run_realloc_churn<A: GlobalAlloc + Sync>(allocator: &A) -> usize {
    let rounds = 4000;
    let mut rng = StdRng::seed_from_u64(BENCH_SEED);
    let mut allocations: Vec<(*mut u8, Layout)> = Vec::with_capacity(rounds);
    for _ in 0..rounds {
        let size = rng.gen_range(16..=512);
        let layout = Layout::from_size_align(size, 8).unwrap();
        let ptr = unsafe { allocator.alloc(layout) };
        if !ptr.is_null() {
            allocations.push((ptr, layout));
        }
    }
    // Grow each allocation a few times.
    for (ptr, layout) in allocations.iter_mut() {
        for _ in 0..3 {
            let new_size = layout.size() + rng.gen_range(16..=128);
            let np = unsafe { allocator.realloc(*ptr, *layout, new_size) };
            if !np.is_null() {
                *ptr = np;
                *layout = Layout::from_size_align(new_size, layout.align()).unwrap();
            }
        }
    }
    for (ptr, layout) in allocations.drain(..) {
        unsafe { allocator.dealloc(ptr, layout) };
    }
    rounds
}

fn bench_realloc_churn(c: &mut Criterion) {
    let mut group = c.benchmark_group("realloc_churn");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));
    group.noise_threshold(0.025);
    group.nresamples(150_000);

    #[cfg(feature = "compare")]
    group.bench_function("system", |b| {
        b.iter(|| black_box(run_realloc_churn(&SYSTEM)))
    });

    group.bench_function("mechanical_mimalloc", |b| {
        b.iter(|| black_box(run_realloc_churn(&MECH_MIMALLOC)))
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

fn bench_single_thread_random_small(c: &mut Criterion) {
    run_single_thread(c, Workload::RandomSmall);
}
fn bench_single_thread_fixed_64(c: &mut Criterion) {
    run_single_thread(c, Workload::FixedSize(64));
}
fn bench_single_thread_fixed_256(c: &mut Criterion) {
    run_single_thread(c, Workload::FixedSize(256));
}
fn bench_single_thread_increasing(c: &mut Criterion) {
    run_single_thread(c, Workload::Increasing);
}
fn bench_single_thread_high_frag(c: &mut Criterion) {
    run_single_thread(c, Workload::HighFragmentation);
}
fn bench_multi_thread_random_small(c: &mut Criterion) {
    run_multi_thread(c, Workload::RandomSmall);
}
fn bench_multi_thread_high_frag(c: &mut Criterion) {
    run_multi_thread(c, Workload::HighFragmentation);
}

criterion_group!(
    single_thread_benches,
    bench_single_thread_random_small,
    bench_single_thread_fixed_64,
    bench_single_thread_fixed_256,
    bench_single_thread_increasing,
    bench_single_thread_high_frag,
);
criterion_group!(
    multi_thread_benches,
    bench_multi_thread_random_small,
    bench_multi_thread_high_frag,
);
criterion_group!(fragmentation_benches, bench_fragmentation);
criterion_group!(realloc_benches, bench_realloc_churn);

criterion_main!(
    single_thread_benches,
    multi_thread_benches,
    fragmentation_benches,
    realloc_benches
);
