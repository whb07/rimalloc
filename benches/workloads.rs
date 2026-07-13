// The GlobalAlloc shim calls unsafe extern "C" fns; bodies are unsafe by intent.
#![allow(unsafe_op_in_unsafe_fn)]

//! Allocator workload benchmarks for mechanical_mimalloc.
//!
//! Mirrors the workload shape from yet-another-mimalloc-rs and measures the
//! crate's public `MiMalloc` `GlobalAlloc` facade.
//!
//! Run with: `cargo bench --features bench`
//! Compare against system: add `--features bench,compare`

mod support;

use criterion::{
    AxisScale, BenchmarkId, Criterion, PlotConfiguration, Throughput, black_box, criterion_group,
    criterion_main,
};
use rand::SeedableRng;
use rand::prelude::*;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use std::alloc::{GlobalAlloc, Layout};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use support::allocators::PUBLIC_MIMALLOC;
#[cfg(feature = "compare")]
use support::allocators::SYSTEM_ALLOCATOR;
use support::pool::PointerPool;

/// Deterministic seed for reproducible benchmarks.
const BENCH_SEED: u64 = 0xDEADBEEF_CAFEBABE;

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
                &SYSTEM_ALLOCATOR,
                &sizes,
                &mut allocations,
            ))
        })
    });

    let mut allocations = Vec::with_capacity(sizes.len());
    group.bench_function("mechanical_mimalloc_public", |b| {
        b.iter(|| {
            black_box(bench_single_thread_workload(
                &PUBLIC_MIMALLOC,
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
            let harness = MultiThreadHarness::new(&SYSTEM_ALLOCATOR, workload, num_threads);
            group.bench_with_input(
                BenchmarkId::new("system", num_threads),
                &num_threads,
                |b, _| b.iter(|| harness.run()),
            );
        }

        let harness = MultiThreadHarness::new(&PUBLIC_MIMALLOC, workload, num_threads);
        group.bench_with_input(
            BenchmarkId::new("mechanical_mimalloc_public", num_threads),
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
                &SYSTEM_ALLOCATOR,
                &layouts,
                &free_order,
                &mut allocations,
            ))
        })
    });

    let mut allocations = vec![std::ptr::null_mut(); layouts.len()];
    group.bench_function("mechanical_mimalloc_public/random_free_order", |b| {
        b.iter(|| {
            black_box(run_fragmentation(
                &PUBLIC_MIMALLOC,
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
                &SYSTEM_ALLOCATOR,
                &initial_layouts,
                &growth,
                &mut allocations,
            ))
        })
    });

    let mut allocations = Vec::with_capacity(initial_layouts.len());
    group.bench_function("mechanical_mimalloc_public", |b| {
        b.iter(|| {
            black_box(run_realloc_churn(
                &PUBLIC_MIMALLOC,
                &initial_layouts,
                &growth,
                &mut allocations,
            ))
        })
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Ownership-transfer topologies. Steady-state cases retain all worker threads
// across samples; lifecycle churn is intentionally reported in a separate
// group because thread creation and teardown are part of that workload.
// ---------------------------------------------------------------------------

const TOPOLOGY_BATCH: usize = 1024;
const TOPOLOGY_SIZE: usize = 64;

fn fill_pool<A: GlobalAlloc>(allocator: &A, layout: Layout, pool: &mut PointerPool) {
    for index in 0..pool.len() {
        let pointer = unsafe { allocator.alloc(layout) };
        assert!(!pointer.is_null(), "topology benchmark allocation failed");
        *pool.slot(index) = (pointer, layout);
    }
}

fn free_pool<A: GlobalAlloc>(allocator: &A, pool: &mut PointerPool) {
    for index in 0..pool.len() {
        let (pointer, layout) = *pool.slot(index);
        unsafe { allocator.dealloc(pointer, layout) };
        *pool.slot(index) = (std::ptr::null_mut(), layout);
    }
}

enum PoolCommand {
    Process(PointerPool),
    Stop,
}

struct FanInHarness<A: GlobalAlloc + Sync + 'static> {
    allocator: &'static A,
    commands: Vec<mpsc::SyncSender<PoolCommand>>,
    replies: Vec<mpsc::Receiver<PointerPool>>,
    pools: Vec<Option<PointerPool>>,
    workers: Vec<thread::JoinHandle<()>>,
}

impl<A: GlobalAlloc + Sync + 'static> FanInHarness<A> {
    fn new(allocator: &'static A, owners: usize, layout: Layout) -> Self {
        let mut commands = Vec::with_capacity(owners);
        let mut replies = Vec::with_capacity(owners);
        let mut workers = Vec::with_capacity(owners);
        for _ in 0..owners {
            let (command_tx, command_rx) = mpsc::sync_channel(1);
            let (reply_tx, reply_rx) = mpsc::sync_channel(1);
            commands.push(command_tx);
            replies.push(reply_rx);
            workers.push(thread::spawn(move || {
                while let Ok(PoolCommand::Process(mut pool)) = command_rx.recv() {
                    fill_pool(allocator, layout, &mut pool);
                    if reply_tx.send(pool).is_err() {
                        break;
                    }
                }
            }));
        }
        Self {
            allocator,
            commands,
            replies,
            pools: (0..owners)
                .map(|_| Some(PointerPool::new(TOPOLOGY_BATCH)))
                .collect(),
            workers,
        }
    }

    fn run(&mut self) {
        for (command, pool) in self.commands.iter().zip(&mut self.pools) {
            command
                .send(PoolCommand::Process(pool.take().expect("fan-in pool")))
                .expect("prepare fan-in pool");
        }
        for (reply, slot) in self.replies.iter().zip(&mut self.pools) {
            let mut pool = reply.recv().expect("receive fan-in pool");
            free_pool(self.allocator, &mut pool);
            *slot = Some(pool);
        }
    }
}

