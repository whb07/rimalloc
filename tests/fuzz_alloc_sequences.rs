use core::ffi::c_void;
use core::ptr;
use std::os::raw::c_long;

use rimalloc::raw::mimalloc::*;

struct ShowErrorsGuard(c_long);

impl ShowErrorsGuard {
    unsafe fn disabled() -> Self {
        let option = mi_option_t::mi_option_show_errors;
        let previous = unsafe { mi_option_get(option) };
        unsafe { mi_option_set(option, 0) };
        Self(previous)
    }
}

impl Drop for ShowErrorsGuard {
    fn drop(&mut self) {
        unsafe { mi_option_set(mi_option_t::mi_option_show_errors, self.0) };
    }
}

#[derive(Clone, Copy)]
struct Block {
    ptr: *mut c_void,
    len: usize,
    alignment: Option<usize>,
    pattern: u8,
    heap_owned: bool,
}

impl Block {
    unsafe fn assert_invariants(self, heap: *mut mi_heap_t) {
        assert!(!self.ptr.is_null());
        assert!(unsafe { mi_usable_size(self.ptr) } >= self.len);
        if let Some(alignment) = self.alignment {
            assert_eq!(self.ptr as usize % alignment, 0);
        }
        if self.heap_owned {
            assert!(unsafe { mi_heap_contains(heap, self.ptr) });
        }
        let bytes = unsafe { core::slice::from_raw_parts(self.ptr.cast::<u8>(), self.len) };
        assert!(bytes.iter().all(|byte| *byte == self.pattern));
    }

    unsafe fn fill(self) {
        let bytes = unsafe { core::slice::from_raw_parts_mut(self.ptr.cast::<u8>(), self.len) };
        bytes.fill(self.pattern);
    }
}

struct Program<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Program<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn next(&mut self) -> Option<u8> {
        let byte = self.bytes.get(self.cursor).copied()?;
        self.cursor += 1;
        Some(byte)
    }

    fn size(&mut self, max: usize) -> usize {
        let lo = self.next().unwrap_or(0) as usize;
        let hi = self.next().unwrap_or(0) as usize;
        ((hi << 8) | lo) % max + 1
    }

    fn alignment(&mut self) -> usize {
        1usize << (3 + (self.next().unwrap_or(0) as usize % 8))
    }

    fn index(&mut self, len: usize) -> Option<usize> {
        if len == 0 {
            None
        } else {
            Some(self.next().unwrap_or(0) as usize % len)
        }
    }
}

unsafe fn assert_zeroed(ptr: *mut c_void, len: usize) {
    let bytes = unsafe { core::slice::from_raw_parts(ptr.cast::<u8>(), len) };
    assert!(bytes.iter().all(|byte| *byte == 0));
}

unsafe fn push_block(blocks: &mut Vec<Block>, heap: *mut mi_heap_t, block: Block) {
    unsafe { block.fill() };
    unsafe { block.assert_invariants(heap) };
    blocks.push(block);
}

