use super::*;
use crate::types::mi_page_t;
use core::mem::MaybeUninit;

#[test]
fn remote_publication_preserves_the_chain_and_transfers_only_a_new_claim() {
    use crate::core_types::{ThreadFreeOwner, ThreadFreeWord};

    #[repr(align(16))]
    struct Blocks([u8; 128]);

    for previous_owner in [ThreadFreeOwner::Owned, ThreadFreeOwner::Unowned] {
        let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
        let mut blocks = Blocks([0; 128]);
        // This fixture owns two live 64-byte blocks and initialized abandoned
        // page metadata. The second block is already remote; publication must
        // prepend the first exactly once without changing the pending count.
        unsafe {
            let raw = raw_page.as_mut_ptr();
            let page = PagePtr::new_unchecked(raw);
            let start = blocks.0.as_mut_ptr();
            let first = BlockPtr::new_unchecked(start.cast());
            let second = BlockPtr::new_unchecked(start.add(64).cast());
            (*raw).block_size = 64;
            (*raw).page_start = first.as_ptr().cast();
            (*raw).used = 2;
            (*raw).capacity = 2;
            (*raw).reserved = 2;
            (*raw).xthread_free = ThreadFreeWord::from_parts(Some(second), previous_owner).raw();
            crate::internal::mi_block_set_next_typed(page, second, None);
            let request = ValidatedFree::from_block_start_unchecked(page, first);
            let result = request.with_foreign(|page, block| page.publish_remote(block));
            match result {
                RemotePublication::Published => assert_eq!(previous_owner, ThreadFreeOwner::Owned),
                RemotePublication::Claimed(claimed) => {
                    assert_eq!(previous_owner, ThreadFreeOwner::Unowned);
                    let (claimed_page, head) = claimed.into_parts();
                    assert_eq!(claimed_page.page(), page);
                    assert_eq!(head.head(), first);
                    head.leave_published();
                }
            }
            let remote =
                super::super::atomic::AtomicThreadFree::from_cell(&*page.thread_free_field());
            assert_eq!(remote.load_relaxed().head(), Some(first));
            assert!(remote.load_relaxed().is_owned());
            assert_eq!(
                crate::internal::mi_block_next(raw, first.as_ptr()),
                second.as_ptr()
            );
            assert!(crate::internal::mi_block_next(raw, second.as_ptr()).is_null());
            assert_eq!((*raw).used, 2);
        }
    }
}

#[test]
fn guarded_pointer_lookup_prepares_guarded_free_witnesses() {
    let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
    let mut raw_block = [0usize; 4];
    let offset = core::mem::size_of::<usize>();

    let page = unsafe { PagePtr::new_unchecked(raw_page.as_mut_ptr()) };
    let block = BlockPtr::new(raw_block.as_mut_ptr().cast()).unwrap();
    // SAFETY: the initialized backing contains the block tag and interior user
    // pointer, and the page describes that exact block throughout the lookup.
    let user = unsafe {
        (*page.as_ptr()).page_start = block.as_ptr().cast();
        (*page.as_ptr()).block_size = core::mem::size_of_val(&raw_block);
        crate::core_types::BlockTag::write_guarded_to(block);
        UserPtr::new(block.as_ptr().cast::<u8>().add(offset).cast()).unwrap()
    };
    let guarded = unsafe { GuardedBlock::after_guarded_detection(block) };

    // SAFETY: the tag, page geometry, block, and interior user offset agree.
    let lookup = unsafe {
        PointerLookup::after_validation(
            user,
            page,
            PointerLocation::Guarded {
                block: guarded,
                offset: BlockOffset(offset),
            },
        )
    };

    let validated = lookup.prepare_free().expect("valid guarded lookup");

    assert_eq!(validated.page().as_ptr(), page.as_ptr());
    assert_eq!(validated.user().as_ptr(), user.as_ptr());
    assert_eq!(validated.block().block().as_ptr(), block.as_ptr());
    assert!(validated.block().is_guarded());
    assert_eq!(validated.block().was_guarded(), WasGuarded::Yes);
}
