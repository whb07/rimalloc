use std::sync::{Arc, Barrier};
use std::thread;

use rimalloc::raw::{mi_free, mi_malloc, mi_process_init, mi_thread_done, mi_thread_init};

#[test]
fn concurrent_thread_heap_creation_and_teardown() {
    unsafe { mi_process_init() };
    const THREADS: usize = 8;
    const CYCLES: usize = 256;
    let start = Arc::new(Barrier::new(THREADS));
    let workers: Vec<_> = (0..THREADS)
        .map(|id| {
            let start = Arc::clone(&start);
            thread::spawn(move || {
                start.wait();
                for cycle in 0..CYCLES {
                    let size = 64 + (id + cycle) % 64;
                    // SAFETY: each allocation remains owned by this worker,
                    // is accessed within its requested size, and is freed once.
                    // Initialization and teardown affect only this worker.
                    unsafe {
                        mi_thread_init();
                        let p = mi_malloc(size).cast::<u8>();
                        assert!(!p.is_null());
                        p.write_bytes(0xa5, size);
                        assert_eq!(*p.add(size - 1), 0xa5);
                        mi_free(p.cast());
                        mi_thread_done();
                    }
                }
                // Mix explicit teardown at exit with automatic teardown after
                // reinitialization while other workers reuse heap metadata.
                if id % 2 == 0 {
                    unsafe { mi_thread_init() };
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
}
