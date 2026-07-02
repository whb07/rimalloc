use super::*;

#[test]
fn phase6_unsafe_op_allowances_stay_intentional() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src");
    let mut sources = Vec::new();
    collect_rs_sources(root, &src, &mut sources);

    let actual = sources
        .iter()
        .filter_map(|(rel, source)| {
            source
                .contains("unsafe_op_in_unsafe_fn")
                .then_some(rel.as_str())
        })
        .collect::<BTreeSet<_>>();
    let expected = BTreeSet::<&str>::new();

    assert_eq!(
        actual, expected,
        "broad unsafe_op_in_unsafe_fn allowances should not be reintroduced"
    );
    assert!(
        !actual.contains("src/allocator/aligned.rs")
            && !actual.contains("src/allocator/alloc.rs")
            && !actual.contains("src/raw/posix.rs")
            && !actual.contains("src/raw/macros.rs")
            && !actual.contains("src/allocator/arena_meta.rs")
            && !actual.contains("src/internal/atomic.rs")
            && !actual.contains("src/internal/bitmap.rs")
            && !actual.contains("src/types/mod.rs")
            && !actual.contains("src/allocator/free.rs")
            && !actual.contains("src/allocator/heap.rs")
            && !actual.contains("src/internal/mod.rs")
            && !actual.contains("src/ffi/libc.rs")
            && !actual.contains("src/allocator/options.rs")
            && !actual.contains("src/platform/os.rs")
            && !actual.contains("src/allocator/page.rs")
            && !actual.contains("src/platform/mod.rs")
            && !actual.contains("src/platform/unix.rs")
            && !actual.contains("src/internal/random.rs")
            && !actual.contains("src/allocator/thread_heap.rs")
            && !actual.contains("src/allocator/threadlocal.rs")
            && !actual.contains("src/allocator/tracking.rs"),
        "allocation helpers, aligned allocation helpers, allocation override shims, POSIX allocation shims, arena metadata, atomic/lock helpers, bitmap helpers, core types, free-path helpers, heap helpers, internal helpers, libc helpers, options helpers, OS boundary helpers, page helpers, Unix/Emscripten/WASI/macOS primitive helpers, primitive TLS helpers, random helpers, thread-heap helpers, thread-local helpers, and tracking shims should not regain unsafe_op_in_unsafe_fn after explicit unsafe cleanup"
    );
}
