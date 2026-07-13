use std::fs;
use std::path::Path;

#[test]
fn compatibility_policy_is_concrete_pure_and_source_enforced() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let module =
        fs::read_to_string(root.join("src/allocator/policy/mod.rs")).expect("read policy module");
    let compat = fs::read_to_string(root.join("src/allocator/policy/compat.rs"))
        .expect("read compatibility policy");
    let decisions = fs::read_to_string(root.join("src/allocator/policy/decisions.rs"))
        .expect("read policy decisions");
    let checker = fs::read_to_string(root.join("scripts/check-policy-boundaries.py"))
        .expect("read policy checker");

    assert!(
        compat.contains("pub(crate) struct CompatPolicy;")
            && module.contains("pub(crate) type ActivePolicy = CompatPolicy;")
            && !compat.contains("dyn ")
            && !decisions.contains("dyn "),
        "the active compatibility policy must remain a zero-sized concrete type without dynamic dispatch"
    );
    for decision in [
        "ThreadHeapPolicy",
        "PageFullRetain",
        "PageExtendCount",
        "GenericMaintenanceInterval",
        "RemoteCollectMode",
        "ReclaimContext",
        "ReclaimDecision",
    ] {
        assert!(
            decisions.contains(decision),
            "missing typed policy result {decision}"
        );
    }
    for option in [
        "mi_option_generic_collect",
        "mi_option_page_reclaim_on_free",
        "mi_option_page_full_retain",
        "mi_option_page_max_candidates",
        "mi_option_page_max_reclaim",
        "mi_option_page_cross_thread_max_reclaim",
    ] {
        assert!(
            checker.contains(option),
            "policy source checker must cover {option}"
        );
    }
}