unsafe fn run_program(input: &[u8]) {
    let heap = unsafe { mi_heap_new() };
    assert!(!heap.is_null());

    let mut program = Program::new(input);
    let mut blocks: Vec<Block> = Vec::new();
    let mut pattern = 1u8;

    while let Some(op) = program.next() {
        pattern = pattern.wrapping_add(1).max(1);
        match op % 12 {
            0 if blocks.len() < 48 => {
                let len = program.size(4096);
                let ptr = unsafe { mi_malloc(len) };
                assert!(!ptr.is_null());
                unsafe {
                    push_block(
                        &mut blocks,
                        heap,
                        Block {
                            ptr,
                            len,
                            alignment: None,
                            pattern,
                            heap_owned: false,
                        },
                    )
                };
            }
            1 if blocks.len() < 48 => {
                let len = program.size(4096);
                let ptr = unsafe { mi_zalloc(len) };
                assert!(!ptr.is_null());
                unsafe { assert_zeroed(ptr, len) };
                unsafe {
                    push_block(
                        &mut blocks,
                        heap,
                        Block {
                            ptr,
                            len,
                            alignment: None,
                            pattern,
                            heap_owned: false,
                        },
                    )
                };
            }
            2 if blocks.len() < 48 => {
                let count = program.size(128);
                let size = program.size(64);
                let len = count * size;
                let ptr = unsafe { mi_calloc(count, size) };
                assert!(!ptr.is_null());
                unsafe { assert_zeroed(ptr, len) };
                unsafe {
                    push_block(
                        &mut blocks,
                        heap,
                        Block {
                            ptr,
                            len,
                            alignment: None,
                            pattern,
                            heap_owned: false,
                        },
                    )
                };
            }
            3 if blocks.len() < 48 => {
                let len = program.size(4096);
                let alignment = program.alignment();
                let ptr = unsafe { mi_malloc_aligned(len, alignment) };
                assert!(!ptr.is_null());
                unsafe {
                    push_block(
                        &mut blocks,
                        heap,
                        Block {
                            ptr,
                            len,
                            alignment: Some(alignment),
                            pattern,
                            heap_owned: false,
                        },
                    )
                };
            }
            4 if blocks.len() < 48 => {
                let len = program.size(4096);
                let ptr = unsafe { mi_heap_malloc(heap, len) };
                assert!(!ptr.is_null());
                unsafe {
                    push_block(
                        &mut blocks,
                        heap,
                        Block {
                            ptr,
                            len,
                            alignment: None,
                            pattern,
                            heap_owned: true,
                        },
                    )
                };
            }
            5 if blocks.len() < 48 => {
                let len = program.size(4096);
                let alignment = program.alignment();
                let ptr = unsafe { mi_heap_malloc_aligned(heap, len, alignment) };
                assert!(!ptr.is_null());
                unsafe {
                    push_block(
                        &mut blocks,
                        heap,
                        Block {
                            ptr,
                            len,
                            alignment: Some(alignment),
                            pattern,
                            heap_owned: true,
                        },
                    )
                };
            }
            6 => {
                if let Some(idx) = program.index(blocks.len()) {
                    let block = blocks.swap_remove(idx);
                    unsafe { block.assert_invariants(heap) };
                    unsafe { mi_free(block.ptr) };
                }
            }
            7 => {
                if let Some(idx) = program.index(blocks.len()) {
                    let block = blocks.swap_remove(idx);
                    if block.alignment.is_some() {
                        unsafe { block.assert_invariants(heap) };
                        blocks.push(block);
                    } else {
                        let old_len = block.len;
                        let new_len = program.size(8192);
                        unsafe { block.assert_invariants(heap) };
                        let ptr = unsafe { mi_realloc(block.ptr, new_len) };
                        assert!(!ptr.is_null());
                        let preserved = old_len.min(new_len);
                        let bytes =
                            unsafe { core::slice::from_raw_parts(ptr.cast::<u8>(), preserved) };
                        assert!(bytes.iter().all(|byte| *byte == block.pattern));
                        let block = Block {
                            ptr,
                            len: new_len,
                            alignment: None,
                            pattern,
                            heap_owned: false,
                        };
                        unsafe { block.fill() };
                        blocks.push(block);
                    }
                }
            }
            8 => {
                if let Some(idx) = program.index(blocks.len()) {
                    let block = blocks.swap_remove(idx);
                    if block.alignment.is_some() {
                        unsafe { block.assert_invariants(heap) };
                        blocks.push(block);
                    } else {
                        let old_len = block.len;
                        let new_len = program.size(8192);
                        unsafe { block.assert_invariants(heap) };
                        let ptr = unsafe { mi_rezalloc(block.ptr, new_len) };
                        assert!(!ptr.is_null());
                        let preserved = old_len.min(new_len);
                        let bytes =
                            unsafe { core::slice::from_raw_parts(ptr.cast::<u8>(), preserved) };
                        assert!(bytes.iter().all(|byte| *byte == block.pattern));
                        if new_len > old_len {
                            let extension = unsafe {
                                core::slice::from_raw_parts(
                                    ptr.cast::<u8>().add(old_len),
                                    new_len - old_len,
                                )
                            };
                            assert!(extension.iter().all(|byte| *byte == 0));
                        }
                        let block = Block {
                            ptr,
                            len: new_len,
                            alignment: None,
                            pattern,
                            heap_owned: false,
                        };
                        unsafe { block.fill() };
                        blocks.push(block);
                    }
                }
            }
            9 => {
                unsafe { mi_collect(false) };
                for &block in &blocks {
                    unsafe { block.assert_invariants(heap) };
                }
            }
            10 => {
                let ptr = unsafe { mi_realloc(ptr::null_mut(), program.size(2048)) };
                assert!(!ptr.is_null());
                unsafe { mi_free(ptr) };
            }
            _ => {
                for &block in &blocks {
                    unsafe { block.assert_invariants(heap) };
                }
            }
        }
    }

    {
        let _quiet_expected_errors = unsafe { ShowErrorsGuard::disabled() };
        assert!(unsafe { mi_mallocn(usize::MAX, 2) }.is_null());
        assert!(unsafe { mi_calloc(usize::MAX, 2) }.is_null());
    }

    for block in blocks {
        unsafe { block.assert_invariants(heap) };
        unsafe { mi_free(block.ptr) };
    }
    unsafe { mi_heap_delete(heap) };
}

#[test]
fn byte_stream_allocation_programs_preserve_invariants() {
    const CORPUS: &[&[u8]] = &[
        b"",
        b"\0\x10\0\x01\x20\0\x02\x08\0\x08\x03\x40\0\x04\x05\x80\0\x06",
        b"\x01\xff\x0f\x08\x10\0\x09\x07\x30\0\x06\x00\x08\x40\0",
        b"\x03\x80\0\x03\x04\x05\x20\0\x05\x05\x09\x09\x09\x06\x00",
        b"\x04\x10\0\x07\x40\0\x08\x80\0\x0a\x22\x11\x06\x00",
    ];

    for input in CORPUS {
        unsafe { run_program(input) };
    }

    let generated: Vec<u8> = (0u8..=255)
        .map(|i| i.wrapping_mul(37).wrapping_add(11))
        .collect();
    unsafe { run_program(&generated) };
}
