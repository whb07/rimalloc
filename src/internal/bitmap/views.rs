//! Borrowed views over the initialized compact bitmap allocation.

use super::*;

/// Shared atomic access to the header and its actual trailing chunk extent.
/// `raw` retains the original allocation provenance for outlined map kernels.
pub(crate) struct BitmapView<'a> {
    raw: *const mi_bitmap_t,
    header: &'a mi_bitmap_t,
    chunks: &'a [mi_bchunk_t],
}

pub(crate) struct BBitmapView<'a> {
    raw: *const mi_bbitmap_t,
    header: &'a mi_bbitmap_t,
    chunks: &'a [mi_bchunk_t],
}

/// A bounded bit range, advanced by complete or partial chunks. While
/// `remaining` is nonzero, `chunk_idx` identifies a live chunk and the remaining
/// bits fit within the slice from `cidx` onward. Only `new` and `next` can
/// change those fields.
struct BitmapChunks<'a> {
    chunks: &'a [mi_bchunk_t],
    chunk_idx: usize,
    cidx: usize,
    remaining: usize,
}

impl<'a> BitmapChunks<'a> {
    #[inline(always)]
    fn new(chunks: &'a [mi_bchunk_t], idx: usize, n: usize) -> Option<Self> {
        let maxbits = chunks.len() * MI_BCHUNK_BITS;
        crate::internal::mi_debug_assert!(n > 0);
        crate::internal::mi_debug_assert!(idx + n <= maxbits);
        if idx >= maxbits {
            return (idx == maxbits && n == 0).then_some(Self {
                chunks,
                chunk_idx: chunks.len(),
                cidx: 0,
                remaining: 0,
            });
        }
        Some(Self {
            chunks,
            chunk_idx: idx / MI_BCHUNK_BITS,
            cidx: idx % MI_BCHUNK_BITS,
            remaining: n.min(maxbits - idx),
        })
    }
}

impl<'a> Iterator for BitmapChunks<'a> {
    type Item = (usize, &'a mi_bchunk_t, usize, core::num::NonZeroUsize);

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let chunk_idx = self.chunk_idx;
        let cidx = self.cidx;
        let n = self.remaining.min(MI_BCHUNK_BITS - cidx);
        // The checked constructor bounds the complete range; each advance
        // consumes a nonempty portion of this live chunk before moving onward.
        let (chunk, count) = unsafe {
            (
                self.chunks.get_unchecked(chunk_idx),
                core::num::NonZeroUsize::new_unchecked(n),
            )
        };
        self.remaining -= n;
        self.chunk_idx += 1;
        self.cidx = 0;
        Some((chunk_idx, chunk, cidx, count))
    }
}

impl<'a> BitmapView<'a> {
    /// # Safety
    /// The compact header and published trailing chunks must be initialized,
    /// aligned, atomically writable and live for `'a`. The count must fit
    /// `MI_BITMAP_MAX_CHUNK_COUNT`; count and backing stay stable during the
    /// loan, and mutations must use the bitmap's atomic protocol.
    #[inline(always)]
    pub(crate) unsafe fn from_raw(raw: *const mi_bitmap_t) -> Self {
        unsafe {
            let count = (*raw).chunk_count.load(Ordering::Relaxed);
            let chunks = ptr::addr_of!((*raw).chunks).cast::<mi_bchunk_t>();
            Self {
                raw,
                header: &*raw,
                chunks: core::slice::from_raw_parts(chunks, count),
            }
        }
    }

    #[inline(always)]
    pub(crate) fn set_n(&self, idx: usize, n: usize, palready_set: Option<&mut usize>) -> bool {
        let Some(chunks) = BitmapChunks::new(self.chunks, idx, n) else {
            return false;
        };
        let mut were_allclear = true;
        let mut already_set = 0;
        for (chunk_idx, chunk, cidx, n) in chunks {
            let mut chunk_already_set = 0;
            were_allclear &= mi_bchunk_setN(chunk, cidx, n.get(), Some(&mut chunk_already_set));
            already_set += chunk_already_set;
            mi_bitmap_chunkmap_set(self.header, chunk_idx);
        }
        if let Some(palready_set) = palready_set {
            *palready_set = already_set;
        }
        were_allclear
    }

    #[inline(always)]
    pub(crate) fn clear_n(&self, idx: usize, n: usize) -> bool {
        let Some(chunks) = BitmapChunks::new(self.chunks, idx, n) else {
            return false;
        };
        let mut were_allset = true;
        for (chunk_idx, chunk, cidx, n) in chunks {
            let mut maybe_all_clear = false;
            were_allset &= mi_bchunk_clearN(chunk, cidx, n.get(), Some(&mut maybe_all_clear));
            if maybe_all_clear {
                unsafe { mi_bitmap_chunkmap_try_clear(self.raw, chunk_idx) };
            }
        }
        were_allset
    }

    #[inline(always)]
    pub(crate) fn popcount_n(&self, idx: usize, n: usize) -> usize {
        let Some(chunks) = BitmapChunks::new(self.chunks, idx, n) else {
            return 0;
        };
        let mut popcount = 0;
        for (_, chunk, cidx, n) in chunks {
            popcount += mi_bchunk_popcountN(chunk, cidx, n.get());
        }
        popcount
    }

    #[inline(always)]
    pub(crate) fn is_xset_n(&self, set: mi_xset_t, idx: usize, n: usize) -> bool {
        let Some(chunks) = BitmapChunks::new(self.chunks, idx, n) else {
            return false;
        };
        for (_, chunk, cidx, n) in chunks {
            if !mi_bchunk_is_xsetN(set, chunk, cidx, n.get()) {
                return false;
            }
        }
        true
    }
}

