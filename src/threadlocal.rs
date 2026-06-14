//! Dynamic thread-local key management.

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    clippy::missing_safety_doc,
    unsafe_op_in_unsafe_fn
)]

use core::alloc::Layout;
use core::cell::Cell;
use core::mem::{align_of, size_of};
use core::ptr;
use core::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::alloc::{alloc_zeroed, dealloc, realloc};

use crate::atomic::{mi_lock_acquire, mi_lock_done, mi_lock_init, mi_lock_release, mi_lock_t};
use crate::bitmap::{
    mi_bitmap_init, mi_bitmap_max_bits, mi_bitmap_set, mi_bitmap_size, mi_bitmap_t,
    mi_bitmap_try_find_and_claim, mi_bitmap_unsafe_setN,
};
use crate::bits::MI_SIZE_BITS;
use crate::types::{mi_arena_t, mi_thread_local_t, mi_thread_locals_t, mi_tls_slot_t};

pub const MI_TLS_IDX_BITS: usize = if MI_SIZE_BITS < 64 {
    MI_SIZE_BITS / 2
} else {
    MI_SIZE_BITS / 4
};
pub const MI_TLS_IDX_MASK: usize = (1usize << MI_TLS_IDX_BITS) - 1;
pub const MI_TLS_IDX_MAX: usize = MI_TLS_IDX_MASK;
pub const MI_TLS_VERSION_MAX: usize = usize::MAX >> MI_TLS_IDX_BITS;

static mi_thread_locals_empty: mi_thread_locals_t = mi_thread_locals_t {
    count: 0,
    slots: [mi_tls_slot_t {
        version: 0,
        value: ptr::null_mut(),
    }],
};

thread_local! {
    pub static mi_thread_locals: Cell<*mut mi_thread_locals_t> = const { Cell::new(ptr::null_mut()) };
}

static mi_thread_locals_lock: mi_lock_t = mi_lock_t::new();
static mi_thread_locals_free: AtomicPtr<mi_bitmap_t> = AtomicPtr::new(ptr::null_mut());
static mi_thread_locals_version: AtomicUsize = AtomicUsize::new(0);

#[inline]
fn thread_locals_ptr() -> *mut mi_thread_locals_t {
    mi_thread_locals.with(|slot| {
        let mut tls = slot.get();
        if tls.is_null() {
            tls = ptr::from_ref(&mi_thread_locals_empty).cast_mut();
            slot.set(tls);
        }
        tls
    })
}

#[inline]
fn set_thread_locals_ptr(tls: *mut mi_thread_locals_t) {
    mi_thread_locals.with(|slot| slot.set(tls));
}

#[inline]
fn tls_slots(tls: *mut mi_thread_locals_t) -> *mut mi_tls_slot_t {
    unsafe { (*tls).slots.as_mut_ptr() }
}

#[inline]
fn thread_locals_layout(count: usize) -> Option<Layout> {
    let bytes = size_of::<mi_thread_locals_t>()
        .checked_add(count.checked_mul(size_of::<mi_tls_slot_t>())?)?;
    Layout::from_size_align(bytes, align_of::<mi_thread_locals_t>()).ok()
}

#[inline]
fn bitmap_layout(bit_count: usize) -> Option<Layout> {
    let size = unsafe { mi_bitmap_size(bit_count, ptr::null_mut()) };
    Layout::from_size_align(size, align_of::<mi_bitmap_t>()).ok()
}

#[inline]
fn mi_key_index(key: usize) -> usize {
    key & MI_TLS_IDX_MASK
}

#[inline]
fn mi_key_version(key: usize) -> usize {
    key >> MI_TLS_IDX_BITS
}

#[inline]
fn mi_key_create(index: usize, version: usize) -> mi_thread_local_t {
    crate::mi_debug_assert!(version != 0 && version <= MI_TLS_VERSION_MAX);
    crate::mi_debug_assert!(index <= MI_TLS_IDX_MAX);
    let key = (version << MI_TLS_IDX_BITS) | index;
    crate::mi_debug_assert!(key != 0);
    key
}

