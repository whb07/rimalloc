const PARITY_DOC: &str = include_str!("../PARITY.md");
const MIMALLOC_H: &[u8] = include_bytes!("../mimalloc-3.3.2/include/mimalloc.h");
const MIMALLOC_STATS_H: &[u8] = include_bytes!("../mimalloc-3.3.2/include/mimalloc-stats.h");
const MIMALLOC_NEW_DELETE_H: &[u8] =
    include_bytes!("../mimalloc-3.3.2/include/mimalloc-new-delete.h");

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[test]
fn parity_matrix_documents_current_supported_rows() {
    assert!(PARITY_DOC.contains("linux-x86_64-gnu-default-debug"));
    assert!(PARITY_DOC.contains("linux-x86_64-gnu-default-release"));
    assert!(PARITY_DOC.contains("linux-x86_64-gnu-secure-dev"));
    assert!(PARITY_DOC.contains("linux-x86_64-gnu-guarded-dev"));
    assert!(PARITY_DOC.contains("MI_DEBUG=2"));
    assert!(PARITY_DOC.contains("MI_DEBUG=0"));
    assert!(PARITY_DOC.contains("MI_SECURE=0"));
    assert!(PARITY_DOC.contains("MI_SECURE=4"));
    assert!(PARITY_DOC.contains("MI_GUARDED=0"));
    assert!(PARITY_DOC.contains("MI_GUARDED=1"));
    assert!(PARITY_DOC.contains("Rust guarded allocation disabled"));
    assert!(PARITY_DOC.contains("guard-page sampling enabled"));
    assert!(PARITY_DOC.contains("padding and encoded free lists enabled"));
    assert!(PARITY_DOC.contains("padding and encoded free lists disabled"));
    assert!(PARITY_DOC.contains("WASI"));
    assert!(PARITY_DOC.contains("Emscripten"));
    assert!(PARITY_DOC.contains("Windows"));
    assert!(PARITY_DOC.contains("macOS"));
    assert!(PARITY_DOC.contains("non-Linux allocator"));
    assert!(PARITY_DOC.contains("ASAN"));
    assert!(PARITY_DOC.contains("TSAN"));
    assert!(PARITY_DOC.contains("UBSAN"));
    assert!(PARITY_DOC.contains("Windows ETW"));
}

#[test]
fn c_baseline_header_hashes_match_vendored_3_3_2_headers() {
    assert_eq!(MIMALLOC_H.len(), 47_719);
    assert_eq!(fnv1a64(MIMALLOC_H), 0x1ecb_c696_d434_39d7);

    assert_eq!(MIMALLOC_STATS_H.len(), 7_349);
    assert_eq!(fnv1a64(MIMALLOC_STATS_H), 0x2552_6128_6353_5d3a);

    assert_eq!(MIMALLOC_NEW_DELETE_H.len(), 4_044);
    assert_eq!(fnv1a64(MIMALLOC_NEW_DELETE_H), 0x8a5a_a49e_0ef8_c48a);
}

#[test]
fn c_baseline_documents_required_public_headers_and_versions() {
    for header in [
        "mimalloc-3.3.2/include/mimalloc.h",
        "mimalloc-3.3.2/include/mimalloc-stats.h",
        "mimalloc-3.3.2/include/mimalloc-new-delete.h",
    ] {
        assert!(
            PARITY_DOC.contains(header),
            "missing baseline header {header}"
        );
    }

    let mimalloc_h = core::str::from_utf8(MIMALLOC_H).expect("mimalloc.h is utf-8");
    let stats_h = core::str::from_utf8(MIMALLOC_STATS_H).expect("mimalloc-stats.h is utf-8");

    assert!(mimalloc_h.contains("#define MI_MALLOC_VERSION 30302"));
    assert!(stats_h.contains("#define MI_STAT_VERSION   5"));
    assert!(PARITY_DOC.contains("| `MI_MALLOC_VERSION` | 30302 |"));
    assert!(PARITY_DOC.contains("| `MI_STAT_VERSION` | 5 |"));
}

#[test]
fn parity_docs_record_cxx_bad_alloc_divergence() {
    assert!(PARITY_DOC.contains("C++ bad-allocation throwing from Rust"));
    assert!(PARITY_DOC.contains("mi_new"));
    assert!(PARITY_DOC.contains("std::bad_alloc"));
    assert!(PARITY_DOC.contains("`extern \"C\"` Rust boundary"));
    assert!(PARITY_DOC.contains("scripts/check-cxx-bad-alloc-divergence.py"));
    assert!(PARITY_DOC.contains("keeps the `mi_new` and C++ override TODO items open"));
}

