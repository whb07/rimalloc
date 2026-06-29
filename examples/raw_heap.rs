use core::ffi::c_void;

// Raw/unsafe heap API example. Prefer `mechanical_mimalloc::Heap` unless you
// need the C-shaped compatibility surface.
fn main() {
    unsafe {
        let heap = mechanical_mimalloc::raw::mi_heap_new();
        assert!(!heap.is_null());

        let ptr = mechanical_mimalloc::raw::mi_heap_malloc(heap, 128);
        assert!(!ptr.is_null());
        assert!(mechanical_mimalloc::raw::mi_heap_contains(
            heap,
            ptr.cast_const()
        ));

        mechanical_mimalloc::raw::mi_free(ptr.cast::<c_void>());
        mechanical_mimalloc::raw::mi_heap_delete(heap);
    }
}
