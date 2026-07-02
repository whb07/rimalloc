//! Bounded mechanical translation of `src/prim/osx/alloc-override-zone.c`.
//!
//! This file is intentionally standalone for now. The crate does not yet wire
//! a `prim::osx` submodule, but these helpers are translated here so the
//! remaining macOS zone-override surface exists in Rust in one place.

#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code,
    unused_imports,
    unused_macros
)]

use core::ffi::{c_char, c_int, c_void};
use core::ptr::{addr_of_mut, null_mut};

use crate::alloc_override::mi_interpose_s;
use crate::mimalloc::mi_any_heap_contains;

pub type boolean_t = c_int;
pub type kern_return_t = c_int;
pub type task_t = u32;
pub type uintptr_t = usize;
pub type vm_address_t = usize;
pub type vm_size_t = usize;

pub const KERN_SUCCESS: kern_return_t = 0;
pub const MI_MALLOC_OVERRIDE_ZONE_C: () = ();

static MI_MALLOC_ZONE_NAME: &[u8; 9] = b"mimalloc\0";

macro_rules! MI_INTERPOSE_FUN {
    ($oldfun:path, $newfun:path) => {
        mi_interpose_s {
            replacement: ($newfun as *const ()) as *const c_void,
            target: ($oldfun as *const ()) as *const c_void,
        }
    };
}

macro_rules! MI_INTERPOSE_ZONE {
    (zone_batch_free) => {
        MI_INTERPOSE_FUN!(malloc_zone_batch_free, crate::prim::osx::zone_batch_free)
    };
    (zone_batch_malloc) => {
        MI_INTERPOSE_FUN!(
            malloc_zone_batch_malloc,
            crate::prim::osx::zone_batch_malloc
        )
    };
    (zone_calloc) => {
        MI_INTERPOSE_FUN!(malloc_zone_calloc, crate::prim::osx::zone_calloc)
    };
    (zone_check) => {
        MI_INTERPOSE_FUN!(malloc_zone_check, crate::prim::osx::zone_check)
    };
    (zone_free) => {
        MI_INTERPOSE_FUN!(malloc_zone_free, crate::prim::osx::zone_free)
    };
    (zone_from_ptr) => {
        MI_INTERPOSE_FUN!(malloc_zone_from_ptr, crate::prim::osx::zone_from_ptr)
    };
    (zone_log) => {
        MI_INTERPOSE_FUN!(malloc_zone_log, crate::prim::osx::zone_log)
    };
    (zone_malloc) => {
        MI_INTERPOSE_FUN!(malloc_zone_malloc, crate::prim::osx::zone_malloc)
    };
    (zone_memalign) => {
        MI_INTERPOSE_FUN!(malloc_zone_memalign, crate::prim::osx::zone_memalign)
    };
    (zone_print) => {
        MI_INTERPOSE_FUN!(malloc_zone_print, crate::prim::osx::zone_print)
    };
    (zone_print_ptr_info) => {
        MI_INTERPOSE_FUN!(
            malloc_zone_print_ptr_info,
            crate::prim::osx::zone_print_ptr_info
        )
    };
    (zone_realloc) => {
        MI_INTERPOSE_FUN!(malloc_zone_realloc, crate::prim::osx::zone_realloc)
    };
    (zone_register) => {
        MI_INTERPOSE_FUN!(malloc_zone_register, crate::prim::osx::zone_register)
    };
    (zone_unregister) => {
        MI_INTERPOSE_FUN!(malloc_zone_unregister, crate::prim::osx::zone_unregister)
    };
    (zone_valloc) => {
        MI_INTERPOSE_FUN!(malloc_zone_valloc, crate::prim::osx::zone_valloc)
    };
}