#[test]
fn parity_docs_record_rust_only_export_safety_contracts() {
    assert!(PARITY_DOC.contains("Rust-Only Public Export Safety Contracts"));
    assert!(PARITY_DOC.contains("symbols/public-symbol-classification.tsv"));
    assert!(PARITY_DOC.contains("scripts/check-public-symbols.py"));

    for export in [
        "_mi_*",
        "mi_arena_from_index",
        "mi_os_alloc",
        "mi_theap_umalloc",
        "mi_subproc_delete",
        "mi_stats_get_json_from",
    ] {
        assert!(PARITY_DOC.contains(export), "missing export {export}");
    }

    for contract_term in [
        "ownership",
        "aliasing",
        "thread",
        "heap",
        "callback",
        "must not unwind",
        "mi_free",
    ] {
        assert!(
            PARITY_DOC.contains(contract_term),
            "missing safety contract term {contract_term}"
        );
    }
}

#[test]
fn parity_docs_record_unresolved_baseline_mismatches() {
    assert!(PARITY_DOC.contains("Known Unresolved Baseline Mismatches"));
    assert!(PARITY_DOC.contains("not accepted Rust non-parity decisions"));
    assert!(PARITY_DOC.contains("vendored public headers declare a symbol"));
    assert!(PARITY_DOC.contains("linkable definition"));
    assert!(PARITY_DOC.contains("scripts/check-c-baseline-missing-exports.py"));

    for api in [
        "mi_stats_merge",
        "mi_collect_reduce",
        "mi_theap_set_default",
        "mi_uzalloc_small",
    ] {
        assert!(
            PARITY_DOC.contains(api),
            "missing unresolved baseline mismatch for {api}"
        );
    }

    assert!(PARITY_DOC.contains("TODO items for these APIs remain open"));
    assert!(PARITY_DOC.contains("theap default-switching TODOs remain open"));
    assert!(PARITY_DOC.contains("allocation-family TODO remains open"));
}

#[test]
fn parity_docs_map_vendored_c_tests_to_rust_artifact_runs() {
    assert!(PARITY_DOC.contains("Vendored C Test Coverage"));
    assert!(PARITY_DOC.contains("cargo test"));

    for mapping in [
        ("main.c", "tests/original_main.rs"),
        ("main-override.c", "tests/original_main_override.rs"),
        ("main-override.cpp", "tests/original_main_override.rs"),
        (
            "main-override-static.c",
            "tests/original_main_override_static.rs",
        ),
        ("main-override-dep.cpp", "tests/original_main_override.rs"),
        ("main-override-dep.h", "tests/original_main_override.rs"),
        ("main-static-dep.cpp", "tests/original_main_static_dep.rs"),
        ("main-static-dep.h", "tests/original_main_static_dep.rs"),
        ("test-api.c", "tests/original_test_api.rs"),
        ("test-api-fill.c", "tests/original_test_api_fill.rs"),
        ("test-stress.c", "tests/original_test_stress.rs"),
        ("testhelper.h", "tests/original_testhelper.rs"),
    ] {
        assert!(PARITY_DOC.contains(mapping.0), "missing {}", mapping.0);
        assert!(PARITY_DOC.contains(mapping.1), "missing {}", mapping.1);
    }

    assert!(PARITY_DOC.contains("test-wrong.c"));
    assert!(PARITY_DOC.contains("Not ported as a passing integration test"));
    assert!(PARITY_DOC.contains("TODO items 166 and 194"));
    assert!(PARITY_DOC.contains("CMakeLists.txt"));
    assert!(PARITY_DOC.contains("readme.md"));
}

#[test]
fn parity_signoff_documents_required_commands() {
    for command in [
        "cargo fmt --check",
        "cargo check --features track-valgrind",
        "cargo check --features track-asan",
        "cargo check --features secure",
        "cargo check --features guarded",
        "cargo clippy --all-targets -- -D warnings",
        "cargo clippy --features secure --all-targets -- -D warnings",
        "cargo clippy --features guarded --all-targets -- -D warnings",
        "cargo test",
        "cargo test --features secure",
        "cargo test --features guarded",
        "cargo test --release",
        "scripts/check-public-abi.py",
        "scripts/check-public-symbols.py",
        "scripts/check-public-symbols.py --missing-only",
        "scripts/check-c-link-public-api.py",
        "scripts/check-c-baseline-missing-exports.py",
        "scripts/check-behavioral-parity.py",
        "scripts/check-cxx-bad-alloc-divergence.py",
        "scripts/check-secure-mode-parity.py",
        "scripts/check-guarded-mode-parity.py",
        "scripts/check-valgrind-tracking-parity.py",
        "scripts/check-asan-tracking-parity.py",
    ] {
        assert!(
            PARITY_DOC.contains(command),
            "missing signoff command {command}"
        );
    }
}
