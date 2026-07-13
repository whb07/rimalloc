use super::*;
use crate::types::{mi_block_t, mi_page_t};

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
