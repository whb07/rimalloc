use super::*;

#[test]
fn phase6_c_compat_allowances_stay_intentional() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src");
    let mut sources = Vec::new();
    collect_rs_sources(root, &src, &mut sources);

    const LINTS: &[&str] = &[
        "non_camel_case_types",
        "non_upper_case_globals",
        "non_snake_case",
        "unused_macros",
        "unused_imports",
        "improper_ctypes_definitions",
    ];
    let expected = BTreeMap::from([
        (("src/ffi/abi.rs", "non_camel_case_types"), 1),
        (("src/ffi/abi.rs", "non_upper_case_globals"), 1),
        (("src/allocator/alloc.rs", "unused_imports"), 6),
        (("src/allocator/alloc/cxx_new.rs", "non_snake_case"), 1),
        (("src/allocator/aligned.rs", "non_camel_case_types"), 1),
        (("src/allocator/aligned.rs", "non_upper_case_globals"), 1),
        (("src/allocator/aligned.rs", "unused_imports"), 1),
        (("src/raw/macros.rs", "non_camel_case_types"), 1),
        (("src/raw/macros.rs", "non_snake_case"), 1),
        (("src/raw/macros.rs", "non_upper_case_globals"), 1),
        (
            (
                "src/raw/macros/header_forwarding.rs",
                "improper_ctypes_definitions",
            ),
            1,
        ),
        (("src/raw/macros/header_forwarding.rs", "non_snake_case"), 1),
        (("src/allocator/arena.rs", "non_upper_case_globals"), 1),
        (("src/allocator/arena.rs", "unused_imports"), 1),
        (("src/allocator/arena/debug.rs", "non_camel_case_types"), 2),
        (
            ("src/allocator/arena/heap_pages.rs", "non_camel_case_types"),
            2,
        ),
        (("src/allocator/arena/prefix.rs", "non_camel_case_types"), 1),
        (("src/allocator/arena/purge.rs", "non_camel_case_types"), 1),
        (("src/allocator/arena_meta.rs", "non_camel_case_types"), 1),
        (("src/allocator/arena_meta.rs", "non_upper_case_globals"), 1),
        (("src/internal/atomic.rs", "non_camel_case_types"), 1),
        (("src/internal/atomic.rs", "non_upper_case_globals"), 1),
        (("src/internal/bitmap.rs", "non_camel_case_types"), 1),
        (("src/internal/bitmap.rs", "non_snake_case"), 1),
        (("src/internal/bitmap.rs", "non_upper_case_globals"), 1),
        (("src/internal/bitmap/chunk_bits.rs", "non_snake_case"), 1),
        (("src/internal/bits.rs", "non_camel_case_types"), 1),
        (("src/internal/bits.rs", "non_upper_case_globals"), 1),
        (("src/ffi/headers.rs", "non_upper_case_globals"), 14),
        (("src/allocator/init.rs", "non_camel_case_types"), 1),
        (("src/allocator/init.rs", "non_snake_case"), 7),
        (("src/allocator/init.rs", "non_upper_case_globals"), 1),
        (("src/internal/mod.rs", "non_camel_case_types"), 1),
        (("src/internal/memid.rs", "non_camel_case_types"), 1),
        (("src/internal/mod.rs", "non_snake_case"), 1),
        (("src/internal/mod.rs", "non_upper_case_globals"), 1),
        (("src/internal/mod.rs", "unused_imports"), 1),
        (("src/lib.rs", "unused_imports"), 2),
        (("src/ffi/libc.rs", "non_camel_case_types"), 1),
        (("src/ffi/libc.rs", "non_upper_case_globals"), 1),
        (("src/allocator/options.rs", "non_camel_case_types"), 1),
        (("src/allocator/options.rs", "non_upper_case_globals"), 1),
        (("src/platform/os.rs", "non_camel_case_types"), 1),
        (("src/platform/os.rs", "non_upper_case_globals"), 1),
        (("src/allocator/page.rs", "non_camel_case_types"), 1),
        (("src/allocator/page.rs", "non_upper_case_globals"), 1),
        (("src/allocator/page.rs", "unused_imports"), 2),
        (("src/allocator/page_map.rs", "non_camel_case_types"), 1),
        (("src/allocator/page_map.rs", "non_upper_case_globals"), 1),
        (("src/platform/mod.rs", "non_camel_case_types"), 1),
        (("src/platform/mod.rs", "non_upper_case_globals"), 1),
        (("src/platform/emscripten.rs", "non_camel_case_types"), 1),
        (("src/platform/emscripten.rs", "non_upper_case_globals"), 1),
        (("src/platform/macos.rs", "non_camel_case_types"), 1),
        (("src/platform/macos.rs", "non_snake_case"), 1),
        (("src/platform/macos.rs", "non_upper_case_globals"), 1),
        (("src/platform/macos.rs", "unused_imports"), 1),
        (("src/platform/macos.rs", "unused_macros"), 1),
        (("src/platform/unix.rs", "non_camel_case_types"), 1),
        (("src/platform/unix.rs", "non_upper_case_globals"), 1),
        (("src/platform/wasi.rs", "non_camel_case_types"), 1),
        (("src/platform/wasi.rs", "non_upper_case_globals"), 1),
        (("src/platform/windows.rs", "non_camel_case_types"), 1),
        (("src/platform/windows.rs", "non_snake_case"), 1),
        (("src/platform/windows.rs", "non_upper_case_globals"), 1),
        (("src/platform/windows.rs", "unused_macros"), 1),
        (("src/internal/random.rs", "non_camel_case_types"), 1),
        (("src/internal/random.rs", "non_upper_case_globals"), 1),
        (("src/raw/mod.rs", "unused_imports"), 1),
        (("src/allocator/stats.rs", "non_camel_case_types"), 1),
        (("src/allocator/stats.rs", "non_upper_case_globals"), 1),
        (("src/allocator/thread_heap.rs", "non_camel_case_types"), 1),
        (("src/allocator/threadlocal.rs", "non_camel_case_types"), 1),
        (
            ("src/allocator/threadlocal.rs", "non_upper_case_globals"),
            1,
        ),
        (("src/types/constants.rs", "non_camel_case_types"), 1),
        (("src/types/constants.rs", "non_upper_case_globals"), 1),
    ]);

    assert_eq!(
        rust_allowance_counts(&sources, LINTS),
        expected,
        "C compatibility allowances should stay limited to audited ABI, re-export, and mechanical-translation surfaces"
    );
    for cleaned in ["src/allocator/static_alloc.rs", "src/allocator/tracking.rs"] {
        assert!(
            !rust_allowance_counts(&sources, LINTS)
                .keys()
                .any(|(rel, _)| *rel == cleaned),
            "{cleaned} should not regain C naming allowances"
        );
    }
}
