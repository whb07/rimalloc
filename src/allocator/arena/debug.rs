//! Arena diagnostics and compatibility printers.
//!
//! This module owns the translated debug rendering helpers for arena slice
//! state. It may inspect arena/page metadata for reporting, but it must not
//! participate in allocation fast paths or mutate allocator ownership state.

use super::*;
#[allow(dead_code, non_camel_case_types)]
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum mi_ansi_color_e {
    MI_BLACK = 30,
    MI_MAROON = 31,
    MI_DARKGREEN = 32,
    MI_ORANGE = 33,
    MI_NAVY = 34,
    MI_PURPLE = 35,
    MI_TEAL = 36,
    MI_GRAY = 37,
    MI_DARKGRAY = 90,
    MI_RED = 91,
    MI_GREEN = 92,
    MI_YELLOW = 93,
    MI_BLUE = 94,
    MI_MAGENTA = 95,
    MI_CYAN = 96,
    MI_WHITE = 97,
}

#[allow(dead_code, non_camel_case_types)]
pub type mi_ansi_color_t = mi_ansi_color_e;

// Retains upstream color constants for arena diagnostics; only a subset is
// used by the current translated output paths.
#[allow(dead_code)]
pub const MI_BLACK: mi_ansi_color_t = mi_ansi_color_e::MI_BLACK;
pub const MI_MAROON: mi_ansi_color_t = mi_ansi_color_e::MI_MAROON;
pub const MI_DARKGREEN: mi_ansi_color_t = mi_ansi_color_e::MI_DARKGREEN;
pub const MI_ORANGE: mi_ansi_color_t = mi_ansi_color_e::MI_ORANGE;
#[allow(dead_code)]
pub const MI_NAVY: mi_ansi_color_t = mi_ansi_color_e::MI_NAVY;
#[allow(dead_code)]
pub const MI_PURPLE: mi_ansi_color_t = mi_ansi_color_e::MI_PURPLE;
pub const MI_TEAL: mi_ansi_color_t = mi_ansi_color_e::MI_TEAL;
pub const MI_GRAY: mi_ansi_color_t = mi_ansi_color_e::MI_GRAY;
#[allow(dead_code)]
pub const MI_DARKGRAY: mi_ansi_color_t = mi_ansi_color_e::MI_DARKGRAY;
#[allow(dead_code)]
pub const MI_RED: mi_ansi_color_t = mi_ansi_color_e::MI_RED;
#[allow(dead_code)]
pub const MI_GREEN: mi_ansi_color_t = mi_ansi_color_e::MI_GREEN;
#[allow(dead_code)]
pub const MI_YELLOW: mi_ansi_color_t = mi_ansi_color_e::MI_YELLOW;
#[allow(dead_code)]
pub const MI_BLUE: mi_ansi_color_t = mi_ansi_color_e::MI_BLUE;
#[allow(dead_code)]
pub const MI_MAGENTA: mi_ansi_color_t = mi_ansi_color_e::MI_MAGENTA;
#[allow(dead_code)]
pub const MI_CYAN: mi_ansi_color_t = mi_ansi_color_e::MI_CYAN;
#[allow(dead_code)]
pub const MI_WHITE: mi_ansi_color_t = mi_ansi_color_e::MI_WHITE;

unsafe fn mi_arena_used_slices(arena: *mut mi_arena_t) -> usize {
    let mut idx = 0usize;
    // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
    if unsafe { mi_bbitmap_bsr_inv((*arena.cast::<MiArenaPrefix>()).slices_free, &mut idx) } {
        idx + 1
    } else {
        unsafe { mi_arena_info_slices(ArenaPtr::new_unchecked(arena)) }
    }
}

unsafe fn mi_debug_show_bfield(field: mi_bfield_t, buf: *mut u8, k: *mut usize) -> usize {
    let mut bit_set_count = 0usize;
    for bit in 0..MI_BFIELD_BITS {
        let is_set = ((1usize << bit) & field) != 0;
        if is_set {
            bit_set_count += 1;
        }
        // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
        unsafe {
            *buf.add(*k) = if is_set { b'x' } else { b'.' };
            *k += 1;
        }
    }
    bit_set_count
}

unsafe fn mi_debug_color(buf: *mut u8, k: *mut usize, color: mi_ansi_color_t) {
    let code = color as i32;
    let tens = ((code / 10) as u8) + b'0';
    let ones = ((code % 10) as u8) + b'0';
    // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
    unsafe {
        *buf.add(*k) = 0x1B;
        *buf.add(*k + 1) = b'[';
        *buf.add(*k + 2) = tens;
        *buf.add(*k + 3) = ones;
        *buf.add(*k + 4) = b'm';
        *k += 5;
    }
}

