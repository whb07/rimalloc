#![no_main]

use core::ffi::c_void;
use libfuzzer_sys::fuzz_target;
use mechanical_mimalloc::raw::mimalloc::*;
use std::thread;

fuzz_target!(|data: &[u8]| {
    let mut slots = vec![core::ptr::null_mut::<c_void>(); 32];
    for command in data.chunks_exact(4) {
        let index = usize::from(command[1]) % slots.len();
        let size = 1 + ((usize::from(command[2]) << 8) | usize::from(command[3]));
        unsafe {
            match command[0] % 7 {
                0 => {
                    mi_free(slots[index]);
                    slots[index] = mi_malloc(size);
                }
                1 => {
                    mi_free(slots[index]);
                    slots[index] = core::ptr::null_mut();
                }
                2 => {
                    let resized = mi_realloc(slots[index], size);
                    if !resized.is_null() {
                        slots[index] = resized;
                    }
                }
                3 => {
                    mi_free(slots[index]);
                    slots[index] = mi_malloc_aligned(size, 8usize << (command[1] % 8));
                }
                4 => {
                    let address = slots[index] as usize;
                    slots[index] = core::ptr::null_mut();
                    thread::spawn(move || mi_free(address as *mut c_void)).join().unwrap();
                }
                5 => {
                    let heap = mi_heap_new();
                    if !heap.is_null() {
                        core::hint::black_box(mi_heap_malloc(heap, size));
                        mi_heap_destroy(heap);
                    }
                }
                6 => {
                    let address = thread::spawn(move || mi_malloc(size) as usize).join().unwrap();
                    mi_free(address as *mut c_void);
                }
                _ => unreachable!(),
            }
        }
    }
    for pointer in slots {
        unsafe { mi_free(pointer) };
    }
});