#[repr(C)]
pub struct malloc_zone_t {
    // SAFETY[AUDIT:macos-zone-override]: malloc-zone callback slots use the
    // exact C ABI signatures expected by Darwin malloc.
    pub size: Option<unsafe extern "C" fn(*mut malloc_zone_t, *const c_void) -> usize>,
    pub malloc: Option<unsafe extern "C" fn(*mut malloc_zone_t, usize) -> *mut c_void>,
    pub calloc: Option<unsafe extern "C" fn(*mut malloc_zone_t, usize, usize) -> *mut c_void>,
    // SAFETY[AUDIT:macos-zone-override]: ownership and lifetime obligations
    // stay at the malloc-zone callback boundary for deallocation/reallocation.
    pub valloc: Option<unsafe extern "C" fn(*mut malloc_zone_t, usize) -> *mut c_void>,
    pub free: Option<unsafe extern "C" fn(*mut malloc_zone_t, *mut c_void)>,
    pub realloc:
        Option<unsafe extern "C" fn(*mut malloc_zone_t, *mut c_void, usize) -> *mut c_void>,
    // SAFETY[AUDIT:macos-zone-override]: destroy callbacks receive the original
    // zone pointer and retain the Darwin C ABI signature.
    pub destroy: Option<unsafe extern "C" fn(*mut malloc_zone_t)>,
    pub zone_name: *const c_char,
    // SAFETY[AUDIT:macos-zone-override]: batch callbacks receive caller-owned
    // pointer arrays whose element storage is managed by the zone operation.
    pub batch_malloc:
        Option<unsafe extern "C" fn(*mut malloc_zone_t, usize, *mut *mut c_void, u32) -> u32>,
    pub batch_free: Option<unsafe extern "C" fn(*mut malloc_zone_t, *mut *mut c_void, u32)>,
}

pub static mut mi_malloc_zone: malloc_zone_t = malloc_zone_t {
    size: Some(zone_size),
    malloc: Some(zone_malloc),
    calloc: Some(zone_calloc),
    valloc: Some(zone_valloc),
    free: Some(zone_free),
    realloc: Some(zone_realloc),
    destroy: Some(zone_destroy),
    zone_name: MI_MALLOC_ZONE_NAME.as_ptr().cast(),
    batch_malloc: Some(zone_batch_malloc),
    batch_free: Some(zone_batch_free),
};

#[cfg(target_os = "macos")]
// SAFETY[AUDIT:macos-zone-override]: declares Darwin malloc-zone imports only;
// local shims validate nullable results before publishing or forwarding them.
unsafe extern "C" {
    fn malloc_default_zone() -> *mut malloc_zone_t;
    fn malloc_default_purgeable_zone() -> *mut malloc_zone_t;
    fn malloc_get_all_zones(
        task: task_t,
        mr: *mut c_void,
        addresses: *mut *mut *mut malloc_zone_t,
        count: *mut u32,
    ) -> kern_return_t;
    fn malloc_zone_register(zone: *mut malloc_zone_t);
    fn malloc_zone_unregister(zone: *mut malloc_zone_t);

    fn malloc_create_zone(size: vm_size_t, flags: u32) -> *mut malloc_zone_t;
    fn malloc_get_zone_name(zone: *mut malloc_zone_t) -> *const c_char;
    fn malloc_set_zone_name(zone: *mut malloc_zone_t, name: *const c_char);
    fn malloc_destroy_zone(zone: *mut malloc_zone_t);
    fn malloc_jumpstart(cookie: uintptr_t) -> c_int;
    fn malloc_printf(fmt: *const c_char, ...);
    fn malloc_zone_batch_free(zone: *mut malloc_zone_t, ps: *mut *mut c_void, count: u32);
    fn malloc_zone_batch_malloc(
        zone: *mut malloc_zone_t,
        size: usize,
        ps: *mut *mut c_void,
        count: u32,
    ) -> u32;
    fn malloc_zone_calloc(zone: *mut malloc_zone_t, count: usize, size: usize) -> *mut c_void;
    fn malloc_zone_check(zone: *mut malloc_zone_t) -> boolean_t;
    fn malloc_zone_free(zone: *mut malloc_zone_t, p: *mut c_void);
    fn malloc_zone_from_ptr(p: *const c_void) -> *mut malloc_zone_t;
    fn malloc_zone_log(zone: *mut malloc_zone_t, p: *mut c_void);
    fn malloc_zone_malloc(zone: *mut malloc_zone_t, size: usize) -> *mut c_void;
    fn malloc_zone_memalign(zone: *mut malloc_zone_t, alignment: usize, size: usize)
    -> *mut c_void;
    fn malloc_zone_print(zone: *mut malloc_zone_t, verbose: boolean_t);
    fn malloc_zone_print_ptr_info(p: *mut c_void);
    fn malloc_zone_realloc(zone: *mut malloc_zone_t, p: *mut c_void, newsize: usize)
    -> *mut c_void;
    fn malloc_zone_valloc(zone: *mut malloc_zone_t, size: usize) -> *mut c_void;
}

