//! Borrowed views over the initialized compact bitmap allocation.
//! Outlined kernels receive the borrowed fields directly so callers do not
//! materialize a temporary view just to pass its address.

#![forbid(unsafe_code)]

use super::*;

/// Shared atomic access to the header and its actual trailing chunk extent.
pub(crate) struct BitmapView<'a> {
    header: &'a mi_bitmap_t,
    chunks: &'a [mi_bchunk_t],
}

pub(crate) struct BBitmapView<'a> {
    header: &'a mi_bbitmap_t,
    chunks: &'a [mi_bchunk_t],
}

impl<'a> BitmapView<'a> {
    #[inline(always)]
    pub(super) fn new(header: &'a mi_bitmap_t, chunks: &'a [mi_bchunk_t]) -> Self {
        Self { header, chunks }
    }

    #[inline(always)]
    pub(crate) fn set_n(&self, idx: usize, n: usize, palready_set: Option<&mut usize>) -> bool {
        Self::set_n_in(self.header, self.chunks, idx, n, palready_set)
    }

    #[inline(never)]
    fn set_n_in(
        header: &mi_bitmap_t,
        chunks: &[mi_bchunk_t],
        idx: usize,
        n: usize,
        palready_set: Option<&mut usize>,
    ) -> bool {
        let Some(chunks) = BitmapChunks::new(chunks, idx, n) else {
            return false;
        };
        let mut were_allclear = true;
        let mut already_set = 0;
        for (chunk_idx, chunk, cidx, n) in chunks {
            let mut chunk_already_set = 0;
            were_allclear &= mi_bchunk_setN(chunk, cidx, n.get(), Some(&mut chunk_already_set));
            already_set += chunk_already_set;
            mi_bitmap_chunkmap_set(header, chunk_idx);
        }
        if let Some(palready_set) = palready_set {
            *palready_set = already_set;
        }
        were_allclear
    }

    #[inline(always)]
    pub(crate) fn clear_n(&self, idx: usize, n: usize) -> bool {
        Self::clear_n_in(self.header, self.chunks, idx, n)
    }

    #[inline(never)]
    fn clear_n_in(header: &mi_bitmap_t, chunks: &[mi_bchunk_t], idx: usize, n: usize) -> bool {
        let Some(chunks) = BitmapChunks::new(chunks, idx, n) else {
            return false;
        };
        let mut were_allset = true;
        for (chunk_idx, chunk, cidx, n) in chunks {
            let mut maybe_all_clear = false;
            were_allset &= mi_bchunk_clearN(chunk, cidx, n.get(), Some(&mut maybe_all_clear));
            if maybe_all_clear {
                mi_bitmap_chunkmap_try_clear(header, chunk_idx, chunk);
            }
        }
        were_allset
    }

    #[inline(always)]
    pub(crate) fn popcount_n(&self, idx: usize, n: usize) -> usize {
        Self::popcount_n_in(self.chunks, idx, n)
    }

