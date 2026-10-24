use super::*;
use core::ptr::NonNull;
use std::alloc::{Layout, alloc_zeroed, dealloc, handle_alloc_error};

/// Allocate the exact compact layout used by arena metadata. Drop also checks
/// cleanup under Miri.
pub(crate) struct BitmapStorage {
    raw: NonNull<u8>,
    layout: Layout,
}

impl BitmapStorage {
    pub(crate) fn new(size: usize) -> Self {
        let layout = Layout::from_size_align(size, MI_BCHUNK_SIZE).unwrap();
        // SAFETY: the layout is nonempty and valid; this fixture owns the result.
        let raw = NonNull::new(unsafe { alloc_zeroed(layout) })
            .unwrap_or_else(|| handle_alloc_error(layout));
        Self { raw, layout }
    }

    pub(crate) fn ptr<T>(&self) -> *mut T {
        self.raw.as_ptr().cast()
    }
}

impl Drop for BitmapStorage {
    fn drop(&mut self) {
        // SAFETY: this is the original allocation and layout, released once.
        unsafe { dealloc(self.raw.as_ptr(), self.layout) };
    }
}

#[test]
fn compact_bitmap_bit_ranges_preserve_bounds_and_counts() {
    for bits in [1, MI_BCHUNK_BITS, MI_BCHUNK_BITS + 1, 3 * MI_BCHUNK_BITS] {
        // SAFETY: the fixture allocates exactly the initialized compact layout.
        unsafe {
            let storage = BitmapStorage::new(mi_bitmap_size(bits, ptr::null_mut()));
            let bitmap = storage.ptr::<mi_bitmap_t>();
            assert_eq!(mi_bitmap_init(bitmap, bits, true), storage.layout.size());
            let max_bits = mi_bitmap_max_bits(bitmap);
            for (start, count) in [
                (0, 1),
                (MI_BFIELD_BITS - 1, 2),
                (MI_BCHUNK_BITS - 1, 2),
                (max_bits - 1, 1),
            ] {
                if start + count > max_bits {
                    continue;
                }
                let mut already_set = usize::MAX;
                assert!(mi_bitmap_setN(bitmap, start, count, &mut already_set));
                assert_eq!(already_set, 0);
                assert!(mi_bitmap_is_setN(bitmap, start, count));
                assert_eq!(mi_bitmap_popcount(bitmap), count);
                assert!(!mi_bitmap_setN(bitmap, start, count, &mut already_set));
                assert_eq!(already_set, count);
                assert!(mi_bitmap_clearN(bitmap, start, count));
                assert!(mi_bitmap_is_all_clear(bitmap));
            }
        }
    }
}

#[test]
fn compact_bitmap_bulk_initialization_covers_partial_and_full_chunks() {
    // SAFETY: the fixture exclusively owns enough zeroed aligned storage for
    // all three chunks through initialization, queries, and deallocation.
    unsafe {
        let bits = 3 * MI_BCHUNK_BITS;
        let storage = BitmapStorage::new(mi_bitmap_size(bits, ptr::null_mut()));
        let bitmap = storage.ptr::<mi_bitmap_t>();
        mi_bitmap_init(bitmap, bits, true);
        mi_bitmap_unsafe_setN(bitmap, 1, bits - 2);
        assert!(mi_bitmap_is_clear(bitmap, 0));
        assert!(mi_bitmap_is_clear(bitmap, bits - 1));
        assert!(mi_bitmap_is_setN(bitmap, 1, bits - 2));
        assert_eq!(mi_bitmap_popcount(bitmap), bits - 2);
    }
}

#[test]
fn compact_bbitmap_claims_field_chunk_and_multi_chunk_ranges() {
    for chunks in 1..=3 {
        // SAFETY: every operation is bounded by this initialized compact
        // allocation, and each successful claim is returned before reuse.
        unsafe {
            let bits = chunks * MI_BCHUNK_BITS;
            let storage = BitmapStorage::new(mi_bbitmap_size(bits, ptr::null_mut()));
            let bitmap = storage.ptr::<mi_bbitmap_t>();
            assert_eq!(mi_bbitmap_init(bitmap, bits, true), storage.layout.size());
            assert!(mi_bbitmap_setN(bitmap, 0, bits));
            for count in [
                1,
                8,
                MI_BFIELD_BITS - 1,
                MI_BFIELD_BITS,
                MI_BFIELD_BITS + 1,
                MI_BCHUNK_BITS,
                MI_BCHUNK_BITS + 1,
            ] {
                if count > bits {
                    continue;
                }
                let mut start = usize::MAX;
                assert!(mi_bbitmap_try_find_and_clearN(bitmap, 0, count, &mut start));
                assert!(start + count <= bits);
                assert!(mi_bbitmap_is_clearN(bitmap, start, count));
                assert!(mi_bbitmap_setN(bitmap, start, count));
                assert!(mi_bbitmap_is_setN(bitmap, 0, bits));
            }
            let mut start = usize::MAX;
            assert!(!mi_bbitmap_try_find_and_clearN(bitmap, 0, 0, &mut start));
            assert!(!mi_bbitmap_try_find_and_clearN(
                bitmap,
                0,
                bits + 1,
                &mut start
            ));
            assert_eq!(start, usize::MAX);
        }
    }
}
