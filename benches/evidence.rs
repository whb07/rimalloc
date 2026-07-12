//! Evidence benchmarks with an explicit split between the public Rust facade
//! and the raw C ABI. All layouts and pointer-slot scratch storage are created
//! before Criterion starts timing an iteration.

use core::alloc::Layout;
use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use mechanical_mimalloc::Allocation;

const BATCH: usize = 2048;
const SIZES: &[usize] = &[16, 64, 256, 1024, 4096];

fn public_facade(c: &mut Criterion) {
    let mut group = c.benchmark_group("evidence/public_facade");
    group.throughput(Throughput::Elements((BATCH * 2) as u64));
    for &size in SIZES {
        let layout = Layout::from_size_align(size, 8).expect("valid benchmark layout");
        let mut slots: Vec<Option<Allocation>> = (0..BATCH).map(|_| None).collect();
        group.bench_with_input(BenchmarkId::from_parameter(size), &layout, |b, &layout| {
            b.iter(|| {
                for slot in &mut slots {
                    *slot = Some(Allocation::new(layout).expect("benchmark allocation"));
                }
                for slot in &mut slots {
                    black_box(slot.take());
                }
            });
        });
    }
    group.finish();
}

fn raw_c_abi(c: &mut Criterion) {
    let mut group = c.benchmark_group("evidence/raw_c_abi");
    group.throughput(Throughput::Elements((BATCH * 2) as u64));
    for &size in SIZES {
        let mut slots = vec![core::ptr::null_mut::<core::ffi::c_void>(); BATCH];
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                for slot in &mut slots {
                    // SAFETY: every successful pointer is retained in `slots`
                    // and freed exactly once below.
                    *slot = unsafe { mechanical_mimalloc::raw::mi_malloc(size) };
                }
                for slot in &mut slots {
                    // SAFETY: the pointer came from `mi_malloc` in this
                    // iteration and has not otherwise been freed.
                    unsafe { mechanical_mimalloc::raw::mi_free(*slot) };
                    *slot = core::ptr::null_mut();
                }
                black_box(&slots);
            });
        });
    }
    group.finish();
}

criterion_group!(evidence_benches, public_facade, raw_c_abi);
criterion_main!(evidence_benches);
