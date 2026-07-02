use super::DatatypeAcceptanceSources;

pub(super) fn assert_padding_and_page_state_acceptance(
    audit_sources: &DatatypeAcceptanceSources<'_>,
) {
    let validation_extra_rs = audit_sources.source("src/types/validation_extra.rs");
    let page_states_rs = audit_sources.source("src/types/page_states.rs");

    let padding_rs = audit_sources.source("src/types/padding.rs");
    for (audit_id, minimum) in [
        ("typed-page-uninit-initialization-boundary", 2),
        ("typed-page-witness-constructor-boundary", 1),
        ("typed-page-all-free-witness-boundary", 3),
        ("typed-page-active-transition-boundary", 6),
        ("typed-page-full-transition-boundary", 2),
        ("typed-page-available-pop-boundary", 3),
        ("typed-page-retire-destroy-boundary", 2),
        ("typed-page-abandoned-claim-boundary", 2),
        ("typed-page-remote-free-classification-boundary", 2),
        ("typed-page-abandoned-reclaim-boundary", 2),
        ("typed-thread-heap-local-witness-boundary", 3),
        ("typed-page-free-owner-classification-boundary", 8),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        let count = page_states_rs.matches(&marker).count();
        assert!(
            count >= minimum,
            "page-state audit boundary {audit_id} should retain at least {minimum} local operation markers; found {count}"
        );
    }
    assert!(
        !page_states_rs.contains("SAFETY[AUDIT:typed-page-state-domain]"),
        "page-state typestate operations should stay split into focused audit IDs"
    );

    assert!(
        padding_rs.contains("pub(crate) struct PaddingCanary(u32);")
            && padding_rs.contains("pub(crate) struct PaddingDelta(u32);")
            && !padding_rs.contains("pub(crate) struct PaddingCanary(pub(crate) u32);")
            && !padding_rs.contains("pub(crate) struct PaddingDelta(pub(crate) u32);")
            && padding_rs.contains("pub(crate) const fn get(self) -> u32"),
        "padding scalar witnesses should keep backing fields private behind accessors"
    );
    assert!(
        padding_rs.contains(
            "pub(crate) struct Padding {\n    canary: PaddingCanary,\n    delta: PaddingDelta,\n}"
        ) && padding_rs.contains(
            "pub(crate) const fn new(canary: PaddingCanary, delta: PaddingDelta) -> Self"
        ) && padding_rs.contains("pub(crate) const fn canary(self) -> PaddingCanary")
            && padding_rs.contains("pub(crate) const fn delta(self) -> PaddingDelta"),
        "Padding should keep aggregate fields private behind constructor/accessors"
    );
    for witness in ["AllocatedBlock", "FreeBlock"] {
        assert!(
            validation_extra_rs.contains(&format!("pub(crate) struct {witness}(BlockPtr);")),
            "{witness} should keep its block pointer field private"
        );
    }
    assert!(
        validation_extra_rs.contains("pub(crate) unsafe fn new_unchecked(block: BlockPtr) -> Self")
            && validation_extra_rs
                .contains("pub(in crate::types) unsafe fn new_unchecked(block: BlockPtr) -> Self")
            && validation_extra_rs.contains("pub(crate) fn block(self) -> BlockPtr"),
        "allocated/free block witnesses should expose constructors only at the needed boundary"
    );
    for witness in ["AllocatedBlock", "FreeBlock"] {
        assert!(
            !validation_extra_rs.contains(&format!(
                "pub(crate) struct {witness}(pub(crate) BlockPtr);"
            )),
            "{witness} should not expose tuple construction outside validation_extra"
        );
    }

    for (rel, source) in audit_sources.rust_sources {
        if rel.ends_with("/tests.rs")
            || rel == "src/types/atomic/concurrency_models.rs"
            || rel == "src/types/layout_tests.rs"
        {
            continue;
        }

        if rel != "src/types/memid.rs" && rel != "src/types/constants.rs" {
            for raw_union_arm in ["mem.os", "mem.arena", "mem.meta"] {
                assert!(
                    !source.contains(raw_union_arm),
                    "{rel} should inspect memid union arms through MemId::view, not {raw_union_arm}"
                );
            }
        }

        if rel != "src/types/atomic.rs" {
            for atomic_cast in [
                "AtomicUsize::from_ptr",
                "cast::<AtomicUsize>",
                "cast::<core::sync::atomic::AtomicUsize>",
            ] {
                assert!(
                    !source.contains(atomic_cast),
                    "{rel} should route pointer-to-atomic casts through core_types::Atomic*Field"
                );
            }
        }

        if rel.starts_with("src/types/") {
            for deref_impl in ["impl core::ops::Deref", "impl Deref", "Deref for"] {
                assert!(
                    !source.contains(deref_impl),
                    "{rel} should keep allocator metadata wrappers raw-pointer based, without Deref"
                );
            }
        }

        if rel != "src/types/freelist_wrappers.rs" {
            for block_next_access in [
                "(*block).next",
                "(*block.cast_mut()).next",
                "(*block.as_ptr()).next",
            ] {
                assert!(
                    !source.contains(block_next_access),
                    "{rel} should read/write mi_block_s.next through BlockLink, EncodedPtr, or free-list wrappers"
                );
            }
        }

        if rel != "src/types/padding.rs" && rel != "src/types/constants.rs" {
            for raw_padding_access in [
                "mi_padding_t",
                "(*padding).canary",
                "(*padding).delta",
                "cast::<mi_padding_t>",
            ] {
                assert!(
                    !source.contains(raw_padding_access),
                    "{rel} should route padding metadata access through core_types::PaddingPtr"
                );
            }
        }
    }
}
