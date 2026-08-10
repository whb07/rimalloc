//! Mechanical translation of `src/alloc-posix.c`.

use crate::internal::{
    _mi_align_up, _mi_is_aligned, _mi_is_power_of_two, _mi_memcpy, mi_mul_overflow,
};
use crate::mimalloc::{
    c_wchar, mi_expand, mi_free, mi_is_in_heap_region, mi_malloc, mi_malloc_aligned, mi_realloc,
    mi_strdup, mi_usable_size,
};
use crate::types::{EINVAL, ENOMEM, EOVERFLOW};
use core::ffi::CStr;
use core::ffi::{c_int, c_void};
use core::{mem::size_of, ptr};

// SAFETY: declares C environment lookup for the POSIX
// compatibility helpers; returned storage remains owned by the process.
unsafe extern "C" {
    fn getenv(name: *const i8) -> *const i8;
}

#[cfg(any(target_os = "linux", target_os = "android"))]
// SAFETY: Linux/Android errno publication uses the
// platform thread-local errno slot exactly as the C compatibility layer does.
unsafe fn mi_set_errno(err: c_int) {
    unsafe extern "C" {
        fn __errno_location() -> *mut c_int;
    }
    // SAFETY: `__errno_location` returns the
    // thread-local errno slot for this platform.
    unsafe { *__errno_location() = err };
}

