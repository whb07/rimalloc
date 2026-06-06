//! Mechanical translation of `src/alloc-override.c`.

#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code,
    unused_macros,
    clippy::missing_safety_doc,
    unsafe_op_in_unsafe_fn
)]

#[cfg(target_os = "macos")]
use core::ffi::c_char;
use core::ffi::c_void;

pub type mi_nothrow_t = *mut c_void;

pub const MI_OSX_IS_INTERPOSED: usize = 0;

#[macro_export]
macro_rules! MI_FORWARD1 {
    ($fun:path, $x:expr) => {{
        return unsafe { $fun($x) };
    }};
}

#[macro_export]
macro_rules! MI_FORWARD3 {
    ($fun:path, $x:expr, $y:expr, $z:expr) => {{
        return unsafe { $fun($x, $y, $z) };
    }};
}

#[macro_export]
macro_rules! MI_FORWARD02 {
    ($fun:path, $x:expr, $y:expr) => {{
        unsafe { $fun($x, $y) };
        return;
    }};
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct mi_interpose_s {
    pub replacement: *const c_void,
    pub target: *const c_void,
}

#[macro_export]
macro_rules! MI_INTERPOSE_FUN {
    ($oldfun:path, $newfun:path) => {
        $crate::alloc_override::mi_interpose_s {
            replacement: ($newfun as *const ()) as *const core::ffi::c_void,
            target: ($oldfun as *const ()) as *const core::ffi::c_void,
        }
    };
}

unsafe extern "C" {
    pub fn vfree(p: *mut c_void);
    pub fn malloc_size(p: *const c_void) -> usize;
    pub fn malloc_good_size(size: usize) -> usize;
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn strndup(s: *const c_char, n: usize) -> *mut c_char;
    fn aligned_alloc(alignment: usize, size: usize) -> *mut c_void;
}

#[cfg(target_os = "macos")]
pub static _mi_interposes_10_7: [mi_interpose_s; 1] =
    [MI_INTERPOSE_FUN!(strndup, crate::mimalloc::mi_strndup)];

#[cfg(target_os = "macos")]
pub static _mi_interposes_10_15: [mi_interpose_s; 1] = [MI_INTERPOSE_FUN!(
    aligned_alloc,
    crate::mimalloc::mi_aligned_alloc
)];

#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_malloc_size_checked(p: *mut c_void) -> usize {
    if !unsafe { crate::mimalloc::mi_is_in_heap_region(p.cast_const()) } {
        return 0;
    }
    unsafe { crate::mimalloc::mi_usable_size(p.cast_const()) }
}

#[inline]
pub unsafe fn mi_forward1<R, A>(fun: unsafe extern "C" fn(A) -> R, x: A) -> R {
    unsafe { fun(x) }
}

#[inline]
pub unsafe fn mi_forward3<R, A, B, C>(
    fun: unsafe extern "C" fn(A, B, C) -> R,
    x: A,
    y: B,
    z: C,
) -> R {
    unsafe { fun(x, y, z) }
}

#[inline]
pub unsafe fn mi_forward02<A, B>(fun: unsafe extern "C" fn(A, B), x: A, y: B) {
    unsafe { fun(x, y) }
}

#[inline]
pub const fn mi_interpose_fun(oldfun: *const c_void, newfun: *const c_void) -> mi_interpose_s {
    mi_interpose_s {
        replacement: newfun,
        target: oldfun,
    }
}

#[inline]
pub const fn MI_INTERPOSE_DECLS<const N: usize>(
    entries: [mi_interpose_s; N],
) -> [mi_interpose_s; N] {
    entries
}

#[allow(improper_ctypes_definitions)]
#[inline]
pub unsafe fn mi_malloc_good_size_forward(size: usize) -> usize {
    unsafe { crate::mimalloc::mi_malloc_good_size(size) }
}

