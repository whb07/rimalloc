use super::super::*;

#[test]
fn cve_and_advisory_policy_is_linked_and_concrete() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let advisory = fs::read_to_string(root.join("docs/security-advisories.md"))
        .expect("read security advisory policy");
    let security = fs::read_to_string(root.join("SECURITY.md")).expect("read SECURITY.md");
    let governance = fs::read_to_string(root.join("docs/governance.md")).expect("read governance");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");

    for required in [
        "Severity Triage",
        "Critical",
        "High",
        "Medium",
        "Low",
        "CVE Handling",
        "Request or reserve a CVE",
        "Embargo Handling",
        "default embargo target is up to 90 calendar days",
        "Embargo extensions require reporter agreement",
        "Coordinated Disclosure",
        "private security branch or private advisory fork",
        "Publish fixed releases, tags, checksums, release notes, changelog, and the\n   advisory in the same disclosure window",
        "Changelog and Advisory Publication",
        "The changelog `Security` entry must include",
        "The GitHub Security Advisory must include",
    ] {
        assert!(
            advisory.contains(required),
            "advisory policy should include marker {required}"
        );
    }

    for required in [
        "affected versions, release lines, target platforms, artifact modes, and\n  features",
        "whether C mimalloc 3.3.2 reproduces the same behavior",
        "affected versions and first fixed version",
        "workaround or mitigation status",
        "links to release notes, changelog, signed checksums",
    ] {
        assert!(
            advisory.contains(required),
            "advisory policy should require publication field {required}"
        );
    }

    assert!(
        security.contains("docs/security-advisories.md")
            && governance.contains("docs/security-advisories.md")
            && release.contains("docs/security-advisories.md"),
        "security, governance, and release docs should link to advisory policy"
    );
    assert!(
        governance.contains("severity triage, CVE handling, embargoes")
            && release.contains("changelog `Security` entry requirements"),
        "governance and release docs should summarize advisory policy obligations"
    );
}

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

