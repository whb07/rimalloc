use core::ptr;

use mechanical_mimalloc::raw::{internal, types};

#[test]
fn default_freelist_links_are_raw_pointers() {
    if cfg!(feature = "secure") || cfg!(debug_assertions) {
        return;
    }

    assert_eq!(types::MI_ENCODE_FREELIST, 0);

    let keys = [0x1234usize, 0x5678usize];
    let raw_next = 0x1234_5678usize as *mut types::mi_block_t;
    let mut block = types::mi_block_t {
        next: raw_next as usize,
    };

    let decoded = unsafe { internal::mi_block_nextx(ptr::null(), &block, &keys) };
    assert_eq!(decoded, raw_next);

    let replacement = 0x8765_4321usize as *const types::mi_block_t;
    unsafe { internal::mi_block_set_nextx(ptr::null(), &mut block, replacement, &keys) };
    assert_eq!(block.next, replacement as usize);
}
