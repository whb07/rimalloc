use std::fs;
use std::path::Path;

#[test]
fn symbol_versioning_policy_matches_symbol_scripts_and_manifests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let policy = fs::read_to_string(root.join("docs/symbol-versioning.md"))
        .expect("read docs/symbol-versioning.md");
    let abi_doc = fs::read_to_string(root.join("docs/abi.md")).expect("read docs/abi.md");
    let release_doc =
        fs::read_to_string(root.join("docs/release.md")).expect("read docs/release.md");
    let readme = fs::read_to_string(root.join("README.md")).expect("read README.md");
    let symbol_script = fs::read_to_string(root.join("scripts/check-public-symbols.py"))
        .expect("read scripts/check-public-symbols.py");
    let abi_diff_script =
        fs::read_to_string(root.join("scripts/check-abi-diff.py")).expect("read abi diff script");
    let classification_tsv =
        fs::read_to_string(root.join("symbols/public-symbol-classification.tsv"))
            .expect("read public symbol classification");

    for heading in [
        "## Public C Header Symbols",
        "## Malloc-Family Interposition Symbols",
        "## C++ Operator Symbols",
        "## Extension ABI Naming Rules",
        "## Static Archive Internal-Symbol Policy",
        "## Release-to-Release ABI Diff Expectations",
        "## Required Checks",
    ] {
        assert!(
            policy.contains(heading),
            "symbol versioning policy should include {heading}"
        );
    }

    for header in [
        "mimalloc-3.3.2/include/mimalloc.h",
        "mimalloc-3.3.2/include/mimalloc-stats.h",
    ] {
        assert!(
            policy.contains(header) && symbol_script.contains(header),
            "symbol policy should document public header source {header}"
        );
    }

    for script_constant in [
        "MALLOC_OVERRIDE_SYMBOLS",
        "LIBC_OVERRIDE_SYMBOLS",
        "MALLOC_FAMILY_SYMBOLS",
        "CXX_OPERATOR_OVERRIDE_SYMBOLS",
    ] {
        assert!(
            policy.contains(script_constant) && symbol_script.contains(script_constant),
            "symbol policy should document script-controlled symbol group {script_constant}"
        );
    }

    for representative_symbol in [
        "malloc",
        "calloc",
        "realloc",
        "free",
        "posix_memalign",
        "__libc_malloc",
        "__posix_memalign",
        "_Znwm",
        "_Znam",
        "_ZdlPv",
        "_ZdaPv",
    ] {
        assert!(
            policy.contains(representative_symbol) && symbol_script.contains(representative_symbol),
            "symbol policy should name representative checked symbol {representative_symbol}"
        );
    }

    for classification in [
        "internal-export-required",
        "rust-extension",
        "platform-helper-extension",
    ] {
        assert!(
            policy.contains(classification) && classification_tsv.contains(classification),
            "symbol policy should define TSV classification {classification}"
        );
    }

    for manifest in [
        "c-abi-dynamic.tsv",
        "c-abi-secure-dynamic.tsv",
        "c-abi-guarded-dynamic.tsv",
        "c-abi-track-valgrind-dynamic.tsv",
        "c-abi-track-asan-dynamic.tsv",
        "interpose-dynamic.tsv",
    ] {
        assert!(
            policy.contains(manifest) && root.join("docs/abi-manifests").join(manifest).exists(),
            "symbol policy should list checked ABI manifest {manifest}"
        );
    }

    for command_or_marker in [
        "scripts/check-public-symbols.py --dynamic-mi",
        "scripts/check-public-symbols.py --dynamic-groups",
        "scripts/check-public-symbols.py --dynamic-manifests",
        "scripts/check-public-symbols.py --write-dynamic-manifests",
        "scripts/check-abi-symbols.sh",
        "scripts/check-abi-diff.py --previous-ref <previous-tag>",
        "scripts/check-abi-diff.py --previous-dir <previous-release>/abi-manifests",
        "ABI-CHANGE: <manifest> <symbol>",
        "ABI-CHANGE: <manifest> *",
    ] {
        assert!(
            policy.contains(command_or_marker),
            "symbol policy should document release command or marker {command_or_marker}"
        );
    }
    assert!(
        abi_diff_script.contains("visibility-changed")
            && abi_diff_script.contains("binding-changed")
            && abi_diff_script.contains("version-changed")
            && abi_diff_script.contains("type-changed")
            && policy.contains("visibility-changed")
            && policy.contains("binding-changed")
            && policy.contains("version-changed")
            && policy.contains("type-changed"),
        "symbol policy should match ABI diff classifications"
    );
    assert!(
        policy.contains("mechanical_mimalloc_*")
            && policy.contains("There is no supported extension ABI in the current release policy")
            && policy.contains("Static archives are preview-only")
            && abi_doc.contains("docs/symbol-versioning.md")
            && release_doc.contains("docs/symbol-versioning.md")
            && readme.contains("docs/symbol-versioning.md"),
        "symbol policy should be linked and should define extension/static policy"
    );
}