/// # Safety
///
/// `zone` must be null or point to a valid malloc zone object for the duration
/// of the call.
#[inline]
pub unsafe fn is_mimalloc_zone(zone: *mut malloc_zone_t) -> bool {
    zone.is_null() || core::ptr::eq(zone.cast_const(), addr_of_mut!(mi_malloc_zone).cast_const())
}

/// # Safety
///
/// Returns the process-global mimalloc zone pointer. Callers must not mutate the
/// returned zone except through the macOS malloc-zone override hooks.
#[inline]
pub unsafe fn mi_get_default_zone() -> *mut malloc_zone_t {
    addr_of_mut!(mi_malloc_zone)
}

/// # Safety
///
/// Must be called only through the macOS malloc-zone creation ABI.
// SAFETY[AUDIT:macos-zone-override]: this C ABI shim always returns the
// process-global mimalloc zone without allocating or retaining caller state.
#[inline]
pub unsafe extern "C" fn mi_malloc_create_zone(
    _size: vm_size_t,
    _flags: u32,
) -> *mut malloc_zone_t {
    // SAFETY[AUDIT:macos-zone-override]: returns the static mimalloc zone
    // pointer used for Darwin malloc-zone interposition.
    unsafe { mi_get_default_zone() }
}

/// # Safety
///
/// Must be called only through the macOS malloc default-zone ABI.
// SAFETY[AUDIT:macos-zone-override]: default-zone interposition publishes the
// same global zone pointer for all default-zone queries.
#[inline]
pub unsafe extern "C" fn mi_malloc_default_zone() -> *mut malloc_zone_t {
    // SAFETY[AUDIT:macos-zone-override]: returns the static mimalloc zone
    // pointer used for Darwin malloc-zone interposition.
    unsafe { mi_get_default_zone() }
}

/// # Safety
///
/// Must be called only through the macOS malloc purgeable-zone ABI.
// SAFETY[AUDIT:macos-zone-override]: purgeable-zone interposition falls back to
// the same immutable mimalloc zone metadata as default-zone interposition.
#[inline]
pub unsafe extern "C" fn mi_malloc_default_purgeable_zone() -> *mut malloc_zone_t {
    // SAFETY[AUDIT:macos-zone-override]: returns the static mimalloc zone
    // pointer used for Darwin malloc-zone interposition.
    unsafe { mi_get_default_zone() }
}

/// # Safety
///
/// `_zone`, when non-null, must be a valid malloc zone pointer supplied by the
/// macOS malloc-zone ABI.
// SAFETY[AUDIT:macos-zone-override]: destroy-zone interposition is a no-op for
// compatibility and does not dereference caller-provided zone pointers.
#[inline]
pub unsafe extern "C" fn mi_malloc_destroy_zone(_zone: *mut malloc_zone_t) {
    // nothing
}

/// # Safety
///
/// `addresses` and `count`, when non-null, must be valid writable out-pointers
/// for the duration of the call.
// SAFETY[AUDIT:macos-zone-override]: all-zones interposition writes only
// nullable caller-provided result slots and reports success with no allocation.
#[inline]
pub unsafe extern "C" fn mi_malloc_get_all_zones(
    _task: task_t,
    _mr: *mut c_void,
    addresses: *mut *mut vm_address_t,
    count: *mut u32,
) -> kern_return_t {
    // SAFETY[AUDIT:macos-zone-override]: nullable out-pointers are checked
    // before stores and no zone array storage is retained.
    unsafe {
        // SAFETY[AUDIT:macos-zone-override]: non-null `addresses` is the
        // Darwin out-pointer for the all-zones array result.
        if !addresses.is_null() {
            *addresses = null_mut();
        }
        // SAFETY[AUDIT:macos-zone-override]: non-null `count` is the Darwin
        // out-pointer for the all-zones count result.
        if !count.is_null() {
            *count = 0;
        }
        KERN_SUCCESS
    }
}

/// # Safety
///
/// `zone`, when non-null, must point to a valid malloc zone object.
// SAFETY[AUDIT:macos-zone-override]: zone-name lookup reads only the selected
// zone metadata and defaults null callers to the mimalloc zone.
#[inline]
pub unsafe extern "C" fn mi_malloc_get_zone_name(zone: *mut malloc_zone_t) -> *const c_char {
    unsafe {
        let zone = if zone.is_null() {
            // SAFETY[AUDIT:macos-zone-override]: null zone queries map to the
            // process-global mimalloc zone metadata.
            mi_get_default_zone()
        } else {
            zone
        };
        // SAFETY[AUDIT:macos-zone-override]: the selected malloc zone is
        // caller-provided or the process-global mimalloc zone.
        (*zone).zone_name
    }
}

