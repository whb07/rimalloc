use super::*;

pub(super) fn assert_threadlocal_runtime_no_exports(threadlocal_rs: &str) {
    for internal_name in [
        "pub extern \"C\" fn _mi_thread_locals_init",
        "pub extern \"C\" fn _mi_thread_locals_done",
        "pub extern \"C\" fn _mi_thread_local_set",
        "pub extern \"C\" fn _mi_thread_local_get",
        "pub extern \"C\" fn _mi_thread_locals_thread_done",
        "pub extern \"C\" fn _mi_thread_local_free",
    ] {
        assert_internal_no_export(
            "src/allocator/threadlocal.rs",
            threadlocal_rs,
            internal_name,
        );
    }
}
