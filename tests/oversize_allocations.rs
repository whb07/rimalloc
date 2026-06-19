use mechanical_mimalloc::raw::mimalloc::{mi_free, mi_malloc};

#[test]
fn max_alloc_size_minus_one_returns_null_without_abort() {
    let p = unsafe { mi_malloc(isize::MAX as usize - 1) };
    assert!(p.is_null());
    unsafe { mi_free(p) };
}