#[inline]
fn mi_thread_locals_expand(least_idx: usize) -> Option<*mut mi_thread_locals_t> {
    let tls_old = thread_locals_ptr();
    let count_old = unsafe { (*tls_old).count };
    let mut count = if count_old == 0 {
        16
    } else if count_old >= 1024 {
        count_old.saturating_add(1024)
    } else {
        count_old.saturating_mul(2)
    };
    if count <= least_idx {
        count = least_idx + 1;
    }
    if count > MI_TLS_IDX_MAX {
        return None;
    }

    let layout_new = thread_locals_layout(count)?;
    let tls = if count_old == 0 {
        unsafe { alloc_zeroed(layout_new) as *mut mi_thread_locals_t }
    } else {
        let layout_old = thread_locals_layout(count_old)?;
        let new_ptr = unsafe { realloc(tls_old.cast::<u8>(), layout_old, layout_new.size()) };
        if new_ptr.is_null() {
            return None;
        }
        let tls = new_ptr.cast::<mi_thread_locals_t>();
        if layout_new.size() > layout_old.size() {
            unsafe {
                ptr::write_bytes(
                    new_ptr.add(layout_old.size()),
                    0,
                    layout_new.size() - layout_old.size(),
                );
            }
        }
        tls
    };

    if tls.is_null() {
        return None;
    }
    unsafe {
        (*tls).count = count;
    }
    set_thread_locals_ptr(tls);
    Some(tls)
}

#[inline(never)]
fn mi_thread_local_set_expand(key: mi_thread_local_t, val: *mut core::ffi::c_void) -> bool {
    if val.is_null() {
        return true;
    }
    let idx = mi_key_index(key);
    let tls = match mi_thread_locals_expand(idx) {
        Some(tls) => tls,
        None => return false,
    };
    crate::mi_debug_assert!(tls == thread_locals_ptr());
    crate::mi_debug_assert!(idx < unsafe { (*tls).count });
    unsafe {
        let slots = tls_slots(tls);
        (*slots.add(idx)).value = val;
        (*slots.add(idx)).version = mi_key_version(key);
    }
    true
}

#[inline]
unsafe extern "C" fn mi_thread_local_claim_fun(
    _slice_index: usize,
    _arena: *mut mi_arena_t,
    keep_set: *mut bool,
) -> bool {
    *keep_set = false;
    true
}

fn mi_thread_local_claim() -> mi_thread_local_t {
    let slots = mi_thread_locals_free.load(Ordering::Acquire);
    let mut idx = 0usize;
    if slots.is_null() {
        return 0;
    }
    let mut claim_fun: crate::types::mi_claim_fun_t = mi_thread_local_claim_fun;
    if unsafe {
        mi_bitmap_try_find_and_claim(
            slots,
            0,
            &mut idx as *mut usize,
            &mut claim_fun as *mut crate::types::mi_claim_fun_t,
            ptr::null_mut(),
        )
    } {
        let mut version = mi_thread_locals_version
            .load(Ordering::Relaxed)
            .wrapping_add(1);
        if version >= MI_TLS_VERSION_MAX {
            version = 1;
        }
        mi_thread_locals_version.store(version, Ordering::Relaxed);
        mi_key_create(idx, version)
    } else {
        0
    }
}

fn mi_thread_local_create_expand() -> bool {
    let slots = mi_thread_locals_free.load(Ordering::Acquire);
    let oldcount = if slots.is_null() {
        0
    } else {
        unsafe { mi_bitmap_max_bits(slots) }
    };
    let Some(newcount) = oldcount.checked_add(1024) else {
        return false;
    };
    if newcount > MI_TLS_IDX_MAX {
        return false;
    }
    let Some(layout_new) = bitmap_layout(newcount) else {
        return false;
    };
    let newslots = unsafe { alloc_zeroed(layout_new).cast::<mi_bitmap_t>() };
    if newslots.is_null() {
        return false;
    }

    if !slots.is_null() {
        let oldsize = unsafe { mi_bitmap_size(oldcount, ptr::null_mut()) };
        let Some(layout_old) = Layout::from_size_align(oldsize, align_of::<mi_bitmap_t>()).ok()
        else {
            unsafe {
                dealloc(newslots.cast::<u8>(), layout_new);
            }
            return false;
        };
        unsafe {
            ptr::copy_nonoverlapping(slots.cast::<u8>(), newslots.cast::<u8>(), oldsize);
            dealloc(slots.cast::<u8>(), layout_old);
        }
    }

    unsafe {
        mi_bitmap_init(newslots, newcount, true);
        mi_bitmap_unsafe_setN(newslots, oldcount, newcount - oldcount);
    }
    mi_thread_locals_free.store(newslots, Ordering::Release);
    true
}

