//! Mechanical translation of `src/alloc-override.c`.

#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

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

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(mi_c_malloc_fast)]
#[unsafe(naked)]
#[unsafe(export_name = "_Znwm")]
pub unsafe extern "C" fn mi_cxx_new_fast_64(_size: usize) -> *mut c_void {
    core::arch::naked_asm!(
        "endbr64",
        "mov rax, qword ptr [rip + __mi_theap_default@GOTTPOFF]",
        "mov rsi, rdi",
        "mov rdi, qword ptr fs:[rax]",
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
        "xor edx, edx",
        "jmp {try_new}",
        try_new = sym crate::alloc::mi_theap_try_new,
    );
}

#[cfg(mi_c_malloc_fast)]
core::arch::global_asm!(".globl _Znam", ".type _Znam,@function", ".set _Znam, _Znwm",);

#[cfg(all(not(windows), mi_c_malloc_fast))]
core::arch::global_asm!(
    ".globl free",
    ".type free,@function",
    ".p2align 4",
    "free:",
    "endbr64",
    "mov rax, qword ptr [rip + {page_map}]",
    "mov rdx, rdi",
    "mov rsi, rdi",
    "shr rdx, 29",
    "mov rdx, qword ptr [rax + rdx*8]",
    "mov rax, rdi",
    "shr rax, 16",
    "and eax, 0x1fff",
    "mov rdi, qword ptr [rdx + rax*8]",
    "test rdi, rdi",
    "je 3f",
    "mov rax, qword ptr fs:[0]",
    "mov rdx, qword ptr [rdi]",
    "cmp rdx, rax",
    "jne 2f",
    "mov rax, qword ptr [rdi + 0x18]",
    "mov qword ptr [rsi], rax",
    "sub word ptr [rdi + 0x10], 1",
    "mov qword ptr [rdi + 0x18], rsi",
    "je 7f",
    "3:",
    "ret",
    "7:",
    "cmp byte ptr [rdi + 0x16], 0",
    "jne 3b",
    "jmp {retire}",
    "2:",
    "xor rdx, rax",
    "cmp rdx, 3",
    "jbe 5f",
    "test dl, 3",
    "jne 6f",
    "mov rcx, rsi",
    "mov rax, qword ptr [rdi + 0x20]",
    "lea rdx, [rdi + 0x20]",
    "or rcx, 1",
    "4:",
    "mov r8, rax",
    "and r8, -2",
    "mov qword ptr [rsi], r8",
    "lock cmpxchg qword ptr [rdx], rcx",
    "jne 4b",
    "test al, 1",
    "jne 3b",
    "jmp {collect_mt}",
    "5:",
    "jmp {generic_local}",
    "6:",
    "jmp {generic_mt}",
    ".size free, .-free",
    ".globl mi_free",
    ".type mi_free,@function",
    ".set mi_free, free",
    ".globl _ZdlPv",
    ".type _ZdlPv,@function",
    ".set _ZdlPv, free",
    ".globl _ZdaPv",
    ".type _ZdaPv,@function",
    ".set _ZdaPv, free",
    ".globl __libc_free",
    ".type __libc_free,@function",
    ".set __libc_free, free",
    ".fill 0x540,1,0x90",
    ".globl malloc",
    ".type malloc,@function",
    ".p2align 4",
    "malloc:",
    "endbr64",
    "mov rax, qword ptr [rip + __mi_theap_default@GOTTPOFF]",
    "mov rsi, rdi",
    "mov rdi, qword ptr fs:[rax]",
    "cmp rsi, 1024",
    "ja .Lmi_malloc_slow",
    "lea rax, [rsi + 7]",
    "shr rax, 3",
    "mov rdx, qword ptr [rdi + rax*8 + 0x110]",
    "mov rax, qword ptr [rdx + 0x8]",
    "test rax, rax",
    "je .Lmi_malloc_slow",
    "mov rcx, qword ptr [rax]",
    "add word ptr [rdx + 0x10], 1",
    "mov qword ptr [rdx + 0x8], rcx",
    "mov qword ptr [rax], 0",
    "ret",
    ".Lmi_malloc_slow:",
    "xor ecx, ecx",
    "xor edx, edx",
    "jmp {malloc_generic}",
    ".size malloc, .-malloc",
    ".globl mi_malloc",
    ".type mi_malloc,@function",
    ".set mi_malloc, malloc",
    ".globl __libc_malloc",
    ".type __libc_malloc,@function",
    ".set __libc_malloc, malloc",
    ".globl calloc",
    ".type calloc,@function",
    ".p2align 4",
    "calloc:",
    "endbr64",
    "mov rdx, qword ptr [rip + __mi_theap_default@GOTTPOFF]",
    "mov rax, rdi",
    "mov rdi, qword ptr fs:[rdx]",
    "cmp rax, 1",
    "je .Lmi_calloc_total",
    "mul rsi",
    "mov rsi, rax",
    "jo .Lmi_calloc_overflow",
    ".Lmi_calloc_total:",
    "cmp rsi, 1024",
    "ja .Lmi_calloc_slow",
    "lea rax, [rsi + 7]",
    "shr rax, 3",
    "mov rax, qword ptr [rdi + rax*8 + 0x110]",
    "mov r8, qword ptr [rax + 0x8]",
    "test r8, r8",
    "je .Lmi_calloc_slow",
    "mov rdx, qword ptr [r8]",
    "add word ptr [rax + 0x10], 1",
    "cmp byte ptr [rax + 0x17], 0",
    "mov rcx, qword ptr [rax + 0x28]",
    "mov qword ptr [rax + 0x8], rdx",
    "je .Lmi_calloc_zero",
    "mov qword ptr [r8], 0",
    "mov rax, r8",
    "ret",
    ".Lmi_calloc_slow:",
    "xor ecx, ecx",
    "mov edx, 1",
    "jmp {malloc_generic}",
    ".Lmi_calloc_overflow:",
    "xor eax, eax",
    "ret",
    ".Lmi_calloc_zero:",
    "cmp qword ptr [rip + {stosb_max}], rcx",
    "jb .Lmi_calloc_memset",
    "mov rdi, r8",
    "xor eax, eax",
    "rep stosb",
    "mov rax, r8",
    "ret",
    ".Lmi_calloc_memset:",
    "sub rsp, 8",
    "mov rdx, rcx",
    "xor esi, esi",
    "mov rdi, r8",
    "call memset",
    "add rsp, 8",
    "ret",
    ".size calloc, .-calloc",
    ".globl mi_calloc",
    ".type mi_calloc,@function",
    ".set mi_calloc, calloc",
    ".globl __libc_calloc",
    ".type __libc_calloc,@function",
    ".set __libc_calloc, calloc",
    page_map = sym crate::page_map::_mi_page_map,
    retire = sym crate::page::_mi_page_retire,
    collect_mt = sym crate::free::mi_free_try_collect_mt,
    generic_local = sym crate::free::mi_free_generic_local,
    generic_mt = sym crate::free::mi_free_generic_mt,
    malloc_generic = sym crate::page::_mi_malloc_generic,
    stosb_max = sym crate::init::_mi_cpu_stosb_max,
);

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

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mi_malloc_size_checked(p: *mut c_void) -> usize {
    if !unsafe { crate::mimalloc::mi_is_in_heap_region(p.cast_const()) } {
        return 0;
    }
    unsafe { crate::mimalloc::mi_usable_size(p.cast_const()) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_forward1<R, A>(fun: unsafe extern "C" fn(A) -> R, x: A) -> R {
    unsafe { fun(x) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_forward3<R, A, B, C>(
    fun: unsafe extern "C" fn(A, B, C) -> R,
    x: A,
    y: B,
    z: C,
) -> R {
    unsafe { fun(x, y, z) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
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

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[allow(improper_ctypes_definitions)]
#[inline]
pub unsafe fn mi_malloc_good_size_forward(size: usize) -> usize {
    unsafe { crate::mimalloc::mi_malloc_good_size(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_vfree_forward(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_malloc_size_forward(p: *const c_void) -> usize {
    unsafe { crate::mimalloc::mi_usable_size(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_aligned_alloc_forward(alignment: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_aligned_alloc(alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _expand(block: *mut c_void, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_expand(block, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _msize_base(block: *mut c_void) -> usize {
    unsafe { crate::mimalloc::mi_malloc_size(block) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _msize(block: *mut c_void) -> usize {
    unsafe { crate::mimalloc::mi_malloc_size(block) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _free_base(block: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(block) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn free(block: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(block) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_malloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _malloc_base(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_malloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _realloc_base(block: *mut c_void, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_realloc(block, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn realloc(block: *mut c_void, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_realloc(block, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _calloc_base(count: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_calloc(count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_calloc(count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _recalloc_base(
    block: *mut c_void,
    count: usize,
    size: usize,
) -> *mut c_void {
    unsafe { crate::mimalloc::mi_recalloc(block, count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _recalloc(block: *mut c_void, count: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_recalloc(block, count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_free(block: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(block) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_malloc(size: usize, alignment: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_malloc_aligned(size, alignment) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_offset_malloc(
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    unsafe { crate::mimalloc::mi_malloc_aligned_at(size, alignment, offset) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
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

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
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

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
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

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_realloc(
    block: *mut c_void,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    unsafe { crate::mimalloc::mi_realloc_aligned(block, size, alignment) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
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

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_c_malloc_fast)))]
#[unsafe(export_name = "_ZdlPv")]
pub unsafe extern "C" fn mi_cxx_delete(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_c_malloc_fast)))]
#[unsafe(export_name = "_ZdaPv")]
pub unsafe extern "C" fn mi_cxx_delete_array(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "_ZdlPvm")]
pub unsafe extern "C" fn mi_cxx_delete_sized(p: *mut c_void, n: usize) {
    unsafe { crate::mimalloc::mi_free_size(p, n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "_ZdaPvm")]
pub unsafe extern "C" fn mi_cxx_delete_array_sized(p: *mut c_void, n: usize) {
    unsafe { crate::mimalloc::mi_free_size(p, n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "_ZdlPvSt11align_val_t")]
pub unsafe extern "C" fn mi_cxx_delete_aligned(p: *mut c_void, al: usize) {
    unsafe { crate::mimalloc::mi_free_aligned(p, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "_ZdaPvSt11align_val_t")]
pub unsafe extern "C" fn mi_cxx_delete_array_aligned(p: *mut c_void, al: usize) {
    unsafe { crate::mimalloc::mi_free_aligned(p, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "_ZdlPvmSt11align_val_t")]
pub unsafe extern "C" fn mi_cxx_delete_sized_aligned(p: *mut c_void, n: usize, al: usize) {
    unsafe { crate::mimalloc::mi_free_size_aligned(p, n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "_ZdaPvmSt11align_val_t")]
pub unsafe extern "C" fn mi_cxx_delete_array_sized_aligned(p: *mut c_void, n: usize, al: usize) {
    unsafe { crate::mimalloc::mi_free_size_aligned(p, n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "_ZdlPvRKSt9nothrow_t")]
pub unsafe extern "C" fn mi_cxx_delete_nothrow(p: *mut c_void, tag: mi_nothrow_t) {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "_ZdaPvRKSt9nothrow_t")]
pub unsafe extern "C" fn mi_cxx_delete_array_nothrow(p: *mut c_void, tag: mi_nothrow_t) {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
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

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
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

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
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

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
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

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[cfg_attr(not(windows), unsafe(export_name = "_ZnwmRKSt9nothrow_t"))]
pub unsafe extern "C" fn mi_cxx_new_64_nothrow(n: usize, tag: mi_nothrow_t) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_nothrow(n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[cfg_attr(not(windows), unsafe(export_name = "_ZnamRKSt9nothrow_t"))]
pub unsafe extern "C" fn mi_cxx_new_array_64_nothrow(n: usize, tag: mi_nothrow_t) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_nothrow(n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[cfg_attr(not(windows), unsafe(export_name = "_ZnwmSt11align_val_t"))]
pub unsafe extern "C" fn mi_cxx_new_64_aligned(n: usize, al: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[cfg_attr(not(windows), unsafe(export_name = "_ZnamSt11align_val_t"))]
pub unsafe extern "C" fn mi_cxx_new_array_64_aligned(n: usize, al: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
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

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
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

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(not(windows), unsafe(export_name = "_Znwj"))]
pub unsafe extern "C" fn mi_cxx_new_32(n: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new(n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(not(windows), unsafe(export_name = "_Znaj"))]
pub unsafe extern "C" fn mi_cxx_new_array_32(n: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new(n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(not(windows), unsafe(export_name = "_ZnwjRKSt9nothrow_t"))]
pub unsafe extern "C" fn mi_cxx_new_32_nothrow(n: usize, tag: mi_nothrow_t) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_nothrow(n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(not(windows), unsafe(export_name = "_ZnajRKSt9nothrow_t"))]
pub unsafe extern "C" fn mi_cxx_new_array_32_nothrow(n: usize, tag: mi_nothrow_t) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_nothrow(n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(not(windows), unsafe(export_name = "_ZnwjSt11align_val_t"))]
pub unsafe extern "C" fn mi_cxx_new_32_aligned(n: usize, al: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(not(windows), unsafe(export_name = "_ZnajSt11align_val_t"))]
pub unsafe extern "C" fn mi_cxx_new_array_32_aligned(n: usize, al: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
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

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
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

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_new_array_nothrow(n: usize, tag: mi_nothrow_t) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_nothrow(n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_delete_sized(p: *mut c_void, n: usize) {
    unsafe { crate::mimalloc::mi_free_size(p, n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_delete_array_sized(p: *mut c_void, n: usize) {
    unsafe { crate::mimalloc::mi_free_size(p, n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_delete_array_aligned(p: *mut c_void, al: usize) {
    unsafe { crate::mimalloc::mi_free_aligned(p, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_delete_sized_aligned(p: *mut c_void, n: usize, al: usize) {
    unsafe { crate::mimalloc::mi_free_size_aligned(p, n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_delete_array_sized_aligned(p: *mut c_void, n: usize, al: usize) {
    unsafe { crate::mimalloc::mi_free_size_aligned(p, n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_delete_aligned_nothrow(p: *mut c_void, al: usize, tag: mi_nothrow_t) {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_free_aligned(p, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_delete_array_aligned_nothrow(
    p: *mut c_void,
    al: usize,
    tag: mi_nothrow_t,
) {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_free_aligned(p, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_new_aligned(n: usize, al: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_new_array_aligned(n: usize, al: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_new_aligned_nothrow(
    n: usize,
    al: usize,
    tag: mi_nothrow_t,
) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_aligned_nothrow(n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_new_array_aligned_nothrow(
    n: usize,
    al: usize,
    tag: mi_nothrow_t,
) -> *mut c_void {
    crate::MI_UNUSED!(tag);
    unsafe { crate::mimalloc::mi_new_aligned_nothrow(n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_malloc(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_malloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_c_malloc_fast)))]
#[unsafe(export_name = "malloc")]
pub unsafe extern "C" fn mi_unix_malloc_export(size: usize) -> *mut c_void {
    unsafe { mi_unix_malloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_calloc(count: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_calloc(count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_c_malloc_fast)))]
#[unsafe(export_name = "calloc")]
pub unsafe extern "C" fn mi_unix_calloc_export(count: usize, size: usize) -> *mut c_void {
    unsafe { mi_unix_calloc(count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_realloc(p: *mut c_void, newsize: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_realloc(p, newsize) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "realloc")]
pub unsafe extern "C" fn mi_unix_realloc_export(p: *mut c_void, newsize: usize) -> *mut c_void {
    unsafe { mi_unix_realloc(p, newsize) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_free(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_c_malloc_fast)))]
#[unsafe(export_name = "free")]
pub unsafe extern "C" fn mi_unix_free_export(p: *mut c_void) {
    unsafe { mi_unix_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_strdup(s: *const c_char) -> *mut c_char {
    unsafe { crate::mimalloc::mi_strdup(s) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "strdup")]
pub unsafe extern "C" fn mi_unix_strdup_export(s: *const c_char) -> *mut c_char {
    unsafe { mi_unix_strdup(s) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_strndup(s: *const c_char, n: usize) -> *mut c_char {
    unsafe { crate::mimalloc::mi_strndup(s, n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "strndup")]
pub unsafe extern "C" fn mi_unix_strndup_export(s: *const c_char, n: usize) -> *mut c_char {
    unsafe { mi_unix_strndup(s, n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_reallocf(p: *mut c_void, newsize: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_reallocf(p, newsize) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "reallocf")]
pub unsafe extern "C" fn mi_unix_reallocf_export(p: *mut c_void, newsize: usize) -> *mut c_void {
    unsafe { mi_unix_reallocf(p, newsize) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_malloc_size(p: *const c_void) -> usize {
    unsafe { crate::mimalloc::mi_usable_size(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "malloc_size")]
pub unsafe extern "C" fn mi_unix_malloc_size_export(p: *const c_void) -> usize {
    unsafe { mi_unix_malloc_size(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_malloc_usable_size(p: *const c_void) -> usize {
    unsafe { crate::mimalloc::mi_usable_size(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_malloc_good_size(size: usize) -> usize {
    unsafe { crate::mimalloc::mi_malloc_good_size(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "malloc_good_size")]
pub unsafe extern "C" fn mi_unix_malloc_good_size_export(size: usize) -> usize {
    unsafe { mi_unix_malloc_good_size(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "malloc_usable_size")]
pub unsafe extern "C" fn mi_unix_malloc_usable_size_export(p: *const c_void) -> usize {
    unsafe { mi_unix_malloc_usable_size(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_valloc(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_valloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "valloc")]
pub unsafe extern "C" fn mi_unix_valloc_export(size: usize) -> *mut c_void {
    unsafe { mi_unix_valloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_vfree(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "vfree")]
pub unsafe extern "C" fn mi_unix_vfree_export(p: *mut c_void) {
    unsafe { mi_unix_vfree(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_posix_memalign(p: *mut *mut c_void, alignment: usize, size: usize) -> c_int {
    unsafe { crate::mimalloc::mi_posix_memalign(p, alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "posix_memalign")]
pub unsafe extern "C" fn mi_unix_posix_memalign_export(
    p: *mut *mut c_void,
    alignment: usize,
    size: usize,
) -> c_int {
    unsafe { mi_unix_posix_memalign(p, alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_aligned_alloc(alignment: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_aligned_alloc(alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "aligned_alloc")]
pub unsafe extern "C" fn mi_unix_aligned_alloc_export(
    alignment: usize,
    size: usize,
) -> *mut c_void {
    unsafe { mi_unix_aligned_alloc(alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_cfree(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "cfree")]
pub unsafe extern "C" fn mi_unix_cfree_export(p: *mut c_void) {
    unsafe { mi_unix_cfree(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_pvalloc(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_pvalloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "pvalloc")]
pub unsafe extern "C" fn mi_unix_pvalloc_export(size: usize) -> *mut c_void {
    unsafe { mi_unix_pvalloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_memalign(alignment: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_memalign(alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "memalign")]
pub unsafe extern "C" fn mi_unix_memalign_export(alignment: usize, size: usize) -> *mut c_void {
    unsafe { mi_unix_memalign(alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_aligned_malloc(size: usize, alignment: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_malloc_aligned(size, alignment) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "_aligned_malloc")]
pub unsafe extern "C" fn mi_unix_aligned_malloc_export(
    size: usize,
    alignment: usize,
) -> *mut c_void {
    unsafe { mi_unix_aligned_malloc(size, alignment) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_reallocarray(p: *mut c_void, count: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_reallocarray(p, count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[unsafe(export_name = "reallocarray")]
pub unsafe extern "C" fn mi_unix_reallocarray_export(
    p: *mut c_void,
    count: usize,
    size: usize,
) -> *mut c_void {
    unsafe { mi_unix_reallocarray(p, count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_reallocarr(ptrp: *mut *mut c_void, count: usize, size: usize) -> c_int {
    unsafe { crate::mimalloc::mi_reallocarr(ptrp, count, size) }
}

#[cfg(target_os = "linux")]
core::arch::global_asm!(
    ".weak reallocarr",
    ".type reallocarr,@function",
    ".set reallocarr, mi_reallocarr",
);

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_wasi_libc_malloc(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_malloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_os = "wasi", not(mi_c_malloc_fast)))]
#[unsafe(export_name = "__libc_malloc")]
pub unsafe extern "C" fn mi_wasi_libc_malloc_export(size: usize) -> *mut c_void {
    unsafe { mi_wasi_libc_malloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_wasi_libc_calloc(count: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_calloc(count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_os = "wasi", not(mi_c_malloc_fast)))]
#[unsafe(export_name = "__libc_calloc")]
pub unsafe extern "C" fn mi_wasi_libc_calloc_export(count: usize, size: usize) -> *mut c_void {
    unsafe { mi_wasi_libc_calloc(count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_wasi_libc_realloc(p: *mut c_void, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_realloc(p, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(target_os = "wasi")]
#[unsafe(export_name = "__libc_realloc")]
pub unsafe extern "C" fn mi_wasi_libc_realloc_export(p: *mut c_void, size: usize) -> *mut c_void {
    unsafe { mi_wasi_libc_realloc(p, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_wasi_libc_free(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_os = "wasi", not(mi_c_malloc_fast)))]
#[unsafe(export_name = "__libc_free")]
pub unsafe extern "C" fn mi_wasi_libc_free_export(p: *mut c_void) {
    unsafe { mi_wasi_libc_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_wasi_libc_memalign(alignment: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_memalign(alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(target_os = "wasi")]
#[unsafe(export_name = "__libc_memalign")]
pub unsafe extern "C" fn mi_wasi_libc_memalign_export(
    alignment: usize,
    size: usize,
) -> *mut c_void {
    unsafe { mi_wasi_libc_memalign(alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_linux_libc_malloc(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_malloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_os = "linux", not(mi_c_malloc_fast)))]
#[unsafe(export_name = "__libc_malloc")]
pub unsafe extern "C" fn mi_linux_libc_malloc_export(size: usize) -> *mut c_void {
    unsafe { mi_linux_libc_malloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_linux_libc_calloc(count: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_calloc(count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_os = "linux", not(mi_c_malloc_fast)))]
#[unsafe(export_name = "__libc_calloc")]
pub unsafe extern "C" fn mi_linux_libc_calloc_export(count: usize, size: usize) -> *mut c_void {
    unsafe { mi_linux_libc_calloc(count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_linux_libc_realloc(p: *mut c_void, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_realloc(p, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(target_os = "linux")]
#[unsafe(export_name = "__libc_realloc")]
pub unsafe extern "C" fn mi_linux_libc_realloc_export(p: *mut c_void, size: usize) -> *mut c_void {
    unsafe { mi_linux_libc_realloc(p, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_linux_libc_free(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_os = "linux", not(mi_c_malloc_fast)))]
#[unsafe(export_name = "__libc_free")]
pub unsafe extern "C" fn mi_linux_libc_free_export(p: *mut c_void) {
    unsafe { mi_linux_libc_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_linux_libc_cfree(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(target_os = "linux")]
#[unsafe(export_name = "__libc_cfree")]
pub unsafe extern "C" fn mi_linux_libc_cfree_export(p: *mut c_void) {
    unsafe { mi_linux_libc_cfree(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_linux_libc_valloc(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_valloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(target_os = "linux")]
#[unsafe(export_name = "__libc_valloc")]
pub unsafe extern "C" fn mi_linux_libc_valloc_export(size: usize) -> *mut c_void {
    unsafe { mi_linux_libc_valloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_linux_libc_pvalloc(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_pvalloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(target_os = "linux")]
#[unsafe(export_name = "__libc_pvalloc")]
pub unsafe extern "C" fn mi_linux_libc_pvalloc_export(size: usize) -> *mut c_void {
    unsafe { mi_linux_libc_pvalloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_linux_libc_memalign(alignment: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_memalign(alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(target_os = "linux")]
#[unsafe(export_name = "__libc_memalign")]
pub unsafe extern "C" fn mi_linux_libc_memalign_export(
    alignment: usize,
    size: usize,
) -> *mut c_void {
    unsafe { mi_linux_libc_memalign(alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_linux_posix_memalign(p: *mut *mut c_void, alignment: usize, size: usize) -> c_int {
    unsafe { crate::mimalloc::mi_posix_memalign(p, alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(target_os = "linux")]
#[unsafe(export_name = "__posix_memalign")]
pub unsafe extern "C" fn mi_linux_posix_memalign_export(
    p: *mut *mut c_void,
    alignment: usize,
    size: usize,
) -> c_int {
    unsafe { mi_linux_posix_memalign(p, alignment, size) }
}
