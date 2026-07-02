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
        ("internal page metadata helper boundary", 33),
        (
            "compatibility boundary retained for C ABI parity or translated-C source shape",
            33,
        ),
        ("C/platform ABI forwarding boundary", 32),
        ("aligned allocation compatibility boundary", 30),
        ("thread-heap compatibility boundary", 24),
        ("scalar domain constructor boundary", 24),
        ("thread-heap allocation compatibility boundary", 18),
        ("realloc allocation compatibility boundary", 13),
        ("layout request constructor boundary", 11),
        ("heap allocation export compatibility boundary", 9),
        ("C++ and guarded allocation compatibility boundary", 7),
        ("guarded configuration constructor boundary", 5),
        ("internal thread-heap helper boundary", 4),
        ("internal free-list helper boundary", 4),
        ("memid byte-count constructor boundary", 2),
    ] {
        assert_eq!(
            permanent_reasons.get(reason),
            Some(&expected),
            "strict-type permanent reason count drifted for {reason}"
        );
    }
    assert!(
        permanent_reasons.values().all(|count| *count <= 40),
        "strict-type allowlist has a new permanent reason group above the 40-entry cap"
    );

    for ((path, domain), expected) in [
        (("src/internal/mod.rs", "mi_page_t"), 33),
        (("src/allocator/alloc.rs", "mi_theap_t"), 30),
        (("src/allocator/page.rs", "mi_page_t"), 25),
        (("src/allocator/thread_heap.rs", "mi_theap_t"), 20),
        (("src/allocator/free.rs", "mi_page_t"), 19),
        (("src/allocator/page.rs", "mi_theap_t"), 19),
    ] {
        assert_eq!(
            raw_pointer_file_domains.get(&(path, domain)),
            Some(&expected),
            "strict-type raw-pointer file/domain hotspot drifted for {path}/{domain}"
        );
    }
    assert!(
        raw_pointer_file_domains.values().all(|count| *count <= 33),
        "strict-type allowlist has a new raw-pointer file/domain hotspot above the documented cap"
    );

    assert_doc_snippets(
        "TODO.md",
        &todo,
        &[
            "No strict-type permanent reason group exceeds 40 entries",
            "internal page metadata helper boundary with 33 entries",
            "C ABI parity or\n  translated-C source shape with 33 entries",
            "C/platform ABI forwarding boundary\n  with 32 entries",
            "aligned allocation compatibility boundary with 30 entries",
            "thread-heap compatibility boundary with 24 entries",
            "scalar domain\n  constructor boundary with 24 entries",
            "largest strict-type raw-pointer file/domain hotspots are\n  `src/internal/mod.rs`/`mi_page_t` with 33 entries",
            "`src/allocator/alloc.rs`/`mi_theap_t` with 30",
            "`src/allocator/page.rs`/`mi_page_t` with 25",
            "`src/allocator/thread_heap.rs`/`mi_theap_t`",
            "`src/allocator/free.rs`/`mi_page_t` with 19",
            "`src/allocator/page.rs`/`mi_theap_t` with 19",
        ],
    );
    assert_doc_snippets(
        "docs/strict-types.md",
        &strict_docs,
        &[
            "Permanent reason-group caps",
            "No permanent reason group exceeds 40 entries",
            "`internal page metadata helper boundary` (33) is a permanent translated-C page metadata edge",
            "`compatibility boundary retained for C ABI parity or translated-C source shape` (33) is a permanent raw ABI/source-shape edge",
            "`C/platform ABI forwarding boundary` (32) is a permanent platform ABI forwarding edge",
            "`aligned allocation compatibility boundary` (30) is a permanent aligned allocation ABI/source-shape edge",
            "`thread-heap compatibility boundary` (24) is a permanent thread-heap source-shape edge",
            "`scalar domain constructor boundary` (24) is a permanent scalar constructor/proof edge",
            "Raw pointer file/domain caps",
            "`src/internal/mod.rs`/`mi_page_t` at 33",
            "`src/allocator/alloc.rs`/`mi_theap_t` at 30",
            "`src/allocator/page.rs`/`mi_page_t` at 25",
            "`src/allocator/thread_heap.rs`/`mi_theap_t` at 20",
            "`src/allocator/free.rs`/`mi_page_t` at 19",
            "`src/allocator/page.rs`/`mi_theap_t` at 19",
        ],
    );
}
