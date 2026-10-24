//! Transparent huge pages only for large heaps (Rust-only; `UPSTREAM_DIFF.md`).
//!
//! C asks for transparent huge pages on every arena it reserves, so on a host in `madvise` THP
//! mode every 2 MiB region a process touches becomes resident; arenas start each thread's search
//! in a different chunk, so a small multi-threaded process holds several mostly empty huge pages.
//! Here arenas are reserved without the request. The first time the arenas have handed out
//! `MI_THP_ARENA_THRESHOLD` of pages, every arena and every later reservation asks for them, and what
//! the arenas already hold is collapsed into huge pages, so a heap that grew past the threshold is
//! covered as under C (`larsonN`: 76 of 82 MiB, against 42 without the collapse). An arena
//! reserved while another thread arms may miss the request; it then only runs without huge pages.

use super::*;

use core::sync::atomic::{AtomicBool, AtomicUsize};

/// Arena page bytes in use above which huge pages are requested.
const MI_THP_ARENA_THRESHOLD: usize = 32 * MI_MiB;

static ARENA_PAGE_BYTES: AtomicUsize = AtomicUsize::new(0);
static THP_ARMED: AtomicBool = AtomicBool::new(false);

/// Whether new OS memory for arenas should ask for transparent huge pages.
#[inline]
pub(crate) fn mi_arena_thp_armed() -> bool {
    THP_ARMED.load(Ordering::Relaxed)
}

/// Count `bytes` of arena pages handed out, and arm huge pages on first crossing the threshold.
///
/// # Safety
/// `subproc` must be the live subprocess whose arenas served the pages.
pub(super) unsafe fn mi_arena_thp_pages_added(subproc: SubprocPtr, bytes: usize) {
    // Once armed the count is no longer needed.
    if THP_ARMED.load(Ordering::Relaxed) {
        return;
    }
    let total = ARENA_PAGE_BYTES.fetch_add(bytes, Ordering::Relaxed) + bytes;
    if total < MI_THP_ARENA_THRESHOLD || THP_ARMED.swap(true, Ordering::AcqRel) {
        return;
    }
    #[cfg(any(windows, target_os = "emscripten", target_os = "wasi"))]
    let _ = subproc;
    // SAFETY: registered arenas stay live, and each arena's header starts its mapping.
    #[cfg(all(not(windows), not(target_os = "emscripten"), not(target_os = "wasi")))]
    unsafe {
        for index in 0..mi_arenas_get_count_typed(subproc) {
            let Some(index) = ArenaIndex::from_usize(index) else {
                continue;
            };
            let arena = mi_arena_from_index_typed(subproc, index);
            if !arena.is_null() {
                let slices = (*arena.cast::<MiArenaPrefix>()).slice_count;
                crate::platform::unix::_mi_prim_thp_enable(arena.cast(), mi_size_of_slices(slices));
            }
        }
    }
}

/// Count `bytes` of arena pages returned.
pub(super) fn mi_arena_thp_pages_removed(bytes: usize) {
    if !THP_ARMED.load(Ordering::Relaxed) {
        ARENA_PAGE_BYTES.fetch_sub(bytes, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Handing out arena pages past the threshold arms huge pages (arming is one-way, so this
    /// holds whatever other tests in the process have allocated).
    #[test]
    #[cfg_attr(
        miri,
        ignore = "requires OS allocator initialization (unsupported sysconf)"
    )]
    fn arena_pages_past_the_threshold_arm_huge_pages() {
        // SAFETY: each block is allocated and freed once on this thread.
        unsafe {
            let blocks: [*mut c_void; 16] = core::array::from_fn(|_| {
                crate::allocator::alloc::mi_malloc(MI_THP_ARENA_THRESHOLD / 8)
            });
            assert!(blocks.iter().all(|p| !p.is_null()));
            assert!(mi_arena_thp_armed());
            for p in blocks {
                crate::allocator::free::mi_free(p);
            }
        }
    }
}