/// # Safety
///
/// `_zone`, when non-null, must be a valid malloc zone pointer. `_name`, when
/// non-null, must point to a valid NUL-terminated C string.
// SAFETY[AUDIT:macos-zone-override]: set-zone-name is intentionally a no-op
// and does not retain or dereference caller-provided strings.
#[inline]
pub unsafe extern "C" fn mi_malloc_set_zone_name(_zone: *mut malloc_zone_t, _name: *const c_char) {
    // nothing
}

/// # Safety
///
/// Must be called only through the macOS malloc jumpstart ABI.
// SAFETY[AUDIT:macos-zone-override]: jumpstart compatibility returns the C
// success sentinel without observing caller state.
#[inline]
pub unsafe extern "C" fn mi_malloc_jumpstart(_cookie: uintptr_t) -> c_int {
    1
}

/// # Safety
///
/// `zone`, when used for forwarding, must point to a valid malloc zone object.
/// `p` must be null or an allocation pointer accepted by either mimalloc or the
/// forwarded zone.
// SAFETY[AUDIT:macos-zone-override]: zone-size preserves mimalloc ownership
// detection before forwarding non-mimalloc pointers to the supplied zone.
#[inline]
pub unsafe extern "C" fn zone_size(zone: *mut malloc_zone_t, p: *const c_void) -> usize {
    // SAFETY[AUDIT:macos-zone-override]: ownership queries accept null or
    // allocator pointers and do not mutate the allocation.
    if unsafe { mi_any_heap_contains(p) } {
        // SAFETY[AUDIT:macos-zone-override]: mimalloc-owned pointers are routed
        // to mimalloc usable-size lookup.
        unsafe { crate::mimalloc::mi_usable_size(p) }
    } else if !unsafe { is_mimalloc_zone(zone) } {
        // SAFETY[AUDIT:macos-zone-override]: non-mimalloc zone forwarding reads
        // only the caller-provided zone callback slot.
        match unsafe { (*zone).size } {
            // SAFETY[AUDIT:macos-zone-override]: forwarding preserves the
            // original zone pointer and pointer argument.
            Some(size_fn) => unsafe { size_fn(zone, p) },
            None => 0,
        }
    } else {
        0
    }
}

/// # Safety
///
/// Must be called only through the macOS malloc-zone ABI.
// SAFETY[AUDIT:macos-zone-override]: zone malloc forwards directly to the
// allocator entry point and does not allocate through Rust facilities.
#[inline]
pub unsafe extern "C" fn zone_malloc(_zone: *mut malloc_zone_t, size: usize) -> *mut c_void {
    // SAFETY[AUDIT:macos-zone-override]: Darwin zone allocation delegates to
    // mimalloc with the caller-provided size.
    unsafe { crate::mimalloc::mi_malloc(size) }
}

/// # Safety
///
/// Must be called only through the macOS malloc-zone ABI.
// SAFETY[AUDIT:macos-zone-override]: zone calloc forwards directly to mimalloc
// and preserves the C product-allocation contract.
#[inline]
pub unsafe extern "C" fn zone_calloc(
    _zone: *mut malloc_zone_t,
    count: usize,
    size: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:macos-zone-override]: Darwin calloc delegates to mimalloc
    // with the caller-provided element count and size.
    unsafe { crate::mimalloc::mi_calloc(count, size) }
}

/// # Safety
///
/// Must be called only through the macOS malloc-zone ABI.
// SAFETY[AUDIT:macos-zone-override]: valloc uses the typed OS page-size helper
// and delegates aligned allocation to mimalloc.
#[inline]
pub unsafe extern "C" fn zone_valloc(_zone: *mut malloc_zone_t, size: usize) -> *mut c_void {
    // SAFETY[AUDIT:macos-zone-override]: page-sized alignment is supplied by
    // the primitive OS page-size wrapper before mimalloc allocation.
    unsafe { crate::mimalloc::mi_malloc_aligned(size, crate::os::_mi_os_page_size_typed().get()) }
}