fn free_thread_locals(tls: *mut mi_thread_locals_t) {
    if tls.is_null() || core::ptr::eq(tls, ptr::from_ref(&mi_thread_locals_empty).cast_mut()) {
        return;
    }
    let count = unsafe { (*tls).count };
    if count == 0 {
        return;
    }
    let Some(layout) = thread_locals_layout(count) else {
        return;
    };
    unsafe {
        dealloc(tls.cast::<u8>(), layout);
    }
}

pub extern "C" fn _mi_thread_locals_init() {
    unsafe {
        mi_lock_init(&mi_thread_locals_lock as *const mi_lock_t as *mut mi_lock_t);
    }
    mi_thread_locals_free.store(ptr::null_mut(), Ordering::Relaxed);
    mi_thread_locals_version.store(0, Ordering::Relaxed);
}

pub extern "C" fn _mi_thread_locals_done() {
    unsafe {
        mi_lock_acquire(&mi_thread_locals_lock as *const mi_lock_t as *mut mi_lock_t);
    }
    let slots = mi_thread_locals_free.swap(ptr::null_mut(), Ordering::AcqRel);
    if !slots.is_null() {
        let count = unsafe { mi_bitmap_max_bits(slots) };
        if let Some(layout) = bitmap_layout(count) {
            unsafe {
                dealloc(slots.cast::<u8>(), layout);
            }
        }
    }
    unsafe {
        mi_lock_release(&mi_thread_locals_lock as *const mi_lock_t as *mut mi_lock_t);
        mi_lock_done(&mi_thread_locals_lock as *const mi_lock_t as *mut mi_lock_t);
    }
}

pub extern "C" fn _mi_thread_local_set(
    key: mi_thread_local_t,
    val: *mut core::ffi::c_void,
) -> bool {
    let tls = thread_locals_ptr();
    crate::mi_debug_assert!(!tls.is_null());
    crate::mi_debug_assert!(key != 0);
    let idx = mi_key_index(key);
    unsafe {
        if idx < (*tls).count {
            let slots = tls_slots(tls);
            (*slots.add(idx)).value = val;
            (*slots.add(idx)).version = mi_key_version(key);
            true
        } else {
            mi_thread_local_set_expand(key, val)
        }
    }
}

pub extern "C" fn _mi_thread_local_get(key: mi_thread_local_t) -> *mut core::ffi::c_void {
    let tls = thread_locals_ptr();
    crate::mi_debug_assert!(!tls.is_null());
    crate::mi_debug_assert!(key != 0);
    let idx = mi_key_index(key);
    unsafe {
        if idx < (*tls).count {
            let slots = tls_slots(tls);
            if mi_key_version(key) == (*slots.add(idx)).version {
                (*slots.add(idx)).value
            } else {
                ptr::null_mut()
            }
        } else {
            ptr::null_mut()
        }
    }
}

pub extern "C" fn _mi_thread_locals_thread_done() {
    let tls = thread_locals_ptr();
    if unsafe { !tls.is_null() && (*tls).count > 0 } {
        free_thread_locals(tls);
        set_thread_locals_ptr(ptr::from_ref(&mi_thread_locals_empty).cast_mut());
    }
}

pub extern "C" fn _mi_thread_local_create() -> mi_thread_local_t {
    unsafe {
        mi_lock_acquire(&mi_thread_locals_lock as *const mi_lock_t as *mut mi_lock_t);
    }
    let mut key = mi_thread_local_claim();
    if key == 0 && mi_thread_local_create_expand() {
        key = mi_thread_local_claim();
    }
    unsafe {
        mi_lock_release(&mi_thread_locals_lock as *const mi_lock_t as *mut mi_lock_t);
    }
    key
}

pub extern "C" fn _mi_thread_local_free(key: mi_thread_local_t) {
    if key == 0 {
        return;
    }
    let idx = mi_key_index(key);
    unsafe {
        mi_lock_acquire(&mi_thread_locals_lock as *const mi_lock_t as *mut mi_lock_t);
    }
    let slots = mi_thread_locals_free.load(Ordering::Acquire);
    if !slots.is_null() && idx < unsafe { mi_bitmap_max_bits(slots) } {
        unsafe {
            mi_bitmap_set(slots, idx);
        }
    }
    unsafe {
        mi_lock_release(&mi_thread_locals_lock as *const mi_lock_t as *mut mi_lock_t);
    }
}
