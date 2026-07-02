use super::*;

#[path = "emscripten/allocation_memory.rs"]
mod allocation_memory;
#[path = "emscripten/clock_thread.rs"]
mod clock_thread;

#[test]
fn emscripten_primitive_backend_stays_bounded_and_audited() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let emscripten_rs = fs::read_to_string(root.join("src/platform/emscripten.rs"))
        .expect("read src/platform/emscripten.rs");
    let allowlist = fs::read_to_string(root.join("docs/unsafe-migration-allowlist.tsv"))
        .expect("read unsafe migration allowlist");

    assert!(
        !allowlist
            .lines()
            .any(|line| line.starts_with("src/platform/emscripten.rs\t")),
        "Emscripten primitive backend should stay off the unsafe migration allowlist"
    );
    assert!(
        !emscripten_rs.contains("SAFETY[AUDIT:emscripten-primitive-boundary]"),
        "Emscripten primitive backend markers should stay split by operation"
    );
    for (audit_id, exact_count) in [
        ("emscripten-import-boundary", 2),
        ("emscripten-allocation-boundary", 10),
        ("emscripten-memory-operation-boundary", 10),
        ("emscripten-clock-io-boundary", 9),
        ("emscripten-pthread-key-boundary", 14),
        ("emscripten-thread-shim-boundary", 3),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert_eq!(
            emscripten_rs.matches(&marker).count(),
            exact_count,
            "Emscripten primitive marker {audit_id} should keep exact operation count"
        );
    }
    for import in [
        "fn emmalloc_free(p: *mut c_void);",
        "fn emmalloc_memalign(alignment: usize, size: usize) -> *mut c_void;",
        "fn emscripten_date_now() -> f64;",
        "fn emscripten_console_error(msg: *const c_char);",
        "fn getentropy(buf: *mut c_void, len: usize) -> c_int;",
        "fn pthread_key_create(",
        "fn pthread_key_delete(key: pthread_key_t) -> c_int;",
        "fn pthread_setspecific(key: pthread_key_t, value: *const c_void) -> c_int;",
        "fn sched_yield() -> c_int;",
    ] {
        assert!(
            emscripten_rs.contains(import),
            "Emscripten primitive import missing: {import}"
        );
    }

    allocation_memory::assert_emscripten_allocation_and_memory(&emscripten_rs);
    clock_thread::assert_emscripten_clock_io_and_thread_boundaries(&emscripten_rs);
}
