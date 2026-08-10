use super::*;
use crate::core_types::AvailablePage;
use crate::types::{mi_block_t, mi_page_t};

const SYNTHETIC_CAPACITY: usize = 8;

struct SyntheticPage {
    page_storage: *mut mi_page_t,
    blocks: *mut mi_block_t,
    page: Option<PagePtr>,
    used: u16,
}

impl SyntheticPage {
    fn new(used: u16) -> Self {
        // SAFETY: allocator-backed
        // fixture storage stays in the allocator heap region required by debug
        // encoded-link checks and is released exactly once by Drop.
        unsafe {
            let page_storage: *mut mi_page_t =
                crate::mimalloc::mi_zalloc(core::mem::size_of::<mi_page_t>()).cast();
            let blocks: *mut mi_block_t =
                crate::mimalloc::mi_zalloc(SYNTHETIC_CAPACITY * core::mem::size_of::<mi_block_t>())
                    .cast();
            if page_storage.is_null() || blocks.is_null() {
                crate::mimalloc::mi_free(blocks.cast());
                crate::mimalloc::mi_free(page_storage.cast());
                panic!("synthetic page allocation failed");
            }
            Self {
                page_storage,
                blocks,
                page: None,
                used,
            }
        }
    }

    fn initialize(&mut self) {
        let raw_page = self.page_storage;
        // SAFETY: allocator-backed
        // page and block storage have stable addresses for the fixture lifetime.
        let page = unsafe { PagePtr::new_unchecked(raw_page) };
        // SAFETY: writes initialize the
        // exact metadata fields read by batch traversal and accounting.
        unsafe {
            (*raw_page).used = self.used;
            (*raw_page).capacity = SYNTHETIC_CAPACITY as u16;
            (*raw_page).reserved = SYNTHETIC_CAPACITY as u16;
            (*raw_page).block_size = core::mem::size_of::<mi_block_t>();
            (*raw_page).page_start = self.blocks.cast();
            page.set_keys(crate::core_types::PageKeys::disabled());
        }
        self.page = Some(page);
    }

    fn page(&self) -> PagePtr {
        self.page
            .expect("synthetic page initialized after placement")
    }

    fn block(&mut self, slot: usize) -> BlockPtr {
        // SAFETY: the bounded slot points
        // into stable allocator-backed block storage for this synthetic page.
        unsafe { BlockPtr::new_unchecked(self.blocks.add(slot)) }
    }

    unsafe fn link(&mut self, indexes: &[usize]) {
        let page = self.page();
        for pair in indexes.windows(2) {
            // SAFETY: both indexes are
            // blocks in this page and use its disabled fixture keys.
            unsafe {
                self.block(pair[0])
                    .set_next(page, Some(self.block(pair[1])))
            };
        }
        if let Some(&tail) = indexes.last() {
            // SAFETY: the selected tail is
            // writable block storage in this synthetic page.
            unsafe { self.block(tail).set_next(page, None) };
        }
    }

    unsafe fn order_from(&self, head: Option<BlockPtr>) -> ListOrder {
        let base = self.blocks as usize;
        let stride = core::mem::size_of::<mi_block_t>();
        let mut order = ListOrder::empty();
        let mut current = head;
        while let Some(block) = current {
            order.push((block.as_ptr() as usize - base) / stride);
            // SAFETY: the synthetic list
            // contains only fixture blocks and is capacity bounded.
            current = unsafe { block.next(self.page()) };
        }
        order
    }

    fn head_index(&self, head: Option<BlockPtr>) -> Option<usize> {
        let base = self.blocks as usize;
        let stride = core::mem::size_of::<mi_block_t>();
        head.map(|block| (block.as_ptr() as usize - base) / stride)
    }

    unsafe fn next_snapshot(&mut self) -> [Option<usize>; SYNTHETIC_CAPACITY] {
        let base = self.blocks as usize;
        let stride = core::mem::size_of::<mi_block_t>();
        let mut snapshot = [None; SYNTHETIC_CAPACITY];
        for (slot, next_slot) in snapshot.iter_mut().enumerate() {
            // SAFETY: every
            // fixture block is readable with the synthetic page's keys.
            *next_slot = unsafe { self.block(slot).next(self.page()) }
                .map(|next| (next.as_ptr() as usize - base) / stride);
        }
        snapshot
    }
}

