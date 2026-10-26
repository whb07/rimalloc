use core::ffi::c_void;
use core::ptr;
use std::env;
use std::fs;
use std::os::raw::c_long;
use std::path::{Path, PathBuf};
use std::thread;

use rimalloc::StatsSnapshot;
use rimalloc::raw::mimalloc::*;

const MAX_BLOCKS: usize = 64;
const MAX_TRACKED_SIZE: usize = 1 << 20;

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

    fn size(&mut self) -> usize {
        match self.next().unwrap_or(0) % 9 {
            0 => 0,
            1 => 1,
            2 => 8,
            3 => 64,
            4 => 4096,
            5 => 65_536,
            6 => MAX_TRACKED_SIZE,
            _ => {
                let lo = self.next().unwrap_or(0) as usize;
                let hi = self.next().unwrap_or(0) as usize;
                ((hi << 8) | lo) % MAX_TRACKED_SIZE
            }
        }
    }

    fn alignment(&mut self) -> usize {
        1usize << (3 + (self.next().unwrap_or(0) as usize % 10))
    }

    fn index(&mut self, len: usize) -> Option<usize> {
        if len == 0 {
            None
        } else {
            Some(self.next().unwrap_or(0) as usize % len)
        }
    }
}

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

struct Harness {
    heap: *mut mi_heap_t,
    blocks: Vec<Block>,
    pattern: u8,
}

impl Harness {
    unsafe fn new() -> Option<Self> {
        let heap = unsafe { mi_heap_new() };
        if heap.is_null() {
            None
        } else {
            Some(Self {
                heap,
                blocks: Vec::new(),
                pattern: 1,
            })
        }
    }

    unsafe fn push(&mut self, block: Block) {
        unsafe { block.fill() };
        unsafe { block.assert_invariants(self.heap) };
        self.blocks.push(block);
    }

    unsafe fn allocate(
        &mut self,
        ptr: *mut c_void,
        len: usize,
        alignment: Option<usize>,
        heap_owned: bool,
    ) {
        if ptr.is_null() || self.blocks.len() >= MAX_BLOCKS {
            if !ptr.is_null() {
                unsafe { mi_free(ptr) };
            }
            return;
        }
        unsafe {
            self.push(Block {
                ptr,
                len,
                alignment,
                pattern: self.pattern,
                heap_owned,
            })
        };
    }

    unsafe fn assert_all(&self) {
        for &block in &self.blocks {
            unsafe { block.assert_invariants(self.heap) };
        }
        assert!(StatsSnapshot::capture().is_ok());
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        for block in self.blocks.drain(..) {
            unsafe { mi_free(block.ptr) };
        }
        unsafe { mi_heap_delete(self.heap) };
    }
}

unsafe fn assert_zeroed(ptr: *mut c_void, len: usize) {
    let bytes = unsafe { core::slice::from_raw_parts(ptr.cast::<u8>(), len) };
    assert!(bytes.iter().all(|byte| *byte == 0));
}

