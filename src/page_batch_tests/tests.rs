use super::*;
use crate::types::{mi_block_t, mi_page_t};
use core::mem::MaybeUninit;

const SYNTHETIC_CAPACITY: usize = 8;

struct SyntheticPage {
    page_storage: Box<MaybeUninit<mi_page_t>>,
    blocks: Box<[mi_block_t; SYNTHETIC_CAPACITY]>,
    page: PagePtr,
}

impl SyntheticPage {
    fn new(used: u16) -> Self {
        let mut page_storage = Box::new(MaybeUninit::<mi_page_t>::zeroed());
        let mut blocks = Box::new([mi_block_t::default(); SYNTHETIC_CAPACITY]);
        let raw_page = page_storage.as_mut_ptr();
        // SAFETY[AUDIT:typed-page-batch-synthetic-fixtures]: the boxed page and block
        // storage remain stable and live for the complete synthetic fixture.
        let page = unsafe { PagePtr::new_unchecked(raw_page) };
        // SAFETY[AUDIT:typed-page-batch-synthetic-fixtures]: writes initialize the
        // exact metadata fields read by batch traversal and accounting.
        unsafe {
            (*raw_page).used = used;
            (*raw_page).capacity = SYNTHETIC_CAPACITY as u16;
            (*raw_page).reserved = SYNTHETIC_CAPACITY as u16;
            (*raw_page).block_size = core::mem::size_of::<mi_block_t>();
            (*raw_page).page_start = blocks.as_mut_ptr().cast();
            page.set_keys(crate::core_types::PageKeys::disabled());
        }
        Self {
            page_storage,
            blocks,
            page,
        }
    }

    fn block(&mut self, index: usize) -> BlockPtr {
        // SAFETY[AUDIT:typed-page-batch-synthetic-fixtures]: the bounded index points
        // into stable boxed block storage for this synthetic page.
        unsafe { BlockPtr::new_unchecked(core::ptr::addr_of_mut!(self.blocks[index])) }
    }

    unsafe fn link(&mut self, indexes: &[usize]) {
        for pair in indexes.windows(2) {
            // SAFETY[AUDIT:typed-page-batch-synthetic-fixtures]: both indexes are
            // blocks in this page and use its disabled fixture keys.
            unsafe {
                self.block(pair[0])
                    .set_next(self.page, Some(self.block(pair[1])))
            };
        }
        if let Some(&tail) = indexes.last() {
            // SAFETY[AUDIT:typed-page-batch-synthetic-fixtures]: the selected tail is
            // writable block storage in this synthetic page.
            unsafe { self.block(tail).set_next(self.page, None) };
        }
    }

    unsafe fn order_from(&self, head: Option<BlockPtr>) -> Vec<usize> {
        let base = self.blocks.as_ptr() as usize;
        let stride = core::mem::size_of::<mi_block_t>();
        let mut order = Vec::new();
        let mut current = head;
        while let Some(block) = current {
            order.push((block.as_ptr() as usize - base) / stride);
            assert!(order.len() <= SYNTHETIC_CAPACITY);
            // SAFETY[AUDIT:typed-page-batch-synthetic-fixtures]: the synthetic list
            // contains only fixture blocks and is capacity bounded.
            current = unsafe { block.next(self.page) };
        }
        order
    }
}

impl Drop for SyntheticPage {
    fn drop(&mut self) {
        let _ = (&self.page_storage, &self.blocks);
    }
}

struct SamePageBlocks {
    all: Vec<*mut mi_block_t>,
    page: PagePtr,
    selected: Vec<BlockPtr>,
}

impl SamePageBlocks {
    fn new(required: usize) -> Self {
        // SAFETY[AUDIT:typed-page-batch-test-fixtures]: allocations stay live
        // for the fixture, and page/block witnesses are used only with their
        // allocator-reported owning page.
        unsafe {
            let mut all = Vec::with_capacity(required * 4);
            let mut selected = Vec::new();
            let mut selected_page: *mut mi_page_t = core::ptr::null_mut();

            while selected.len() < required {
                let block = crate::mimalloc::mi_malloc(16).cast::<mi_block_t>();
                assert!(!block.is_null());
                all.push(block);
                let page = crate::internal::_mi_ptr_page(block.cast());
                if selected_page.is_null() {
                    selected_page = page;
                }
                if page == selected_page {
                    selected.push(BlockPtr::new_unchecked(block));
                }
            }

            Self {
                all,
                page: PagePtr::new_unchecked(selected_page),
                selected,
            }
        }
    }

