use std::fs;
use std::path::Path;

#[test]
fn fuzz_corpus_and_reproducer_policy_maps_to_security_and_unsafe_audit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let testing = fs::read_to_string(root.join("docs/testing.md")).expect("read testing");
    let security = fs::read_to_string(root.join("SECURITY.md")).expect("read SECURITY.md");
    let unsafe_audit =
        fs::read_to_string(root.join("docs/unsafe-audit.md")).expect("read unsafe audit");

    for required in [
        "fuzz/corpus/alloc_sequences/<descriptive-name>",
        "fuzz/fuzz_targets/alloc_sequences.rs",
        "A focused Rust regression test under `tests/` or the owning source module",
        "Corpus additions are required",
        "valid\nsafe API use, documented raw/C ABI use, release artifact behavior",
        "allocator-owned memory corruption, realloc preservation, zeroing,\nalignment, heap ownership, cross-thread free",
        "Do not commit corpus inputs that depend on invalid caller UB",
        "A\nfinding is an allocator bug",
        "A\nfinding is invalid-caller UB",
        "forged pointers, foreign\nallocations, use-after-free",
        "Confirmed fuzz findings that touch unsafe invariants must update the unsafe\naudit mapping",
        "docs/unsafe-audit-evidence.tsv",
        "SAFETY[AUDIT:*]",
        "Private security reproducers stay out of the public corpus",
    ] {
        assert!(
            testing.contains(required),
            "testing docs should define fuzz reproducer policy marker {required}"
        );
    }

    for required in [
        "docs/testing.md",
        "fuzz corpus and minimized reproducer policy",
        "invalid-caller undefined behavior versus allocator bugs",
        "unsafe-audit evidence",
    ] {
        assert!(
            security.contains(required),
            "security policy should link fuzz policy marker {required}"
        );
    }

    assert!(
        unsafe_audit.contains("Fuzz Finding Mapping")
            && unsafe_audit.contains("docs/testing.md")
            && unsafe_audit.contains("docs/unsafe-audit-evidence.tsv")
            && unsafe_audit.contains("SAFETY[AUDIT:*]"),
        "unsafe audit docs should map confirmed fuzz findings to audit evidence"
    );
}