impl<A: GlobalAlloc + Sync + 'static> Drop for FanInHarness<A> {
    fn drop(&mut self) {
        for command in &self.commands {
            let _ = command.send(PoolCommand::Stop);
        }
        for worker in self.workers.drain(..) {
            worker.join().expect("fan-in owner thread");
        }
        assert!(
            self.pools
                .iter()
                .all(|pool| pool.as_ref().is_some_and(PointerPool::all_empty))
        );
    }
}

struct FanOutHarness<A: GlobalAlloc + Sync + 'static> {
    allocator: &'static A,
    layout: Layout,
    commands: Vec<mpsc::SyncSender<PoolCommand>>,
    replies: Vec<mpsc::Receiver<PointerPool>>,
    pools: Vec<Option<PointerPool>>,
    workers: Vec<thread::JoinHandle<()>>,
}

impl<A: GlobalAlloc + Sync + 'static> FanOutHarness<A> {
    fn new(allocator: &'static A, free_workers: usize, layout: Layout) -> Self {
        let mut commands = Vec::with_capacity(free_workers);
        let mut replies = Vec::with_capacity(free_workers);
        let mut workers = Vec::with_capacity(free_workers);
        for _ in 0..free_workers {
            let (command_tx, command_rx) = mpsc::sync_channel(1);
            let (reply_tx, reply_rx) = mpsc::sync_channel(1);
            commands.push(command_tx);
            replies.push(reply_rx);
            workers.push(thread::spawn(move || {
                while let Ok(PoolCommand::Process(mut pool)) = command_rx.recv() {
                    free_pool(allocator, &mut pool);
                    if reply_tx.send(pool).is_err() {
                        break;
                    }
                }
            }));
        }
        Self {
            allocator,
            layout,
            commands,
            replies,
            pools: (0..free_workers)
                .map(|_| Some(PointerPool::new(TOPOLOGY_BATCH)))
                .collect(),
            workers,
        }
    }

    fn run(&mut self) {
        for (command, pool) in self.commands.iter().zip(&mut self.pools) {
            let mut pool = pool.take().expect("fan-out pool");
            fill_pool(self.allocator, self.layout, &mut pool);
            command
                .send(PoolCommand::Process(pool))
                .expect("send fan-out pool");
        }
        for (reply, slot) in self.replies.iter().zip(&mut self.pools) {
            *slot = Some(reply.recv().expect("receive empty fan-out pool"));
        }
    }
}

impl<A: GlobalAlloc + Sync + 'static> Drop for FanOutHarness<A> {
    fn drop(&mut self) {
        for command in &self.commands {
            let _ = command.send(PoolCommand::Stop);
        }
        for worker in self.workers.drain(..) {
            worker.join().expect("fan-out free thread");
        }
        assert!(
            self.pools
                .iter()
                .all(|pool| pool.as_ref().is_some_and(PointerPool::all_empty))
        );
    }
}

struct CycleHarness<A: GlobalAlloc + Sync + 'static> {
    allocator: &'static A,
    layout: Layout,
    entry: Option<mpsc::SyncSender<PointerPool>>,
    exit: mpsc::Receiver<PointerPool>,
    pool: Option<PointerPool>,
    workers: Vec<thread::JoinHandle<()>>,
}

