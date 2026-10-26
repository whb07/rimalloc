use core::ffi::c_void;

// Raw/unsafe heap API example. Prefer `rimalloc::Heap` unless you
// need the C-shaped compatibility surface.
fn main() {
    unsafe {
        let heap = rimalloc::raw::mi_heap_new();
        assert!(!heap.is_null());

        let ptr = rimalloc::raw::mi_heap_malloc(heap, 128);
        assert!(!ptr.is_null());
        assert!(rimalloc::raw::mi_heap_contains(heap, ptr.cast_const()));

        rimalloc::raw::mi_free(ptr.cast::<c_void>());
        rimalloc::raw::mi_heap_delete(heap);
    }
}