#[test]
fn unsafe_audit_evidence_prefers_focused_release_audit_modules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let evidence = fs::read_to_string(root.join("docs/unsafe-audit-evidence.tsv"))
        .expect("read unsafe audit evidence");
    let unsafe_audit = fs::read_to_string(root.join("UNSAFE_AUDIT.md")).expect("read unsafe audit");
    let unsafe_audit_doc =
        fs::read_to_string(root.join("docs/unsafe-audit.md")).expect("read unsafe audit doc");

    for (audit_id, focused_path) in [
        (
            "allocation-platform-shim-boundary",
            "tests/release_fast_path_audit/unsafe_performance/allocation_free.rs",
        ),
        (
            "allocation-product-entry-boundary",
            "tests/release_fast_path_audit/unsafe_performance/allocation_free.rs",
        ),
        (
            "allocation-realloc-entry-boundary",
            "tests/release_fast_path_audit/unsafe_performance/allocation_free.rs",
        ),
        (
            "allocation-string-path-boundary",
            "tests/release_fast_path_audit/unsafe_performance/allocation_free.rs",
        ),
        (
            "allocation-cxx-new-boundary",
            "tests/release_fast_path_audit/unsafe_performance/allocation_free.rs",
        ),
        (
            "allocation-heap-dispatch-boundary",
            "tests/release_fast_path_audit/unsafe_performance/allocation_free.rs",
        ),
        (
            "allocation-layout-request-boundary",
            "tests/release_fast_path_audit/unsafe_performance/allocation_free.rs",
        ),
        (
            "allocation-page-refill-boundary",
            "tests/release_fast_path_audit/unsafe_performance/allocation_free.rs",
        ),
        (
            "aligned-padding-boundary",
            "tests/release_fast_path_audit/unsafe_performance/allocation_free.rs",
        ),
        (
            "aligned-heap-allocation-boundary",
            "tests/release_fast_path_audit/unsafe_performance/allocation_free.rs",
        ),
        (
            "aligned-realloc-boundary",
            "tests/release_fast_path_audit/unsafe_performance/allocation_free.rs",
        ),
        (
            "raw-macro-bootstrap-boundary",
            "tests/release_fast_path_audit/unsafe_performance/raw_facade.rs",
        ),
        (
            "raw-macro-override-entry-boundary",
            "tests/release_fast_path_audit/unsafe_performance/raw_facade.rs",
        ),
        (
            "raw-macro-override-entry-boundary",
            "tests/release_fast_path_audit/release_policy/api_surface.rs",
        ),
        (
            "raw-macro-libc-forwarding-boundary",
            "tests/release_fast_path_audit/unsafe_performance/raw_facade.rs",
        ),
        (
            "raw-macro-allocation-query-boundary",
            "tests/release_fast_path_audit/unsafe_performance/raw_facade.rs",
        ),
        (
            "raw-macro-cxx-boundary",
            "tests/release_fast_path_audit/unsafe_performance/raw_facade.rs",
        ),
        (
            "raw-macro-header-forwarding-boundary",
            "tests/release_fast_path_audit/release_policy/api_surface.rs",
        ),
        (
            "os-memory-config-boundary",
            "tests/release_fast_path_audit/unsafe_performance/os_memory.rs",
        ),
        (
            "os-memory-guard-boundary",
            "tests/release_fast_path_audit/unsafe_performance/os_memory.rs",
        ),
        (
            "os-memory-commit-boundary",
            "tests/release_fast_path_audit/unsafe_performance/os_memory.rs",
        ),
        (
            "os-memory-free-boundary",
            "tests/release_fast_path_audit/unsafe_performance/os_memory.rs",
        ),
        (
            "os-memory-allocation-boundary",
            "tests/release_fast_path_audit/unsafe_performance/os_memory.rs",
        ),
        (
            "os-memory-huge-page-boundary",
            "tests/release_fast_path_audit/unsafe_performance/os_memory.rs",
        ),
        (
            "os-memory-numa-boundary",
            "tests/release_fast_path_audit/unsafe_performance/os_memory.rs",
        ),
        (
            "page-map-global-boundary",
            "tests/release_fast_path_audit/unsafe_performance/local_boundaries.rs",
        ),
        (
            "page-map-init-boundary",
            "tests/release_fast_path_audit/unsafe_performance/local_boundaries.rs",
        ),
        (
            "page-map-teardown-boundary",
            "tests/release_fast_path_audit/unsafe_performance/local_boundaries.rs",
        ),
        (
            "page-map-register-boundary",
            "tests/release_fast_path_audit/unsafe_performance/local_boundaries.rs",
        ),
        (
            "page-map-lookup-boundary",
            "tests/release_fast_path_audit/unsafe_performance/local_boundaries.rs",
        ),
        (
            "page-map-global-boundary",
            "tests/release_fast_path_audit/unsafe_performance/page_arena_behavior.rs",
        ),
        (
            "page-map-init-boundary",
            "tests/release_fast_path_audit/unsafe_performance/page_arena_behavior.rs",
        ),
        (
            "page-map-teardown-boundary",
            "tests/release_fast_path_audit/unsafe_performance/page_arena_behavior.rs",
        ),
        (
            "page-map-register-boundary",
            "tests/release_fast_path_audit/unsafe_performance/page_arena_behavior.rs",
        ),
        (
            "page-map-lookup-boundary",
            "tests/release_fast_path_audit/unsafe_performance/page_arena_behavior.rs",
        ),
        (
            "stats-counter-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-counter-merge-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-clock-boundary",
            "tests/release_fast_path_audit/primitive_options.rs",
        ),
        (
            "stats-heap-merge-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-public-merge-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-heap-visitor-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-public-export-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-output-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-process-info-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "stats-json-boundary",
            "tests/release_fast_path_audit/unsafe_performance/thread_stats.rs",
        ),
        (
            "typed-pointer-constructor-boundary",
            "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs",
        ),
        (
            "typed-page-accessors",
            "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs",
        ),
        (
            "typed-block-accessors",
            "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs",
        ),
        (
            "typed-heap-accessors",
            "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs",
        ),
        (
            "typed-thread-heap-accessors",
            "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs",
        ),
        (
            "typed-tld-accessors",
            "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs",
        ),
        (
            "typed-arena-accessors",
            "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs",
        ),
        (
            "typed-subproc-accessors",
            "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs",
        ),
        (
            "typed-page-queue-accessors",
            "tests/release_fast_path_audit/datatype_acceptance/strict_type.rs",
        ),
    ] {
        let row_prefix = format!("{audit_id}\ttest\t{focused_path}\t");
        assert!(
            evidence.lines().any(|line| line.starts_with(&row_prefix)),
            "unsafe audit evidence should map {audit_id} to focused release-audit module {focused_path}"
        );
    }

    let tracked_ids = [
        "allocation-platform-shim-boundary",
        "allocation-product-entry-boundary",
        "allocation-heap-dispatch-boundary",
        "allocation-layout-request-boundary",
        "allocation-realloc-entry-boundary",
        "allocation-string-path-boundary",
        "allocation-cxx-new-boundary",
        "allocation-page-refill-boundary",
        "aligned-padding-boundary",
        "aligned-heap-allocation-boundary",
        "aligned-realloc-boundary",
        "raw-macro-bootstrap-boundary",
        "raw-macro-override-entry-boundary",
        "raw-macro-libc-forwarding-boundary",
        "raw-macro-allocation-query-boundary",
        "raw-macro-cxx-boundary",
        "raw-macro-header-forwarding-boundary",
        "os-memory-config-boundary",
        "os-memory-guard-boundary",
        "os-memory-commit-boundary",
        "os-memory-free-boundary",
        "os-memory-allocation-boundary",
        "os-memory-huge-page-boundary",
        "os-memory-numa-boundary",
        "page-map-global-boundary",
        "page-map-init-boundary",
        "page-map-teardown-boundary",
        "page-map-register-boundary",
        "page-map-lookup-boundary",
        "stats-counter-boundary",
        "stats-counter-merge-boundary",
        "stats-clock-boundary",
        "stats-heap-merge-boundary",
        "stats-public-merge-boundary",
        "stats-heap-visitor-boundary",
        "stats-public-export-boundary",
        "stats-output-boundary",
        "stats-process-info-boundary",
        "stats-json-boundary",
        "typed-pointer-constructor-boundary",
        "typed-page-accessors",
        "typed-block-accessors",
        "typed-heap-accessors",
        "typed-thread-heap-accessors",
        "typed-tld-accessors",
        "typed-arena-accessors",
        "typed-subproc-accessors",
        "typed-page-queue-accessors",
    ];
    let stale_paths = [
        "tests/release_fast_path_audit.rs",
        "tests/release_fast_path_audit/unsafe_performance.rs",
        "tests/release_fast_path_audit/release_policy.rs",
        "tests/release_fast_path_audit/datatype_acceptance.rs",
    ];

    for line in evidence.lines().skip(1) {
        let mut fields = line.split('\t');
        let audit_id = fields.next().expect("evidence row audit id");
        let _kind = fields.next().expect("evidence row kind");
        let path = fields.next().expect("evidence row path");
        let detail = fields.next().expect("evidence row detail");
        assert!(
            fields.next().is_none() && !detail.is_empty(),
            "evidence row should stay four tab-separated fields: {line}"
        );
        assert!(
            !tracked_ids.contains(&audit_id) || !stale_paths.contains(&path),
            "{audit_id} evidence should cite a focused release-audit module instead of stale aggregate path {path}"
        );
    }

    assert!(
        unsafe_audit.contains("the evidence row should name that focused\nmodule")
            && unsafe_audit_doc
                .contains("Release-audit evidence rows should cite the focused module"),
        "unsafe audit docs should describe focused release-audit evidence ownership"
    );
}