unsafe fn run_input(input: &[u8]) {
    let _quiet_expected_errors = unsafe { ShowErrorsGuard::disabled() };
    let Some(mut harness) = (unsafe { Harness::new() }) else {
        return;
    };
    let mut program = Program::new(input);

    while let Some(op) = program.next() {
        harness.pattern = harness.pattern.wrapping_add(1).max(1);
        match op % 15 {
            0 => {
                let len = program.size();
                let ptr = unsafe { mi_malloc(len) };
                unsafe { harness.allocate(ptr, len, None, false) };
            }
            1 => {
                let len = program.size();
                let ptr = unsafe { mi_zalloc(len) };
                if !ptr.is_null() {
                    unsafe { assert_zeroed(ptr, len) };
                }
                unsafe { harness.allocate(ptr, len, None, false) };
            }
            2 => {
                let count = program.size().min(4096);
                let size = program.size().min(4096);
                let ptr = unsafe { mi_calloc(count, size) };
                if let Some(len) = count.checked_mul(size) {
                    if !ptr.is_null() {
                        unsafe { assert_zeroed(ptr, len) };
                    }
                    unsafe { harness.allocate(ptr, len, None, false) };
                } else {
                    assert!(ptr.is_null());
                }
            }
            3 => {
                let len = program.size();
                let alignment = program.alignment();
                let ptr = unsafe { mi_malloc_aligned(len, alignment) };
                unsafe { harness.allocate(ptr, len, Some(alignment), false) };
            }
            4 => {
                let len = program.size();
                let ptr = unsafe { mi_heap_malloc(harness.heap, len) };
                unsafe { harness.allocate(ptr, len, None, true) };
            }
            5 => {
                let len = program.size();
                let alignment = program.alignment();
                let ptr = unsafe { mi_heap_malloc_aligned(harness.heap, len, alignment) };
                unsafe { harness.allocate(ptr, len, Some(alignment), true) };
            }
            6 => {
                if let Some(index) = program.index(harness.blocks.len()) {
                    let block = harness.blocks.swap_remove(index);
                    unsafe { block.assert_invariants(harness.heap) };
                    unsafe { mi_free(block.ptr) };
                }
            }
            7 | 8 => {
                if let Some(index) = program.index(harness.blocks.len()) {
                    let block = harness.blocks.swap_remove(index);
                    if block.alignment.is_some() {
                        harness.blocks.push(block);
                        continue;
                    }
                    let old_len = block.len;
                    let new_len = program.size();
                    unsafe { block.assert_invariants(harness.heap) };
                    let ptr = if op % 15 == 7 {
                        unsafe { mi_realloc(block.ptr, new_len) }
                    } else {
                        unsafe { mi_rezalloc(block.ptr, new_len) }
                    };
                    if ptr.is_null() {
                        harness.blocks.push(block);
                        continue;
                    }
                    let preserved = old_len.min(new_len);
                    let bytes = unsafe { core::slice::from_raw_parts(ptr.cast::<u8>(), preserved) };
                    assert!(bytes.iter().all(|byte| *byte == block.pattern));
                    unsafe { harness.allocate(ptr, new_len, None, false) };
                }
            }
            9 => {
                unsafe { mi_collect(false) };
                unsafe { harness.assert_all() };
            }
            10 => {
                let len = program.size();
                let ptr = unsafe { mi_realloc(ptr::null_mut(), len) };
                if !ptr.is_null() {
                    assert!(unsafe { mi_usable_size(ptr) } >= len);
                    unsafe { mi_free(ptr) };
                }
            }
            11 => {
                unsafe { mi_free(ptr::null_mut()) };
                assert!(unsafe { mi_mallocn(usize::MAX, 2) }.is_null());
                assert!(unsafe { mi_calloc(usize::MAX, 2) }.is_null());
            }
            12 => {
                if let Some(index) = program.index(harness.blocks.len()) {
                    let block = harness.blocks.swap_remove(index);
                    unsafe { block.assert_invariants(harness.heap) };
                    let addr = block.ptr as usize;
                    thread::spawn(move || unsafe { mi_free(addr as *mut c_void) })
                        .join()
                        .expect("cross-thread free worker");
                }
            }
            13 => {
                assert!(StatsSnapshot::capture().is_ok());
            }
            _ => {
                unsafe { mi_heap_collect(harness.heap, false) };
                unsafe { harness.assert_all() };
            }
        }
    }
}

fn generated_input(seed: u64, max_len: usize) -> Vec<u8> {
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let len = (state as usize % max_len.max(1)).max(1);
    let mut bytes = Vec::with_capacity(len);
    for _ in 0..len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        bytes.push(state as u8);
    }
    bytes
}

fn corpus_files(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(path).expect("read corpus directory") {
        let entry = entry.expect("read corpus entry");
        if entry.path().is_file() {
            files.push(entry.path());
        }
    }
    files.sort();
    files
}

fn main() {
    let mut runs = 256usize;
    let mut max_len = 256usize;
    let mut paths = Vec::new();
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--runs" => {
                runs = args
                    .next()
                    .expect("--runs value")
                    .parse()
                    .expect("--runs must be a number");
            }
            "--max-len" => {
                max_len = args
                    .next()
                    .expect("--max-len value")
                    .parse()
                    .expect("--max-len must be a number");
            }
            path => paths.push(PathBuf::from(path)),
        }
    }

    for path in &paths {
        for file in corpus_files(path) {
            let input = fs::read(&file).expect("read corpus input");
            unsafe { run_input(&input) };
        }
    }

    for seed in 0..runs as u64 {
        let input = generated_input(seed, max_len);
        unsafe { run_input(&input) };
    }
}
