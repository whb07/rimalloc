use super::DatatypeAcceptanceSources;

pub(super) fn assert_typed_fixture_audit_splits(audit_sources: &DatatypeAcceptanceSources<'_>) {
    let ptr_tests_rs = audit_sources.source("src/types/ptr/tests.rs");
    let page_tests_rs = audit_sources.source("src/types/page/tests.rs");
    let heap_tests_rs = audit_sources.source("src/types/heap/tests.rs");

    for (source_name, source, audit_id, exact_count) in [
        (
            "src/types/ptr/tests.rs",
            ptr_tests_rs,
            "typed-page-pointer-test-fixtures",
            13,
        ),
        (
            "src/types/ptr/tests.rs",
            ptr_tests_rs,
            "typed-block-pointer-test-fixtures",
            8,
        ),
        (
            "src/types/ptr/tests.rs",
            ptr_tests_rs,
            "typed-heap-pointer-test-fixtures",
            8,
        ),
        (
            "src/types/ptr/tests.rs",
            ptr_tests_rs,
            "typed-thread-heap-pointer-test-fixtures",
            12,
        ),
        (
            "src/types/ptr/tests.rs",
            ptr_tests_rs,
            "typed-tld-pointer-test-fixtures",
            13,
        ),
        (
            "src/types/ptr/tests.rs",
            ptr_tests_rs,
            "typed-pointer-address-test-fixtures",
            2,
        ),
        (
            "src/types/ptr/tests.rs",
            ptr_tests_rs,
            "typed-arena-subproc-pointer-test-fixtures",
            17,
        ),
        (
            "src/types/ptr/tests.rs",
            ptr_tests_rs,
            "typed-page-queue-pointer-test-fixtures",
            7,
        ),
        (
            "src/types/ptr/tests.rs",
            ptr_tests_rs,
            "typed-provenance-pointer-test-fixtures",
            7,
        ),
        (
            "src/types/page/tests.rs",
            page_tests_rs,
            "typed-page-test-fixtures",
            2,
        ),
        (
            "src/types/heap/tests.rs",
            heap_tests_rs,
            "typed-main-heap-test-fixtures",
            12,
        ),
        (
            "src/types/heap/tests.rs",
            heap_tests_rs,
            "typed-owned-heap-test-fixtures",
            8,
        ),
        (
            "src/types/heap/tests.rs",
            heap_tests_rs,
            "typed-borrowed-heap-test-fixtures",
            9,
        ),
        (
            "src/types/heap/tests.rs",
            heap_tests_rs,
            "typed-thread-heap-state-test-fixtures",
            15,
        ),
        (
            "src/types/heap/tests.rs",
            heap_tests_rs,
            "typed-thread-local-heap-test-fixtures",
            11,
        ),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert_eq!(
            source.matches(&marker).count(),
            exact_count,
            "{audit_id} in {source_name} should retain exact fixture marker count"
        );
    }

    assert!(
        !ptr_tests_rs.contains("SAFETY[AUDIT:typed-pointer-test-fixtures]")
            && !ptr_tests_rs.contains("SAFETY[AUDIT:typed-heap-thread-tld-pointer-test-fixtures]")
            && !heap_tests_rs.contains("SAFETY[AUDIT:typed-heap-test-fixtures]"),
        "typed pointer and heap test fixtures should stay split by proof responsibility"
    );
}
