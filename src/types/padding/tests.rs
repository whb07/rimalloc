use super::*;
use core::mem::{align_of, size_of};

#[test]
fn padding_wrappers_match_raw_layout() {
    assert_eq!(size_of::<Padding>(), size_of::<mi_padding_t>());
    assert_eq!(align_of::<Padding>(), align_of::<mi_padding_t>());
    assert_eq!(size_of::<PaddingPtr>(), size_of::<NonNull<mi_padding_t>>());
    assert_eq!(
        align_of::<PaddingPtr>(),
        align_of::<NonNull<mi_padding_t>>()
    );
    assert_eq!(
        size_of::<Option<PaddingPtr>>(),
        size_of::<NonNull<mi_padding_t>>()
    );
    assert_eq!(size_of::<PaddingCanary>(), size_of::<u32>());
    assert_eq!(align_of::<PaddingCanary>(), align_of::<u32>());
    assert_eq!(size_of::<PaddingDelta>(), size_of::<u32>());
    assert_eq!(align_of::<PaddingDelta>(), align_of::<u32>());
}

#[test]
fn padding_delta_preserves_usable_delta() {
    let delta = PaddingDelta::for_usable(12, UsableSize::new(16)).unwrap();
    assert_eq!(delta.get(), 12);
}

#[test]
fn padding_delta_rejects_out_of_range_values() {
    assert_eq!(PaddingDelta::for_usable(17, UsableSize::new(16)), None);
    assert_eq!(
        PaddingDelta::for_usable(u32::MAX as usize + 1, UsableSize::new(usize::MAX)),
        None
    );
}

#[test]
fn padding_canary_uses_typed_page_and_block() {
    // SAFETY[AUDIT:typed-padding-test-fixtures]: dangling non-null pointers are
    // used only as opaque canary inputs and are never dereferenced.
    let page = unsafe { PagePtr::new_unchecked(core::ptr::NonNull::dangling().as_ptr()) };
    // SAFETY[AUDIT:typed-padding-test-fixtures]: dangling non-null pointers are
    // used only as opaque canary inputs and are never dereferenced.
    let block = unsafe { BlockPtr::new_unchecked(core::ptr::NonNull::dangling().as_ptr()) };
    let keys = PageKeys([1, 2]);

    assert_eq!(
        PaddingCanary::from_page_block_keys(page, block, keys).get(),
        mi_ptr_encode_canary(page.as_ptr().cast(), block.as_ptr().cast(), &keys.0)
    );
}

#[test]
fn padding_ptr_reads_and_writes_stack_metadata() {
    let mut raw = mi_padding_t {
        canary: 0x1234_5678,
        delta: 4,
    };
    // SAFETY[AUDIT:typed-padding-test-fixtures]: `raw` is stack-local writable
    // padding metadata for exercising the typed padding pointer wrapper.
    let ptr = unsafe { PaddingPtr::new_unchecked(core::ptr::addr_of_mut!(raw)) };

    // SAFETY[AUDIT:typed-padding-test-fixtures]: `ptr` points at initialized
    // stack-local padding metadata.
    let initial = unsafe { ptr.read() };
    assert_eq!(initial.canary(), PaddingCanary(0x1234_5678));
    assert_eq!(initial.delta(), PaddingDelta(4));

    let updated = Padding::new(PaddingCanary(0x8765_4321), PaddingDelta(8));
    // SAFETY[AUDIT:typed-padding-test-fixtures]: `ptr` points at writable
    // stack-local padding metadata.
    unsafe { ptr.write(updated) };

    assert_eq!(raw.canary, 0x8765_4321);
    assert_eq!(raw.delta, 8);
    // SAFETY[AUDIT:typed-padding-test-fixtures]: `ptr` still points at the same
    // initialized stack-local padding metadata after the write.
    assert_eq!(unsafe { ptr.read() }.delta(), PaddingDelta(8));
}

#[cfg(not(mi_c_malloc_fast))]
#[test]
fn padding_ptr_can_update_delta_without_touching_canary() {
    let mut raw = mi_padding_t {
        canary: 0xfeed_beef,
        delta: 1,
    };
    // SAFETY[AUDIT:typed-padding-test-fixtures]: `raw` is stack-local writable
    // padding metadata for exercising the delta-only update path.
    let ptr = unsafe { PaddingPtr::new_unchecked(core::ptr::addr_of_mut!(raw)) };

    // SAFETY[AUDIT:typed-padding-test-fixtures]: `ptr` points at writable
    // stack-local padding metadata.
    unsafe { ptr.write_delta(PaddingDelta(9)) };

    assert_eq!(raw.canary, 0xfeed_beef);
    assert_eq!(raw.delta, 9);
}

#[test]
fn padding_ptr_from_block_usable_points_after_usable_bytes() {
    let mut bytes = [0u8; 64];
    // SAFETY[AUDIT:typed-padding-test-fixtures]: stack-local bytes provide a
    // non-null aligned block fixture for pointer arithmetic only.
    let block = unsafe { BlockPtr::new_unchecked(bytes.as_mut_ptr().cast()) };
    // SAFETY[AUDIT:typed-padding-test-fixtures]: the usable size is within the
    // stack-local byte fixture, so the derived padding pointer remains in-bounds.
    let padding = unsafe { PaddingPtr::from_block_usable(block, UsableSize::new(32)) };

    // SAFETY[AUDIT:typed-padding-test-fixtures]: adding 32 stays within the
    // stack-local byte fixture used for the expected pointer comparison.
    assert_eq!(padding.as_ptr().cast::<u8>(), unsafe {
        bytes.as_mut_ptr().add(32)
    });
}
