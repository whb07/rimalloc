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
    assert!(PARITY_DOC.contains("MI_DEBUG=0"));
    assert!(PARITY_DOC.contains("MI_SECURE=0"));
    assert!(PARITY_DOC.contains("MI_GUARDED=0"));
    assert!(PARITY_DOC.contains("WASI"));
    assert!(PARITY_DOC.contains("Emscripten"));
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
fn parity_signoff_documents_required_commands() {
    for command in [
        "cargo fmt --check",
        "cargo clippy --all-targets -- -D warnings",
        "cargo test",
        "cargo test --release",
        "scripts/check-public-abi.py",
        "scripts/check-public-symbols.py",
        "scripts/check-public-symbols.py --missing-only",
        "scripts/check-c-link-public-api.py",
        "scripts/check-behavioral-parity.py",
    ] {
        assert!(
            PARITY_DOC.contains(command),
            "missing signoff command {command}"
        );
    }
}
