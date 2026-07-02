pub(super) fn assert_focused_evidence_docs(unsafe_audit: &str, unsafe_audit_doc: &str) {
    assert!(
        unsafe_audit.contains("the evidence row should name that focused\nmodule")
            && unsafe_audit_doc
                .contains("Release-audit evidence rows should cite the focused module"),
        "unsafe audit docs should describe focused release-audit evidence ownership"
    );
}