unsafe fn mi_page_commit_usage(
    page: PagePtr,
    committed_size: crate::core_types::CommittedBytes,
) -> c_int {
    let page = page.as_ptr();
    let committed_size = committed_size.get();
    let used_size =
        // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
        unsafe { crate::page::mi_page_used_count(page).as_usize() * mi_page_block_size(page) };
    (used_size * 100 / committed_size) as c_int
}

unsafe fn mi_debug_show_page_bfield(
    buf: *mut u8,
    k: *mut usize,
    arena: *mut mi_arena_t,
    slice_index: SliceIndex,
    pbit_of_page: *mut isize,
    pcolor_of_page: *mut mi_ansi_color_t,
) -> usize {
    let mut bit_set_count = 0usize;
    // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
    let mut bit_of_page = unsafe { *pbit_of_page };
    let mut color = unsafe { *pcolor_of_page };
    let mut prev_color = MI_GRAY;
    for bit in 0..MI_BFIELD_BITS {
        let Some(slice) = slice_index.checked_add(bit) else {
            crate::internal::mi_assert_internal!(false);
            break;
        };
        let slice_index_usize = slice.get();
        // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
        let start = unsafe {
            mi_arena_slice_start(ArenaPtr::new_unchecked(arena), slice)
                .as_ptr()
                .cast::<c_void>()
        };
        // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
        let page = unsafe { _mi_safe_ptr_page(start) };
        if !page.is_null() && start == unsafe { mi_page_slice_start(page) }.cast::<c_void>() {
            crate::internal::mi_assert_internal!(bit_of_page <= 0);
            bit_set_count += 1;
            let mut c = b'p';
            // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
            if unsafe { mi_page_is_singleton(page) } {
                c = b's';
            } else if unsafe { mi_page_is_full(page) } {
                c = b'f';
            }
            // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
            if !unsafe { mi_page_is_abandoned(page) } {
                c = _mi_toupper(c as i8) as u8;
            }
            let page_ptr = unsafe { PagePtr::new_unchecked(page) };
            let commit_usage =
                // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
                unsafe { mi_page_commit_usage(page_ptr, mi_page_committed_typed(page)) };
            if commit_usage < 25 {
                color = MI_MAROON;
            } else if commit_usage < 50 {
                color = MI_ORANGE;
            } else if commit_usage < 75 {
                color = MI_TEAL;
            } else {
                color = MI_DARKGREEN;
            }
            // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
            bit_of_page = match crate::core_types::MemId::view(unsafe { (*page).memid }) {
                crate::core_types::MemIdView::Arena(a) => a.range.count_usize() as isize,
                _ => {
                    crate::internal::mi_assert_internal!(false);
                    0
                }
            };
            if color != prev_color {
                // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
                unsafe { mi_debug_color(buf, k, color) };
                prev_color = color;
            }
            unsafe {
                *buf.add(*k) = c;
                *k += 1;
            }
        } else {
            let mut c = b'?';
            if bit_of_page > 0 {
                c = b'-';
            // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
            } else if unsafe { _mi_meta_is_meta_page(start) } {
                c = b'm';
                color = MI_GRAY;
            } else if slice_index_usize < unsafe { (*arena.cast::<MiArenaPrefix>()).info_slices } {
                c = b'i';
                color = MI_GRAY;
            // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
            } else if unsafe {
                mi_bbitmap_is_setN(
                    (*arena.cast::<MiArenaPrefix>()).slices_free,
                    slice_index_usize,
                    1,
                )
            } {
                // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
                if unsafe {
                    mi_bitmap_is_set(
                        (*arena.cast::<MiArenaPrefix>()).slices_purge,
                        slice_index_usize,
                    )
                } {
                    c = b'~';
                    color = MI_ORANGE;
                // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
                } else if unsafe {
                    mi_bitmap_is_set(
                        (*arena.cast::<MiArenaPrefix>()).slices_committed,
                        slice_index_usize,
                    )
                } {
                    c = b'_';
                    color = MI_GRAY;
                } else {
                    c = b'.';
                    color = MI_GRAY;
                }
            }
            if bit == MI_BFIELD_BITS - 1 && bit_of_page > 1 {
                c = b'>';
            }
            if color != prev_color {
                // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
                unsafe { mi_debug_color(buf, k, color) };
                prev_color = color;
            }
            unsafe {
                *buf.add(*k) = c;
                *k += 1;
            }
        }
        bit_of_page -= 1;
    }
    // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
    unsafe { mi_debug_color(buf, k, MI_GRAY) };
    unsafe {
        *pbit_of_page = bit_of_page;
        *pcolor_of_page = color;
    }
    bit_set_count
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
unsafe fn mi_debug_show_chunks(
    header1: &str,
    header2: &str,
    header3: &str,
    slice_count: SliceCount,
    chunk_count: usize,
    chunks: *mut mi_bchunk_t,
    chunk_bins: *mut mi_bchunkmap_t,
    invert: bool,
    arena: *mut mi_arena_t,
    narrow: bool,
) -> usize {
    crate::options::_mi_raw_message(format_args!(
        "\x1B[37m{}{}{} (use/commit: \x1B[31m0 - 25%\x1B[33m - 50%\x1B[36m - 75%\x1B[32m - 100%\x1B[0m)\n",
        header1, header2, header3
    ));
    let fields_per_line = if narrow { 2 } else { 4 };
    let slice_count = slice_count.get() as usize;
    // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
    let used_slice_count = unsafe { mi_arena_used_slices(arena) };
    let mut bit_count = 0usize;
    let mut bit_set_count = 0usize;
    let mut bit_of_page = 0isize;
    let mut color_of_page = MI_GRAY;
    let mut i = 0usize;
    while i < chunk_count {
        if bit_count >= slice_count {
            break;
        }
        let mut buf = [0u8; 5 * MI_BCHUNK_BITS + 64];
        if bit_count > used_slice_count && i + 2 < chunk_count {
            let diff = chunk_count - 1 - i;
            bit_count += diff * MI_BCHUNK_BITS;
            crate::options::_mi_raw_message(format_args!("  |\n"));
            i = chunk_count - 1;
        }

        let mut k = 0usize;
        if i < 10 {
            buf[k] = b'0' + i as u8;
            buf[k + 1] = b' ';
            buf[k + 2] = b' ';
            k += 3;
        } else if i < 100 {
            buf[k] = b'0' + (i / 10) as u8;
            buf[k + 1] = b'0' + (i % 10) as u8;
            buf[k + 2] = b' ';
            k += 3;
        } else if i < 1000 {
            buf[k] = b'0' + (i / 100) as u8;
            buf[k + 1] = b'0' + ((i % 100) / 10) as u8;
            buf[k + 2] = b'0' + (i % 10) as u8;
            k += 3;
        }

        let mut chunk_kind = b' ';
        if !chunk_bins.is_null() {
            // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
            chunk_kind = match unsafe { mi_bbitmap_debug_get_bin(chunk_bins, i) } {
                MI_CBIN_SMALL => b'S',
                MI_CBIN_MEDIUM => b'M',
                MI_CBIN_LARGE => b'L',
                MI_CBIN_HUGE => b'H',
                MI_CBIN_OTHER => b'X',
                _ => b' ',
            };
        }
        buf[k] = chunk_kind;
        buf[k + 1] = b' ';
        k += 2;

        for j in 0..MI_BCHUNK_FIELDS {
            if j > 0 && (j % fields_per_line) == 0 {
                // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
                let line = unsafe { core::str::from_utf8_unchecked(&buf[..k]) };
                crate::options::_mi_raw_message(format_args!("  {}\n\x1B[37m", line));
                buf.fill(0);
                buf[..5].fill(b' ');
                k = 5;
            }
            if bit_count < slice_count {
                let mut bfield = 0usize;
                if !chunks.is_null() {
                    // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
                    bfield = unsafe { (*chunks.add(i)).bfields[j].load(Ordering::Relaxed) };
                }
                if invert {
                    bfield = !bfield;
                }
                let mut line_k = k;
                let xcount = if chunks.is_null() {
                    // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
                    unsafe {
                        mi_debug_show_page_bfield(
                            buf.as_mut_ptr(),
                            &mut line_k,
                            arena,
                            SliceIndex::new(bit_count),
                            &mut bit_of_page,
                            &mut color_of_page,
                        )
                    }
                } else {
                    // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
                    unsafe { mi_debug_show_bfield(bfield, buf.as_mut_ptr(), &mut line_k) }
                };
                k = line_k;
                bit_set_count += if invert {
                    MI_BFIELD_BITS - xcount
                } else {
                    xcount
                };
                buf[k] = b' ';
                k += 1;
            } else {
                buf[k..k + MI_BFIELD_BITS].fill(b'o');
                k += MI_BFIELD_BITS;
            }
            bit_count += MI_BFIELD_BITS;
        }
        // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
        let line = unsafe { core::str::from_utf8_unchecked(&buf[..k]) };
        crate::options::_mi_raw_message(format_args!("  {}\n\x1B[37m", line));
        i += 1;
    }
    crate::options::_mi_raw_message(format_args!("\x1B[0m  total pages: {}\n", bit_set_count));
    bit_set_count
}

unsafe fn mi_debug_show_arenas_ex(heap: BorrowedHeap, show_pages: bool, narrow: bool) {
    // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
    let Some(subproc) = (unsafe { heap.ptr().subproc() }) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    let subproc_ptr = subproc;
    let subproc = subproc.as_ptr();
    // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
    let max_arenas = unsafe { mi_arenas_get_count_typed(subproc_ptr) };
    let mut page_total = 0usize;
    for i in 0..max_arenas {
        let Some(arena_idx) = ArenaIndex::from_usize(i) else {
            crate::internal::mi_assert_internal!(false);
            continue;
        };
        let arena =
            // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
            unsafe { mi_arena_from_index_typed(SubprocPtr::new_unchecked(subproc), arena_idx) };
        if arena.is_null() {
            break;
        }
        crate::internal::mi_assert!(unsafe { (*arena.cast::<MiArenaPrefix>()).subproc == subproc });
        crate::options::_mi_raw_message(format_args!(
            "{}arena {} at {:p}: {} slices ({} MiB){}{}, subproc: {:p}, numa: {}\n",
            // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
            if unsafe { (*arena.cast::<MiArenaPrefix>()).parent.is_null() } {
                ""
            } else {
                "(sub)"
            },
            i,
            arena,
            // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
            unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count },
            mi_size_of_slices(unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count }) / MI_MiB,
            if unsafe { (*arena.cast::<MiArenaPrefix>()).memid.is_pinned } {
                ", pinned"
            } else {
                ""
            },
            // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
            if unsafe { (*arena.cast::<MiArenaPrefix>()).is_exclusive } {
                ", exclusive"
            } else {
                ""
            },
            // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
            unsafe { (*arena.cast::<MiArenaPrefix>()).subproc },
            unsafe { (*arena.cast::<MiArenaPrefix>()).numa_node },
        ));
        if show_pages {
            let header1 = "chunks (p:page, f:full, s:singleton, P,F,S:not abandoned, i:arena-info, m:meta-data, ~:free-purgable, _:free-committed, .:free-reserved)";
            let header2 = if narrow { "\n       " } else { " " };
            let header3 = "(chunk bin: S:small, M : medium, L : large, X : other)";
            // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
            let slices_free = unsafe { (*arena.cast::<MiArenaPrefix>()).slices_free };
            let Some(slice_count) =
                SliceCount::from_usize(unsafe { (*arena.cast::<MiArenaPrefix>()).slice_count })
            else {
                crate::internal::mi_assert_internal!(false);
                continue;
            };
            // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
            page_total += unsafe {
                mi_debug_show_chunks(
                    header1,
                    header2,
                    header3,
                    slice_count,
                    mi_bbitmap_chunk_count(slices_free),
                    ptr::null_mut(),
                    (*slices_free).chunkmap_bins.as_mut_ptr(),
                    false,
                    arena,
                    narrow,
                )
            };
        }
    }
    if show_pages {
        crate::options::_mi_raw_message(format_args!("total pages in arenas: {}\n", page_total));
    }
}

/// # Safety
/// Matches the C API and reads the current sub-process arena registry.
// SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_debug_show_arenas() {
    let Some(heap) = HeapPtr::new(unsafe { mi_heap_main() }) else {
        crate::internal::mi_assert_internal!(false);
        return;
    };
    // SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
    unsafe { mi_debug_show_arenas_ex(BorrowedHeap::new_unchecked(heap), true, true) }
}

/// # Safety
/// Matches the C API and forwards to the current arena debug printer.
// SAFETY[AUDIT:arena-debug-render]: Diagnostic-only rendering reads allocator-owned arena/page metadata and writes local output buffers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_arenas_print() {
    unsafe { mi_debug_show_arenas() }
}

type MiDebugShowChunksFunT = unsafe fn(
    &str,
    &str,
    &str,
    SliceCount,
    usize,
    *mut mi_bchunk_t,
    *mut mi_bchunkmap_t,
    bool,
    *mut mi_arena_t,
    bool,
) -> usize;

#[used]
static MI_DEBUG_SHOW_CHUNKS_KEEP: MiDebugShowChunksFunT = mi_debug_show_chunks;
