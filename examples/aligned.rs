use core::ffi::c_void;

fn main() {
    unsafe {
        let alignment = 64;
        let ptr = rimalloc::raw::mi_malloc_aligned(256, alignment);
        assert!(!ptr.is_null());
        assert_eq!((ptr as usize) % alignment, 0);

        let grown = rimalloc::raw::mi_realloc_aligned(ptr, 512, alignment);
        assert!(!grown.is_null());
        assert_eq!((grown as usize) % alignment, 0);

        rimalloc::raw::mi_free(grown.cast::<c_void>());
    }
}