    fn block(&self, index: usize) -> BlockPtr {
        self.selected[index]
    }

    fn link_prefix(&self, len: usize) {
        // SAFETY[AUDIT:typed-page-batch-test-fixtures]: selected blocks belong
        // to this page and remain live for the fixture.
        unsafe {
            for pair in self.selected[..len].windows(2) {
                pair[0].set_next(self.page, Some(pair[1]));
            }
            self.selected[len - 1].set_next(self.page, None);
        }
    }

    fn full_page() -> Self {
        let mut fixture = Self::new(1);
        // SAFETY[AUDIT:typed-page-batch-test-fixtures]: each added allocation
        // stays live, and the assertion checks page identity before creating a
        // block witness.
        unsafe {
            loop {
                let capacity = fixture.page.capacity().as_usize();
                if fixture.selected.len() == capacity {
                    return fixture;
                }
                let block = crate::mimalloc::mi_malloc(16).cast::<mi_block_t>();
                assert!(!block.is_null());
                fixture.all.push(block);
                let page = crate::internal::_mi_ptr_page(block.cast());
                assert_eq!(page, fixture.page.as_ptr(), "page changed before capacity");
                fixture.selected.push(BlockPtr::new_unchecked(block));
            }
        }
    }
}

impl Drop for SamePageBlocks {
    fn drop(&mut self) {
        // SAFETY[AUDIT:typed-page-batch-test-fixtures]: every pointer was
        // returned by mi_malloc and is freed exactly once at fixture teardown.
        unsafe {
            for block in self.all.drain(..) {
                crate::mimalloc::mi_free(block.cast());
            }
        }
    }
}

#[test]
fn scans_basic_detached_chain_lengths_exactly() {
    // SAFETY[AUDIT:typed-page-batch-test-fixtures]: the fixture owns each block
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
            assert_eq!(core.len().as_usize(), len);
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
fn scans_capacity_boundary_lengths_exactly() {
    // SAFETY[AUDIT:typed-page-batch-test-fixtures]: the fixture owns a complete
    // page-capacity set of live blocks and page-local encoded links.
    unsafe {
        let fixture = SamePageBlocks::full_page();
        let capacity = fixture.page.capacity();
        assert!(capacity.get() > 1);
        for len in [capacity.as_usize() - 1, capacity.as_usize()] {
            fixture.link_prefix(len);
            let core = scan_detached_chain(fixture.page, fixture.block(0), capacity)
                .expect("capacity-bounded detached chain");
            assert_eq!(core.len().as_usize(), len);
            assert_eq!(core.tail(), fixture.block(len - 1));
            assert!(core.tail().next(fixture.page).is_none());
        }
    }
}

#[test]
fn capacity_bound_turns_cycles_and_self_loops_into_too_long() {
    // SAFETY[AUDIT:typed-page-batch-test-fixtures]: all cyclic links remain
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
    }
}

#[test]
fn rejects_a_link_to_a_different_page_without_following_it() {
    // SAFETY[AUDIT:typed-page-batch-test-fixtures]: all allocations remain live;
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
    // SAFETY[AUDIT:typed-page-batch-test-fixtures]: the intentionally interior
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
    // SAFETY[AUDIT:typed-page-batch-test-fixtures]: the live head is paired with
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
        // SAFETY[AUDIT:typed-page-batch-synthetic-fixtures]: all list indexes belong
        // to the stable synthetic page and each chain is uniquely owned.
        unsafe {
            fixture.link(&[0, 1, 2]);
            fixture.link(destination);
            let local = match destination.first() {
                Some(&head) => LocalFreeList::from_head(fixture.block(head)),
                None => LocalFreeList::empty(),
            };
            fixture.page.set_local_free(match destination.first() {
                Some(&head) => LocalFreeList::from_head(fixture.block(head)),
                None => LocalFreeList::empty(),
            });
            let capacity = fixture.page.capacity();
            let used = fixture.page.used();
            let batch =
                RemoteFreeBatch::from_detached(fixture.page, fixture.block(0), capacity, used)
                    .expect("valid synthetic remote batch");
            let owner = LocalPage::new_unchecked(fixture.page);
            let collected = batch.collect_into_local_page(&owner, local);

            let mut expected = vec![0, 1, 2];
            expected.extend_from_slice(destination);
            assert_eq!(fixture.order_from(collected.head()), expected);
            assert_eq!(fixture.page.used().get(), 4);
            assert_eq!(fixture.page.local_free().as_raw(), collected.as_raw());
        }
    }
}

