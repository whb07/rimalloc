use std::fs;
use std::path::Path;

#[path = "unsafe_audit_evidence/docs_policy.rs"]
mod docs_policy;
#[path = "unsafe_audit_evidence/focused_paths.rs"]
mod focused_paths;
#[path = "unsafe_audit_evidence/source_marker_caps.rs"]
mod source_marker_caps;
#[path = "unsafe_audit_evidence/stale_paths.rs"]
mod stale_paths;

#[test]
fn unsafe_audit_evidence_prefers_focused_release_audit_modules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let evidence = fs::read_to_string(root.join("docs/unsafe-audit-evidence.tsv"))
        .expect("read unsafe audit evidence");
    let unsafe_audit = fs::read_to_string(root.join("UNSAFE_AUDIT.md")).expect("read unsafe audit");
    let unsafe_audit_doc =
        fs::read_to_string(root.join("docs/unsafe-audit.md")).expect("read unsafe audit doc");

    focused_paths::assert_focused_evidence_paths(&evidence);
    stale_paths::assert_no_stale_aggregate_paths(&evidence);
    docs_policy::assert_focused_evidence_docs(&unsafe_audit, &unsafe_audit_doc);
    source_marker_caps::assert_source_marker_cap(root, 17);
}