impl Drop for SyntheticPage {
    fn drop(&mut self) {
        // SAFETY: both pointers were
        // returned by mi_zalloc and remain uniquely owned by this fixture.
        unsafe {
            crate::mimalloc::mi_free(self.blocks.cast());
            crate::mimalloc::mi_free(self.page_storage.cast());
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ListOrder {
    indexes: [usize; SYNTHETIC_CAPACITY],
    len: usize,
}

impl ListOrder {
    const fn empty() -> Self {
        Self {
            indexes: [0; SYNTHETIC_CAPACITY],
            len: 0,
        }
    }

    fn from_slices(first: &[usize], second: &[usize]) -> Self {
        let mut order = Self::empty();
        for &index in first.iter().chain(second) {
            order.push(index);
        }
        order
    }

    fn push(&mut self, ordinal: usize) {
        assert!(self.len < SYNTHETIC_CAPACITY);
        self.indexes[self.len] = ordinal;
        self.len += 1;
    }
}

fn next_random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

fn shuffled_indexes(state: &mut u64) -> [usize; SYNTHETIC_CAPACITY] {
    let mut indexes = core::array::from_fn(|index| index);
    for end in (1..SYNTHETIC_CAPACITY).rev() {
        let selected = next_random(state) as usize % (end + 1);
        indexes.swap(end, selected);
    }
    indexes
}

unsafe fn reference_prepend_local_to_available(page: PagePtr) {
    // SAFETY: reference fixtures
    // contain valid null-terminated page-local chains.
    let local = unsafe { page.local_free() };
    let Some(mut tail) = local.head() else {
        return;
    };
    loop {
        // SAFETY: tail remains in
        // the bounded reference local chain.
        let Some(next) = (unsafe { tail.next(page) }) else {
            break;
        };
        tail = next;
    }
    // SAFETY: the reference tail
    // is writable and the destination is the same page's available list.
    unsafe {
        let available = page.free();
        tail.set_next(page, available.head());
        page.set_free(local.into_available());
        page.set_local_free(LocalFreeList::empty());
    }
}

unsafe fn reference_collect_remote_into_local(page: PagePtr, head: BlockPtr) {
    let mut tail = head;
    let mut count = 1u16;
    loop {
        // SAFETY: the reference
        // remote chain is valid, page-local, and capacity bounded.
        let Some(next) = (unsafe { tail.next(page) }) else {
            break;
        };
        tail = next;
        count += 1;
    }
    // SAFETY: the known tail and
    // current local head share this page's encoding domain.
    unsafe {
        let local = page.local_free();
        tail.set_next(page, local.head());
        page.set_local_free(LocalFreeList::from_head(head));
        page.set_used(UsedBlockCount::new(page.used().get() - count));
    }
}

unsafe fn reference_available_pop(page: PagePtr) -> BlockPtr {
    // SAFETY: the reference page
    // has a nonempty valid available chain and spare used capacity.
    unsafe {
        let available = page.free();
        let Some(head) = available.head() else {
            core::hint::unreachable_unchecked()
        };
        let next = head.next(page);
        page.set_free(match next {
            Some(next) => AvailableFreeList::from_head(next),
            None => AvailableFreeList::empty(),
        });
        page.set_used(page.used().increment_wrapping());
        head
    }
}

const MAX_PAGE_BLOCKS: usize = crate::types::constants::MI_SMALL_PAGE_SIZE / 16;
const MAX_TRACKED_BLOCKS: usize = MAX_PAGE_BLOCKS * 2;

struct SamePageBlocks {
    all: [*mut mi_block_t; MAX_TRACKED_BLOCKS],
    all_len: usize,
    page: PagePtr,
    selected: [Option<BlockPtr>; MAX_PAGE_BLOCKS],
    selected_len: usize,
}

impl SamePageBlocks {
    fn new(required: usize) -> Self {
        // SAFETY: allocations stay live
        // for the fixture, and page/block witnesses are used only with their
        // allocator-reported owning page.
        unsafe {
            assert!(required <= MAX_PAGE_BLOCKS);
            let mut all = [core::ptr::null_mut(); MAX_TRACKED_BLOCKS];
            let mut all_len = 0;
            let mut selected = [None; MAX_PAGE_BLOCKS];
            let mut selected_len = 0;
            let mut selected_page: *mut mi_page_t = core::ptr::null_mut();

            while selected_len < required {
                let block = crate::mimalloc::mi_malloc(16).cast::<mi_block_t>();
                assert!(!block.is_null());
                assert!(
                    all_len < MAX_TRACKED_BLOCKS,
                    "fixture allocation bound exceeded"
                );
                all[all_len] = block;
                all_len += 1;
                let page = crate::internal::_mi_ptr_page(block.cast());
                if selected_page.is_null() {
                    selected_page = page;
                }
                if page == selected_page {
                    selected[selected_len] = Some(BlockPtr::new_unchecked(block));
                    selected_len += 1;
                }
            }

            Self {
                all,
                all_len,
                page: PagePtr::new_unchecked(selected_page),
                selected,
                selected_len,
            }
        }
    }

    fn block(&self, slot: usize) -> BlockPtr {
        assert!(slot < self.selected_len);
        self.selected[slot].expect("selected block slot is initialized")
    }

    fn link_prefix(&self, len: usize) {
        // SAFETY: selected blocks belong
        // to this page and remain live for the fixture.
        unsafe {
            assert!(len > 0 && len <= self.selected_len);
            for slot in 1..len {
                self.block(slot - 1)
                    .set_next(self.page, Some(self.block(slot)));
            }
            self.block(len - 1).set_next(self.page, None);
        }
    }

    fn full_page() -> Self {
        // SAFETY: a dedicated heap keeps
        // unrelated parallel tests from redirecting same-size allocations to
        // another page before this fixture reaches the reported capacity.
        unsafe {
            let heap = crate::allocator::heap::mi_heap_new();
            assert!(!heap.is_null());
            let first = crate::allocator::alloc::mi_heap_malloc(heap, 16).cast::<mi_block_t>();
            assert!(!first.is_null());
            let page = crate::internal::_mi_ptr_page(first.cast());
            let mut fixture = Self {
                all: [core::ptr::null_mut(); MAX_TRACKED_BLOCKS],
                all_len: 1,
                page: PagePtr::new_unchecked(page),
                selected: [None; MAX_PAGE_BLOCKS],
                selected_len: 1,
            };
            fixture.all[0] = first;
            fixture.selected[0] = Some(BlockPtr::new_unchecked(first));

            loop {
                let capacity = fixture.page.capacity().as_usize();
                assert!(capacity <= MAX_PAGE_BLOCKS);
                if fixture.selected_len == capacity {
                    crate::mimalloc::mi_heap_delete(heap);
                    return fixture;
                }
                let block = crate::allocator::alloc::mi_heap_malloc(heap, 16).cast::<mi_block_t>();
                assert!(!block.is_null());
                assert!(
                    fixture.all_len < MAX_TRACKED_BLOCKS,
                    "fixture allocation bound exceeded"
                );
                fixture.all[fixture.all_len] = block;
                fixture.all_len += 1;
                assert_eq!(
                    crate::internal::_mi_ptr_page(block.cast()),
                    fixture.page.as_ptr(),
                    "dedicated heap changed page before capacity"
                );
                fixture.selected[fixture.selected_len] = Some(BlockPtr::new_unchecked(block));
                fixture.selected_len += 1;
            }
        }
    }
}

impl Drop for SamePageBlocks {
    fn drop(&mut self) {
        // SAFETY: every pointer was
        // returned by mi_malloc and is freed exactly once at fixture teardown.
        unsafe {
            for &block in &self.all[..self.all_len] {
                crate::mimalloc::mi_free(block.cast());
            }
        }
    }
}

#[test]
fn scans_basic_detached_chain_lengths_exactly() {
    // SAFETY: the fixture owns each block
    // and supplies the allocator-reported page and capacity.
    unsafe {
        let fixture = SamePageBlocks::new(16);
        let capacity = fixture.page.capacity();
        for len in [1usize, 2, 3, 4, 7, 8, 15, 16] {
            fixture.link_prefix(len);
            let core = scan_detached_chain(fixture.page, fixture.block(0), capacity)
                .expect("valid detached chain");
            assert_eq!(core.page(), fixture.page);
            assert_eq!(core.head(), fixture.block(0));
            assert_eq!(core.tail(), fixture.block(len - 1));
            assert_eq!(core.len().get() as usize, len);
            assert!(core.tail().next(fixture.page).is_none());

            let mut current = Some(core.head());
            for index in 0..len {
                let expected = fixture.block(index);
                assert_eq!(current, Some(expected));
                current = current.and_then(|block| block.next(fixture.page));
            }
            assert!(current.is_none());
        }
    }
}

#[test]
fn trusted_and_validating_scans_match_for_valid_chains() {
    // SAFETY: the fixture owns every
    // block and supplies a valid page-homogeneous chain to both scanners.
    unsafe {
        let fixture = SamePageBlocks::new(16);
        let capacity = fixture.page.capacity();
        for len in [1usize, 2, 4, 8, 16] {
            fixture.link_prefix(len);
            let validating = scan_detached_chain(fixture.page, fixture.block(0), capacity)
                .expect("validating scan");
            let trusted = scan_trusted_detached_chain(fixture.page, fixture.block(0), capacity)
                .expect("trusted scan");
            assert_eq!(trusted.page(), validating.page());
            assert_eq!(trusted.head(), validating.head());
            assert_eq!(trusted.tail(), validating.tail());
            assert_eq!(trusted.len(), validating.len());
        }
    }
}

#[test]
fn scans_capacity_boundary_lengths_exactly() {
    // SAFETY: the fixture owns a complete
    // page-capacity set of live blocks and page-local encoded links.
    unsafe {
        let fixture = SamePageBlocks::full_page();
        let capacity = fixture.page.capacity();
        assert!(capacity.get() > 1);
        for len in [capacity.as_usize() - 1, capacity.as_usize()] {
            fixture.link_prefix(len);
            let core = scan_detached_chain(fixture.page, fixture.block(0), capacity)
                .expect("capacity-bounded detached chain");
            assert_eq!(core.len().get() as usize, len);
            assert_eq!(core.tail(), fixture.block(len - 1));
            assert!(core.tail().next(fixture.page).is_none());
        }
    }
}

#[test]
fn capacity_bound_rejects_cycles_self_loops_and_duplicate_nodes() {
    // SAFETY: all cyclic links remain
    // within the live fixture page and traversal is capacity bounded.
    unsafe {
        let fixture = SamePageBlocks::new(3);
        let capacity = fixture.page.capacity();

        fixture
            .block(0)
            .set_next(fixture.page, Some(fixture.block(0)));
        assert_eq!(
            scan_detached_chain(fixture.page, fixture.block(0), capacity).unwrap_err(),
            BatchValidationError::TooLong {
                observed_at_least: capacity.as_usize() + 1,
                capacity,
            }
        );

        fixture
            .block(0)
            .set_next(fixture.page, Some(fixture.block(1)));
        fixture
            .block(1)
            .set_next(fixture.page, Some(fixture.block(2)));
        fixture
            .block(2)
            .set_next(fixture.page, Some(fixture.block(0)));
        assert!(matches!(
            scan_detached_chain(fixture.page, fixture.block(0), capacity),
            Err(BatchValidationError::TooLong {
                observed_at_least,
                ..
            }) if observed_at_least == capacity.as_usize() + 1
        ));

        fixture
            .block(0)
            .set_next(fixture.page, Some(fixture.block(1)));
        fixture
            .block(1)
            .set_next(fixture.page, Some(fixture.block(0)));
        assert!(matches!(
            scan_detached_chain(fixture.page, fixture.block(0), capacity),
            Err(BatchValidationError::TooLong { .. })
        ));
    }
}

#[test]
fn rejects_a_link_to_a_different_page_without_following_it() {
    // SAFETY: all allocations remain live;
    // page identity is checked before constructing the foreign link witness.
    unsafe {
        let first = SamePageBlocks::new(1);
        let foreign_raw = crate::mimalloc::mi_malloc(32768).cast::<mi_block_t>();
        assert!(!foreign_raw.is_null());
        assert_ne!(
            crate::internal::_mi_ptr_page(foreign_raw.cast()),
            first.page.as_ptr()
        );
        let foreign = BlockPtr::new_unchecked(foreign_raw);
        first.block(0).set_next(first.page, Some(foreign));
        assert_eq!(
            scan_detached_chain(first.page, first.block(0), first.page.capacity()).unwrap_err(),
            BatchValidationError::DifferentPage
        );
        crate::mimalloc::mi_free(foreign_raw.cast());
    }
}

#[test]
fn rejects_a_misaligned_in_page_link_without_following_it() {
    // SAFETY: the intentionally interior
    // pointer is only encoded and classified; the scan must not dereference it.
    unsafe {
        let fixture = SamePageBlocks::new(1);
        let interior = fixture.block(0).as_ptr().cast::<u8>().add(1).cast();
        let interior = BlockPtr::new_unchecked(interior);
        fixture.block(0).set_next(fixture.page, Some(interior));
        assert_eq!(
            scan_detached_chain(fixture.page, fixture.block(0), fixture.page.capacity())
                .unwrap_err(),
            BatchValidationError::InvalidLink
        );
    }
}

#[test]
fn zero_capacity_rejects_a_nonempty_chain_before_traversal() {
    // SAFETY: the live head is paired with
    // an artificial zero capacity specifically to prove pre-traversal rejection.
    unsafe {
        let fixture = SamePageBlocks::new(1);
        let capacity = PageCapacity::new_unchecked(0);
        assert_eq!(
            scan_detached_chain(fixture.page, fixture.block(0), capacity).unwrap_err(),
            BatchValidationError::TooLong {
                observed_at_least: 1,
                capacity,
            }
        );
    }
}

#[test]
fn empty_local_list_converts_to_no_batch_without_mutation() {
    let mut fixture = SyntheticPage::new(2);
    fixture.initialize();
    // SAFETY: the synthetic page has
    // an initialized empty local list and valid page metadata.
    unsafe {
        let owner = LocalPage::new_unchecked(fixture.page());
        assert!(
            owner
                .detach_local_free_batch()
                .expect("empty local list is valid")
                .is_none()
        );
        assert!(fixture.page().local_free().is_empty());
        assert_eq!(fixture.page().used().get(), 2);
    }
}

#[test]
fn batch_capabilities_are_linear_stack_values() {
    assert_eq!(
        core::mem::size_of::<RemoteFreeBatch>(),
        core::mem::size_of::<(PagePtr, BlockPtr, BlockPtr, BatchLen)>()
    );
}

#[test]
fn remote_batch_splices_destinations_and_subtracts_used_once() {
    for destination in [&[][..], &[3][..], &[3, 4, 5][..]] {
        let mut fixture = SyntheticPage::new(7);
        fixture.initialize();
        // SAFETY: all list indexes belong
        // to the stable synthetic page and each chain is uniquely owned.
        unsafe {
            fixture.link(&[0, 1, 2]);
            fixture.link(destination);
            let local = match destination.first() {
                Some(&head) => LocalFreeList::from_head(fixture.block(head)),
                None => LocalFreeList::empty(),
            };
            fixture.page().set_local_free(match destination.first() {
                Some(&head) => LocalFreeList::from_head(fixture.block(head)),
                None => LocalFreeList::empty(),
            });
            let capacity = fixture.page().capacity();
            let used = fixture.page().used();
            let page = fixture.page();
            let batch = RemoteFreeBatch::from_detached(page, fixture.block(0), capacity, used)
                .expect("valid synthetic remote batch");
            let owner = LocalPage::new_unchecked(page);
            let collected = batch.collect_into_local_page(&owner, local);

            let expected = ListOrder::from_slices(&[0, 1, 2], destination);
            assert_eq!(fixture.order_from(collected.head()), expected);
            assert_eq!(fixture.page().used().get(), 4);
            assert_eq!(fixture.page().local_free().as_raw(), collected.as_raw());
        }
    }
}

#[test]
fn local_batch_splices_destinations_without_changing_used() {
    for destination in [&[][..], &[3][..], &[3, 4, 5][..]] {
        let mut fixture = SyntheticPage::new(4);
        fixture.initialize();
        // SAFETY: all page metadata and
        // encoded links are confined to the stable synthetic fixture.
        unsafe {
            fixture.link(&[0, 1, 2]);
            fixture.link(destination);
            fixture
                .page()
                .set_local_free(LocalFreeList::from_head(fixture.block(0)));
            let available = match destination.first() {
                Some(&head) => AvailableFreeList::from_head(fixture.block(head)),
                None => AvailableFreeList::empty(),
            };
            let owner = LocalPage::new_unchecked(fixture.page());
            let batch = owner
                .detach_local_free_batch()
                .expect("valid local chain")
                .expect("nonempty local chain");
            assert!(fixture.page().local_free().is_empty());
            let available = batch.prepend_to_available(&owner, available);

            let expected = ListOrder::from_slices(&[0, 1, 2], destination);
            assert_eq!(fixture.order_from(available.head()), expected);
            assert_eq!(fixture.page().used().get(), 4);
        }
    }
}

#[test]
fn full_collection_matches_c_local_list_cases() {
    // Empty local list leaves the available head and page fields unchanged.
    let mut empty = SyntheticPage::new(4);
    empty.initialize();
    unsafe {
        empty.link(&[3, 4]);
        empty
            .page()
            .set_free(AvailableFreeList::from_head(empty.block(3)));
        (*empty.page_storage).free_is_zero = true;
        let links = empty.next_snapshot();
        let owner = LocalPage::new_unchecked(empty.page());
        crate::allocator::page::collect_page_free_lists_local(&owner, false);
        assert_eq!(empty.head_index(empty.page().free().head()), Some(3));
        assert!(empty.page().local_free().is_empty());
        assert_eq!(empty.page().used().get(), 4);
        assert_eq!(empty.page().capacity().get(), SYNTHETIC_CAPACITY as u16);
        assert!((*empty.page_storage).free_is_zero);
        assert_eq!(empty.next_snapshot(), links);
    }

    // Empty available list receives the exact local head without touching links.
    let mut transfer = SyntheticPage::new(4);
    transfer.initialize();
    unsafe {
        transfer.link(&[0, 1, 2]);
        transfer
            .page()
            .set_local_free(LocalFreeList::from_head(transfer.block(0)));
        (*transfer.page_storage).free_is_zero = true;
        let links = transfer.next_snapshot();
        let owner = LocalPage::new_unchecked(transfer.page());
        crate::allocator::page::collect_page_free_lists_local(&owner, false);
        assert_eq!(transfer.head_index(transfer.page().free().head()), Some(0));
        assert!(transfer.page().local_free().is_empty());
        assert_eq!(transfer.next_snapshot(), links);
        assert_eq!(transfer.page().used().get(), 4);
        assert_eq!(transfer.page().capacity().get(), SYNTHETIC_CAPACITY as u16);
        assert!(!(*transfer.page_storage).free_is_zero);
    }

    // A nonempty destination is untouched unless force is requested.
    for force in [false, true] {
        let mut fixture = SyntheticPage::new(4);
        fixture.initialize();
        unsafe {
            fixture.link(&[0, 1, 2]);
            fixture.link(&[3, 4]);
            fixture
                .page()
                .set_local_free(LocalFreeList::from_head(fixture.block(0)));
            fixture
                .page()
                .set_free(AvailableFreeList::from_head(fixture.block(3)));
            let owner = LocalPage::new_unchecked(fixture.page());
            crate::allocator::page::collect_page_free_lists_local(&owner, force);
            if force {
                assert!(fixture.page().local_free().is_empty());
                assert_eq!(
                    fixture.order_from(fixture.page().free().head()),
                    ListOrder::from_slices(&[0, 1, 2], &[3, 4])
                );
            } else {
                assert_eq!(
                    fixture.order_from(fixture.page().local_free().head()),
                    ListOrder::from_slices(&[0, 1, 2], &[])
                );
                assert_eq!(
                    fixture.order_from(fixture.page().free().head()),
                    ListOrder::from_slices(&[3, 4], &[])
                );
            }
            assert_eq!(fixture.page().used().get(), 4);
            assert_eq!(fixture.page().capacity().get(), SYNTHETIC_CAPACITY as u16);
        }
    }
}

#[test]
fn full_collection_preserves_local_and_abandoned_owner_state() {
    for abandoned in [false, true] {
        let mut fixture = SyntheticPage::new(3);
        fixture.initialize();
        unsafe {
            fixture.link(&[0, 1]);
            fixture
                .page()
                .set_local_free(LocalFreeList::from_head(fixture.block(0)));
            let thread_id = crate::core_types::ThreadId::new(crate::prim::_mi_prim_thread_id())
                .expect("current thread id is nonzero and unflagged");
            let page_owner = if abandoned {
                crate::core_types::PageOwnerId::Abandoned
            } else {
                crate::core_types::PageOwnerId::Thread(thread_id)
            };
            fixture.page().thread_id_cell().store_owner(page_owner);
            let owner_bits = fixture.page().thread_id_cell().load().owner_bits();
            (*fixture.page_storage).xthread_free = crate::core_types::ThreadFreeWord::from_parts(
                None,
                crate::core_types::ThreadFreeOwner::Owned,
            )
            .raw();

            if abandoned {
                let owner = OwnedAbandonedPage::new_unchecked(fixture.page());
                crate::allocator::page::collect_page_free_lists_abandoned(&owner, false);
            } else {
                let owner = LocalPage::new_unchecked(fixture.page());
                crate::allocator::page::collect_page_free_lists_local(&owner, false);
            }

            assert_eq!(
                fixture.page().thread_id_cell().load().owner_bits(),
                owner_bits
            );
            assert!(fixture.page().thread_free_cell().load_relaxed().is_owned());
            assert_eq!(fixture.head_index(fixture.page().free().head()), Some(0));
            assert!(fixture.page().local_free().is_empty());
        }
    }
}

#[cfg(feature = "secure")]
#[test]
fn empty_available_transfer_preserves_secure_encoded_links() {
    let mut fixture = SyntheticPage::new(3);
    fixture.initialize();
    unsafe {
        fixture
            .page()
            .set_keys(crate::core_types::PageKeys::from_raw([
                0x9e37_79b9_7f4a_7c15,
                0xd1b5_4a32_d192_ed03,
            ]));
        fixture.link(&[0, 1, 2]);
        fixture
            .page()
            .set_local_free(LocalFreeList::from_head(fixture.block(0)));
        let encoded_links = fixture.next_snapshot();
        let owner = LocalPage::new_unchecked(fixture.page());
        crate::allocator::page::collect_page_free_lists_local(&owner, false);
        assert_eq!(fixture.next_snapshot(), encoded_links);
        assert_eq!(
            fixture.order_from(fixture.page().free().head()),
            ListOrder::from_slices(&[0, 1, 2], &[])
        );
    }
}

#[test]
fn abandoned_owner_detaches_and_splices_local_batch_without_local_proof() {
    let mut fixture = SyntheticPage::new(4);
    fixture.initialize();
    // SAFETY: this fixture models a
    // uniquely claimed abandoned page; its local and available chains are
    // page-homogeneous and no other collector can observe them.
    unsafe {
        fixture.link(&[0, 1, 2]);
        fixture.link(&[3, 4]);
        fixture
            .page()
            .set_local_free(LocalFreeList::from_head(fixture.block(0)));
        let available = AvailableFreeList::from_head(fixture.block(3));
        let owner = OwnedAbandonedPage::new_unchecked(fixture.page());
        let batch = owner
            .detach_local_free_batch()
            .expect("valid abandoned local chain")
            .expect("nonempty abandoned local chain");
        let available = batch.prepend_to_owned_abandoned_page(&owner, available);

        assert!(fixture.page().local_free().is_empty());
        assert_eq!(
            fixture.order_from(available.head()),
            ListOrder::from_slices(&[0, 1, 2], &[3, 4])
        );
        assert_eq!(fixture.page().used().get(), 4);
    }
}

#[test]
fn remote_batch_rejects_length_greater_than_used_without_mutation() {
    let mut fixture = SyntheticPage::new(2);
    fixture.initialize();
    // SAFETY: the three-block detached
    // chain belongs to the fixture page; used is intentionally smaller.
    unsafe {
        fixture.link(&[0, 1, 2]);
        let error = RemoteFreeBatch::from_detached(
            fixture.page(),
            fixture.block(0),
            fixture.page().capacity(),
            fixture.page().used(),
        )
        .unwrap_err();
        assert_eq!(
            error,
            BatchValidationError::ExceedsUsed {
                // SAFETY: three is non-zero.
                len: BatchLen::new_unchecked(3),
                used: UsedBlockCount::new(2),
            }
        );
        assert_eq!(fixture.page().used().get(), 2);
        let head = fixture.block(0);
        assert_eq!(
            fixture.order_from(Some(head)),
            ListOrder::from_slices(&[0, 1, 2], &[])
        );
    }
}

#[test]
fn randomized_reference_algorithms_match_batch_transitions() {
    const INITIAL_SEED: u64 = 0xD1FF_E2E3_5A3B_0003;
    let mut state = INITIAL_SEED;

    for iteration in 0..256u64 {
        let case_seed = state;
        let indexes = shuffled_indexes(&mut state);
        let local_len = (next_random(&mut state) as usize % 4) + 1;
        let destination_len =
            next_random(&mut state) as usize % (SYNTHETIC_CAPACITY - local_len + 1);
        let used = next_random(&mut state) as u16 % (SYNTHETIC_CAPACITY as u16 + 1);
        let mut reference = SyntheticPage::new(used);
        let mut candidate = SyntheticPage::new(used);
        reference.initialize();
        candidate.initialize();

        // SAFETY: both fixtures
        // receive identical valid topologies in independent stable storage.
        unsafe {
            for fixture in [&mut reference, &mut candidate] {
                fixture.link(&indexes[..local_len]);
                fixture.link(&indexes[local_len..local_len + destination_len]);
                fixture
                    .page()
                    .set_local_free(LocalFreeList::from_head(fixture.block(indexes[0])));
                let available = match destination_len {
                    0 => AvailableFreeList::empty(),
                    _ => AvailableFreeList::from_head(fixture.block(indexes[local_len])),
                };
                fixture.page().set_free(available);
            }

            reference_prepend_local_to_available(reference.page());
            let owner = LocalPage::new_unchecked(candidate.page());
            let batch_result = owner.detach_local_free_batch();
            assert!(
                matches!(batch_result, Ok(Some(_))),
                "local diagnostic mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
            let batch = batch_result
                .expect("checked successful local conversion")
                .expect("checked nonempty local conversion");
            let available = candidate.page().free();
            let available = batch.prepend_to_available(&owner, available);
            candidate.page().set_free(available);

            assert_eq!(
                reference.order_from(reference.page().free().head()),
                candidate.order_from(candidate.page().free().head()),
                "local order mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
            assert_eq!(
                reference.next_snapshot(),
                candidate.next_snapshot(),
                "local links mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
            assert_eq!(
                reference.head_index(reference.page().free().head()),
                candidate.head_index(candidate.page().free().head()),
                "local free-head mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
            assert_eq!(
                reference.page().used(),
                candidate.page().used(),
                "local used mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
            assert_eq!(
                reference.page().capacity(),
                candidate.page().capacity(),
                "local capacity mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
            assert!(
                reference.page().local_free().is_empty()
                    && candidate.page().local_free().is_empty(),
                "local head mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
        }

        let indexes = shuffled_indexes(&mut state);
        let remote_len = (next_random(&mut state) as usize % 4) + 1;
        let local_len = next_random(&mut state) as usize % (SYNTHETIC_CAPACITY - remote_len + 1);
        let used = remote_len as u16
            + next_random(&mut state) as u16 % (SYNTHETIC_CAPACITY as u16 - remote_len as u16 + 1);
        let mut reference = SyntheticPage::new(used);
        let mut candidate = SyntheticPage::new(used);
        reference.initialize();
        candidate.initialize();

        // SAFETY: both remote
        // fixtures contain the same valid detached and local page chains.
        unsafe {
            for fixture in [&mut reference, &mut candidate] {
                fixture.link(&indexes[..remote_len]);
                fixture.link(&indexes[remote_len..remote_len + local_len]);
                let local = match local_len {
                    0 => LocalFreeList::empty(),
                    _ => LocalFreeList::from_head(fixture.block(indexes[remote_len])),
                };
                fixture.page().set_local_free(local);
            }

            reference_collect_remote_into_local(reference.page(), reference.block(indexes[0]));
            let head = candidate.block(indexes[0]);
            let batch_result = RemoteFreeBatch::from_detached(
                candidate.page(),
                head,
                candidate.page().capacity(),
                candidate.page().used(),
            );
            assert!(
                batch_result.is_ok(),
                "remote diagnostic mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
            let batch = batch_result.expect("checked successful remote conversion");
            let owner = LocalPage::new_unchecked(candidate.page());
            let local = candidate.page().local_free();
            let _ = batch.collect_into_local_page(&owner, local);

            assert_eq!(
                reference.order_from(reference.page().local_free().head()),
                candidate.order_from(candidate.page().local_free().head()),
                "remote order mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
            assert_eq!(
                reference.next_snapshot(),
                candidate.next_snapshot(),
                "remote links mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
            assert_eq!(
                reference.head_index(reference.page().local_free().head()),
                candidate.head_index(candidate.page().local_free().head()),
                "remote head mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
            assert_eq!(
                reference.page().used(),
                candidate.page().used(),
                "remote used mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
            assert_eq!(
                reference.page().capacity(),
                candidate.page().capacity(),
                "remote capacity mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
            assert_eq!(
                reference.head_index(reference.page().free().head()),
                candidate.head_index(candidate.page().free().head()),
                "remote free-head mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
        }

        let indexes = shuffled_indexes(&mut state);
        let available_len = (next_random(&mut state) as usize % SYNTHETIC_CAPACITY) + 1;
        let used =
            next_random(&mut state) as u16 % (SYNTHETIC_CAPACITY as u16 - available_len as u16 + 1);
        let mut reference = SyntheticPage::new(used);
        let mut candidate = SyntheticPage::new(used);
        reference.initialize();
        candidate.initialize();

        // SAFETY: both fixtures
        // contain identical nonempty available lists with consistent counts.
        unsafe {
            for fixture in [&mut reference, &mut candidate] {
                fixture.link(&indexes[..available_len]);
                fixture
                    .page()
                    .set_free(AvailableFreeList::from_head(fixture.block(indexes[0])));
            }
            let reference_block = reference_available_pop(reference.page());
            let available = AvailablePage::new_unchecked(candidate.page());
            let (candidate_block, _) = available.pop_free_block();

            assert_eq!(
                reference_block,
                reference.block(indexes[0]),
                "reference pop mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
            assert_eq!(
                candidate_block.into_ptr(),
                candidate.block(indexes[0]),
                "candidate pop mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
            assert_eq!(
                reference.order_from(reference.page().free().head()),
                candidate.order_from(candidate.page().free().head()),
                "pop order mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
            assert_eq!(
                reference.next_snapshot(),
                candidate.next_snapshot(),
                "pop links mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
            assert_eq!(
                reference.head_index(reference.page().free().head()),
                candidate.head_index(candidate.page().free().head()),
                "pop head mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
            assert_eq!(
                reference.page().used(),
                candidate.page().used(),
                "pop used mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
            assert_eq!(
                reference.page().capacity(),
                candidate.page().capacity(),
                "pop capacity mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
            assert_eq!(
                reference.head_index(reference.page().local_free().head()),
                candidate.head_index(candidate.page().local_free().head()),
                "pop local-head mismatch: initial_seed={INITIAL_SEED:#x} case_seed={case_seed:#x} iteration={iteration}"
            );
        }
    }
}
