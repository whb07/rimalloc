use core::ffi::c_void;
use core::ptr;

use mechanical_mimalloc::raw::mimalloc::*;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

#[derive(Clone, Copy)]
struct Block {
    ptr: *mut c_void,
    len: usize,
    alignment: Option<usize>,
    pattern: u8,
    from_heap: bool,
}

fn assert_aligned(ptr: *mut c_void, alignment: usize) {
    assert_eq!(ptr as usize % alignment, 0);
}

unsafe fn fill_block(block: Block) {
    let bytes = unsafe { core::slice::from_raw_parts_mut(block.ptr.cast::<u8>(), block.len) };
    bytes.fill(block.pattern);
}

unsafe fn assert_block(block: Block) {
    assert!(!block.ptr.is_null());
    assert!(unsafe { mi_usable_size(block.ptr) } >= block.len);
    if let Some(alignment) = block.alignment {
        assert_aligned(block.ptr, alignment);
    }
    let bytes = unsafe { core::slice::from_raw_parts(block.ptr.cast::<u8>(), block.len) };
    assert!(bytes.iter().all(|byte| *byte == block.pattern));
}

unsafe fn assert_zeroed(ptr: *mut c_void, len: usize) {
    let bytes = unsafe { core::slice::from_raw_parts(ptr.cast::<u8>(), len) };
    assert!(bytes.iter().all(|byte| *byte == 0));
}

unsafe fn allocate_block(rng: &mut StdRng, heap: *mut mi_heap_t, pattern: u8) -> Block {
    let len = rng.gen_range(1..=4096);
    match rng.gen_range(0..4) {
        0 => {
            let ptr = unsafe { mi_malloc(len) };
            assert!(!ptr.is_null());
            let block = Block {
                ptr,
                len,
                alignment: None,
                pattern,
                from_heap: false,
            };
            unsafe { fill_block(block) };
            block
        }
        1 => {
            let ptr = unsafe { mi_calloc(len, 1) };
            assert!(!ptr.is_null());
            unsafe { assert_zeroed(ptr, len) };
            let block = Block {
                ptr,
                len,
                alignment: None,
                pattern,
                from_heap: false,
            };
            unsafe { fill_block(block) };
            block
        }
        2 => {
            let alignment = 1usize << rng.gen_range(3..=10);
            let ptr = unsafe { mi_malloc_aligned(len, alignment) };
            assert!(!ptr.is_null());
            assert_aligned(ptr, alignment);
            let block = Block {
                ptr,
                len,
                alignment: Some(alignment),
                pattern,
                from_heap: false,
            };
            unsafe { fill_block(block) };
            block
        }
        _ => {
            let ptr = unsafe { mi_heap_malloc(heap, len) };
            assert!(!ptr.is_null());
            assert!(unsafe { mi_heap_contains(heap, ptr) });
            let block = Block {
                ptr,
                len,
                alignment: None,
                pattern,
                from_heap: true,
            };
            unsafe { fill_block(block) };
            block
        }
    }
}

unsafe fn reallocate_block(rng: &mut StdRng, mut block: Block) -> Block {
    let old_len = block.len;
    let old_pattern = block.pattern;
    let old_bytes = unsafe { core::slice::from_raw_parts(block.ptr.cast::<u8>(), old_len) };
    assert!(old_bytes.iter().all(|byte| *byte == old_pattern));

    let new_len = rng.gen_range(1..=8192);
    let ptr = unsafe { mi_realloc(block.ptr, new_len) };
    assert!(!ptr.is_null());

    let preserved = old_len.min(new_len);
    let new_bytes = unsafe { core::slice::from_raw_parts(ptr.cast::<u8>(), preserved) };
    assert!(new_bytes.iter().all(|byte| *byte == old_pattern));

    block.ptr = ptr;
    block.len = new_len;
    block.alignment = None;
    block.from_heap = false;
    unsafe { fill_block(block) };
    block
}

unsafe fn run_seed(seed: u64) {
    let mut rng = StdRng::seed_from_u64(seed);
    let heap = unsafe { mi_heap_new() };
    assert!(!heap.is_null());

    let mut blocks: Vec<Block> = Vec::new();
    for step in 0..512 {
        match rng.gen_range(0..100) {
            0..=49 if blocks.len() < 64 => {
                blocks.push(unsafe { allocate_block(&mut rng, heap, step as u8) });
            }
            50..=74 if !blocks.is_empty() => {
                let idx = rng.gen_range(0..blocks.len());
                let block = blocks.swap_remove(idx);
                unsafe { assert_block(block) };
                unsafe { mi_free(block.ptr) };
            }
            75..=89 if !blocks.is_empty() => {
                let idx = rng.gen_range(0..blocks.len());
                if blocks[idx].alignment.is_none() {
                    let block = blocks.swap_remove(idx);
                    blocks.push(unsafe { reallocate_block(&mut rng, block) });
                }
            }
            _ => {
                for &block in &blocks {
                    unsafe { assert_block(block) };
                    if block.from_heap {
                        assert!(unsafe { mi_heap_contains(heap, block.ptr) });
                    }
                }
            }
        }
    }

    for block in blocks {
        unsafe { assert_block(block) };
        unsafe { mi_free(block.ptr) };
    }
    unsafe { mi_heap_delete(heap) };
}

#[test]
fn seeded_allocation_sequences_preserve_basic_properties() {
    for seed in 0..8 {
        unsafe { run_seed(seed) };
    }
}

#[test]
fn null_realloc_behaves_like_malloc() {
    unsafe {
        let ptr = mi_realloc(ptr::null_mut(), 128);
        assert!(!ptr.is_null());
        assert!(mi_usable_size(ptr) >= 128);
        mi_free(ptr);
    }
}