/// # Safety
///
/// `zone`, when used for forwarding, must point to a valid malloc zone object.
/// `p` must be null or an allocation pointer accepted by either mimalloc or the
/// forwarded zone.
// SAFETY[AUDIT:macos-zone-override]: zone free routes mimalloc-owned pointers
// locally and forwards only non-mimalloc pointers to non-mimalloc zones.
#[inline]
pub unsafe extern "C" fn zone_free(zone: *mut malloc_zone_t, p: *mut c_void) {
    // SAFETY[AUDIT:macos-zone-override]: ownership query determines whether
    // `p` belongs to mimalloc before any free callback is selected.
    if unsafe { mi_any_heap_contains(p.cast_const()) } {
        // SAFETY[AUDIT:macos-zone-override]: mimalloc-owned pointers are freed
        // through the mimalloc deallocator.
        unsafe { crate::mimalloc::mi_free(p) };
    } else if !unsafe { is_mimalloc_zone(zone) }
        && let Some(free_fn) = unsafe { (*zone).free }
    {
        // SAFETY[AUDIT:macos-zone-override]: non-mimalloc pointers are forwarded
        // to the provided zone's free callback without changing ownership.
        unsafe { free_fn(zone, p) };
    }
}

/// # Safety
///
/// `zone`, when used for forwarding, must point to a valid malloc zone object.
/// `p` must be null or an allocation pointer accepted by either mimalloc or the
/// forwarded zone.
// SAFETY[AUDIT:macos-zone-override]: zone realloc preserves null/mimalloc
// routing and forwards only non-mimalloc pointers to non-mimalloc zones.
#[inline]
pub unsafe extern "C" fn zone_realloc(
    zone: *mut malloc_zone_t,
    p: *mut c_void,
    newsize: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:macos-zone-override]: null and mimalloc-owned pointers are
    // valid inputs to mimalloc realloc.
    if p.is_null() || unsafe { mi_any_heap_contains(p.cast_const()) } {
        // SAFETY[AUDIT:macos-zone-override]: mimalloc handles null and owned
        // pointer reallocation with the requested new size.
        unsafe { crate::mimalloc::mi_realloc(p, newsize) }
    } else if !unsafe { is_mimalloc_zone(zone) } {
        // SAFETY[AUDIT:macos-zone-override]: non-mimalloc zone forwarding reads
        // only the caller-provided zone callback slot.
        match unsafe { (*zone).realloc } {
            // SAFETY[AUDIT:macos-zone-override]: forwarding preserves the
            // original zone pointer, allocation pointer, and requested size.
            Some(realloc_fn) => unsafe { realloc_fn(zone, p, newsize) },
            None => null_mut(),
        }
    } else {
        null_mut()
    }
}

/// # Safety
///
/// Must be called only through the macOS malloc-zone ABI with an alignment
/// accepted by `mi_malloc_aligned`.
// SAFETY[AUDIT:macos-zone-override]: zone memalign forwards the C alignment and
// size pair directly to mimalloc's aligned allocation entry point.
#[inline]
pub unsafe extern "C" fn zone_memalign(
    _zone: *mut malloc_zone_t,
    alignment: usize,
    size: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:macos-zone-override]: caller supplies the Darwin zone
    // alignment contract; mimalloc validates or returns null.
    unsafe { crate::mimalloc::mi_malloc_aligned(size, alignment) }
}

/// # Safety
///
/// `zone`, when not the mimalloc zone, must point to a valid malloc zone object.
// SAFETY[AUDIT:macos-zone-override]: destroy forwarding is limited to
// non-mimalloc zones so the process-global mimalloc zone remains live.
#[inline]
pub unsafe extern "C" fn zone_destroy(zone: *mut malloc_zone_t) {
    if !unsafe { is_mimalloc_zone(zone) }
        && let Some(destroy_fn) = unsafe { (*zone).destroy }
    {
        // SAFETY[AUDIT:macos-zone-override]: non-mimalloc destroy callbacks are
        // invoked with their original zone pointer.
        unsafe { destroy_fn(zone) };
    }
}

/// # Safety
///
/// `ps` must point to writable storage for `count` pointers. `zone` must be a
/// valid malloc zone pointer accepted by `zone_malloc`.
// SAFETY[AUDIT:macos-zone-override]: batch allocation writes only the provided
// pointer array slots and stops at the first allocation failure.
#[inline]
pub unsafe extern "C" fn zone_batch_malloc(
    zone: *mut malloc_zone_t,
    size: usize,
    ps: *mut *mut c_void,
    count: u32,
) -> u32 {
    let mut i: u32 = 0;
    while i < count {
        // SAFETY[AUDIT:macos-zone-override]: each element allocation delegates
        // through the zone allocation shim with unchanged arguments.
        let p = unsafe { zone_malloc(zone, size) };
        // SAFETY[AUDIT:macos-zone-override]: `ps` points to `count` writable
        // result slots supplied by the Darwin batch-allocation caller.
        unsafe { *ps.add(i as usize) = p };
        if p.is_null() {
            break;
        }
        i += 1;
    }
    i
}

