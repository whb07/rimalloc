#[cfg(feature = "compare")]
use std::alloc::System;
use std::alloc::{GlobalAlloc, Layout};

pub struct PublicMiMalloc;
pub struct RawMiMalloc;
#[cfg(feature = "compare")]
pub struct SystemAllocator;

pub static PUBLIC_MIMALLOC: PublicMiMalloc = PublicMiMalloc;
pub static RAW_MIMALLOC: RawMiMalloc = RawMiMalloc;
#[cfg(feature = "compare")]
pub static SYSTEM_ALLOCATOR: SystemAllocator = SystemAllocator;

unsafe impl GlobalAlloc for PublicMiMalloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { rimalloc::MiMalloc.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        unsafe { rimalloc::MiMalloc.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { rimalloc::MiMalloc.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unsafe { rimalloc::MiMalloc.realloc(ptr, layout, new_size) }
    }
}

unsafe impl GlobalAlloc for RawMiMalloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.align() <= 16 {
            unsafe { rimalloc::raw::mi_malloc(layout.size()).cast() }
        } else {
            unsafe { rimalloc::raw::mi_malloc_aligned(layout.size(), layout.align()).cast() }
        }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        if layout.align() <= 16 {
            unsafe { rimalloc::raw::mi_calloc(1, layout.size()).cast() }
        } else {
            let ptr = unsafe { self.alloc(layout) };
            if !ptr.is_null() {
                unsafe { ptr.write_bytes(0, layout.size()) };
            }
            ptr
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        unsafe { rimalloc::raw::mi_free(ptr.cast()) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if layout.align() <= 16 {
            unsafe { rimalloc::raw::mi_realloc(ptr.cast(), new_size).cast() }
        } else {
            unsafe {
                rimalloc::raw::mi_realloc_aligned(ptr.cast(), new_size, layout.align()).cast()
            }
        }
    }
}

#[cfg(feature = "compare")]
unsafe impl GlobalAlloc for SystemAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}