#[test]
fn local_batch_splices_destinations_without_changing_used() {
    for destination in [&[][..], &[3][..], &[3, 4, 5][..]] {
        let mut fixture = SyntheticPage::new(4);
        // SAFETY[AUDIT:typed-page-batch-synthetic-fixtures]: all page metadata and
        // encoded links are confined to the stable synthetic fixture.
        unsafe {
            fixture.link(&[0, 1, 2]);
            fixture.link(destination);
            fixture
                .page
                .set_local_free(LocalFreeList::from_head(fixture.block(0)));
            let available = match destination.first() {
                Some(&head) => AvailableFreeList::from_head(fixture.block(head)),
                None => AvailableFreeList::empty(),
            };
            let owner = LocalPage::new_unchecked(fixture.page);
            let batch = owner
                .detach_local_free_batch()
                .expect("valid local chain")
                .expect("nonempty local chain");
            assert!(fixture.page.local_free().is_empty());
            let available = batch.prepend_to_available(&owner, available);

            let mut expected = vec![0, 1, 2];
            expected.extend_from_slice(destination);
            assert_eq!(fixture.order_from(available.head()), expected);
            assert_eq!(fixture.page.used().get(), 4);
        }
    }
}

#[test]
fn lease_take_and_return_preserve_counted_allocations() {
    let mut fixture = SyntheticPage::new(2);
    // SAFETY[AUDIT:typed-page-batch-synthetic-fixtures]: the available chain and
    // page accounting are initialized consistently in the synthetic fixture.
    unsafe {
        fixture.link(&[0, 1, 2, 3, 4, 5]);
        fixture
            .page
            .set_free(AvailableFreeList::from_head(fixture.block(0)));
        let available_page = AvailablePage::new_unchecked(fixture.page);
        let limit = BatchLimit::new(4).expect("nonzero lease limit");
        let (lease, active) = available_page.lease(limit);
        assert_eq!(lease.len().get(), 4);
        assert_eq!(fixture.page.used().get(), 6);
        assert_eq!(fixture.order_from(fixture.page.free().head()), vec![4, 5]);

        let (first, remaining) = lease.take_one();
        assert_eq!(first.into_ptr(), fixture.block(0));
        let (second, remaining) = remaining.expect("three leased blocks remain").take_one();
        assert_eq!(second.into_ptr(), fixture.block(1));
        let remaining = remaining.expect("two leased blocks remain");
        assert_eq!(remaining.len().get(), 2);
        assert_eq!(fixture.page.used().get(), 6);

        let owner = LocalPage::new_unchecked(active.page());
        let current = fixture.page.free();
        let (available, used) = remaining.return_to_available(&owner, current);
        fixture.page.set_free(available);
        fixture.page.set_used(used);
        assert_eq!(fixture.page.used().get(), 4);
        assert_eq!(
            fixture.order_from(fixture.page.free().head()),
            vec![2, 3, 4, 5]
        );
    }
}

#[test]
fn remote_batch_rejects_length_greater_than_used_without_mutation() {
    let mut fixture = SyntheticPage::new(2);
    // SAFETY[AUDIT:typed-page-batch-synthetic-fixtures]: the three-block detached
    // chain belongs to the fixture page; used is intentionally smaller.
    unsafe {
        fixture.link(&[0, 1, 2]);
        let error = RemoteFreeBatch::from_detached(
            fixture.page,
            fixture.block(0),
            fixture.page.capacity(),
            fixture.page.used(),
        )
        .unwrap_err();
        assert_eq!(
            error,
            BatchValidationError::ExceedsUsed {
                len: BatchLen::new(3).expect("nonempty length"),
                used: UsedBlockCount::new(2),
            }
        );
        assert_eq!(fixture.page.used().get(), 2);
        let head = fixture.block(0);
        assert_eq!(fixture.order_from(Some(head)), vec![0, 1, 2]);
    }
}
