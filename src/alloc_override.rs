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

use core::ffi::{c_char, c_int, c_void};

pub type mi_nothrow_t = *mut c_void;

pub const MI_OSX_IS_INTERPOSED: usize = 0;
#[cfg(target_os = "macos")]
pub const MI_OSX_ZONE: usize = 0;
pub const MI_FORWARD: () = ();
pub const MI_FORWARD1: () = ();
pub const MI_FORWARD02: () = ();
pub const MI_FORWARD3: () = ();

#[macro_export]
macro_rules! MI_FORWARD1 {
    ($fun:path, $x:expr) => {{
        return unsafe { $fun($x) };
    }};
}

#[cfg(mi_c_malloc_fast)]
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malloc(_size: usize) -> *mut c_void {
    core::arch::naked_asm!(
        "mov rax, qword ptr [rip + mi_tls_theap_default@GOTTPOFF]",
        "mov rsi, rdi",
        "mov rdi, qword ptr fs:[rax]",
        "test rdi, rdi",
        "je 2f",
        "cmp rsi, 1024",
        "ja 2f",
        "lea rax, [rsi + 7]",
        "shr rax, 3",
        "mov rdx, qword ptr [rdi + rax*8 + 0x110]",
        "mov rax, qword ptr [rdx + 0x8]",
        "test rax, rax",
        "je 2f",
        "mov rcx, qword ptr [rax]",
        "add word ptr [rdx + 0x10], 1",
        "mov qword ptr [rdx + 0x8], rcx",
        "mov qword ptr [rax], 0",
        "ret",
        "2:",
        "xor ecx, ecx",
        "xor edx, edx",
        "jmp {slow}",
        slow = sym crate::page::_mi_malloc_generic,
    );
}

#[cfg(mi_c_malloc_fast)]
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_malloc(_size: usize) -> *mut c_void {
    core::arch::naked_asm!(
        "mov rax, qword ptr [rip + mi_tls_theap_default@GOTTPOFF]",
        "mov rsi, rdi",
        "mov rdi, qword ptr fs:[rax]",
        "test rdi, rdi",
        "je 2f",
        "cmp rsi, 1024",
        "ja 2f",
        "lea rax, [rsi + 7]",
        "shr rax, 3",
        "mov rdx, qword ptr [rdi + rax*8 + 0x110]",
        "mov rax, qword ptr [rdx + 0x8]",
        "test rax, rax",
        "je 2f",
        "mov rcx, qword ptr [rax]",
        "add word ptr [rdx + 0x10], 1",
        "mov qword ptr [rdx + 0x8], rcx",
        "mov qword ptr [rax], 0",
        "ret",
        "2:",
        "xor ecx, ecx",
        "xor edx, edx",
        "jmp {slow}",
        slow = sym crate::page::_mi_malloc_generic,
    );
}

#[cfg(mi_c_malloc_fast)]
#[unsafe(naked)]
#[unsafe(export_name = "_Znwm")]
pub unsafe extern "C" fn mi_cxx_new_fast_64(_size: usize) -> *mut c_void {
    core::arch::naked_asm!(
        "mov rax, qword ptr [rip + mi_tls_theap_default@GOTTPOFF]",
        "mov rsi, rdi",
        "mov rdi, qword ptr fs:[rax]",
        "test rdi, rdi",
        "je 2f",
        "cmp rsi, 1024",
        "ja 2f",
        "lea rax, [rsi + 7]",
        "shr rax, 3",
        "mov rdx, qword ptr [rdi + rax*8 + 0x110]",
        "mov rax, qword ptr [rdx + 0x8]",
        "test rax, rax",
        "je 2f",
        "mov rcx, qword ptr [rax]",
        "add word ptr [rdx + 0x10], 1",
        "mov qword ptr [rdx + 0x8], rcx",
        "mov qword ptr [rax], 0",
        "ret",
        "2:",
        "mov rdi, rsi",
        "jmp mi_new",
    );
}

