
use super::*;

#[test]
fn thread_free_word_round_trips_owner_without_head() {
    let owned = ThreadFreeWord::from_parts(None, ThreadFreeOwner::Owned);
    assert_eq!(owned.raw(), ThreadFreeWord::OWNED_BIT);
    assert!(owned.is_owned());
    assert!(owned.head().is_none());
    assert!(owned.head_raw().is_null());

    let unowned = ThreadFreeWord::from_parts(None, ThreadFreeOwner::Unowned);
    assert_eq!(unowned.raw(), 0);
    assert!(!unowned.is_owned());
    assert!(unowned.head().is_none());
    assert!(unowned.head_raw().is_null());
}

#[test]
fn thread_free_word_preserves_aligned_head_pointer() {
    let mut block = crate::types::mi_block_t::default();
    let head = unsafe { BlockPtr::new_unchecked(core::ptr::addr_of_mut!(block)) };

    let word = ThreadFreeWord::from_parts(Some(head), ThreadFreeOwner::Owned);
    assert!(word.is_owned());
    assert_eq!(word.head().map(BlockPtr::as_ptr), Some(head.as_ptr()));
    assert_eq!(word.head_raw(), head.as_ptr());
    assert_eq!(
        word.raw() & !ThreadFreeWord::OWNED_BIT,
        head.as_ptr() as usize
    );
}

#[test]
fn thread_free_word_updates_owner_without_decoding_head() {
    let mut block = crate::types::mi_block_t::default();
    let head = unsafe { BlockPtr::new_unchecked(core::ptr::addr_of_mut!(block)) };
    let word = ThreadFreeWord::from_parts(Some(head), ThreadFreeOwner::Owned);

    let unowned = word.with_owner(ThreadFreeOwner::Unowned);
    assert!(!unowned.is_owned());
    assert_eq!(unowned.head().map(BlockPtr::as_ptr), Some(head.as_ptr()));

    let owned = unowned.with_owner(ThreadFreeOwner::Owned);
    assert_eq!(owned.raw(), word.raw());
}

#[test]
fn thread_free_word_clears_head_without_decoding_owner() {
    let mut block = crate::types::mi_block_t::default();
    let head = unsafe { BlockPtr::new_unchecked(core::ptr::addr_of_mut!(block)) };
    let owned = ThreadFreeWord::from_parts(Some(head), ThreadFreeOwner::Owned);
    let unowned = ThreadFreeWord::from_parts(Some(head), ThreadFreeOwner::Unowned);

    let owned_without_head = owned.without_head();
    assert!(owned_without_head.is_owned());
    assert!(owned_without_head.head().is_none());
    assert_eq!(owned_without_head.raw(), ThreadFreeWord::OWNED_BIT);

    let unowned_without_head = unowned.without_head();
    assert!(!unowned_without_head.is_owned());
    assert!(unowned_without_head.head().is_none());
    assert_eq!(unowned_without_head.raw(), 0);
}
