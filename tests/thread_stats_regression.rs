use std::sync::{Arc, Barrier};
use std::thread;

use mechanical_mimalloc::raw::{
    MI_STAT_VERSION, mi_free, mi_malloc, mi_process_init, mi_stats_get, mi_stats_t,
};

fn initialized_stats() -> mi_stats_t {
    mi_stats_t {
        size: core::mem::size_of::<mi_stats_t>(),
        version: MI_STAT_VERSION,
        ..Default::default()
    }
}

#[test]
fn joined_worker_threads_return_current_thread_stats_to_main() {
    unsafe { mi_process_init() };

    let mut before = initialized_stats();
    assert!(unsafe { mi_stats_get(&mut before) });

    const THREADS: usize = 64;
    let barrier = Arc::new(Barrier::new(THREADS));
    let mut workers = Vec::with_capacity(THREADS);

    for i in 0..THREADS {
        let barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || unsafe {
            let p = mi_malloc(32 + (i % 17));
            assert!(!p.is_null());
            barrier.wait();
            mi_free(p);
        }));
    }

    for worker in workers {
        worker.join().unwrap();
    }

    let mut stats = initialized_stats();
    assert!(unsafe { mi_stats_get(&mut stats) });
    assert_eq!(stats.threads.current, before.threads.current);
}
