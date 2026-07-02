use std::fs;
use std::path::Path;

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
