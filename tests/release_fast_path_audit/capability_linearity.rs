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

#[test]
fn raw_bridge_and_remote_list_ownership_are_enforced() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let states =
        fs::read_to_string(root.join("src/types/page_states.rs")).expect("read page-state source");
    let remote = fs::read_to_string(root.join("src/types/atomic/thread_free.rs"))
        .expect("read remote-list source");
    let lifecycle = fs::read_to_string(root.join("src/allocator/page/lifecycle.rs"))
        .expect("read page lifecycle source");
    let bridge =
        fs::read_to_string(root.join("src/types/raw_bridge.rs")).expect("read raw bridge source");
    let checker = fs::read_to_string(root.join("scripts/check-strict-types.py"))
        .expect("read strict-type checker");

    assert!(
        states.contains("pub(in crate::types) unsafe fn new_unchecked(page: PagePtr) -> Self")
            && states
                .contains("pub(in crate::types) unsafe fn new_unchecked(ptr: PagePtr) -> Self")
            && bridge.contains("pub(crate) unsafe fn assume_uninit_page")
            && checker.contains("CAPABILITY_CONSTRUCTOR_RE"),
        "page capability construction must remain private to types and allocator ingress must use the audited bridge"
    );
    assert!(
        remote.contains("pub(crate) struct PublishedRemoteHead(BlockPtr);")
            && remote.contains("pub(crate) struct DetachedRemoteList(Option<BlockPtr>);")
            && remote.contains("pub(crate) unsafe fn detach_tail(")
            && remote.contains("pub(crate) unsafe fn detach_head(self) -> DetachedRemoteList")
            && checker.contains("DetachedRemoteList"),
        "published and detached remote chains must remain distinct and source-enforced"
    );
    assert!(
        lifecycle.contains("collect_remote_for_local_page(")
            && lifecycle.contains("collect_remote_for_owned_abandoned_page(")
            && !lifecycle
                .split("pub(crate) unsafe fn collect_remote_for_local_page")
                .nth(1)
                .expect("local collector")
                .split("pub(crate) unsafe fn collect_remote_for_owned_abandoned_page")
                .next()
                .expect("local collector body")
                .contains("OwnedAbandonedPage"),
        "local detached-list collection must not fabricate abandoned ownership"
    );
}

#[test]
fn fallible_page_transitions_return_the_original_capability() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let states =
        fs::read_to_string(root.join("src/types/page_states.rs")).expect("read page-state source");
    let queues =
        fs::read_to_string(root.join("src/types/page_queues.rs")).expect("read page-queue source");

    for signature in [
        "Result<AllFreePage<Active>, ActivePage>",
        "Result<AllFreePage<Full>, FullPage>",
        "Result<AllFreePage<Abandoned>, OwnedAbandonedPage>",
        "Result<OwnedAbandonedPage, AbandonedPageType>",
    ] {
        assert!(
            states.contains(signature),
            "missing linear failure signature: {signature}"
        );
    }
    assert!(
        queues.contains("Result<Self, crate::core_types::ActivePage>")
            && queues.contains("Result<Self, crate::core_types::FullPage>"),
        "failed queue-membership checks must return the original page capability"
    );
}