impl<A: GlobalAlloc + Sync + 'static> CycleHarness<A> {
    fn new(allocator: &'static A, layout: Layout) -> Self {
        let (entry, first_rx) = mpsc::sync_channel(1);
        let (first_tx, second_rx) = mpsc::sync_channel(1);
        let (second_tx, third_rx) = mpsc::sync_channel(1);
        let (third_tx, exit) = mpsc::sync_channel(1);
        let first = thread::spawn(move || {
            while let Ok(pool) = first_rx.recv() {
                if first_tx.send(pool).is_err() {
                    break;
                }
            }
        });
        let second = thread::spawn(move || {
            while let Ok(pool) = second_rx.recv() {
                if second_tx.send(pool).is_err() {
                    break;
                }
            }
        });
        let third = thread::spawn(move || {
            while let Ok(mut pool) = third_rx.recv() {
                free_pool(allocator, &mut pool);
                if third_tx.send(pool).is_err() {
                    break;
                }
            }
        });
        Self {
            allocator,
            layout,
            entry: Some(entry),
            exit,
            pool: Some(PointerPool::new(TOPOLOGY_BATCH)),
            workers: vec![first, second, third],
        }
    }

    fn run(&mut self) {
        let mut pool = self.pool.take().expect("cycle pool");
        fill_pool(self.allocator, self.layout, &mut pool);
        self.entry
            .as_ref()
            .expect("cycle entry")
            .send(pool)
            .expect("begin ownership cycle");
        self.pool = Some(self.exit.recv().expect("complete ownership cycle"));
    }
}

impl<A: GlobalAlloc + Sync + 'static> Drop for CycleHarness<A> {
    fn drop(&mut self) {
        drop(self.entry.take());
        for worker in self.workers.drain(..) {
            worker.join().expect("cycle transfer thread");
        }
        assert!(self.pool.as_ref().is_some_and(PointerPool::all_empty));
    }
}

fn bench_remote_topologies(c: &mut Criterion) {
    let layout = Layout::from_size_align(TOPOLOGY_SIZE, 8).unwrap();
    let cpu_limit = logical_cpus();
    let mut steady = c.benchmark_group("ownership_transfer/steady_state");
    steady.throughput(Throughput::Elements(TOPOLOGY_BATCH as u64));
    for workers in [2, 4, 8].into_iter().filter(|count| *count <= cpu_limit) {
        steady.throughput(Throughput::Elements((TOPOLOGY_BATCH * workers) as u64));
        let mut fan_in = FanInHarness::new(&PUBLIC_MIMALLOC, workers, layout);
        steady.bench_function(
            format!("fan_in/mechanical_mimalloc_public/{workers}"),
            |b| b.iter(|| fan_in.run()),
        );
        let mut fan_out = FanOutHarness::new(&PUBLIC_MIMALLOC, workers, layout);
        steady.bench_function(
            format!("fan_out/mechanical_mimalloc_public/{workers}"),
            |b| b.iter(|| fan_out.run()),
        );
    }
    steady.throughput(Throughput::Elements(TOPOLOGY_BATCH as u64));
    let mut cycle = CycleHarness::new(&PUBLIC_MIMALLOC, layout);
    steady.bench_function("cyclic/mechanical_mimalloc_public/3", |b| {
        b.iter(|| cycle.run())
    });
    let oversubscribed_workers = logical_cpus().saturating_mul(2).max(2);
    steady.throughput(Throughput::Elements(
        (TOPOLOGY_BATCH * oversubscribed_workers) as u64,
    ));
    let mut oversubscribed = FanOutHarness::new(&PUBLIC_MIMALLOC, oversubscribed_workers, layout);
    steady.bench_function(
        format!("oversubscribed/mechanical_mimalloc_public/{oversubscribed_workers}"),
        |b| b.iter(|| oversubscribed.run()),
    );
    steady.finish();
}

fn owner_exit_once(layout: Layout) {
    let owner = thread::spawn(move || {
        let mut pool = PointerPool::new(TOPOLOGY_BATCH);
        fill_pool(&PUBLIC_MIMALLOC, layout, &mut pool);
        pool
    });
    let mut pool = owner.join().expect("owner exit thread");
    free_pool(&PUBLIC_MIMALLOC, &mut pool);
}

fn thread_churn_once(layout: Layout) {
    for _ in 0..8 {
        owner_exit_once(layout);
    }
}

fn bench_remote_lifecycle(c: &mut Criterion) {
    let layout = Layout::from_size_align(TOPOLOGY_SIZE, 8).unwrap();
    let mut lifecycle = c.benchmark_group("ownership_transfer/lifecycle_churn");
    lifecycle.throughput(Throughput::Elements(TOPOLOGY_BATCH as u64));
    lifecycle.bench_function(
        "owner_exit_after_publication/mechanical_mimalloc_public",
        |b| b.iter(|| owner_exit_once(layout)),
    );
    lifecycle.throughput(Throughput::Elements((TOPOLOGY_BATCH * 8) as u64));
    lifecycle.bench_function("thread_churn/mechanical_mimalloc_public", |b| {
        b.iter(|| thread_churn_once(layout))
    });
    lifecycle.finish();
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
criterion_group!(
    topology_benches,
    bench_remote_topologies,
    bench_remote_lifecycle
);

criterion_main!(
    single_thread_benches,
    multi_thread_benches,
    fragmentation_benches,
    realloc_benches,
    topology_benches
);