#[inline]
pub unsafe fn mi_vfree_forward(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

#[inline]
pub unsafe fn mi_malloc_size_forward(p: *const c_void) -> usize {
    unsafe { crate::mimalloc::mi_usable_size(p) }
}

#[inline]
pub unsafe fn mi_aligned_alloc_forward(alignment: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_aligned_alloc(alignment, size) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _expand(block: *mut c_void, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_expand(block, size) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _msize_base(block: *mut c_void) -> usize {
    unsafe { crate::mimalloc::mi_malloc_size(block) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _msize(block: *mut c_void) -> usize {
    unsafe { crate::mimalloc::mi_malloc_size(block) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _free_base(block: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(block) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn free(block: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(block) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_malloc(size) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _malloc_base(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_malloc(size) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _realloc_base(block: *mut c_void, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_realloc(block, size) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn realloc(block: *mut c_void, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_realloc(block, size) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _calloc_base(count: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_calloc(count, size) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_calloc(count, size) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _recalloc_base(
    block: *mut c_void,
    count: usize,
    size: usize,
) -> *mut c_void {
    unsafe { crate::mimalloc::mi_recalloc(block, count, size) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _recalloc(block: *mut c_void, count: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_recalloc(block, count, size) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_free(block: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(block) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_malloc(size: usize, alignment: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_malloc_aligned(size, alignment) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_offset_malloc(
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    unsafe { crate::mimalloc::mi_malloc_aligned_at(size, alignment, offset) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_msize(
    block: *mut c_void,
    alignment: usize,
    offset: usize,
) -> usize {
    crate::MI_UNUSED!(alignment);
    crate::MI_UNUSED!(offset);
    unsafe { crate::mimalloc::mi_malloc_size(block) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_offset_realloc(
    block: *mut c_void,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    unsafe { crate::mimalloc::mi_realloc_aligned_at(block, size, alignment, offset) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_offset_recalloc(
    block: *mut c_void,
    count: usize,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    unsafe { crate::mimalloc::mi_recalloc_aligned_at(block, count, size, alignment, offset) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_realloc(
    block: *mut c_void,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    unsafe { crate::mimalloc::mi_realloc_aligned(block, size, alignment) }
}

#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_recalloc(
    block: *mut c_void,
    count: usize,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    unsafe { crate::mimalloc::mi_recalloc_aligned(block, count, size, alignment) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "_ZdlPv")]
pub unsafe extern "C" fn mi_cxx_delete(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "_ZdaPv")]
pub unsafe extern "C" fn mi_cxx_delete_array(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "_ZdlPvm")]
pub unsafe extern "C" fn mi_cxx_delete_sized(p: *mut c_void, n: usize) {
    unsafe { crate::mimalloc::mi_free_size(p, n) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "_ZdaPvm")]
pub unsafe extern "C" fn mi_cxx_delete_array_sized(p: *mut c_void, n: usize) {
    unsafe { crate::mimalloc::mi_free_size(p, n) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "_ZdlPvSt11align_val_t")]
pub unsafe extern "C" fn mi_cxx_delete_aligned(p: *mut c_void, al: usize) {
    unsafe { crate::mimalloc::mi_free_aligned(p, al) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "_ZdaPvSt11align_val_t")]
pub unsafe extern "C" fn mi_cxx_delete_array_aligned(p: *mut c_void, al: usize) {
    unsafe { crate::mimalloc::mi_free_aligned(p, al) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "_ZdlPvmSt11align_val_t")]
pub unsafe extern "C" fn mi_cxx_delete_sized_aligned(p: *mut c_void, n: usize, al: usize) {
    unsafe { crate::mimalloc::mi_free_size_aligned(p, n, al) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "_ZdaPvmSt11align_val_t")]
pub unsafe extern "C" fn mi_cxx_delete_array_sized_aligned(p: *mut c_void, n: usize, al: usize) {
    unsafe { crate::mimalloc::mi_free_size_aligned(p, n, al) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "_ZdlPvRKSt9nothrow_t")]
pub unsafe extern "C" fn mi_cxx_delete_nothrow(p: *mut c_void, tag: mi_nothrow_t) {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_free(p) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "_ZdaPvRKSt9nothrow_t")]
pub unsafe extern "C" fn mi_cxx_delete_array_nothrow(p: *mut c_void, tag: mi_nothrow_t) {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_free(p) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "_ZdlPvSt11align_val_tRKSt9nothrow_t")]
pub unsafe extern "C" fn mi_cxx_delete_aligned_nothrow(
    p: *mut c_void,
    al: usize,
    tag: mi_nothrow_t,
) {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_free_aligned(p, al) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "_ZdaPvSt11align_val_tRKSt9nothrow_t")]
pub unsafe extern "C" fn mi_cxx_delete_array_aligned_nothrow(
    p: *mut c_void,
    al: usize,
    tag: mi_nothrow_t,
) {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_free_aligned(p, al) }
}

#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[unsafe(export_name = "_Znwm")]
pub unsafe extern "C" fn mi_cxx_new_64(n: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new(n) }
}

#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[unsafe(export_name = "_Znam")]
pub unsafe extern "C" fn mi_cxx_new_array_64(n: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new(n) }
}

#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[unsafe(export_name = "_ZnwmRKSt9nothrow_t")]
pub unsafe extern "C" fn mi_cxx_new_64_nothrow(n: usize, tag: mi_nothrow_t) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_nothrow(n) }
}

#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[unsafe(export_name = "_ZnamRKSt9nothrow_t")]
pub unsafe extern "C" fn mi_cxx_new_array_64_nothrow(n: usize, tag: mi_nothrow_t) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_nothrow(n) }
}

#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[unsafe(export_name = "_ZnwmSt11align_val_t")]
pub unsafe extern "C" fn mi_cxx_new_64_aligned(n: usize, al: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[unsafe(export_name = "_ZnamSt11align_val_t")]
pub unsafe extern "C" fn mi_cxx_new_array_64_aligned(n: usize, al: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[unsafe(export_name = "_ZnwmSt11align_val_tRKSt9nothrow_t")]
pub unsafe extern "C" fn mi_cxx_new_64_aligned_nothrow(
    n: usize,
    al: usize,
    tag: mi_nothrow_t,
) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_aligned_nothrow(n, al) }
}

#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[unsafe(export_name = "_ZnamSt11align_val_tRKSt9nothrow_t")]
pub unsafe extern "C" fn mi_cxx_new_array_64_aligned_nothrow(
    n: usize,
    al: usize,
    tag: mi_nothrow_t,
) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_aligned_nothrow(n, al) }
}

#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[unsafe(export_name = "_Znwj")]
pub unsafe extern "C" fn mi_cxx_new_32(n: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new(n) }
}

#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[unsafe(export_name = "_Znaj")]
pub unsafe extern "C" fn mi_cxx_new_array_32(n: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new(n) }
}

#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[unsafe(export_name = "_ZnwjRKSt9nothrow_t")]
pub unsafe extern "C" fn mi_cxx_new_32_nothrow(n: usize, tag: mi_nothrow_t) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_nothrow(n) }
}

#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[unsafe(export_name = "_ZnajRKSt9nothrow_t")]
pub unsafe extern "C" fn mi_cxx_new_array_32_nothrow(n: usize, tag: mi_nothrow_t) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_nothrow(n) }
}

#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[unsafe(export_name = "_ZnwjSt11align_val_t")]
pub unsafe extern "C" fn mi_cxx_new_32_aligned(n: usize, al: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[unsafe(export_name = "_ZnajSt11align_val_t")]
pub unsafe extern "C" fn mi_cxx_new_array_32_aligned(n: usize, al: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[unsafe(export_name = "_ZnwjSt11align_val_tRKSt9nothrow_t")]
pub unsafe extern "C" fn mi_cxx_new_32_aligned_nothrow(
    n: usize,
    al: usize,
    tag: mi_nothrow_t,
) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_aligned_nothrow(n, al) }
}

#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[unsafe(export_name = "_ZnajSt11align_val_tRKSt9nothrow_t")]
pub unsafe extern "C" fn mi_cxx_new_array_32_aligned_nothrow(
    n: usize,
    al: usize,
    tag: mi_nothrow_t,
) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_aligned_nothrow(n, al) }
}