#[cfg(mi_c_malloc_fast)]
#[unsafe(naked)]
#[unsafe(export_name = "_Znam")]
pub unsafe extern "C" fn mi_cxx_new_array_fast_64(_size: usize) -> *mut c_void {
    core::arch::naked_asm!(
        "mov rax, qword ptr [rip + mi_tls_theap_default@GOTTPOFF]",
        "mov rsi, rdi",
        "mov rdi, qword ptr fs:[rax]",
        "test rdi, rdi",
        "je 2f",
        "cmp rsi, 1024",
        "ja 2f",
        "lea rax, [rsi + 7]",
        "shr rax, 3",
        "mov rdx, qword ptr [rdi + rax*8 + 0x110]",
        "mov rax, qword ptr [rdx + 0x8]",
        "test rax, rax",
        "je 2f",
        "mov rcx, qword ptr [rax]",
        "add word ptr [rdx + 0x10], 1",
        "mov qword ptr [rdx + 0x8], rcx",
        "mov qword ptr [rax], 0",
        "ret",
        "2:",
        "mov rdi, rsi",
        "jmp mi_new",
    );
}

macro_rules! mi_free_fast_body {
    () => {
        core::arch::naked_asm!(
            "mov rax, qword ptr [rip + {page_map}]",
            "mov rcx, rdi",
            "shr rcx, 29",
            "mov rsi, qword ptr [rax + rcx*8]",
            "mov ecx, edi",
            "shr ecx, 13",
            "and ecx, 0xfff8",
            "mov rsi, qword ptr [rsi + rcx]",
            "test rsi, rsi",
            "je 3f",
            "mov rax, qword ptr fs:[0]",
            "mov rdx, qword ptr [rsi]",
            "xor rdx, rax",
            "jne 2f",
            "mov rax, qword ptr [rsi + 0x18]",
            "mov qword ptr [rdi], rax",
            "mov qword ptr [rsi + 0x18], rdi",
            "dec word ptr [rsi + 0x10]",
            "jne 3f",
            "cmp byte ptr [rsi + 0x16], 0",
            "jne 3f",
            "mov rdi, rsi",
            "jmp {retire}",
            "2:",
            "jmp {slow}",
            "3:",
            "ret",
            page_map = sym crate::page_map::_mi_page_map,
            retire = sym crate::page::_mi_page_retire,
            slow = sym crate::free::mi_free_ex_slow,
        )
    };
}

#[cfg(mi_c_malloc_fast)]
#[unsafe(naked)]
#[unsafe(export_name = "free")]
pub unsafe extern "C" fn mi_unix_free_fast(_p: *mut c_void) {
    mi_free_fast_body!();
}

#[cfg(all(not(windows), mi_c_malloc_fast))]
#[unsafe(naked)]
#[unsafe(export_name = "_ZdlPv")]
pub unsafe extern "C" fn mi_cxx_delete_fast(_p: *mut c_void) {
    mi_free_fast_body!();
}