    #[inline(never)]
    fn popcount_n_in(chunks: &[mi_bchunk_t], idx: usize, n: usize) -> usize {
        let Some(chunks) = BitmapChunks::new(chunks, idx, n) else {
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
        Self::is_xset_n_in(self.chunks, set, idx, n)
    }

    #[inline(never)]
    fn is_xset_n_in(chunks: &[mi_bchunk_t], set: mi_xset_t, idx: usize, n: usize) -> bool {
        let Some(chunks) = BitmapChunks::new(chunks, idx, n) else {
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
    #[inline(always)]
    pub(super) fn new(header: &'a mi_bbitmap_t, chunks: &'a [mi_bchunk_t]) -> Self {
        Self { header, chunks }
    }

    #[inline(always)]
    pub(crate) fn try_find_and_clear_n(&self, tseq: usize, n: usize, pidx: &mut usize) -> bool {
        let found = if n == 1 {
            Self::try_find_one(self.header, self.chunks, tseq, pidx)
        } else if n == 8 {
            Self::try_find_eight(self.header, self.chunks, tseq, pidx)
        } else if n == 0 {
            false
        } else if n <= MI_BFIELD_BITS {
            Self::try_find_field(self.header, self.chunks, tseq, n, pidx)
        } else if n <= MI_BCHUNK_BITS {
            Self::try_find_chunk(self.header, self.chunks, tseq, n, pidx)
        } else {
            Self::try_find_chunks(self.header, self.chunks, n, pidx)
        };
        #[cfg(feature = "bench-diagnostics")]
        crate::allocator::bench_diagnostics::bitmap_find_clear(found);
        found
    }

    #[inline(never)]
    fn try_find_one(
        header: &mi_bbitmap_t,
        chunks: &[mi_bchunk_t],
        tseq: usize,
        pidx: &mut usize,
    ) -> bool {
        Self::try_find_and_clear_generic(
            header,
            chunks,
            tseq,
            1,
            pidx,
            mi_bchunk_try_find_and_clear_1,
        )
    }

    #[inline(never)]
    fn try_find_eight(
        header: &mi_bbitmap_t,
        chunks: &[mi_bchunk_t],
        tseq: usize,
        pidx: &mut usize,
    ) -> bool {
        Self::try_find_and_clear_generic(
            header,
            chunks,
            tseq,
            8,
            pidx,
            mi_bchunk_try_find_and_clear_8,
        )
    }

    #[inline(never)]
    fn try_find_field(
        header: &mi_bbitmap_t,
        chunks: &[mi_bchunk_t],
        tseq: usize,
        n: usize,
        pidx: &mut usize,
    ) -> bool {
        crate::internal::mi_debug_assert!(n <= MI_BFIELD_BITS);
        Self::try_find_and_clear_generic(
            header,
            chunks,
            tseq,
            n,
            pidx,
            mi_bchunk_try_find_and_clearNX,
        )
    }

    #[inline(never)]
    fn try_find_chunk(
        header: &mi_bbitmap_t,
        chunks: &[mi_bchunk_t],
        tseq: usize,
        n: usize,
        pidx: &mut usize,
    ) -> bool {
        crate::internal::mi_debug_assert!(n <= MI_BCHUNK_BITS);
        Self::try_find_and_clear_generic(
            header,
            chunks,
            tseq,
            n,
            pidx,
            mi_bchunk_try_find_and_clearNC,
        )
    }

    #[inline(never)]
    fn try_find_chunks(
        header: &mi_bbitmap_t,
        chunks: &[mi_bchunk_t],
        n: usize,
        pidx: &mut usize,
    ) -> bool {
        let chunk_req = n.div_ceil(MI_BCHUNK_BITS);
        if chunks.len() < chunk_req {
            return false;
        }
        let mut chunk_idx = 0usize;
        while chunk_idx <= chunks.len() - chunk_req {
            let remaining = &chunks[chunk_idx..];
            let count = remaining
                .iter()
                .take(chunk_req)
                .take_while(|chunk| mi_bchunk_all_are_set_relaxed(chunk))
                .count();
            if count == chunk_req && Self::try_clear_prefix(&remaining[..chunk_req], n) {
                *pidx = chunk_idx * MI_BCHUNK_BITS;
                for i in 0..count {
                    mi_bbitmap_set_chunk_bin(header, chunk_idx + i, MI_CBIN_HUGE);
                }
                crate::internal::mi_debug_assert!(*pidx + n <= chunks.len() * MI_BCHUNK_BITS);
                return true;
            }
            chunk_idx += count + 1;
        }
        false
    }

    #[inline]
    fn try_clear_prefix(chunks: &[mi_bchunk_t], mut n: usize) -> bool {
        for (completed, chunk) in chunks.iter().enumerate() {
            if !mi_bchunk_try_clearN(chunk, 0, n.min(MI_BCHUNK_BITS), None) {
                for chunk in chunks[..completed].iter().rev() {
                    mi_bchunk_setN(chunk, 0, MI_BCHUNK_BITS, None);
                }
                return false;
            }
            n = n.saturating_sub(MI_BCHUNK_BITS);
            if n == 0 {
                return true;
            }
        }
        false
    }

    #[inline(always)]
    pub(crate) fn set_n(&self, idx: usize, n: usize) -> bool {
        Self::set_n_in(self.header, self.chunks, idx, n)
    }

    #[inline(never)]
    fn set_n_in(header: &mi_bbitmap_t, chunks: &[mi_bchunk_t], idx: usize, n: usize) -> bool {
        let Some(chunks) = BitmapChunks::new(chunks, idx, n) else {
            return false;
        };
        let mut were_allclear = true;
        for (chunk_idx, chunk, cidx, n) in chunks {
            were_allclear &= mi_bchunk_setN(chunk, cidx, n.get(), None);
            mi_bbitmap_chunkmap_set(header, chunk_idx, chunk, true);
        }
        were_allclear
    }

    #[inline(always)]
    pub(crate) fn is_xset_n(&self, set: mi_xset_t, idx: usize, n: usize) -> bool {
        Self::is_xset_n_in(self.chunks, set, idx, n)
    }

    #[inline(never)]
    fn is_xset_n_in(chunks: &[mi_bchunk_t], set: mi_xset_t, idx: usize, n: usize) -> bool {
        let Some(chunks) = BitmapChunks::new(chunks, idx, n) else {
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
    fn try_find_and_clear_generic<F>(
        header: &mi_bbitmap_t,
        chunks: &[mi_bchunk_t],
        tseq: usize,
        n: usize,
        pidx: &mut usize,
        on_find: F,
    ) -> bool
    where
        F: Fn(&mi_bchunk_t, usize, &mut usize) -> bool,
    {
        let chunk_count = chunks.len();
        let cmap_max_count = chunks.len().div_ceil(MI_BFIELD_BITS);
        let chunk_acc = header.chunk_max_accessed.load(Ordering::Relaxed);
        let cmap_acc = chunk_acc / MI_BFIELD_BITS;
        let cmap_acc_bits = 1 + (chunk_acc % MI_BFIELD_BITS);
        let cmap_mask = mi_bfield_mask(cmap_max_count, 0);
        let cmap_cycle = cmap_acc + 1;
        let bbin = mi_chunkbin_of(n);
        let mut cmap_idx = 0usize;
        let mut found = false;
        let _ = mi_bfield_cycle_iterate(cmap_mask, tseq, cmap_cycle, |cmap_idx_in_cycle| {
            cmap_idx = cmap_idx_in_cycle;
            let cmap_entry = header.chunkmap.bfields[cmap_idx].load(Ordering::Relaxed);
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
                let cmap_bin =
                    header.chunkmap_bins[ibin as usize].bfields[cmap_idx].load(Ordering::Relaxed);
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
                        let chunk = &chunks[chunk_idx];
                        let mut cidx = 0usize;
                        if on_find(chunk, n, &mut cidx) {
                            if cidx == 0 && ibin == MI_CBIN_NONE {
                                mi_bbitmap_set_chunk_bin(header, chunk_idx, bbin);
                            }
                            *pidx = (chunk_idx * MI_BCHUNK_BITS) + cidx;
                            crate::internal::mi_debug_assert!(
                                *pidx + n <= chunks.len() * MI_BCHUNK_BITS
                            );
                            found = true;
                            return false;
                        }
                        mi_bbitmap_chunkmap_try_clear(header, chunk_idx, chunk);
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
