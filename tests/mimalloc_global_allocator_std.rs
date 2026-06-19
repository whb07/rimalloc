use mechanical_mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[repr(align(4096))]
struct PageAligned([u8; 64]);

#[test]
fn rust_std_allocations_use_mimalloc_global_allocator() {
    let boxed = Box::new(1234usize);
    assert_eq!(*boxed, 1234);

    let mut values = Vec::with_capacity(128);
    values.extend(0..128usize);
    assert_eq!(values.len(), 128);
    assert_eq!(values.iter().sum::<usize>(), 8128);

    let mut text = String::from("mechanical");
    text.push_str("_mimalloc");
    assert_eq!(text, "mechanical_mimalloc");

    let aligned = Box::new(PageAligned([0xA5; 64]));
    let ptr = (&*aligned as *const PageAligned).cast::<u8>() as usize;
    assert_eq!(ptr % core::mem::align_of::<PageAligned>(), 0);
    assert_eq!(aligned.0[0], 0xA5);
}

#[test]
fn rust_std_allocations_work_across_threads() {
    let handles: Vec<_> = (0..8usize)
        .map(|thread_idx| {
            std::thread::spawn(move || {
                for iter in 0..256usize {
                    let mut values = Vec::with_capacity(64);
                    values.extend((0..64usize).map(|n| n + iter + thread_idx));

                    let text = format!("thread-{thread_idx}-iter-{iter}");
                    let boxed = Box::new((values[0], text.len()));

                    assert_eq!(boxed.0, iter + thread_idx);
                    assert!(boxed.1 >= "thread-0-iter-0".len());
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("allocation thread panicked");
    }
}
