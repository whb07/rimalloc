use super::super::DatatypeAcceptanceSources;

pub(super) fn assert_page_thread_and_free_bit_domains(
    audit_sources: &DatatypeAcceptanceSources<'_>,
) {
    let page_core_rs = audit_sources.source("src/types/page.rs");
    let atomic_page_thread_rs = audit_sources.source("src/types/atomic/page_thread.rs");
    let atomic_thread_free_rs = audit_sources.source("src/types/atomic/thread_free.rs");
    let strict_check_rs = audit_sources.strict_type_checker;
    assert!(
        page_core_rs.contains("pub(crate) struct PageFlags(usize);")
            && page_core_rs.contains("pub(crate) const fn new(bits: usize) -> Option<Self>")
            && page_core_rs.contains("if (bits & !MI_PAGE_FLAG_MASK) == 0")
            && page_core_rs.contains("pub(crate) struct PageThreadId(usize);")
            && page_core_rs.contains("pub(crate) fn from_parts(owner: PageOwnerId, flags: PageFlags) -> Self")
            && page_core_rs.contains("Self(tid_val | flags.bits())")
            && page_core_rs.contains("pub(crate) fn flags(self) -> PageFlags")
            && page_core_rs.contains("pub(crate) fn owner_bits(self) -> usize")
            && page_core_rs.contains("self.0 & !MI_PAGE_FLAG_MASK")
            && page_core_rs.contains("pub(crate) fn classify_free_path(self, current: ThreadId) -> FreePathKind")
            && atomic_page_thread_rs
                .contains("pub(crate) struct AtomicPageThreadId(AtomicUsizeField);")
            && atomic_page_thread_rs
                .contains("pub(crate) fn from_field(field: AtomicUsizeField) -> Self")
            && atomic_page_thread_rs.contains("pub(crate) unsafe fn load(self) -> PageThreadId")
            && atomic_page_thread_rs.contains("pub(crate) unsafe fn set_flags(self, flags: PageFlags, set: bool) -> PageThreadId")
            && atomic_page_thread_rs.contains("unsafe { self.0.fetch_or(flags.bits(), Ordering::Relaxed) }")
            && atomic_page_thread_rs.contains("unsafe { self.0.fetch_and(!flags.bits(), Ordering::Relaxed) }")
            && atomic_page_thread_rs.contains("pub(crate) unsafe fn store_owner(self, owner: PageOwnerId)")
            && atomic_page_thread_rs.contains("pub(crate) unsafe fn compare_exchange_owner(\n        self,\n        expected: &mut PageThreadId,")
            && strict_check_rs.contains("ALLOWED_XTHREAD_ID = {")
            && strict_check_rs.contains("\"src/types/page.rs\"")
            && strict_check_rs.contains("\"src/types/atomic.rs\"")
            && strict_check_rs.contains("\"src/types/atomic/page_thread.rs\"")
            && strict_check_rs.contains("kind=\"xthread-id-bitop\"")
            && strict_check_rs.contains("message=\"direct xthread_id bit operation outside typed helpers\""),
        "xthread_id bit operations should be isolated to PageThreadId/PageFlags/AtomicPageThreadId and enforced by the strict checker"
    );
    let freelist_rs = audit_sources.source("src/types/freelist.rs");
    assert!(
        freelist_rs.contains("pub(crate) struct ThreadFreeWord(usize);")
            && freelist_rs.contains("pub(crate) const fn from_raw(raw: usize) -> Self")
            && freelist_rs.contains("pub(crate) const OWNED_BIT: usize = 1;")
            && freelist_rs.contains("let ptr_val = self.0 & !Self::OWNED_BIT;")
            && freelist_rs.contains("use super::constants::mi_block_t;")
            && freelist_rs.contains("pub(crate) fn head_raw(self) -> *mut mi_block_t")
            && freelist_rs.contains("pub(crate) const fn is_owned(self) -> bool")
            && freelist_rs.contains("(self.0 & Self::OWNED_BIT) != 0")
            && freelist_rs.contains(
                "pub(crate) fn from_parts(head: Option<BlockPtr>, owned: ThreadFreeOwner) -> Self"
            )
            && freelist_rs
                .contains("crate::internal::mi_assert_internal!((val & Self::OWNED_BIT) == 0);")
            && freelist_rs.contains("Self(ptr_val | bit)")
            && atomic_thread_free_rs.contains("ThreadFreeWord::from_raw(")
            && atomic_thread_free_rs
                .contains("pub(crate) struct AtomicThreadFree(AtomicUsizeField);")
            && atomic_thread_free_rs
                .contains("pub(crate) fn from_field(field: AtomicUsizeField) -> Self")
            && atomic_thread_free_rs.contains("pub(crate) fn load_relaxed(self) -> ThreadFreeWord")
            && atomic_thread_free_rs.contains("pub(crate) fn push_remote_acq_rel(")
            && atomic_thread_free_rs.contains(
                "pub(crate) unsafe fn release_acq_rel(self, expected: &mut ThreadFreeWord) -> bool"
            )
            && atomic_thread_free_rs
                .contains("pub(crate) unsafe fn collect_acq_rel(self) -> RemoteFreeList")
            && atomic_thread_free_rs.contains("pub(crate) unsafe fn claim_acq_rel(self) -> bool")
            && strict_check_rs.contains("ALLOWED_XTHREAD_FREE = {")
            && strict_check_rs.contains("\"src/types/freelist.rs\"")
            && strict_check_rs.contains("\"src/types/atomic.rs\"")
            && strict_check_rs.contains("\"src/types/atomic/thread_free.rs\"")
            && strict_check_rs.contains("kind=\"xthread-free-bitop\"")
            && strict_check_rs
                .contains("message=\"direct xthread_free bit operation outside typed helpers\""),
        "xthread_free bit operations should be isolated to ThreadFreeWord/AtomicThreadFree and enforced by the strict checker"
    );
}
