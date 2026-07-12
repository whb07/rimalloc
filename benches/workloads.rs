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
use std::sync::mpsc;
use std::time::Duration;

/// Deterministic seed for reproducible benchmarks.
const BENCH_SEED: u64 = 0xDEADBEEF_CAFEBABE;

/// `GlobalAlloc` shim over mechanical_mimalloc's C-ABI entry points.
struct MechanicalMiMalloc;

unsafe impl GlobalAlloc for MechanicalMiMalloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.align() <= 16 {
            mechanical_mimalloc::raw::mi_malloc(layout.size()).cast()
        } else {
            mechanical_mimalloc::raw::mi_malloc_aligned(layout.size(), layout.align()).cast()
        }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if layout.align() <= 16 {
            mechanical_mimalloc::raw::mi_calloc(1, layout.size()).cast()
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
        mechanical_mimalloc::raw::mi_free(ptr.cast());
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if layout.align() <= 16 {
            mechanical_mimalloc::raw::mi_realloc(ptr.cast(), new_size).cast()
        } else {
            mechanical_mimalloc::raw::mi_realloc_aligned(ptr.cast(), new_size, layout.align())
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

fn bench_single_thread_workload<A: GlobalAlloc + Sync>(
    allocator: &A,
    sizes: &[usize],
    allocations: &mut Vec<(SendPtr, Layout)>,
) -> usize {
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
        let mut allocations = Vec::with_capacity(sizes.len());
        b.iter(|| {
            black_box(bench_single_thread_workload(
                &SYSTEM,
                &sizes,
                &mut allocations,
            ))
        })
    });

    let mut allocations = Vec::with_capacity(sizes.len());
    group.bench_function("mechanical_mimalloc", |b| {
        b.iter(|| {
            black_box(bench_single_thread_workload(
                &MECH_MIMALLOC,
                &sizes,
                &mut allocations,
            ))
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Multi-threaded alloc/free loop (contention)
// ---------------------------------------------------------------------------

enum WorkerCommand {
    Run,
    Stop,
}

struct MultiThreadHarness {
    commands: Vec<mpsc::SyncSender<WorkerCommand>>,
    done: mpsc::Receiver<()>,
    workers: Vec<std::thread::JoinHandle<()>>,
}

impl MultiThreadHarness {
    fn new<A: GlobalAlloc + Sync + Send + 'static>(
        allocator: &'static A,
        workload: Workload,
        threads: usize,
    ) -> Self {
        let (done_tx, done_rx) = mpsc::sync_channel(threads);
        let mut commands = Vec::with_capacity(threads);
        let workers = (0..threads)
            .map(|tid| {
                let (command_tx, command_rx) = mpsc::sync_channel(1);
                commands.push(command_tx);
                let done_tx = done_tx.clone();
                std::thread::spawn(move || {
                    let mut rng = StdRng::seed_from_u64(BENCH_SEED.wrapping_add(tid as u64));
                    let sizes: Vec<_> = (0..5000)
                        .map(|_| match workload {
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
                        })
                        .collect();
                    let mut allocations = Vec::with_capacity(sizes.len());
                    while let Ok(command) = command_rx.recv() {
                        if matches!(command, WorkerCommand::Stop) {
                            break;
                        }
                        for &size in &sizes {
                            let layout = Layout::from_size_align(size, 8).unwrap();
                            let ptr = unsafe { allocator.alloc(layout) };
                            if !ptr.is_null() {
                                allocations.push((SendPtr(ptr), layout));
                            }
                        }
                        for (ptr, layout) in allocations.drain(..) {
                            unsafe { allocator.dealloc(ptr.0, layout) };
                        }
                        done_tx.send(()).unwrap();
                    }
                })
            })
            .collect();
        Self {
            commands,
            done: done_rx,
            workers,
        }
    }

    fn run(&self) {
        for command in &self.commands {
            command.send(WorkerCommand::Run).unwrap();
        }
        for _ in &self.commands {
            self.done.recv().unwrap();
        }
    }
}

impl Drop for MultiThreadHarness {
    fn drop(&mut self) {
        for command in &self.commands {
            let _ = command.send(WorkerCommand::Stop);
        }
        for worker in self.workers.drain(..) {
            worker.join().unwrap();
        }
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
        {
            let harness = MultiThreadHarness::new(&SYSTEM, workload, num_threads);
            group.bench_with_input(
                BenchmarkId::new("system", num_threads),
                &num_threads,
                |b, _| b.iter(|| harness.run()),
            );
        }

        let harness = MultiThreadHarness::new(&MECH_MIMALLOC, workload, num_threads);
        group.bench_with_input(
            BenchmarkId::new("mechanical_mimalloc", num_threads),
            &num_threads,
            |b, _| b.iter(|| harness.run()),
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Fragmentation stress (random free order)
// ---------------------------------------------------------------------------

fn run_fragmentation<A: GlobalAlloc + Sync>(
    allocator: &A,
    layouts: &[Layout],
    free_order: &[usize],
    allocations: &mut [*mut u8],
) -> usize {
    for (slot, &layout) in allocations.iter_mut().zip(layouts) {
        let ptr = unsafe { allocator.alloc(layout) };
        *slot = ptr;
    }
    for &index in free_order {
        let ptr = allocations[index];
        if !ptr.is_null() {
            unsafe { allocator.dealloc(ptr, layouts[index]) };
        }
    }
    layouts.len()
}

fn bench_fragmentation(c: &mut Criterion) {
    let mut group = c.benchmark_group("fragmentation");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(12));
    group.noise_threshold(0.025);
    group.nresamples(150_000);
    group.throughput(Throughput::Elements(100_000));
    let mut rng = StdRng::seed_from_u64(BENCH_SEED);
    let layouts: Vec<_> = (0..100_000)
        .map(|_| Layout::from_size_align(rng.gen_range(24..=256), 8).unwrap())
        .collect();
    let mut free_order: Vec<_> = (0..layouts.len()).collect();
    free_order.shuffle(&mut rng);

    #[cfg(feature = "compare")]
    group.bench_function("system/random_free_order", |b| {
        let mut allocations = vec![std::ptr::null_mut(); layouts.len()];
        b.iter(|| {
            black_box(run_fragmentation(
                &SYSTEM,
                &layouts,
                &free_order,
                &mut allocations,
            ))
        })
    });

    let mut allocations = vec![std::ptr::null_mut(); layouts.len()];
    group.bench_function("mechanical_mimalloc/random_free_order", |b| {
        b.iter(|| {
            black_box(run_fragmentation(
                &MECH_MIMALLOC,
                &layouts,
                &free_order,
                &mut allocations,
            ))
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Lifecycle churn: repeated alloc/grow-via-realloc/free
// ---------------------------------------------------------------------------

fn run_realloc_churn<A: GlobalAlloc + Sync>(
    allocator: &A,
    initial_layouts: &[Layout],
    growth: &[[usize; 3]],
    allocations: &mut Vec<(*mut u8, Layout)>,
) -> usize {
    for &layout in initial_layouts {
        let ptr = unsafe { allocator.alloc(layout) };
        if !ptr.is_null() {
            allocations.push((ptr, layout));
        }
    }
    // Grow each allocation a few times.
    for ((ptr, layout), increments) in allocations.iter_mut().zip(growth) {
        for increment in increments {
            let new_size = layout.size() + increment;
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
    initial_layouts.len()
}

fn bench_realloc_churn(c: &mut Criterion) {
    let mut group = c.benchmark_group("realloc_churn");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));
    group.noise_threshold(0.025);
    group.nresamples(150_000);
    let mut rng = StdRng::seed_from_u64(BENCH_SEED);
    let initial_layouts: Vec<_> = (0..4000)
        .map(|_| Layout::from_size_align(rng.gen_range(16..=512), 8).unwrap())
        .collect();
    let growth: Vec<_> = (0..4000)
        .map(|_| {
            [
                rng.gen_range(16..=128),
                rng.gen_range(16..=128),
                rng.gen_range(16..=128),
            ]
        })
        .collect();

    #[cfg(feature = "compare")]
    group.bench_function("system", |b| {
        let mut allocations = Vec::with_capacity(initial_layouts.len());
        b.iter(|| {
            black_box(run_realloc_churn(
                &SYSTEM,
                &initial_layouts,
                &growth,
                &mut allocations,
            ))
        })
    });

    let mut allocations = Vec::with_capacity(initial_layouts.len());
    group.bench_function("mechanical_mimalloc", |b| {
        b.iter(|| {
            black_box(run_realloc_churn(
                &MECH_MIMALLOC,
                &initial_layouts,
                &growth,
                &mut allocations,
            ))
        })
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