#[cfg(all(not(windows), mi_c_malloc_fast))]
#[unsafe(naked)]
#[unsafe(export_name = "_ZdaPv")]
pub unsafe extern "C" fn mi_cxx_delete_array_fast(_p: *mut c_void) {
    mi_free_fast_body!();
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

#[macro_export]
macro_rules! MI_INTERPOSE_MI {
    (malloc) => {
        $crate::MI_INTERPOSE_FUN!(malloc, $crate::mimalloc::mi_malloc)
    };
    (calloc) => {
        $crate::MI_INTERPOSE_FUN!(calloc, $crate::mimalloc::mi_calloc)
    };
    (realloc) => {
        $crate::MI_INTERPOSE_FUN!(realloc, $crate::mimalloc::mi_realloc)
    };
    (strdup) => {
        $crate::MI_INTERPOSE_FUN!(strdup, $crate::mimalloc::mi_strdup)
    };
    (realpath) => {
        $crate::MI_INTERPOSE_FUN!(realpath, $crate::mimalloc::mi_realpath)
    };
    (posix_memalign) => {
        $crate::MI_INTERPOSE_FUN!(posix_memalign, $crate::mimalloc::mi_posix_memalign)
    };
    (reallocf) => {
        $crate::MI_INTERPOSE_FUN!(reallocf, $crate::mimalloc::mi_reallocf)
    };
    (valloc) => {
        $crate::MI_INTERPOSE_FUN!(valloc, $crate::mimalloc::mi_valloc)
    };
    (malloc_good_size) => {
        $crate::MI_INTERPOSE_FUN!(malloc_good_size, $crate::mimalloc::mi_malloc_good_size)
    };
    (strndup) => {
        $crate::MI_INTERPOSE_FUN!(strndup, $crate::mimalloc::mi_strndup)
    };
    (aligned_alloc) => {
        $crate::MI_INTERPOSE_FUN!(aligned_alloc, $crate::mimalloc::mi_aligned_alloc)
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
    fn malloc(size: usize) -> *mut c_void;
    fn calloc(count: usize, size: usize) -> *mut c_void;
    fn realloc(p: *mut c_void, newsize: usize) -> *mut c_void;
    fn strdup(s: *const c_char) -> *mut c_char;
    fn realpath(fname: *const c_char, resolved_name: *mut c_char) -> *mut c_char;
    fn posix_memalign(p: *mut *mut c_void, alignment: usize, size: usize) -> i32;
    fn reallocf(p: *mut c_void, newsize: usize) -> *mut c_void;
    fn valloc(size: usize) -> *mut c_void;
    fn free(p: *mut c_void);
    fn _ZdlPv(p: *mut c_void);
    fn _ZdaPv(p: *mut c_void);
    fn _ZdlPvm(p: *mut c_void, n: usize);
    fn _ZdaPvm(p: *mut c_void, n: usize);
    fn _Znwm(n: usize) -> *mut c_void;
    fn _Znam(n: usize) -> *mut c_void;
    fn _ZnwmRKSt9nothrow_t(n: usize, tag: mi_nothrow_t) -> *mut c_void;
    fn _ZnamRKSt9nothrow_t(n: usize, tag: mi_nothrow_t) -> *mut c_void;
}

#[cfg(target_os = "macos")]
#[used]
#[unsafe(link_section = "__DATA,__interpose")]
pub static _mi_interposes: [mi_interpose_s; 12] = [
    MI_INTERPOSE_FUN!(malloc, crate::mimalloc::mi_malloc),
    MI_INTERPOSE_FUN!(calloc, crate::mimalloc::mi_calloc),
    MI_INTERPOSE_FUN!(realloc, crate::mimalloc::mi_realloc),
    MI_INTERPOSE_FUN!(strdup, crate::mimalloc::mi_strdup),
    MI_INTERPOSE_FUN!(realpath, crate::mimalloc::mi_realpath),
    MI_INTERPOSE_FUN!(posix_memalign, crate::mimalloc::mi_posix_memalign),
    MI_INTERPOSE_FUN!(reallocf, crate::mimalloc::mi_reallocf),
    MI_INTERPOSE_FUN!(valloc, crate::mimalloc::mi_valloc),
    MI_INTERPOSE_FUN!(malloc_size, crate::alloc_override::mi_malloc_size_checked),
    MI_INTERPOSE_FUN!(malloc_good_size, crate::mimalloc::mi_malloc_good_size),
    MI_INTERPOSE_FUN!(free, crate::mimalloc::mi_cfree),
    MI_INTERPOSE_FUN!(vfree, crate::mimalloc::mi_cfree),
];

#[cfg(target_os = "macos")]
pub static _mi_interposes_10_7: [mi_interpose_s; 1] =
    [MI_INTERPOSE_FUN!(strndup, crate::mimalloc::mi_strndup)];

#[cfg(target_os = "macos")]
pub static _mi_interposes_10_15: [mi_interpose_s; 1] = [MI_INTERPOSE_FUN!(
    aligned_alloc,
    crate::mimalloc::mi_aligned_alloc
)];

#[cfg(target_os = "macos")]
#[used]
#[unsafe(link_section = "__DATA,__interpose")]
pub static _mi_cxx_interposes: [mi_interpose_s; 8] = [
    MI_INTERPOSE_FUN!(_ZdlPv, crate::mimalloc::mi_free),
    MI_INTERPOSE_FUN!(_ZdaPv, crate::mimalloc::mi_free),
    MI_INTERPOSE_FUN!(_ZdlPvm, crate::mimalloc::mi_free_size),
    MI_INTERPOSE_FUN!(_ZdaPvm, crate::mimalloc::mi_free_size),
    MI_INTERPOSE_FUN!(_Znwm, crate::mimalloc::mi_new),
    MI_INTERPOSE_FUN!(_Znam, crate::mimalloc::mi_new),
    MI_INTERPOSE_FUN!(_ZnwmRKSt9nothrow_t, crate::mimalloc::mi_new_nothrow),
    MI_INTERPOSE_FUN!(_ZnamRKSt9nothrow_t, crate::mimalloc::mi_new_nothrow),
];

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

#[cfg(all(not(windows), not(mi_c_malloc_fast)))]
#[unsafe(export_name = "_ZdlPv")]
pub unsafe extern "C" fn mi_cxx_delete(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

#[cfg(all(not(windows), not(mi_c_malloc_fast)))]
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
#[cfg_attr(
    all(not(windows), not(mi_c_malloc_fast)),
    unsafe(export_name = "_Znwm")
)]
pub unsafe extern "C" fn mi_cxx_new_64(n: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new(n) }
}