impl<'a> BBitmapView<'a> {
    /// # Safety
    /// The compact header and published trailing chunks must be initialized,
    /// aligned, atomically writable and live for `'a`. The count must fit
    /// `MI_BITMAP_MAX_CHUNK_COUNT`; count and backing stay stable during the
    /// loan, and mutations must use the bitmap's atomic protocol.
    #[inline(always)]
    pub(crate) unsafe fn from_raw(raw: *const mi_bbitmap_t) -> Self {
        unsafe {
            let count = (*raw).chunk_count.load(Ordering::Relaxed);
            let chunks = ptr::addr_of!((*raw).chunks).cast::<mi_bchunk_t>();
            Self {
                raw,
                header: &*raw,
                chunks: core::slice::from_raw_parts(chunks, count),
            }
        }
    }

    #[inline(always)]
    fn max_bits(&self) -> usize {
        self.chunks.len() * MI_BCHUNK_BITS
    }

    #[inline(always)]
    pub(crate) fn set_n(&self, idx: usize, n: usize) -> bool {
        let Some(chunks) = BitmapChunks::new(self.chunks, idx, n) else {
            return false;
        };
        let mut were_allclear = true;
        for (chunk_idx, chunk, cidx, n) in chunks {
            were_allclear &= mi_bchunk_setN(chunk, cidx, n.get(), None);
            unsafe { mi_bbitmap_chunkmap_set(self.raw, chunk_idx, true) };
        }
        were_allclear
    }

    #[inline(always)]
    pub(crate) fn is_xset_n(&self, set: mi_xset_t, idx: usize, n: usize) -> bool {
        let Some(chunks) = BitmapChunks::new(self.chunks, idx, n) else {
            return false;
        };
        for (_, chunk, cidx, n) in chunks {
            if !mi_bchunk_is_xsetN(set, chunk, cidx, n.get()) {
                return false;
            }
        }
        true
    }

    #[inline(always)]
    pub(super) fn try_find_and_clear_generic<F>(
        &self,
        tseq: usize,
        n: usize,
        pidx: &mut usize,
        on_find: F,
    ) -> bool
    where
        F: Fn(&mi_bchunk_t, usize, &mut usize) -> bool,
    {
        let chunk_count = self.chunks.len();
        let cmap_max_count = self.chunks.len().div_ceil(MI_BFIELD_BITS);
        let chunk_acc = self.header.chunk_max_accessed.load(Ordering::Relaxed);
        let cmap_acc = chunk_acc / MI_BFIELD_BITS;
        let cmap_acc_bits = 1 + (chunk_acc % MI_BFIELD_BITS);
        let cmap_mask = mi_bfield_mask(cmap_max_count, 0);
        let cmap_cycle = cmap_acc + 1;
        let bbin = mi_chunkbin_of(n);
        let mut cmap_idx = 0usize;
        let mut found = false;
        let _ = mi_bfield_cycle_iterate(cmap_mask, tseq, cmap_cycle, |cmap_idx_in_cycle| {
            cmap_idx = cmap_idx_in_cycle;
            let cmap_entry = self.header.chunkmap.bfields[cmap_idx].load(Ordering::Relaxed);
            let cmap_entry_cycle = if cmap_idx != cmap_acc {
                MI_BFIELD_BITS
            } else {
                cmap_acc_bits
            };
            if cmap_entry == 0 {
                return true;
            }
            let mut cmap_bins = [0usize; MI_CBIN_COUNT as usize];
            cmap_bins[MI_CBIN_NONE as usize] = cmap_entry;
            let mut ibin = MI_CBIN_SMALL;
            while (ibin as i32) < (MI_CBIN_NONE as i32) {
                let cmap_bin = self.header.chunkmap_bins[ibin as usize].bfields[cmap_idx]
                    .load(Ordering::Relaxed);
                cmap_bins[ibin as usize] = cmap_bin & cmap_entry;
                cmap_bins[MI_CBIN_NONE as usize] &= !cmap_bin;
                ibin = mi_chunkbin_inc(ibin);
            }
            let mut ibin = MI_CBIN_SMALL;
            loop {
                let cmap_bin = cmap_bins[ibin as usize];
                let mut eidx = 0usize;
                let _ =
                    mi_bfield_cycle_iterate(cmap_bin, tseq, cmap_entry_cycle, |eidx_in_cycle| {
                        eidx = eidx_in_cycle;
                        let chunk_idx = (cmap_idx * MI_BFIELD_BITS) + eidx;
                        if chunk_idx >= chunk_count {
                            return true;
                        }
                        let chunk = &self.chunks[chunk_idx];
                        let mut cidx = 0usize;
                        if on_find(chunk, n, &mut cidx) {
                            if cidx == 0 && ibin == MI_CBIN_NONE {
                                mi_bbitmap_set_chunk_bin(self.header, chunk_idx, bbin);
                            }
                            *pidx = (chunk_idx * MI_BCHUNK_BITS) + cidx;
                            crate::internal::mi_debug_assert!(*pidx + n <= self.max_bits());
                            found = true;
                            return false;
                        }
                        unsafe { mi_bbitmap_chunkmap_try_clear(self.raw, chunk_idx) };
                        true
                    });
                if found {
                    break;
                }
                if ibin == MI_CBIN_NONE {
                    break;
                }
                ibin = if ibin == bbin {
                    MI_CBIN_NONE
                } else {
                    mi_chunkbin_inc(ibin)
                };
            }
            !found
        });
        found
    }
}