#[test]
fn hardening_threat_model_is_concrete_and_linked_from_security_policy() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let threat_model = fs::read_to_string(root.join("docs/hardening-threat-model.md"))
        .expect("read hardening threat model");
    let security = fs::read_to_string(root.join("SECURITY.md")).expect("read SECURITY.md");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");

    for required in [
        "Hardening Threat Model",
        "Baseline Assumptions",
        "The `secure` feature is expected to mitigate or detect selected allocator\nmetadata corruptions",
        "secure-mode padding, canaries, encoded free lists",
        "Non-goals",
        "The `guarded` feature is expected to detect selected out-of-bounds and\nuse-after-free patterns",
        "guarded size bounds, sample rate, sample\ncount",
        "Out Of Scope For Malicious Unsafe Callers",
        "forged pointers, foreign allocations, or interior pointers",
        "use-after-free, double-free, or realloc-after-free",
        "arbitrary process-memory write, code execution, or data-race capability",
        "Entropy, Canaries, And Metadata Corruption",
        "predictable entropy, repeated\ncanaries, or failed random initialization",
        "not a cryptographic boundary",
        "Allocator metadata corruption is in scope",
        "Hardening reports should include",
    ] {
        assert!(
            threat_model.contains(required),
            "hardening threat model should include marker {required}"
        );
    }

    assert!(
        security.contains("docs/hardening-threat-model.md")
            && security.contains("`secure`, `guarded`, malicious unsafe callers")
            && release.contains("docs/hardening-threat-model.md"),
        "security and release docs should link to hardening threat model"
    );
}

#[test]
fn memory_corruption_triage_checklist_is_concrete_in_security_policy() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let security = fs::read_to_string(root.join("SECURITY.md")).expect("read SECURITY.md");

    for required in [
        "Memory Corruption Triage Checklist",
        "allocation/free/realloc sequence",
        "API names, sizes, alignments",
        "zeroing expectations, preserved-byte expectations",
        "allocation origin, heap or\n  main-heap ownership",
        "thread ownership, cross-thread transfer",
        "feature set and target platform",
        "Cargo features, target triple",
        "`secure`/`guarded`/tracking mode",
        "sanitizer/Valgrind/Miri evidence where applicable",
        "exact commands,\n  logs, stack traces, core dump notes",
        "upstream C mimalloc comparison against vendored C mimalloc 3.3.2",
        "FFI symbol/artifact mode details",
        "`c-abi-cdylib`, `interpose-cdylib`",
        "`LD_PRELOAD`, `dlopen`, C++\n  operator override use",
        "unexpected exported or resolved symbol",
    ] {
        assert!(
            security.contains(required),
            "security policy should include memory-corruption checklist marker {required}"
        );
    }
}