#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[cfg_attr(
    all(not(windows), not(mi_c_malloc_fast)),
    unsafe(export_name = "_Znam")
)]
pub unsafe extern "C" fn mi_cxx_new_array_64(n: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new(n) }
}

#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[cfg_attr(not(windows), unsafe(export_name = "_ZnwmRKSt9nothrow_t"))]
pub unsafe extern "C" fn mi_cxx_new_64_nothrow(n: usize, tag: mi_nothrow_t) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_nothrow(n) }
}

#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[cfg_attr(not(windows), unsafe(export_name = "_ZnamRKSt9nothrow_t"))]
pub unsafe extern "C" fn mi_cxx_new_array_64_nothrow(n: usize, tag: mi_nothrow_t) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_nothrow(n) }
}

#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[cfg_attr(not(windows), unsafe(export_name = "_ZnwmSt11align_val_t"))]
pub unsafe extern "C" fn mi_cxx_new_64_aligned(n: usize, al: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[cfg_attr(not(windows), unsafe(export_name = "_ZnamSt11align_val_t"))]
pub unsafe extern "C" fn mi_cxx_new_array_64_aligned(n: usize, al: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[cfg_attr(
    not(windows),
    unsafe(export_name = "_ZnwmSt11align_val_tRKSt9nothrow_t")
)]
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
#[cfg_attr(
    not(windows),
    unsafe(export_name = "_ZnamSt11align_val_tRKSt9nothrow_t")
)]
pub unsafe extern "C" fn mi_cxx_new_array_64_aligned_nothrow(
    n: usize,
    al: usize,
    tag: mi_nothrow_t,
) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_aligned_nothrow(n, al) }
}

#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(not(windows), unsafe(export_name = "_Znwj"))]
pub unsafe extern "C" fn mi_cxx_new_32(n: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new(n) }
}

#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(not(windows), unsafe(export_name = "_Znaj"))]
pub unsafe extern "C" fn mi_cxx_new_array_32(n: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new(n) }
}

#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(not(windows), unsafe(export_name = "_ZnwjRKSt9nothrow_t"))]
pub unsafe extern "C" fn mi_cxx_new_32_nothrow(n: usize, tag: mi_nothrow_t) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_nothrow(n) }
}

#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(not(windows), unsafe(export_name = "_ZnajRKSt9nothrow_t"))]
pub unsafe extern "C" fn mi_cxx_new_array_32_nothrow(n: usize, tag: mi_nothrow_t) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_nothrow(n) }
}

#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(not(windows), unsafe(export_name = "_ZnwjSt11align_val_t"))]
pub unsafe extern "C" fn mi_cxx_new_32_aligned(n: usize, al: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(not(windows), unsafe(export_name = "_ZnajSt11align_val_t"))]
pub unsafe extern "C" fn mi_cxx_new_array_32_aligned(n: usize, al: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(
    not(windows),
    unsafe(export_name = "_ZnwjSt11align_val_tRKSt9nothrow_t")
)]
pub unsafe extern "C" fn mi_cxx_new_32_aligned_nothrow(
    n: usize,
    al: usize,
    tag: mi_nothrow_t,
) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_aligned_nothrow(n, al) }
}

#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(
    not(windows),
    unsafe(export_name = "_ZnajSt11align_val_tRKSt9nothrow_t")
)]
pub unsafe extern "C" fn mi_cxx_new_array_32_aligned_nothrow(
    n: usize,
    al: usize,
    tag: mi_nothrow_t,
) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_aligned_nothrow(n, al) }
}

#[inline]
pub unsafe fn mi_operator_new_array_nothrow(n: usize, tag: mi_nothrow_t) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_nothrow(n) }
}

#[inline]
pub unsafe fn mi_operator_delete_sized(p: *mut c_void, n: usize) {
    unsafe { crate::mimalloc::mi_free_size(p, n) }
}

