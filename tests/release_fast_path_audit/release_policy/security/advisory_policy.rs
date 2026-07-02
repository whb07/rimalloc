use std::fs;
use std::path::Path;

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
