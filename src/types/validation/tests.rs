use super::*;
use crate::types::{mi_block_t, mi_page_t};
use core::mem::MaybeUninit;

#[test]
fn guarded_pointer_lookup_prepares_guarded_free_witnesses() {
    let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
    let mut raw_block = mi_block_t::default();
    let mut raw_user = 0usize;

    let page = unsafe { PagePtr::new_unchecked(raw_page.as_mut_ptr()) };
    let block = unsafe { BlockPtr::new_unchecked(core::ptr::addr_of_mut!(raw_block)) };
    let user = unsafe { UserPtr::new_unchecked(core::ptr::addr_of_mut!(raw_user).cast()) };
    let guarded = unsafe { GuardedBlock::after_guarded_detection(block) };

    let lookup = PointerLookup::valid(
        user,
        page,
        PointerLocation::Guarded {
            block: guarded,
            offset: BlockOffset(8),
        },
    );

    let validated = lookup.prepare_free().expect("valid guarded lookup");

    assert_eq!(validated.page().as_ptr(), page.as_ptr());
    assert_eq!(validated.user().as_ptr(), user.as_ptr());
    assert_eq!(validated.block().block().as_ptr(), block.as_ptr());
    assert!(validated.block().is_guarded());
    assert_eq!(validated.block().was_guarded(), WasGuarded::Yes);
}