#[inline]
pub unsafe fn mi_operator_delete_array_sized(p: *mut c_void, n: usize) {
    unsafe { crate::mimalloc::mi_free_size(p, n) }
}

#[inline]
pub unsafe fn mi_operator_delete_array_aligned(p: *mut c_void, al: usize) {
    unsafe { crate::mimalloc::mi_free_aligned(p, al) }
}

#[inline]
pub unsafe fn mi_operator_delete_sized_aligned(p: *mut c_void, n: usize, al: usize) {
    unsafe { crate::mimalloc::mi_free_size_aligned(p, n, al) }
}

#[inline]
pub unsafe fn mi_operator_delete_array_sized_aligned(p: *mut c_void, n: usize, al: usize) {
    unsafe { crate::mimalloc::mi_free_size_aligned(p, n, al) }
}

#[inline]
pub unsafe fn mi_operator_delete_aligned_nothrow(p: *mut c_void, al: usize, tag: mi_nothrow_t) {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_free_aligned(p, al) }
}

#[inline]
pub unsafe fn mi_operator_delete_array_aligned_nothrow(
    p: *mut c_void,
    al: usize,
    tag: mi_nothrow_t,
) {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_free_aligned(p, al) }
}

#[inline]
pub unsafe fn mi_operator_new_aligned(n: usize, al: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

#[inline]
pub unsafe fn mi_operator_new_array_aligned(n: usize, al: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

#[inline]
pub unsafe fn mi_operator_new_aligned_nothrow(
    n: usize,
    al: usize,
    tag: mi_nothrow_t,
) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_aligned_nothrow(n, al) }
}

#[inline]
pub unsafe fn mi_operator_new_array_aligned_nothrow(
    n: usize,
    al: usize,
    tag: mi_nothrow_t,
) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_aligned_nothrow(n, al) }
}

#[inline]
pub unsafe fn mi_unix_malloc(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_malloc(size) }
}

#[cfg(all(not(windows), not(mi_c_malloc_fast)))]
#[unsafe(export_name = "malloc")]
pub unsafe extern "C" fn mi_unix_malloc_export(size: usize) -> *mut c_void {
    unsafe { mi_unix_malloc(size) }
}

#[inline]
pub unsafe fn mi_unix_calloc(count: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_calloc(count, size) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "calloc")]
pub unsafe extern "C" fn mi_unix_calloc_export(count: usize, size: usize) -> *mut c_void {
    unsafe { mi_unix_calloc(count, size) }
}

#[inline]
pub unsafe fn mi_unix_realloc(p: *mut c_void, newsize: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_realloc(p, newsize) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "realloc")]
pub unsafe extern "C" fn mi_unix_realloc_export(p: *mut c_void, newsize: usize) -> *mut c_void {
    unsafe { mi_unix_realloc(p, newsize) }
}

#[inline]
pub unsafe fn mi_unix_free(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

#[cfg(all(not(windows), not(mi_c_malloc_fast)))]
#[unsafe(export_name = "free")]
pub unsafe extern "C" fn mi_unix_free_export(p: *mut c_void) {
    unsafe { mi_unix_free(p) }
}

#[inline]
pub unsafe fn mi_unix_strdup(s: *const c_char) -> *mut c_char {
    unsafe { crate::mimalloc::mi_strdup(s) }
}

#[inline]
pub unsafe fn mi_unix_strndup(s: *const c_char, n: usize) -> *mut c_char {
    unsafe { crate::mimalloc::mi_strndup(s, n) }
}

#[inline]
pub unsafe fn mi_unix_reallocf(p: *mut c_void, newsize: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_reallocf(p, newsize) }
}

#[inline]
pub unsafe fn mi_unix_malloc_size(p: *const c_void) -> usize {
    unsafe { crate::mimalloc::mi_usable_size(p) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "malloc_size")]
pub unsafe extern "C" fn mi_unix_malloc_size_export(p: *const c_void) -> usize {
    unsafe { mi_unix_malloc_size(p) }
}

#[inline]
pub unsafe fn mi_unix_malloc_usable_size(p: *const c_void) -> usize {
    unsafe { crate::mimalloc::mi_usable_size(p) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "malloc_usable_size")]
pub unsafe extern "C" fn mi_unix_malloc_usable_size_export(p: *const c_void) -> usize {
    unsafe { mi_unix_malloc_usable_size(p) }
}