/// # Safety
///
/// `ps` must point to writable storage for `count` pointers. Each stored pointer
/// must be null or accepted by `zone_free` for `zone`.
// SAFETY[AUDIT:macos-zone-override]: batch free reads each caller-provided
// pointer slot, frees through the zone shim, then clears the slot.
#[inline]
pub unsafe extern "C" fn zone_batch_free(
    zone: *mut malloc_zone_t,
    ps: *mut *mut c_void,
    count: u32,
) {
    let mut i: u32 = 0;
    while i < count {
        // SAFETY[AUDIT:macos-zone-override]: `ps` points to `count` readable
        // slots supplied by the Darwin batch-free caller.
        let p = unsafe { *ps.add(i as usize) };
        // SAFETY[AUDIT:macos-zone-override]: each stored pointer is routed
        // through the same zone-free ownership checks as scalar free.
        unsafe { zone_free(zone, p) };
        // SAFETY[AUDIT:macos-zone-override]: clearing consumed slots preserves
        // Darwin batch-free postconditions for the caller-owned array.
        unsafe { *ps.add(i as usize) = null_mut() };
        i += 1;
    }
}

/// # Safety
///
/// Must be called only by the macOS malloc fork-prepare hook.
// SAFETY[AUDIT:macos-zone-override]: fork-prepare compatibility hook performs
// no allocator state mutation in this bounded source-present shim.
#[inline]
pub unsafe fn mi__malloc_fork_prepare() {
    // nothing
}

/// # Safety
///
/// Must be called only by the macOS malloc fork-prepare hook.
// SAFETY[AUDIT:macos-zone-override]: exported fork-prepare hook preserves the
// Darwin symbol and forwards to the local no-op compatibility hook.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _malloc_fork_prepare() {
    // SAFETY[AUDIT:macos-zone-override]: forwarding keeps all fork-prepare
    // behavior inside the audited no-op hook.
    unsafe { mi__malloc_fork_prepare() }
}

/// # Safety
///
/// Must be called only by the macOS malloc fork-parent hook.
// SAFETY[AUDIT:macos-zone-override]: fork-parent compatibility hook performs
// no allocator state mutation in this bounded source-present shim.
#[inline]
pub unsafe fn mi__malloc_fork_parent() {
    // nothing
}

/// # Safety
///
/// Must be called only by the macOS malloc fork-parent hook.
// SAFETY[AUDIT:macos-zone-override]: exported fork-parent hook preserves the
// Darwin symbol and forwards to the local no-op compatibility hook.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _malloc_fork_parent() {
    // SAFETY[AUDIT:macos-zone-override]: forwarding keeps all fork-parent
    // behavior inside the audited no-op hook.
    unsafe { mi__malloc_fork_parent() }
}

/// # Safety
///
/// Must be called only by the macOS malloc fork-child hook.
// SAFETY[AUDIT:macos-zone-override]: fork-child compatibility hook performs no
// allocator state mutation in this bounded source-present shim.
#[inline]
pub unsafe fn mi__malloc_fork_child() {
    // nothing
}

/// # Safety
///
/// Must be called only by the macOS malloc fork-child hook.
// SAFETY[AUDIT:macos-zone-override]: exported fork-child hook preserves the
// Darwin symbol and forwards to the local no-op compatibility hook.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _malloc_fork_child() {
    // SAFETY[AUDIT:macos-zone-override]: forwarding keeps all fork-child
    // behavior inside the audited no-op hook.
    unsafe { mi__malloc_fork_child() }
}

/// # Safety
///
/// `_fmt`, when non-null, must point to a valid NUL-terminated C format string.
// SAFETY[AUDIT:macos-zone-override]: malloc printf compatibility hook is a
// no-op and does not dereference or retain the format pointer.
#[inline]
pub unsafe fn mi_malloc_printf(_fmt: *const c_char) {
    // nothing
}

/// # Safety
///
/// `_zone`, when non-null, must be a valid malloc zone pointer.
// SAFETY[AUDIT:macos-zone-override]: zone check reports success without
// dereferencing caller-provided zone metadata.
#[inline]
pub unsafe extern "C" fn zone_check(_zone: *mut malloc_zone_t) -> boolean_t {
    1
}