#[cfg(target_vendor = "apple")]
// SAFETY: Apple errno publication uses the platform
// thread-local errno slot exactly as the C compatibility layer does.
unsafe fn mi_set_errno(err: c_int) {
    unsafe extern "C" {
        fn __error() -> *mut c_int;
    }
    // SAFETY: `__error` returns the thread-local errno
    // slot for this platform.
    unsafe { *__error() = err };
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
// SAFETY: unsupported errno targets intentionally keep
// the C-compatible helper as a no-op fallback.
unsafe fn mi_set_errno(_err: c_int) {}

// SAFETY: exports the POSIX usable-size compatibility
// symbol without changing its raw pointer contract.
#[unsafe(no_mangle)]
/// # Safety
///
/// `p` may be null or must be a pointer previously returned by mimalloc or a
/// compatible allocation entry point.
// SAFETY: forwards the raw allocation pointer to the
// mimalloc usable-size boundary under the caller's ownership contract.
pub unsafe extern "C" fn mi_malloc_usable_size(p: *const c_void) -> usize {
    unsafe { mi_usable_size(p) }
}

// SAFETY: exports the malloc-size compatibility alias
// with the same raw pointer contract as usable-size.
#[unsafe(no_mangle)]
/// # Safety
///
/// `p` may be null or must be a pointer previously returned by mimalloc or a
/// compatible allocation entry point.
// SAFETY: forwards the raw allocation pointer to the
// mimalloc usable-size boundary under the caller's ownership contract.
pub unsafe extern "C" fn mi_malloc_size(p: *const c_void) -> usize {
    unsafe { mi_usable_size(p) }
}

// SAFETY: exports the cfree compatibility symbol while
// keeping ownership validation inside the body.
#[unsafe(no_mangle)]
/// # Safety
///
/// `p` may be null. A non-null pointer must be suitable for mimalloc heap-region
/// inspection and, when owned by mimalloc, valid to free.
// SAFETY: heap-region inspection gates the raw free so
// non-mimalloc pointers are ignored like the C compatibility helper.
pub unsafe extern "C" fn mi_cfree(p: *mut c_void) {
    if unsafe { mi_is_in_heap_region(p.cast_const()) } {
        // SAFETY: the region check accepted `p`; the
        // function contract requires owned mimalloc pointers to be freeable.
        unsafe { mi_free(p) };
    }
}

// SAFETY: exports POSIX memalign with its raw
// out-pointer contract and errno-style integer result.
#[unsafe(no_mangle)]
/// # Safety
///
/// `p` must be valid for writing one pointer. On success the written pointer
/// must be released through mimalloc-compatible free APIs.
// SAFETY: validates the POSIX alignment and nullable
// allocation result before writing through the caller-provided pointer slot.
pub unsafe extern "C" fn mi_posix_memalign(
    p: *mut *mut c_void,
    alignment: usize,
    size: usize,
) -> c_int {
    if p.is_null() {
        return EINVAL;
    }
    if !alignment.is_multiple_of(size_of::<*mut c_void>()) {
        return EINVAL;
    }
    if alignment == 0 || !_mi_is_power_of_two(alignment) {
        return EINVAL;
    }
    // SAFETY: valid POSIX alignment was checked above;
    // mimalloc owns the returned allocation on success.
    let q = unsafe { mi_malloc_aligned(size, alignment) };
    if q.is_null() && size != 0 {
        return ENOMEM;
    }
    crate::internal::mi_assert_internal!(_mi_is_aligned(q, alignment));
    // SAFETY: `p` was checked non-null and is valid for
    // one pointer by this C ABI contract.
    unsafe { *p = q };
    0
}

// SAFETY: exports memalign as a raw allocation
// compatibility symbol.
#[unsafe(no_mangle)]
/// # Safety
///
/// The returned pointer, when non-null, must be released through
/// mimalloc-compatible free APIs.
// SAFETY: forwards to mimalloc's aligned allocator and
// preserves the raw free ownership contract.
pub unsafe extern "C" fn mi_memalign(alignment: usize, size: usize) -> *mut c_void {
    let p = unsafe { mi_malloc_aligned(size, alignment) };
    crate::internal::mi_assert_internal!(_mi_is_aligned(p, alignment));
    p
}

// SAFETY: exports valloc as a page-aligned raw
// allocation compatibility symbol.
#[unsafe(no_mangle)]
/// # Safety
///
/// The returned pointer, when non-null, must be released through
/// mimalloc-compatible free APIs.
// SAFETY: uses the current OS page-size wrapper before
// forwarding to the memalign compatibility helper.
pub unsafe extern "C" fn mi_valloc(size: usize) -> *mut c_void {
    unsafe { mi_memalign(crate::os::_mi_os_page_size_typed().get(), size) }
}

// SAFETY: exports pvalloc as a page-rounded raw
// allocation compatibility symbol.
#[unsafe(no_mangle)]
/// # Safety
///
/// The returned pointer, when non-null, must be released through
/// mimalloc-compatible free APIs.
// SAFETY: page-size lookup and aligned allocation
// preserve the C helper's overflow/null behavior.
pub unsafe extern "C" fn mi_pvalloc(size: usize) -> *mut c_void {
    let psize = unsafe { crate::os::_mi_os_page_size_typed().get() };
    if size >= usize::MAX - psize {
        return ptr::null_mut();
    }
    let asize = _mi_align_up(size, psize);
    // SAFETY: `asize` was page-rounded without
    // overflow and `psize` is the OS page alignment.
    unsafe { mi_malloc_aligned(asize, psize) }
}

// SAFETY: exports C11 aligned_alloc with the raw
// allocation/free contract.
#[unsafe(no_mangle)]
/// # Safety
///
/// The returned pointer, when non-null, must be released through
/// mimalloc-compatible free APIs.
// SAFETY: forwards directly to mimalloc's aligned
// allocation path and preserves the caller's raw ownership obligations.
pub unsafe extern "C" fn mi_aligned_alloc(alignment: usize, size: usize) -> *mut c_void {
    let p = unsafe { mi_malloc_aligned(size, alignment) };
    crate::internal::mi_assert_internal!(_mi_is_aligned(p, alignment));
    p
}

// SAFETY: exports reallocarray with the POSIX
// overflow-to-null and errno behavior.
#[unsafe(no_mangle)]
/// # Safety
///
/// `p` may be null or must be a pointer previously returned by mimalloc or a
/// compatible allocation entry point. The returned pointer, when non-null, must
/// be released through mimalloc-compatible free APIs.
// SAFETY: validates the product through
// `CallocRequest` before forwarding to the raw realloc boundary.
pub unsafe extern "C" fn mi_reallocarray(p: *mut c_void, count: usize, size: usize) -> *mut c_void {
    if let Ok(req) = crate::core_types::CallocRequest::new(count, size) {
        // SAFETY: `p` follows the realloc input
        // contract and the requested total was checked above.
        let newp = unsafe { mi_realloc(p, req.total().get()) };
        if newp.is_null() {
            // SAFETY: publishes POSIX allocation
            // failure through the platform errno shim.
            unsafe { mi_set_errno(ENOMEM) };
        }
        newp
    } else {
        // SAFETY: publishes POSIX product overflow
        // through the platform errno shim.
        unsafe { mi_set_errno(EOVERFLOW) };
        ptr::null_mut()
    }
}

// SAFETY: exports reallocarr with its in/out pointer
// slot contract and errno-style result.
#[unsafe(no_mangle)]
/// # Safety
///
/// `ptrp` must be valid for reading and writing one pointer. `*ptrp` may be
/// null or must be a mimalloc-compatible allocation pointer.
// SAFETY: validates the pointer slot and product before
// reading or rewriting the caller's allocation pointer.
pub unsafe extern "C" fn mi_reallocarr(ptrp: *mut *mut c_void, count: usize, size: usize) -> c_int {
    crate::internal::mi_assert!(size != 0);
    crate::internal::mi_assert!(!ptrp.is_null());
    if ptrp.is_null() || size == 0 {
        // SAFETY: publishes POSIX invalid-argument
        // failure through the platform errno shim.
        unsafe { mi_set_errno(EINVAL) };
        return EINVAL;
    }

    let Ok(req) = crate::core_types::CallocRequest::new(count, size) else {
        // SAFETY: publishes POSIX product overflow
        // through the platform errno shim.
        unsafe { mi_set_errno(EOVERFLOW) };
        return EOVERFLOW;
    };
    let total = req.total().get();

    if total == 0 {
        // SAFETY: `ptrp` was checked non-null and is
        // valid for one pointer by the C ABI contract.
        let oldp = unsafe { *ptrp };
        // SAFETY: `oldp` is null or a
        // mimalloc-compatible allocation pointer by the function contract.
        unsafe { mi_free(oldp) };
        // SAFETY: `ptrp` was checked non-null and is
        // valid for one pointer by the C ABI contract.
        unsafe { *ptrp = ptr::null_mut() };
        return 0;
    }

    // SAFETY: `ptrp` was checked non-null and is valid
    // for one pointer by the C ABI contract.
    let oldp = unsafe { *ptrp };
    // SAFETY: `oldp` is null or a mimalloc-compatible
    // allocation pointer and `total` was checked above.
    let newp = unsafe { mi_realloc(oldp, total) };
    if newp.is_null() {
        // SAFETY: publishes POSIX allocation failure
        // through the platform errno shim.
        unsafe { mi_set_errno(ENOMEM) };
        return ENOMEM;
    }
    // SAFETY: `ptrp` is valid for one pointer and
    // receives the successful realloc result.
    unsafe { *ptrp = newp };
    0
}

// SAFETY: exports the POSIX expand compatibility
// symbol with the raw realloc-like pointer contract.
#[unsafe(no_mangle)]
/// # Safety
///
/// `p` may be null or must be a pointer previously returned by mimalloc or a
/// compatible allocation entry point.
// SAFETY: forwards to mimalloc expand and publishes
// allocation failure through errno.
pub unsafe extern "C" fn mi__expand(p: *mut c_void, newsize: usize) -> *mut c_void {
    let res = unsafe { mi_expand(p, newsize) };
    if res.is_null() {
        // SAFETY: publishes POSIX allocation failure
        // through the platform errno shim.
        unsafe { mi_set_errno(ENOMEM) };
    }
    res
}

// SAFETY: exports mbsdup with nullable C string input
// and mimalloc-owned duplicate output.
#[unsafe(no_mangle)]
/// # Safety
///
/// `s` may be null or must point to a readable nul-terminated multibyte string.
/// The returned pointer, when non-null, must be freed through mimalloc.
// SAFETY: forwards the nullable C string to mimalloc's
// strdup helper and preserves ownership of the returned duplicate.
pub unsafe extern "C" fn mi_mbsdup(s: *const u8) -> *mut u8 {
    unsafe { mi_strdup(s.cast::<i8>()).cast::<u8>() }
}

// SAFETY: exports wcsdup with nullable wide-string
// input and mimalloc-owned duplicate output.
#[unsafe(no_mangle)]
/// # Safety
///
/// `s` may be null or must point to a readable nul-terminated wide string. The
/// returned pointer, when non-null, must be freed through mimalloc.
// SAFETY: scans the caller-provided wide string up to
// the first nul or representable size bound before allocating a duplicate.
pub unsafe extern "C" fn mi_wcsdup(s: *const c_wchar) -> *mut c_wchar {
    if s.is_null() {
        return ptr::null_mut();
    }
    let mut wlen = 0usize;
    // SAFETY: non-null `s` is readable as a
    // nul-terminated wide string by the function contract.
    while unsafe { *s.add(wlen) } != 0 && wlen < isize::MAX as usize {
        wlen += 1;
    }
    let mut size = 0usize;
    if mi_mul_overflow(wlen + 1, size_of::<c_wchar>(), &mut size) || size > isize::MAX as usize {
        return ptr::null_mut();
    }
    // SAFETY: `size` was checked for overflow and is
    // the byte length of the wide-string duplicate including its terminator.
    let p = unsafe { mi_malloc(size) }.cast::<c_wchar>();
    if !p.is_null() {
        // SAFETY: `p` is writable for `size` bytes from
        // mimalloc and `s` is readable for the same nul-terminated range.
        unsafe { _mi_memcpy(p.cast::<c_void>(), s.cast::<c_void>(), size) };
    }
    p
}

// SAFETY: exports the wide environment duplication
// fallback with raw nullable out-pointers.
#[unsafe(no_mangle)]
/// # Safety
///
/// `buf` and `size`, when non-null, must be valid for writing one value.
/// `_name`, when non-null, must point to a readable nul-terminated wide string.
// SAFETY: writes only validated out-pointers and
// preserves the current unsupported wide-environment result.
pub unsafe extern "C" fn mi_wdupenv_s(
    buf: *mut *mut c_wchar,
    size: *mut usize,
    _name: *const c_wchar,
) -> c_int {
    if !size.is_null() {
        // SAFETY: non-null `size` is valid for one
        // `usize` write by the C ABI contract.
        unsafe { *size = 0 };
    }
    if buf.is_null() || _name.is_null() {
        return EINVAL;
    }
    // SAFETY: non-null `buf` is valid for one pointer
    // write by the C ABI contract.
    unsafe { *buf = ptr::null_mut() };
    EINVAL
}

// SAFETY: exports aligned offset recalloc as a raw
// allocation compatibility shim.
#[unsafe(no_mangle)]
/// # Safety
///
/// `p` may be null or must be a mimalloc-compatible allocation pointer. The
/// returned pointer, when non-null, must be released through mimalloc-compatible
/// free APIs.
// SAFETY: forwards raw arguments to the aligned
// allocator boundary, which owns product/alignment validation.
pub unsafe extern "C" fn mi_aligned_offset_recalloc(
    p: *mut c_void,
    newcount: usize,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    // SAFETY: forwards raw arguments to the aligned
    // allocator boundary, which owns product/alignment validation.
    unsafe { crate::alloc_aligned::mi_recalloc_aligned_at(p, newcount, size, alignment, offset) }
}

// SAFETY: exports aligned recalloc as a raw allocation
// compatibility shim.
#[unsafe(no_mangle)]
/// # Safety
///
/// `p` may be null or must be a mimalloc-compatible allocation pointer. The
/// returned pointer, when non-null, must be released through mimalloc-compatible
/// free APIs.
// SAFETY: forwards raw arguments to the aligned
// allocator boundary, which owns product/alignment validation.
pub unsafe extern "C" fn mi_aligned_recalloc(
    p: *mut c_void,
    newcount: usize,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    // SAFETY: forwards raw arguments to the aligned
    // allocator boundary, which owns product/alignment validation.
    unsafe { crate::alloc_aligned::mi_recalloc_aligned(p, newcount, size, alignment) }
}

// SAFETY: exports dupenv_s with raw nullable
// out-pointers and process environment lookup.
#[unsafe(no_mangle)]
/// # Safety
///
/// `buf` and `size`, when non-null, must be valid for writing one value. `name`
/// must point to a readable nul-terminated C string.
// SAFETY: validates out-pointers before process
// environment lookup and mimalloc-owned duplication.
pub unsafe extern "C" fn mi_dupenv_s(
    buf: *mut *mut i8,
    size: *mut usize,
    name: *const i8,
) -> c_int {
    if !size.is_null() {
        // SAFETY: non-null `size` is valid for one
        // `usize` write by the C ABI contract.
        unsafe { *size = 0 };
    }
    if buf.is_null() || name.is_null() {
        return EINVAL;
    }
    // SAFETY: non-null `name` is a readable C string by
    // the function contract; `getenv` returns process-owned storage.
    let value = unsafe { getenv(name) };
    if value.is_null() {
        // SAFETY: non-null `buf` is valid for one
        // pointer write by the C ABI contract.
        unsafe { *buf = ptr::null_mut() };
        return 0;
    }
    // SAFETY: `value` is a non-null C string returned
    // by `getenv`; mimalloc owns the duplicate on success.
    let dup = unsafe { mi_strdup(value) };
    // SAFETY: non-null `buf` is valid for one pointer
    // write by the C ABI contract.
    unsafe { *buf = dup };
    if dup.is_null() {
        return ENOMEM;
    }
    if !size.is_null() {
        // SAFETY: `value` is a readable environment C
        // string and `size` is valid for one `usize` write.
        unsafe { *size = CStr::from_ptr(value).to_bytes_with_nul().len() };
    }
    0
}
