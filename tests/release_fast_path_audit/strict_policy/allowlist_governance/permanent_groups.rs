use super::*;

#[test]
fn strict_type_permanent_reasons_and_raw_file_domains_stay_capped() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let entries = read_allowlist(root);
    let todo = fs::read_to_string(root.join("TODO.md")).expect("read TODO.md");
    let strict_docs =
        fs::read_to_string(root.join("docs/strict-types.md")).expect("read strict type docs");

    let mut permanent_reasons = BTreeMap::new();
    let mut raw_pointer_file_domains = BTreeMap::new();
    for entry in &entries {
        increment(&mut permanent_reasons, entry.permanent_reason());
        if entry.kind == "raw-pointer-parameter" {
            increment(
                &mut raw_pointer_file_domains,
                (entry.path.as_str(), entry.domain.as_str()),
            );
        }
    }

    for (reason, expected) in [
        (
            "compatibility boundary retained for C ABI parity or translated-C source shape",
            32,
        ),
        ("C/platform ABI forwarding boundary", 32),
        ("aligned allocation compatibility boundary", 30),
        ("internal page metadata helper boundary", 26),
        ("thread-heap compatibility boundary", 24),
        ("scalar domain constructor boundary", 24),
        ("thread-heap allocation compatibility boundary", 16),
        ("realloc allocation compatibility boundary", 13),
        ("layout request constructor boundary", 11),
        ("heap allocation export compatibility boundary", 9),
        ("internal page location helper boundary", 7),
        ("C++ and guarded allocation compatibility boundary", 7),
        ("guarded configuration constructor boundary", 5),
        ("internal thread-heap helper boundary", 4),
        ("internal free-list helper boundary", 4),
        ("memid byte-count constructor boundary", 2),
        ("output buffer pointer/integer compatibility boundary", 1),
    ] {
        assert_eq!(
            permanent_reasons.get(reason),
            Some(&expected),
            "strict-type permanent reason count drifted for {reason}"
        );
    }
    assert!(
        permanent_reasons.values().all(|count| *count <= 32),
        "strict-type allowlist has a new permanent reason group above the 32-entry cap"
    );

    for ((path, domain), expected) in [
        (("src/allocator/thread_heap.rs", "mi_theap_t"), 20),
        (("src/allocator/free.rs", "mi_page_t"), 19),
        (("src/allocator/alloc.rs", "mi_theap_t"), 18),
        (("src/allocator/page.rs", "mi_theap_t"), 15),
        (("src/internal/page_metadata.rs", "mi_page_t"), 15),
    ] {
        assert_eq!(
            raw_pointer_file_domains.get(&(path, domain)),
            Some(&expected),
            "strict-type raw-pointer file/domain hotspot drifted for {path}/{domain}"
        );
    }
    assert!(
        raw_pointer_file_domains.values().all(|count| *count <= 20),
        "strict-type allowlist has a new raw-pointer file/domain hotspot above the documented cap"
    );

    assert_doc_snippets(
        "TODO.md",
        &todo,
        &[
            "No strict-type permanent reason group exceeds 32 entries",
            "C/platform ABI forwarding boundary with 32 entries",
            "C ABI parity or\n  translated-C source shape with 32 entries",
            "internal page metadata helper boundary with 26\n  entries",
            "aligned allocation compatibility\n  boundary with 30 entries",
            "thread-heap compatibility boundary with 24 entries",
            "scalar domain\n  constructor boundary with 24 entries",
            "largest strict-type raw-pointer file/domain hotspots are\n  `src/allocator/thread_heap.rs`/`mi_theap_t` with 20 entries",
            "`src/allocator/thread_heap.rs`/`mi_theap_t` with 20",
            "`src/allocator/free.rs`/`mi_page_t` with 19",
            "`src/allocator/alloc.rs`/`mi_theap_t` with 18",
            "`src/allocator/page.rs`/`mi_theap_t` with 15",
            "`src/internal/page_metadata.rs`/`mi_page_t` with 15",
        ],
    );
    assert_doc_snippets(
        "docs/strict-types.md",
        &strict_docs,
        &[
            "Permanent reason-group caps",
            "No permanent reason group exceeds 32 entries",
            "`C/platform ABI forwarding boundary` (32) is a permanent platform ABI forwarding edge",
            "`compatibility boundary retained for C ABI parity or translated-C source shape` (32) is a permanent raw ABI/source-shape edge",
            "`aligned allocation compatibility boundary` (30) is a permanent aligned allocation ABI/source-shape edge",
            "`internal page metadata helper boundary` (26) is a permanent translated-C page metadata edge",
            "`thread-heap compatibility boundary` (24) is a permanent thread-heap source-shape edge",
            "`scalar domain constructor boundary` (24) is a permanent scalar constructor/proof edge",
            "Raw pointer file/domain caps",
            "No raw pointer file/domain\npair exceeds 20 entries",
            "`src/allocator/thread_heap.rs`/`mi_theap_t` is at 20",
            "`src/allocator/free.rs`/`mi_page_t` is at 19",
            "`src/allocator/alloc.rs`/`mi_theap_t` is at 18",
            "`src/allocator/page.rs`/`mi_theap_t` is at 15",
            "`src/internal/page_metadata.rs`/`mi_page_t` is at 15",
        ],
    );
}