/// # Safety
///
/// `p` must be null or an allocation pointer suitable for ownership queries.
// SAFETY[AUDIT:macos-zone-override]: zone-from-pointer exposes only mimalloc
// ownership classification and returns null for foreign pointers.
#[inline]
pub unsafe extern "C" fn zone_from_ptr(p: *const c_void) -> *mut malloc_zone_t {
    // SAFETY[AUDIT:macos-zone-override]: ownership query accepts null or
    // allocator pointers and does not mutate the allocation.
    if unsafe { mi_any_heap_contains(p) } {
        // SAFETY[AUDIT:macos-zone-override]: mimalloc-owned pointers map to the
        // process-global mimalloc zone metadata.
        unsafe { mi_get_default_zone() }
    } else {
        null_mut()
    }
}

/// # Safety
///
/// `_zone`, when non-null, must be a valid malloc zone pointer. `_p` must be
/// null or a pointer accepted by the zone logging ABI.
// SAFETY[AUDIT:macos-zone-override]: zone logging compatibility hook is a
// no-op and does not dereference caller-provided pointers.
#[inline]
pub unsafe extern "C" fn zone_log(_zone: *mut malloc_zone_t, _p: *mut c_void) {
    // nothing
}

/// # Safety
///
/// `_zone`, when non-null, must be a valid malloc zone pointer.
// SAFETY[AUDIT:macos-zone-override]: zone print compatibility hook is a no-op
// and does not dereference caller-provided zone metadata.
#[inline]
pub unsafe extern "C" fn zone_print(_zone: *mut malloc_zone_t, _verbose: boolean_t) {
    // nothing
}

/// # Safety
///
/// `_p` must be null or a pointer accepted by the macOS malloc pointer-info ABI.
// SAFETY[AUDIT:macos-zone-override]: pointer-info compatibility hook is a
// no-op and does not dereference caller-provided pointers.
#[inline]
pub unsafe extern "C" fn zone_print_ptr_info(_p: *mut c_void) {
    // nothing
}

/// # Safety
///
/// `_zone`, when non-null, must be a valid malloc zone pointer.
// SAFETY[AUDIT:macos-zone-override]: register compatibility hook is a no-op and
// does not mutate the caller-provided zone.
#[inline]
pub unsafe extern "C" fn zone_register(_zone: *mut malloc_zone_t) {
    // nothing
}

/// # Safety
///
/// `_zone`, when non-null, must be a valid malloc zone pointer.
// SAFETY[AUDIT:macos-zone-override]: unregister compatibility hook is a no-op
// and does not mutate the caller-provided zone.
#[inline]
pub unsafe extern "C" fn zone_unregister(_zone: *mut malloc_zone_t) {
    // nothing
}

#[cfg(all(target_os = "macos", mi_osx_interpose))]
#[used]
// SAFETY[AUDIT:macos-zone-override]: the interpose section publishes only
// Darwin malloc-zone replacement/target pairs with stable symbol addresses.
#[unsafe(link_section = "__DATA,__interpose")]
pub static _mi_zone_interposes: [mi_interpose_s; 27] = [
    MI_INTERPOSE_FUN!(malloc_create_zone, crate::prim::osx::mi_malloc_create_zone),
    MI_INTERPOSE_FUN!(
        malloc_default_purgeable_zone,
        crate::prim::osx::mi_malloc_default_purgeable_zone
    ),
    MI_INTERPOSE_FUN!(
        malloc_default_zone,
        crate::prim::osx::mi_malloc_default_zone
    ),
    MI_INTERPOSE_FUN!(
        malloc_destroy_zone,
        crate::prim::osx::mi_malloc_destroy_zone
    ),
    MI_INTERPOSE_FUN!(
        malloc_get_all_zones,
        crate::prim::osx::mi_malloc_get_all_zones
    ),
    MI_INTERPOSE_FUN!(
        malloc_get_zone_name,
        crate::prim::osx::mi_malloc_get_zone_name
    ),
    MI_INTERPOSE_FUN!(malloc_jumpstart, crate::prim::osx::mi_malloc_jumpstart),
    MI_INTERPOSE_FUN!(malloc_printf, crate::prim::osx::mi_malloc_printf),
    MI_INTERPOSE_FUN!(
        malloc_set_zone_name,
        crate::prim::osx::mi_malloc_set_zone_name
    ),
    MI_INTERPOSE_FUN!(_malloc_fork_child, crate::prim::osx::_malloc_fork_child),
    MI_INTERPOSE_FUN!(_malloc_fork_parent, crate::prim::osx::_malloc_fork_parent),
    MI_INTERPOSE_FUN!(_malloc_fork_prepare, crate::prim::osx::_malloc_fork_prepare),
    MI_INTERPOSE_ZONE!(zone_batch_free),
    MI_INTERPOSE_ZONE!(zone_batch_malloc),
    MI_INTERPOSE_ZONE!(zone_calloc),
    MI_INTERPOSE_ZONE!(zone_check),
    MI_INTERPOSE_ZONE!(zone_free),
    MI_INTERPOSE_ZONE!(zone_from_ptr),
    MI_INTERPOSE_ZONE!(zone_log),
    MI_INTERPOSE_ZONE!(zone_malloc),
    MI_INTERPOSE_ZONE!(zone_memalign),
    MI_INTERPOSE_ZONE!(zone_print),
    MI_INTERPOSE_ZONE!(zone_print_ptr_info),
    MI_INTERPOSE_ZONE!(zone_realloc),
    MI_INTERPOSE_ZONE!(zone_register),
    MI_INTERPOSE_ZONE!(zone_unregister),
    MI_INTERPOSE_ZONE!(zone_valloc),
];

