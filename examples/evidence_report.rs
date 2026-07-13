//! Emits one machine-readable evidence sample. Run with
//! `cargo run --release --features "std bench-diagnostics" --example evidence_report`.

#[cfg(feature = "bench-diagnostics")]
use core::ffi::c_char;
use mechanical_mimalloc::{Allocation, StatsSnapshot};
use std::sync::{Arc, Barrier, mpsc};
use std::thread;
use std::time::Instant;

#[cfg(feature = "bench-diagnostics")]
unsafe extern "C" {
    fn mi_bench_diagnostics_reset();
    fn mi_bench_diagnostics_json(buf: *mut c_char, len: usize) -> usize;
    fn mi_bench_page_occupancy(used_bytes: *mut usize, capacity_bytes: *mut usize) -> bool;
}

fn percentile(samples: &mut [u64], numerator: usize, denominator: usize) -> u64 {
    samples.sort_unstable();
    let index = (samples.len() * numerator)
        .div_ceil(denominator)
        .saturating_sub(1);
    samples[index.min(samples.len() - 1)]
}

fn usage() -> libc::rusage {
    let mut usage = core::mem::MaybeUninit::<libc::rusage>::zeroed();
    assert_eq!(
        unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) },
        0
    );
    unsafe { usage.assume_init() }
}

