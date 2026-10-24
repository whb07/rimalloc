use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use mechanical_mimalloc::{MiMalloc, raw};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn run_worker(jobs: Receiver<usize>) {
    // This grace period is an application choice, not an allocator default.
    let idle_grace = Duration::from_millis(100);
    let maintenance_interval = Duration::from_secs(1);
    let mut next_maintenance = Instant::now() + maintenance_interval;
    // SAFETY: initialize this worker before collecting its default heap.
    unsafe { raw::mi_thread_init() };

    loop {
        let count = match jobs.recv_timeout(idle_grace) {
            Ok(count) => count,
            Err(RecvTimeoutError::Timeout) => {
                // SAFETY: collection runs on the initialized allocation owner.
                unsafe { raw::mi_collect(true) };
                next_maintenance = Instant::now() + maintenance_interval;
                // Collect once per idle interval, then sleep until work arrives.
                match jobs.recv() {
                    Ok(count) => count,
                    Err(_) => break,
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        };

        let blocks: Vec<_> = (0..count).map(|_| vec![0xa5_u8; 64]).collect();
        std::hint::black_box(&blocks);
        drop(blocks);

        // A busy worker may never become idle. Check between completed batches.
        if Instant::now() >= next_maintenance {
            // SAFETY: this worker owns its initialized default heap.
            unsafe { raw::mi_collect(false) };
            next_maintenance = Instant::now() + maintenance_interval;
        }
    }

    // SAFETY: this worker still owns its initialized default heap.
    unsafe { raw::mi_collect(true) };
}

fn main() {
    let (sender, receiver) = mpsc::channel();
    let worker = std::thread::spawn(move || run_worker(receiver));
    sender.send(16_384).unwrap();
    std::thread::sleep(Duration::from_millis(10));
    sender.send(16_384).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    sender.send(16_384).unwrap();
    drop(sender);
    worker.join().unwrap();
}