#[cfg(all(target_os = "macos", mi_osx_interpose))]
/// # Safety
///
/// Must be called during macOS process initialization before allocator override
/// interposition is used concurrently.
// SAFETY[AUDIT:macos-zone-override]: process initialization registers the
// mimalloc zone and preserves original default/purgeable zone ordering.
pub unsafe extern "C" fn _mi_macos_override_malloc() {
    // SAFETY[AUDIT:macos-zone-override]: Darwin default purgeable-zone lookup
    // returns a nullable process-global zone pointer.
    let purgeable_zone = unsafe { malloc_default_purgeable_zone() };

    // SAFETY[AUDIT:macos-zone-override]: registers the process-global mimalloc
    // zone before default-zone replacement.
    unsafe { malloc_zone_register(addr_of_mut!(mi_malloc_zone)) };
    let mut zones: *mut *mut malloc_zone_t = null_mut();
    let mut count: u32 = 0;
    // SAFETY[AUDIT:macos-zone-override]: local stack out-pointers receive the
    // current Darwin malloc-zone array and count.
    let ret = unsafe { malloc_get_all_zones(0, null_mut(), &mut zones, &mut count) };
    let default_zone = if ret == KERN_SUCCESS && count > 0 && !zones.is_null() {
        // SAFETY[AUDIT:macos-zone-override]: successful `malloc_get_all_zones`
        // with positive count publishes at least the first zone pointer.
        unsafe { *zones }
    } else {
        // SAFETY[AUDIT:macos-zone-override]: fallback default-zone lookup
        // returns a nullable process-global zone pointer.
        unsafe { malloc_default_zone() }
    };
    if !core::ptr::eq(
        default_zone.cast_const(),
        addr_of_mut!(mi_malloc_zone).cast_const(),
    ) {
        // SAFETY[AUDIT:macos-zone-override]: default zone is temporarily
        // unregistered and re-registered to keep it after mimalloc in lookup order.
        unsafe { malloc_zone_unregister(default_zone) };
        // SAFETY[AUDIT:macos-zone-override]: the original default zone pointer
        // is restored after mimalloc registration.
        unsafe { malloc_zone_register(default_zone) };
    }

    if !purgeable_zone.is_null() {
        // SAFETY[AUDIT:macos-zone-override]: nullable purgeable zone was checked
        // before Darwin unregister/re-register ordering.
        unsafe { malloc_zone_unregister(purgeable_zone) };
        // SAFETY[AUDIT:macos-zone-override]: the original purgeable zone pointer
        // is restored after mimalloc registration.
        unsafe { malloc_zone_register(purgeable_zone) };
    }
}

#[cfg(all(target_os = "macos", mi_osx_interpose))]
#[used]
// SAFETY[AUDIT:macos-zone-override]: the constructor section stores the Darwin
// process-initialization callback pointer for malloc-zone override setup.
#[unsafe(link_section = "__DATA,__mod_init_func")]
pub static _mi_macos_override_malloc_ctor: unsafe extern "C" fn() = _mi_macos_override_malloc;
