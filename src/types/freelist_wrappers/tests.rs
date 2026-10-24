use super::*;
use crate::core_types::{ActivePage, PagePtr};
use crate::internal::mi_block_set_next;
use crate::types::{mi_block_t, mi_page_t};
use core::{mem::MaybeUninit, ptr};

#[test]
fn encoded_pointer_round_trips_block_and_null() {
    let mut page = MaybeUninit::<mi_page_t>::uninit();
    let mut block = mi_block_t::default();
    unsafe {
        let raw_page = page.as_mut_ptr();
        #[cfg(any(
            feature = "secure",
            debug_assertions,
            feature = "track-valgrind",
            feature = "track-asan"
        ))]
        ptr::addr_of_mut!((*raw_page).keys).write([0x1234_5678, 0x8765_4321]);
        let page_ptr = PagePtr::new_unchecked(raw_page);
        let block_ptr = BlockPtr::new_unchecked(ptr::addr_of_mut!(block));

        let keys = page_ptr.keys();
        let encoded = EncodedPtr::encode_block(Some(block_ptr), page_ptr, keys);
        assert_eq!(
            encoded.decode_block(page_ptr, keys).map(BlockPtr::as_ptr),
            Some(block_ptr.as_ptr())
        );

        let encoded_null = EncodedPtr::encode_block(None, page_ptr, keys);
        assert!(encoded_null.decode_block(page_ptr, keys).is_none());
    }
}

#[test]
fn block_tag_explicit_aligned_write_is_not_guarded() {
    let mut block = mi_block_t::default();
    unsafe {
        let block_ptr = BlockPtr::new_unchecked(ptr::addr_of_mut!(block));
        BlockTag::write_guarded_to(block_ptr);
        assert!(BlockLink::is_guarded_raw(ptr::addr_of!(block)));

        BlockTag::write_aligned_to(block_ptr);
        assert!(!BlockLink::is_guarded_raw(ptr::addr_of!(block)));
    }
}

#[test]
fn available_pop_consumes_available_page_witness() {
    let mut page = MaybeUninit::<mi_page_t>::uninit();
    let mut block = mi_block_t::default();
    unsafe {
        let raw_page = page.as_mut_ptr();
        #[cfg(any(
            feature = "secure",
            debug_assertions,
            feature = "track-valgrind",
            feature = "track-asan"
        ))]
        ptr::addr_of_mut!((*raw_page).keys).write([0x1234_5678, 0x8765_4321]);
        ptr::addr_of_mut!((*raw_page).free).write(ptr::addr_of_mut!(block));
        ptr::addr_of_mut!((*raw_page).used).write(0);
        mi_block_set_next(raw_page, ptr::addr_of_mut!(block), ptr::null());

        let active = ActivePage::new_unchecked(PagePtr::new_unchecked(raw_page));
        let available = active
            .try_available()
            .expect("free head creates available page");
        let (allocated, next) = AvailableFreeList::pop_one(available);

        assert_eq!(allocated.as_ptr().as_ptr(), ptr::addr_of_mut!(block));
        assert!(next.is_empty());
    }
}

#[test]
fn owner_push_requires_scoped_free_block_witness() {
    let mut page = MaybeUninit::<mi_page_t>::uninit();
    let mut block = mi_block_t::default();
    unsafe {
        let raw_page = page.as_mut_ptr();
        #[cfg(any(
            feature = "secure",
            debug_assertions,
            feature = "track-valgrind",
            feature = "track-asan"
        ))]
        ptr::addr_of_mut!((*raw_page).keys).write([0x1234_5678, 0x8765_4321]);
        ptr::addr_of_mut!((*raw_page).free).write(ptr::null_mut());

        let page_ptr = PagePtr::new_unchecked(raw_page);
        ptr::addr_of_mut!((*raw_page).used).write(1);
        ptr::addr_of_mut!((*raw_page).free_is_zero).write(true);
        let block_ptr = BlockPtr::new_unchecked(ptr::addr_of_mut!(block));
        let free =
            crate::core_types::ValidatedFree::from_block_start_unchecked(page_ptr, block_ptr);
        free.with_local(|page, block| {
            page.push_free_block(block);
        });

        assert_eq!((*raw_page).free, block_ptr.as_ptr());
        assert!(mi_block_next(raw_page, block_ptr.as_ptr()).is_null());
    }
}
