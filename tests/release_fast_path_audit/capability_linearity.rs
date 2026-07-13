use std::fs;
use std::path::Path;

#[test]
fn validation_and_block_capabilities_are_linear() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let validation = fs::read_to_string(root.join("src/types/validation.rs"))
        .expect("read validation capability source");
    let validation_extra = fs::read_to_string(root.join("src/types/validation_extra.rs"))
        .expect("read block capability source");
    let checker = fs::read_to_string(root.join("scripts/check-strict-types.py"))
        .expect("read strict-type checker");
    let inventory = fs::read_to_string(root.join("docs/capability-inventory.md"))
        .expect("read capability inventory");

    for capability in [
        "GuardedBlock",
        "PointerLocation",
        "LocatedBlock",
        "ValidPointerLookup",
        "BlockForFree",
        "ValidatedFree",
        "ValidatedFreeParts",
    ] {
        assert!(
            validation.contains(capability)
                && inventory.contains(&format!("`{capability}`"))
                && checker.contains(&format!("\"{capability}\"")),
            "{capability} should be inventoried and enforced as a linear capability"
        );
    }
    for capability in ["AllocatedBlock", "FreeBlock"] {
        assert!(
            validation_extra.contains(&format!("struct {capability}"))
                && inventory.contains(&format!("`{capability}`"))
                && checker.contains(&format!("\"{capability}\"")),
            "{capability} should be inventoried and enforced as a linear capability"
        );
    }
    assert!(
        validation.contains("pub(crate) fn block(&self) -> &BlockForFree")
            && validation.contains("pub(crate) fn into_parts(self) -> ValidatedFreeParts")
            && validation_extra.contains("pub(crate) fn into_ptr(self) -> BlockPtr"),
        "inspection should borrow capabilities and ownership transfer should consume them"
    );
}

#[test]
fn page_and_queue_capabilities_are_linear() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let checker = fs::read_to_string(root.join("scripts/check-strict-types.py"))
        .expect("read strict-type checker");
    let inventory = fs::read_to_string(root.join("docs/capability-inventory.md"))
        .expect("read capability inventory");

    for capability in [
        "ActivePage",
        "FullPage",
        "RetiredPage",
        "AvailablePage",
        "LocalPage",
        "ForeignPage",
        "AbandonedPageType",
        "AbandonedMappedPageType",
        "OwnedAbandonedPage",
        "DestroyedPage",
        "AllFreePage",
        "PageForAbandon",
        "PageAbandonOutcome",
        "PageFreeOwner",
        "QueuedPage",
        "UnqueuedPage",
    ] {
        assert!(
            inventory.contains(&format!("`{capability}"))
                && checker.contains(&format!("\"{capability}\"")),
            "{capability} should be inventoried and enforced as a linear capability"
        );
    }
}

#[test]
fn capability_inventory_records_thread_affinity_and_atomic_snapshot_exceptions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let inventory = fs::read_to_string(root.join("docs/capability-inventory.md"))
        .expect("read capability inventory");
    assert!(
        inventory.contains("`ThreadHeapLocal` is intentionally an identity")
            && inventory.contains("`ThreadFreeWord` is a value snapshot"),
        "copyable exceptions should have explicit ownership rationale"
    );
}
