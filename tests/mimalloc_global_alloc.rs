use core::alloc::{GlobalAlloc, Layout};

use mechanical_mimalloc::MiMalloc;

fn assert_traits<T: GlobalAlloc + Copy + Clone + Default + core::fmt::Debug + Send + Sync>() {}

#[test]
fn mimalloc_facade_has_expected_traits() {
    assert_traits::<MiMalloc>();
    let allocator = MiMalloc;
    let copied = allocator;
    let cloned = allocator;
    let defaulted = MiMalloc;
    assert_eq!(format!("{:?}", copied), "MiMalloc");
    let _ = (cloned, defaulted);
}

#[test]
fn alloc_returns_requested_alignment() {
    let alignments = [1usize, 2, 4, 8, 16, 32, 64, 4096, 1usize << 20];

    for alignment in alignments {
        let layout = Layout::from_size_align(257, alignment).expect("layout");
        let ptr = unsafe { MiMalloc.alloc(layout) };
        assert!(
            !ptr.is_null(),
            "allocation failed for alignment {alignment}"
        );
        assert_eq!(
            ptr as usize % alignment,
            0,
            "pointer was not aligned to {alignment}"
        );
        unsafe { MiMalloc.dealloc(ptr, layout) };
    }
}

#[test]
fn realloc_preserves_contents_when_growing_and_shrinking() {
    realloc_case(16, 128, 8);
    realloc_case(128, 32, 8);
    realloc_case(128, 1024, 64);
    realloc_case(1024, 128, 64);
    realloc_case(256, 2048, 4096);
    realloc_case(2048, 256, 4096);
}

fn realloc_case(old_size: usize, new_size: usize, alignment: usize) {
    let old_layout = Layout::from_size_align(old_size, alignment).expect("old layout");
    let ptr = unsafe { MiMalloc.alloc(old_layout) };
    assert!(!ptr.is_null(), "initial allocation failed");
    assert_eq!(ptr as usize % alignment, 0);

    for offset in 0..old_size {
        unsafe { ptr.add(offset).write((offset % 251) as u8) };
    }

    let new_ptr = unsafe { MiMalloc.realloc(ptr, old_layout, new_size) };
    assert!(!new_ptr.is_null(), "reallocation failed");
    assert_eq!(new_ptr as usize % alignment, 0);

    for offset in 0..old_size.min(new_size) {
        let byte = unsafe { new_ptr.add(offset).read() };
        assert_eq!(byte, (offset % 251) as u8, "byte {offset} changed");
    }

    let new_layout = Layout::from_size_align(new_size, alignment).expect("new layout");
    unsafe { MiMalloc.dealloc(new_ptr, new_layout) };
}

#[test]
fn tiny_allocations_preserve_requested_alignment() {
    // Keep blocks live together: immediately freeing each block can repeatedly
    // reuse an accidentally aligned address. Run without debug/secure padding too.
    for alignment in [2, 4, 8, 16] {
        for size in 1..alignment {
            for zeroed in [false, true] {
                let layout = Layout::from_size_align(size, alignment).expect("layout");
                let allocations: [*mut u8; 32] = core::array::from_fn(|_| {
                    let ptr = unsafe {
                        if zeroed {
                            MiMalloc.alloc_zeroed(layout)
                        } else {
                            MiMalloc.alloc(layout)
                        }
                    };
                    assert!(!ptr.is_null(), "allocation failed");
                    ptr
                });

                for &ptr in &allocations {
                    assert_eq!(ptr as usize % alignment, 0, "size {size}, align {alignment}");
                    if zeroed {
                        let bytes = unsafe { core::slice::from_raw_parts(ptr, size) };
                        assert!(bytes.iter().all(|&byte| byte == 0));
                    }
                }
                for ptr in allocations {
                    unsafe { MiMalloc.dealloc(ptr, layout) };
                }
            }
        }
    }
}

#[test]
fn tiny_reallocations_preserve_alignment_and_contents() {
    for alignment in [8, 16] {
        for new_size in 1..alignment {
            let old_layout = Layout::from_size_align(64, alignment).expect("old layout");
            let new_layout = Layout::from_size_align(new_size, alignment).expect("new layout");
            let allocations: [*mut u8; 32] = core::array::from_fn(|_| {
                let ptr = unsafe { MiMalloc.alloc(old_layout) };
                assert!(!ptr.is_null(), "initial allocation failed");
                unsafe { ptr.write_bytes(0xA5, old_layout.size()) };
                ptr
            });
            let resized = allocations.map(|ptr| {
                let new_ptr = unsafe { MiMalloc.realloc(ptr, old_layout, new_size) };
                assert!(!new_ptr.is_null(), "reallocation failed");
                new_ptr
            });

            for &ptr in &resized {
                assert_eq!(ptr as usize % alignment, 0, "size {new_size}, align {alignment}");
                let bytes = unsafe { core::slice::from_raw_parts(ptr, new_size) };
                assert!(bytes.iter().all(|&byte| byte == 0xA5));
            }
            for ptr in resized {
                unsafe { MiMalloc.dealloc(ptr, new_layout) };
            }
        }
    }
}