fn main() {
    let usage_before = usage();
    #[cfg(feature = "bench-diagnostics")]
    unsafe {
        mi_bench_diagnostics_reset();
    }
    let mut samples = Vec::with_capacity(20_000);
    for index in 0..20_000 {
        let size = 8 + (index * 131 % 8192);
        let started = Instant::now();
        let allocation = Allocation::new_bytes(size).expect("evidence allocation");
        drop(allocation);
        samples.push(started.elapsed().as_nanos() as u64);
    }
    let (request_tx, request_rx) = mpsc::sync_channel(1);
    let (allocation_tx, allocation_rx) = mpsc::sync_channel(1);
    let owner = thread::spawn(move || {
        while let Ok(size) = request_rx.recv() {
            if allocation_tx
                .send(Allocation::new_bytes(size).unwrap())
                .is_err()
            {
                break;
            }
        }
    });
    let mut remote_samples = Vec::with_capacity(20_000);
    for index in 0..20_000 {
        request_tx.send(8 + index * 131 % 8192).unwrap();
        let allocation = allocation_rx.recv().unwrap();
        let started = Instant::now();
        drop(allocation);
        remote_samples.push(started.elapsed().as_nanos() as u64);
    }
    drop(request_tx);
    owner.join().unwrap();
    let worker_count = thread::available_parallelism().map_or(4, |count| count.get().min(8));
    let barrier = Arc::new(Barrier::new(worker_count + 1));
    let (latency_tx, latency_rx) = mpsc::sync_channel(worker_count);
    let mut concurrent_senders = Vec::with_capacity(worker_count);
    let concurrent_workers: Vec<_> = (0..worker_count)
        .map(|_| {
            let (tx, rx) = mpsc::sync_channel(1);
            concurrent_senders.push(tx);
            let barrier = Arc::clone(&barrier);
            let latency_tx = latency_tx.clone();
            thread::spawn(move || {
                let allocations: Vec<Allocation> = rx.recv().unwrap();
                barrier.wait();
                let latencies: Vec<_> = allocations
                    .into_iter()
                    .map(|allocation| {
                        let started = Instant::now();
                        drop(allocation);
                        started.elapsed().as_nanos() as u64
                    })
                    .collect();
                latency_tx.send(latencies).unwrap();
            })
        })
        .collect();
    for (worker, sender) in concurrent_senders.iter().enumerate() {
        let allocations = (0..4096)
            .map(|index| Allocation::new_bytes(64 + (index + worker) % 64).unwrap())
            .collect();
        sender.send(allocations).unwrap();
    }
    barrier.wait();
    let mut concurrent_remote_samples = Vec::with_capacity(worker_count * 4096);
    for _ in 0..worker_count {
        concurrent_remote_samples.extend(latency_rx.recv().unwrap());
    }
    for worker in concurrent_workers {
        worker.join().unwrap();
    }
    // SAFETY: force collection has no pointer arguments and runs after all
    // remote-free workers have joined.
    unsafe { mechanical_mimalloc::raw::mi_collect(true) };
    let live: Vec<_> = (0..4096)
        .map(|index| Allocation::new_bytes(8 + index % 8192).expect("live evidence allocation"))
        .collect();
    let live_requested_bytes: usize = live.iter().map(Allocation::len).sum();
    let stats = StatsSnapshot::capture().expect("statistics snapshot");
    let usage_after = usage();
    #[cfg(feature = "bench-diagnostics")]
    let diagnostics = {
        let mut buffer = vec![0_u8; 8192];
        unsafe { mi_bench_diagnostics_json(buffer.as_mut_ptr().cast(), buffer.len()) };
        std::ffi::CStr::from_bytes_until_nul(&buffer)
            .expect("diagnostic JSON terminator")
            .to_str()
            .expect("diagnostic JSON UTF-8")
            .to_owned()
    };
    #[cfg(not(feature = "bench-diagnostics"))]
    let diagnostics = "{}";
    let mut occupied_bytes = 0usize;
    let mut capacity_bytes = 0usize;
    // SAFETY: both outputs are valid local `usize` storage.
    let has_occupancy =
        unsafe { mi_bench_page_occupancy(&mut occupied_bytes, &mut capacity_bytes) };
    let occupancy_ratio = if has_occupancy {
        (occupied_bytes as f64 / capacity_bytes as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let search_count = stats.page_search_count().get();
    let search_length = if search_count == 0 {
        0.0
    } else {
        stats.page_searches_total().get() as f64 / search_count as f64
    };
    let p99 = percentile(&mut samples, 99, 100);
    let p999 = percentile(&mut samples, 999, 1000);
    let remote_p99 = percentile(&mut remote_samples, 99, 100);
    let remote_p999 = percentile(&mut remote_samples, 999, 1000);
    let concurrent_remote_p99 = percentile(&mut concurrent_remote_samples, 99, 100);
    let concurrent_remote_p999 = percentile(&mut concurrent_remote_samples, 999, 1000);
    println!(
        "{{\"schema\":1,\"artifact_mode\":\"default-rust\",\"peak_rss_kib\":{},\"baseline_peak_rss_kib\":{},\"peak_rss_delta_kib\":{},\"committed_bytes\":{},\"live_requested_bytes\":{},\"page_occupancy\":{},\"page_occupied_bytes\":{},\"page_capacity_bytes\":{},\"page_search_length\":{},\"minor_page_faults\":{},\"major_page_faults\":{},\"sampled_alloc_free_p99_ns\":{},\"sampled_alloc_free_p999_ns\":{},\"sampled_remote_free_p99_ns\":{},\"sampled_remote_free_p999_ns\":{},\"sampled_concurrent_remote_free_p99_ns\":{},\"sampled_concurrent_remote_free_p999_ns\":{},\"allocator\":{}}}",
        usage_after.ru_maxrss,
        usage_before.ru_maxrss,
        usage_after.ru_maxrss.saturating_sub(usage_before.ru_maxrss),
        stats.committed_current().get(),
        live_requested_bytes,
        occupancy_ratio,
        occupied_bytes,
        capacity_bytes,
        search_length,
        usage_after.ru_minflt.saturating_sub(usage_before.ru_minflt),
        usage_after.ru_majflt.saturating_sub(usage_before.ru_majflt),
        p99,
        p999,
        remote_p99,
        remote_p999,
        concurrent_remote_p99,
        concurrent_remote_p999,
        diagnostics,
    );
    drop(live);
}
