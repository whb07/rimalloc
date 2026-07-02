use super::*;

#[test]
fn libc_helpers_use_bounded_stack_buffers_and_split_markers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let unsafe_allowlist = fs::read_to_string(root.join("docs/unsafe-migration-allowlist.tsv"))
        .expect("read unsafe migration allowlist");
    let libc_rs = fs::read_to_string(root.join("src/ffi/libc.rs")).expect("read src/ffi/libc.rs");

    assert!(
        !libc_rs.contains("let mut rendered = String::new()")
            && !libc_rs.contains("rendered.as_bytes()")
            && libc_rs.contains("struct FixedBuffer")
            && libc_rs.contains("impl fmt::Write for FixedBuffer")
            && libc_rs.contains("ptr::copy_nonoverlapping(s.as_ptr(), self.ptr.add(self.used), n)")
            && libc_rs.contains("*out.ptr.add(out.used) = 0;")
            && libc_rs.contains("#[cfg(unix)]\nunsafe extern \"C\"")
            && libc_rs.contains("fn vsnprintf(")
            && libc_rs.contains("#[cfg(not(unix))]"),
        "Rust-side bounded snprintf formatting should write directly into the caller buffer"
    );
    assert!(
        !unsafe_allowlist.contains("src/ffi/libc.rs")
            && !libc_rs.contains("SAFETY[AUDIT:ffi-libc-helpers]"),
        "libc helper unsafe boundaries should stay covered by local operation markers without a migration allowlist entry"
    );
    assert!(
        !libc_rs.contains("SAFETY[AUDIT:ffi-libc-string-boundary]"),
        "libc string helpers should stay split by compare/copy/scan/search/test fixture responsibility"
    );
    for (audit_id, min_count) in [
        ("ffi-libc-string-compare-boundary", 4),
        ("ffi-libc-string-copy-boundary", 7),
        ("ffi-libc-string-scan-boundary", 4),
        ("ffi-libc-string-search-boundary", 4),
        ("ffi-libc-string-test-fixtures", 8),
        ("ffi-libc-env-boundary", 2),
        ("ffi-libc-output-boundary", 5),
        ("ffi-libc-format-boundary", 8),
        ("ffi-libc-retention-boundary", 8),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert!(
            libc_rs.matches(&marker).count() >= min_count,
            "libc helper boundary {audit_id} should retain at least {min_count} local operation markers"
        );
    }
}
