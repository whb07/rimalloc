use super::*;
use core::mem::MaybeUninit;

#[test]
fn block_ptr_free_list_link_helpers_delegate_to_encoded_links() {
    let mut raw_page = MaybeUninit::<mi_page_t>::zeroed();
    // SAFETY: the page pointer wraps
    // stack-local page storage used only by this free-list fixture.
    let page = unsafe { PagePtr::new_unchecked(raw_page.as_mut_ptr()) };
    // SAFETY: key writes mutate only the
    // stack-local page fixture and avoid encoded-key randomness in this test.
    unsafe { page.set_keys(PageKeys::disabled()) };

    let mut first = MaybeUninit::<mi_block_t>::zeroed();
    let mut second = MaybeUninit::<mi_block_t>::zeroed();
    // SAFETY: both block pointers wrap
    // stack-local block fixtures that stay live through the link assertions.
    let first = unsafe { BlockPtr::new_unchecked(first.as_mut_ptr()) };
    // SAFETY: `second` wraps stack-local
    // block storage used only as an encoded next pointer target.
    let second = unsafe { BlockPtr::new_unchecked(second.as_mut_ptr()) };

    // SAFETY: link writes target stack-local
    // block metadata using the live stack-local page key fixture.
    unsafe { first.set_next(page, Some(second)) };
    // SAFETY: the encoded link was written
    // above and both block fixtures remain live.
    unsafe {
        assert_eq!(
            first.next(page).map(BlockPtr::as_ptr),
            Some(second.as_ptr())
        );
    }

    // SAFETY: link clearing mutates only the
    // stack-local first block fixture.
    unsafe { first.set_next(page, None) };
    // SAFETY: the encoded link was just
    // cleared and the first block fixture remains live.
    unsafe {
        assert!(first.next(page).is_none());
    }
}

#[test]
fn provenance_address_helpers_match_raw_pointer_math() {
    let bytes = [0u8; 16];
    let base = bytes.as_ptr();
    // SAFETY: offset 9 is within the live
    // stack-local byte buffer and is used only for address-difference checks.
    let at_9 = unsafe { base.add(9) };

    assert_eq!(ptr_exposed_addr(base), base as usize);
    assert_eq!(ptr_addr_diff(at_9, base), 9);
    assert!(ptr_is_aligned(base, 1));
    assert!(ptr_is_aligned(base, 0));
}
