use core::alloc::{GlobalAlloc, Layout};

use rimalloc::MiMalloc;

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
fn tiny_allocations_preserve_alignment_without_debug_padding() {
    for size in [1, 7, 8, 9, 15] {
        for alignment in [8, 16, 32] {
            let layout = Layout::from_size_align(size, alignment).expect("layout");
            for zeroed in [false, true] {
                let pointers: [_; 32] = core::array::from_fn(|_| unsafe {
                    if zeroed {
                        MiMalloc.alloc_zeroed(layout)
                    } else {
                        MiMalloc.alloc(layout)
                    }
                });
                for p in pointers {
                    assert!(!p.is_null());
                    assert_eq!(
                        p as usize % alignment,
                        0,
                        "size={size}, align={alignment}, zeroed={zeroed}"
                    );
                    if zeroed {
                        assert!(
                            unsafe { core::slice::from_raw_parts(p, size) }
                                .iter()
                                .all(|&b| b == 0)
                        );
                    }
                    unsafe { MiMalloc.dealloc(p, layout) };
                }
            }
            realloc_case(128, size, alignment);
            realloc_case(size, size + 1, alignment);
        }
    }
}
