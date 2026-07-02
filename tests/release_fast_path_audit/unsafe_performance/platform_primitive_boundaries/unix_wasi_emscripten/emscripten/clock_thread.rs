use super::*;

pub(super) fn assert_emscripten_clock_io_and_thread_boundaries(emscripten_rs: &str) {
    let clock = source_item(
        "src/platform/emscripten.rs",
        emscripten_rs,
        "pub unsafe extern \"C\" fn _mi_prim_clock_now(",
    );
    assert!(
        clock.contains("unsafe { emscripten_date_now() as mi_msecs_t }")
            && clock.contains("SystemTime::now()")
            && clock.contains("duration_since(UNIX_EPOCH)")
            && clock.contains(".unwrap_or(0)"),
        "Emscripten clock primitive should keep platform import and host fallback"
    );

    let stderr = source_item(
        "src/platform/emscripten.rs",
        emscripten_rs,
        "pub unsafe extern \"C\" fn _mi_prim_out_stderr(",
    );
    assert!(
        stderr.contains("if msg.is_null()")
            && stderr.contains("unsafe { emscripten_console_error(msg) };")
            && stderr.contains("CStr::from_ptr(msg)")
            && stderr.contains("stderr.write_all(bytes)")
            && stderr.contains("stderr.flush()"),
        "Emscripten stderr primitive should null-check and preserve console/host fallback output"
    );

    let getenv = source_item(
        "src/platform/emscripten.rs",
        emscripten_rs,
        "pub unsafe extern \"C\" fn _mi_prim_getenv(",
    );
    assert!(
        getenv.contains("let _ = name;")
            && getenv.contains("let _ = result;")
            && getenv.contains("let _ = result_size;")
            && getenv.contains("false")
            && !getenv.contains("*name")
            && !getenv.contains("*result"),
        "Emscripten getenv primitive should stay an unsupported non-dereferencing shim"
    );

    let pthread_done = source_item(
        "src/platform/emscripten.rs",
        emscripten_rs,
        "pub unsafe extern \"C\" fn mi_pthread_done(",
    );
    assert!(
        pthread_done.contains("if value.is_null()")
            && pthread_done.contains(
                "unsafe { crate::internal::_mi_thread_done(value.cast::<mi_theap_t>()) };"
            ),
        "Emscripten pthread destructor should null-check and call thread teardown through the local boundary"
    );

    let thread_init = source_item(
        "src/platform/emscripten.rs",
        emscripten_rs,
        "pub unsafe extern \"C\" fn _mi_prim_thread_init_auto_done(",
    );
    assert!(
        thread_init.contains(
            "let key_is_invalid = unsafe { _mi_heap_default_key == PTHREAD_KEY_INVALID };"
        ) && thread_init.contains("pthread_key_create(&mut key, Some(mi_pthread_done))")
            && thread_init.contains("unsafe { _mi_heap_default_key = key };")
            && thread_init.contains("unsafe { _mi_heap_default_key = 0 };"),
        "Emscripten thread init should preserve key creation, destructor registration, and fallback key sentinel"
    );

    let thread_done = source_item(
        "src/platform/emscripten.rs",
        emscripten_rs,
        "pub unsafe extern \"C\" fn _mi_prim_thread_done_auto_done(",
    );
    assert!(
        thread_done.contains("let key = unsafe { _mi_heap_default_key };")
            && thread_done.contains("unsafe { pthread_key_delete(key) }")
            && thread_done
                .contains("MI_EM_PTHREAD_DEFAULT_THEAP.with(|value| value.set(ptr::null_mut()))")
            && thread_done.contains("unsafe { _mi_heap_default_key = PTHREAD_KEY_INVALID };"),
        "Emscripten thread teardown should snapshot/delete the key and clear fallback TLS"
    );

    let thread_assoc = source_item(
        "src/platform/emscripten.rs",
        emscripten_rs,
        "pub unsafe extern \"C\" fn _mi_prim_thread_associate_default_theap(",
    );
    assert!(
        thread_assoc.contains("let key = unsafe { _mi_heap_default_key };")
            && thread_assoc
                .contains("unsafe { pthread_setspecific(key, theap.cast_const().cast()) }")
            && thread_assoc
                .contains("MI_EM_PTHREAD_DEFAULT_THEAP.with(|value| value.set(theap.cast()))"),
        "Emscripten thread association should snapshot the key and preserve pthread/fallback storage"
    );

    let thread_yield = source_item(
        "src/platform/emscripten.rs",
        emscripten_rs,
        "pub unsafe extern \"C\" fn _mi_prim_thread_yield(",
    );
    assert!(
        thread_yield.contains("let _ = unsafe { sched_yield() };")
            && thread_yield.contains("std::thread::yield_now();"),
        "Emscripten thread yield should preserve scheduler import and host fallback"
    );
}
