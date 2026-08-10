//! Targeted Stage 3 slow-path benchmarks. Opaque fixtures are prepared outside
//! each timed region and exercise the production forced, local, and remote
//! collection transitions through benchmark-only C symbols.

use core::ffi::c_void;
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use mechanical_mimalloc::raw::{
    mi_bench_page_batch_fixture_destroy, mi_bench_page_batch_fixture_new,
    mi_bench_page_batch_fixture_prepare, mi_bench_page_batch_fixture_run,
};
use std::env;
use std::time::{Duration, Instant};

const FORCE: u8 = 0;
const LOCAL: u8 = 1;
const REMOTE: u8 = 2;
const LENGTHS: &[usize] = &[1, 8, 32, 128];

fn configured_sample_size() -> usize {
    env::var("MIMALLOC_HOTPATH_SAMPLE_SIZE")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(100)
        .max(10)
}

fn configured_duration(name: &str, default: u64) -> Duration {
    Duration::from_secs(
        env::var(name)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
            .max(1),
    )
}

fn configure_group(group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>) {
    group.sample_size(configured_sample_size());
    group.measurement_time(configured_duration("MIMALLOC_HOTPATH_MEASUREMENT_SECS", 10));
    group.warm_up_time(configured_duration("MIMALLOC_HOTPATH_WARMUP_SECS", 1));
}

struct Fixture(*mut c_void);

impl Fixture {
    fn new(kind: u8, len: usize, destination_len: usize) -> Self {
        // SAFETY: the benchmark-only constructor validates its scalar inputs
        // and returns an opaque uniquely owned handle.
        let fixture = mi_bench_page_batch_fixture_new(kind, len, destination_len);
        assert!(!fixture.is_null(), "page-batch fixture allocation failed");
        Self(fixture)
    }

    fn prepare(&mut self) {
        // SAFETY: `self` owns the live handle and serializes all operations.
        assert!(unsafe { mi_bench_page_batch_fixture_prepare(self.0) });
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // SAFETY: this is the final use of the uniquely owned live handle.
        unsafe { mi_bench_page_batch_fixture_destroy(self.0) };
    }
}

fn measure_prepared_transition(fixture: &mut Fixture, iterations: u64) -> Duration {
    let mut elapsed = Duration::ZERO;
    for _ in 0..iterations {
        fixture.prepare();
        let start = Instant::now();
        // SAFETY: prepare restored the fixture and `&mut` serializes the timed
        // transition with all setup and teardown outside this interval.
        let completed = unsafe { mi_bench_page_batch_fixture_run(fixture.0) };
        elapsed += start.elapsed();
        assert!(completed, "prepared page-batch transition failed");
    }
    elapsed
}

fn benchmark_page_batches(c: &mut Criterion) {
    let encoding = if cfg!(feature = "secure") {
        "secure_encoded"
    } else {
        "plain_links"
    };

    let mut forced = c.benchmark_group(format!("page_batch/{encoding}/forced_page_free_collect"));
    configure_group(&mut forced);
    forced.throughput(Throughput::Elements(128));
    for (destination, destination_len) in [("empty", 0), ("nonempty", 3)] {
        let mut fixture = Fixture::new(FORCE, 128, destination_len);
        forced.bench_function(format!("len128/{destination}_destination"), |b| {
            b.iter_custom(|iterations| measure_prepared_transition(&mut fixture, iterations));
        });
    }
    forced.finish();

    for (name, kind) in [("local_to_available", LOCAL), ("detached_remote", REMOTE)] {
        let mut group = c.benchmark_group(format!("page_batch/{encoding}/{name}"));
        configure_group(&mut group);
        for &len in LENGTHS {
            group.throughput(Throughput::Elements(len as u64));
            for (destination, destination_len) in [("empty", 0), ("nonempty", 3)] {
                let mut fixture = Fixture::new(kind, len, destination_len);
                group.bench_with_input(BenchmarkId::new(destination, len), &len, |b, _| {
                    b.iter_custom(|iterations| {
                        measure_prepared_transition(&mut fixture, iterations)
                    });
                });
            }
        }
        group.finish();
    }
}

criterion_group!(page_batch_benches, benchmark_page_batches);
criterion_main!(page_batch_benches);