#[inline]
pub unsafe fn mi_unix_valloc(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_valloc(size) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "valloc")]
pub unsafe extern "C" fn mi_unix_valloc_export(size: usize) -> *mut c_void {
    unsafe { mi_unix_valloc(size) }
}

#[inline]
pub unsafe fn mi_unix_vfree(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

#[inline]
pub unsafe fn mi_unix_posix_memalign(p: *mut *mut c_void, alignment: usize, size: usize) -> c_int {
    unsafe { crate::mimalloc::mi_posix_memalign(p, alignment, size) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "posix_memalign")]
pub unsafe extern "C" fn mi_unix_posix_memalign_export(
    p: *mut *mut c_void,
    alignment: usize,
    size: usize,
) -> c_int {
    unsafe { mi_unix_posix_memalign(p, alignment, size) }
}

#[inline]
pub unsafe fn mi_unix_aligned_alloc(alignment: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_aligned_alloc(alignment, size) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "aligned_alloc")]
pub unsafe extern "C" fn mi_unix_aligned_alloc_export(
    alignment: usize,
    size: usize,
) -> *mut c_void {
    unsafe { mi_unix_aligned_alloc(alignment, size) }
}

#[inline]
pub unsafe fn mi_unix_cfree(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "cfree")]
pub unsafe extern "C" fn mi_unix_cfree_export(p: *mut c_void) {
    unsafe { mi_unix_cfree(p) }
}

#[inline]
pub unsafe fn mi_unix_pvalloc(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_pvalloc(size) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "pvalloc")]
pub unsafe extern "C" fn mi_unix_pvalloc_export(size: usize) -> *mut c_void {
    unsafe { mi_unix_pvalloc(size) }
}

#[inline]
pub unsafe fn mi_unix_memalign(alignment: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_memalign(alignment, size) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "memalign")]
pub unsafe extern "C" fn mi_unix_memalign_export(alignment: usize, size: usize) -> *mut c_void {
    unsafe { mi_unix_memalign(alignment, size) }
}

#[inline]
pub unsafe fn mi_unix_aligned_malloc(size: usize, alignment: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_malloc_aligned(size, alignment) }
}

#[inline]
pub unsafe fn mi_unix_reallocarray(p: *mut c_void, count: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_reallocarray(p, count, size) }
}

#[cfg(not(windows))]
#[unsafe(export_name = "reallocarray")]
pub unsafe extern "C" fn mi_unix_reallocarray_export(
    p: *mut c_void,
    count: usize,
    size: usize,
) -> *mut c_void {
    unsafe { mi_unix_reallocarray(p, count, size) }
}

#[inline]
pub unsafe fn mi_unix_reallocarr(ptrp: *mut *mut c_void, count: usize, size: usize) -> c_int {
    unsafe { crate::mimalloc::mi_reallocarr(ptrp, count, size) }
}

#[inline]
pub unsafe fn mi_wasi_libc_malloc(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_malloc(size) }
}

#[inline]
pub unsafe fn mi_wasi_libc_calloc(count: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_calloc(count, size) }
}

#[inline]
pub unsafe fn mi_wasi_libc_realloc(p: *mut c_void, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_realloc(p, size) }
}

#[inline]
pub unsafe fn mi_wasi_libc_free(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

#[inline]
pub unsafe fn mi_wasi_libc_memalign(alignment: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_memalign(alignment, size) }
}

#[inline]
pub unsafe fn mi_linux_libc_malloc(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_malloc(size) }
}

#[inline]
pub unsafe fn mi_linux_libc_calloc(count: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_calloc(count, size) }
}

#[inline]
pub unsafe fn mi_linux_libc_realloc(p: *mut c_void, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_realloc(p, size) }
}

#[inline]
pub unsafe fn mi_linux_libc_free(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

#[inline]
pub unsafe fn mi_linux_libc_cfree(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

#[inline]
pub unsafe fn mi_linux_libc_valloc(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_valloc(size) }
}

#[inline]
pub unsafe fn mi_linux_libc_pvalloc(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_pvalloc(size) }
}

#[inline]
pub unsafe fn mi_linux_libc_memalign(alignment: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_memalign(alignment, size) }
}

#[inline]
pub unsafe fn mi_linux_posix_memalign(p: *mut *mut c_void, alignment: usize, size: usize) -> c_int {
    unsafe { crate::mimalloc::mi_posix_memalign(p, alignment, size) }
}
