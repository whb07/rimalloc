use super::*;
use crate::types::mi_page_t;
use core::mem::MaybeUninit;

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
