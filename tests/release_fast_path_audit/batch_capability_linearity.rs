use std::fs;
use std::path::Path;

#[test]
fn free_list_and_batch_ownership_is_linear_and_source_enforced() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let inventory = fs::read_to_string(root.join("docs/capability-inventory.md"))
        .expect("read capability inventory");
    let checker = fs::read_to_string(root.join("scripts/check-strict-types.py"))
        .expect("read strict-type checker");
    let wrappers = fs::read_to_string(root.join("src/types/freelist_wrappers.rs"))
        .expect("read free-list wrappers");
    let batches = fs::read_to_string(root.join("src/types/page_batch.rs"))
        .expect("read batch capability source");

    for capability in [
        "AvailableFreeList",
        "LocalFreeList",
        "RemoteFreeBatch",
        "LocalFreeBatch",
        "LeasedBatch",
    ] {
        assert!(
            inventory.contains(&format!("`{capability}`"))
                && checker.contains(&format!("\"{capability}\"")),
            "{capability} should be inventoried and enforced as linear"
        );
    }
    for (source, declaration) in [
        (
            wrappers.as_str(),
            "#[must_use = \"available free-list ownership must be stored or consumed\"]\npub(crate) struct AvailableFreeList",
        ),
        (
            wrappers.as_str(),
            "#[must_use = \"local free-list ownership must be stored or consumed\"]\npub(crate) struct LocalFreeList",
        ),
        (
            batches.as_str(),
            "#[must_use = \"remote free batches must be collected or republished\"]\npub(crate) struct RemoteFreeBatch",
        ),
        (
            batches.as_str(),
            "#[must_use = \"local free batches must be returned to page availability\"]\npub(crate) struct LocalFreeBatch",
        ),
        (
            batches.as_str(),
            "#[must_use = \"leased batches must be allocated from or returned to their page\"]\npub(crate) struct LeasedBatch",
        ),
    ] {
        assert!(
            source.contains(declaration),
            "linear owner should retain its must-use declaration: {declaration}"
        );
    }
    assert!(
        !checker.contains("\"src/types/page_batch.rs\",")
            && !batches.contains("ActivePage::new_unchecked"),
        "batch code must use page-state transitions instead of a broad constructor exception"
    );
}

#[test]
fn allocation_paths_do_not_use_stage_three_lease_primitives() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for path in [
        "src/allocator/alloc.rs",
        "src/allocator/alloc/public_malloc.rs",
        "src/allocator/free.rs",
    ] {
        let source = fs::read_to_string(root.join(path)).expect("read allocation path source");
        assert!(
            !source.contains("LeasedBatch") && !source.contains(".lease("),
            "Stage 3 lease primitives must remain unwired from {path}"
        );
    }
}
