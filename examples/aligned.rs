use core::ffi::c_void;

fn main() {
    unsafe {
        let alignment = 64;
        let ptr = mechanical_mimalloc::raw::mi_malloc_aligned(256, alignment);
        assert!(!ptr.is_null());
        assert_eq!((ptr as usize) % alignment, 0);

        let grown = mechanical_mimalloc::raw::mi_realloc_aligned(ptr, 512, alignment);
        assert!(!grown.is_null());
        assert_eq!((grown as usize) % alignment, 0);

        mechanical_mimalloc::raw::mi_free(grown.cast::<c_void>());
    }
}
