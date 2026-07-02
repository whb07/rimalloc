#[path = "subproc_fast_paths/heap_runtime.rs"]
mod heap_runtime;
#[path = "subproc_fast_paths/main_heap.rs"]
mod main_heap;

pub(super) fn assert_subproc_fast_paths(
    init_rs: &str,
    heap_rs: &str,
    free_rs: &str,
    page_rs: &str,
) {
    main_heap::assert_main_heap_fast_paths(init_rs);
    heap_runtime::assert_heap_runtime_fast_paths(heap_rs, free_rs, page_rs);
}
