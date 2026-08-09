//! Mechanical translation of `src/alloc-override.c`.

// Retains upstream override/interposition shims for platforms and C++ ABI
// variants that are selected by target linker/runtime behavior.
#![allow(clippy::must_use_candidate)]
#![allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals
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

mod header_forwarding;
mod linux_libc;

pub use linux_libc::{
    mi_linux_libc_calloc, mi_linux_libc_cfree, mi_linux_libc_free, mi_linux_libc_malloc,
    mi_linux_libc_memalign, mi_linux_libc_pvalloc, mi_linux_libc_realloc, mi_linux_libc_valloc,
    mi_linux_posix_memalign,
};

#[cfg(all(target_family = "unix", not(mi_c_malloc_fast)))]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
mod unix_bootstrap {
    use crate::core_types::ptr_exposed_addr;
    use core::ffi::{c_int, c_void};
    use core::ptr;
    #[cfg(not(mi_sanitizer_runtime))]
    use core::sync::atomic::{AtomicUsize, Ordering};

    const BOOTSTRAP_PAGE_SIZE: usize = 4096;
    #[cfg(mi_sanitizer_runtime)]
    const BOOTSTRAP_SLOT_COUNT: usize = 65_536;
    #[cfg(not(mi_sanitizer_runtime))]
    const BOOTSTRAP_SLOT_COUNT: usize = 256;
    const BOOTSTRAP_MAGIC: usize = 0x6d69_626f;
    const MAP_FAILED: *mut c_void = usize::MAX as *mut c_void;

    #[cfg(any(target_os = "linux", target_os = "android"))]
    const MAP_ANONYMOUS: c_int = 0x20;
    #[cfg(any(
        target_os = "macos",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd"
    ))]
    const MAP_ANONYMOUS: c_int = 0x1000;
    #[cfg(not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "openbsd",
        target_os = "netbsd"
    )))]
    const MAP_ANONYMOUS: c_int = 0;

    const MAP_PRIVATE: c_int = 0x02;
    const PROT_READ: c_int = 0x01;
    const PROT_WRITE: c_int = 0x02;

    #[repr(C, align(16))]
    struct BootstrapHeader {
        magic: usize,
        map_size: usize,
        requested: usize,
        _padding: usize,
    }

    #[cfg(not(mi_sanitizer_runtime))]
    static BOOTSTRAP_PTRS: [AtomicUsize; BOOTSTRAP_SLOT_COUNT] =
        [const { AtomicUsize::new(0) }; BOOTSTRAP_SLOT_COUNT];
    #[cfg(mi_sanitizer_runtime)]
    static mut BOOTSTRAP_PTRS: [usize; BOOTSTRAP_SLOT_COUNT] = [0; BOOTSTRAP_SLOT_COUNT];

    // SAFETY[AUDIT:raw-macro-bootstrap-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe extern "C" {
        fn mmap(
            addr: *mut c_void,
            len: usize,
            prot: c_int,
            flags: c_int,
            fd: c_int,
            offset: i64,
        ) -> *mut c_void;
        fn munmap(addr: *mut c_void, len: usize) -> c_int;
    }

    #[inline]
    unsafe fn header_from_user(p: *mut c_void) -> *mut BootstrapHeader {
        // SAFETY[AUDIT:raw-macro-bootstrap-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
        unsafe {
            p.cast::<u8>()
                .sub(core::mem::size_of::<BootstrapHeader>())
                .cast::<BootstrapHeader>()
        }
    }

    #[cfg(mi_sanitizer_runtime)]
    fn register(p: *mut c_void) -> bool {
        let value = ptr_exposed_addr(p);
        let mut i = 0;
        // SAFETY[AUDIT:raw-macro-bootstrap-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
        unsafe {
            while i < BOOTSTRAP_SLOT_COUNT {
                if BOOTSTRAP_PTRS[i] == 0 {
                    BOOTSTRAP_PTRS[i] = value;
                    return true;
                }
                i += 1;
            }
        }
        false
    }

    #[cfg(not(mi_sanitizer_runtime))]
    fn register(p: *mut c_void) -> bool {
        let value = ptr_exposed_addr(p);
        let mut i = 0;
        while i < BOOTSTRAP_SLOT_COUNT {
            if BOOTSTRAP_PTRS[i]
                .compare_exchange(0, value, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return true;
            }
            i += 1;
        }
        false
    }

    #[cfg(mi_sanitizer_runtime)]
    pub(crate) fn contains(p: *const c_void) -> bool {
        if p.is_null() {
            return false;
        }
        let value = ptr_exposed_addr(p);
        let mut i = 0;
        // SAFETY[AUDIT:raw-macro-bootstrap-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
        unsafe {
            while i < BOOTSTRAP_SLOT_COUNT {
                if BOOTSTRAP_PTRS[i] == value {
                    return true;
                }
                i += 1;
            }
        }
        false
    }

    #[cfg(not(mi_sanitizer_runtime))]
    pub(crate) fn contains(p: *const c_void) -> bool {
        if p.is_null() {
            return false;
        }
        let value = ptr_exposed_addr(p);
        let mut i = 0;
        while i < BOOTSTRAP_SLOT_COUNT {
            if BOOTSTRAP_PTRS[i].load(Ordering::Acquire) == value {
                return true;
            }
            i += 1;
        }
        false
    }

    #[cfg(mi_sanitizer_runtime)]
    fn unregister(p: *mut c_void) -> bool {
        if p.is_null() {
            return false;
        }
        let value = ptr_exposed_addr(p);
        let mut i = 0;
        // SAFETY[AUDIT:raw-macro-bootstrap-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
        unsafe {
            while i < BOOTSTRAP_SLOT_COUNT {
                if BOOTSTRAP_PTRS[i] == value {
                    BOOTSTRAP_PTRS[i] = 0;
                    return true;
                }
                i += 1;
            }
        }
        false
    }

    #[cfg(not(mi_sanitizer_runtime))]
    fn unregister(p: *mut c_void) -> bool {
        if p.is_null() {
            return false;
        }
        let value = ptr_exposed_addr(p);
        let mut i = 0;
        while i < BOOTSTRAP_SLOT_COUNT {
            if BOOTSTRAP_PTRS[i]
                .compare_exchange(value, 0, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return true;
            }
            i += 1;
        }
        false
    }

    pub(crate) unsafe fn malloc(size: usize) -> *mut c_void {
        // SAFETY[AUDIT:raw-macro-bootstrap-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
        unsafe {
            let requested = if size == 0 { 1 } else { size };
            let header_size = core::mem::size_of::<BootstrapHeader>();
            if requested > usize::MAX - header_size - (BOOTSTRAP_PAGE_SIZE - 1) {
                return ptr::null_mut();
            }
            let total_size = header_size + requested;
            let map_size = (total_size + (BOOTSTRAP_PAGE_SIZE - 1)) & !(BOOTSTRAP_PAGE_SIZE - 1);
            let base = mmap(
                ptr::null_mut(),
                map_size,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS,
                -1,
                0,
            );
            if base.is_null() || base == MAP_FAILED {
                return ptr::null_mut();
            }

            let header = base.cast::<BootstrapHeader>();
            header.write(BootstrapHeader {
                magic: BOOTSTRAP_MAGIC,
                map_size,
                requested,
                _padding: 0,
            });
            let user = header.add(1).cast::<c_void>();
            if register(user) {
                user
            } else {
                let _ = munmap(base, map_size);
                ptr::null_mut()
            }
        }
    }

    pub(crate) unsafe fn calloc(count: usize, size: usize) -> *mut c_void {
        if count != 0 && size > usize::MAX / count {
            return ptr::null_mut();
        }
        let total = count * size;
        // SAFETY[AUDIT:raw-macro-bootstrap-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
        unsafe { malloc(total) }
    }

    pub(crate) unsafe fn usable_size(p: *const c_void) -> usize {
        if !contains(p) {
            return 0;
        }
        // SAFETY[AUDIT:raw-macro-bootstrap-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
        unsafe {
            let header = header_from_user(p.cast_mut());
            if (*header).magic == BOOTSTRAP_MAGIC {
                (*header).requested
            } else {
                0
            }
        }
    }

    pub(crate) unsafe fn free(p: *mut c_void) -> bool {
        if p.is_null() {
            return false;
        }
        if !unregister(p) {
            return false;
        }
        // SAFETY[AUDIT:raw-macro-bootstrap-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
        unsafe {
            let header = header_from_user(p);
            if (*header).magic != BOOTSTRAP_MAGIC {
                return true;
            }
            (*header).magic = 0;
            let map_size = (*header).map_size;
            let _ = munmap(header.cast::<c_void>(), map_size);
            true
        }
    }
}

#[cfg(all(mi_c_malloc_fast, mi_cxx_shim_linked))]
unsafe extern "C" {
    fn mi_cxx_new_slow(size: usize) -> *mut c_void;
}

#[cfg(mi_c_malloc_fast)]
#[inline(never)]
// SAFETY[AUDIT:raw-macro-fast-override-boundary]: the assembly entrypoint supplies the current thread heap, requested size, and throwing allocation mode.
unsafe extern "C" fn mi_cxx_new_fast_slow(
    theap: *mut crate::types::mi_theap_t,
    size: usize,
    nothrow: bool,
) -> *mut c_void {
    #[cfg(mi_cxx_shim_linked)]
    {
        let _ = (theap, nothrow);
        // SAFETY: the C++ slow helper implements the throwing operator-new retry contract.
        unsafe { mi_cxx_new_slow(size) }
    }
    #[cfg(not(mi_cxx_shim_linked))]
    {
        // SAFETY: the assembly caller supplies its live thread heap and allocation mode.
        unsafe { crate::alloc::mi_theap_try_new(theap, size, nothrow) }
    }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(mi_c_malloc_fast)]
// SAFETY[AUDIT:raw-macro-fast-override-boundary]: fast override assembly preserves C malloc/free and C++ new/delete ABI entrypoints.
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
        try_new = sym mi_cxx_new_fast_slow,
    );
}

#[cfg(mi_c_malloc_fast)]
// SAFETY[AUDIT:raw-macro-fast-override-boundary]: fast override assembly preserves C malloc/free and C++ new/delete ABI entrypoints.
core::arch::global_asm!(".globl _Znam", ".type _Znam,@function", ".set _Znam, _Znwm",);

// Keep the fast override entrypoints on the validated cache-line layout. The
// sized-delete stubs below remove an extra Rust call frame; this pad avoids
// regressing Redis while preserving the larson win from the stubs.
#[cfg(all(not(windows), mi_c_malloc_fast))]
// SAFETY[AUDIT:raw-macro-fast-override-boundary]: fast override assembly preserves C malloc/free and C++ new/delete ABI entrypoints.
core::arch::global_asm!(
    ".fill 0x30,1,0x90",
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
    ".globl _ZdlPvm",
    ".type _ZdlPvm,@function",
    ".p2align 4",
    "_ZdlPvm:",
    "endbr64",
    "jmp mi_free",
    ".size _ZdlPvm, .-_ZdlPvm",
    ".globl _ZdaPvm",
    ".type _ZdaPvm,@function",
    ".set _ZdaPvm, _ZdlPvm",
    ".globl _ZdlPvmSt11align_val_t",
    ".type _ZdlPvmSt11align_val_t,@function",
    ".p2align 4",
    "_ZdlPvmSt11align_val_t:",
    "endbr64",
    "jmp mi_free_size_aligned",
    ".size _ZdlPvmSt11align_val_t, .-_ZdlPvmSt11align_val_t",
    ".globl _ZdaPvmSt11align_val_t",
    ".type _ZdaPvmSt11align_val_t,@function",
    ".set _ZdaPvmSt11align_val_t, _ZdlPvmSt11align_val_t",
    ".fill 0x520,1,0x90",
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

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct mi_interpose_s {
    pub replacement: *const c_void,
    pub target: *const c_void,
}

// SAFETY[AUDIT:raw-macro-macos-interpose-boundary]: macOS interpose tables preserve replacement/target symbol pairs for libc and C++ allocation APIs.
unsafe impl Sync for mi_interpose_s {}

pub use header_forwarding::{
    MI_INTERPOSE_DECLS, mi_aligned_alloc_forward, mi_forward1, mi_forward02, mi_forward3,
    mi_interpose_fun, mi_malloc_good_size_forward, mi_malloc_size_forward, mi_vfree_forward,
};

#[cfg(all(target_os = "macos", mi_osx_interpose))]
macro_rules! MI_INTERPOSE_FUN {
    ($oldfun:path, $newfun:path) => {
        crate::alloc_override::mi_interpose_s {
            replacement: ($newfun as *const ()) as *const core::ffi::c_void,
            target: ($oldfun as *const ()) as *const core::ffi::c_void,
        }
    };
}

// SAFETY[AUDIT:raw-macro-macos-interpose-boundary]: macOS interpose tables preserve replacement/target symbol pairs for libc and C++ allocation APIs.
unsafe extern "C" {
    pub fn vfree(p: *mut c_void);
    pub fn malloc_size(p: *const c_void) -> usize;
    pub fn malloc_good_size(size: usize) -> usize;
}

#[cfg(all(target_os = "macos", mi_osx_interpose))]
// SAFETY[AUDIT:raw-macro-macos-interpose-boundary]: macOS interpose tables preserve replacement/target symbol pairs for libc and C++ allocation APIs.
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

#[cfg(all(target_os = "macos", mi_osx_interpose))]
#[used]
// SAFETY[AUDIT:raw-macro-macos-interpose-boundary]: macOS interpose tables preserve replacement/target symbol pairs for libc and C++ allocation APIs.
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

#[cfg(all(target_os = "macos", mi_osx_interpose))]
pub static _mi_interposes_10_7: [mi_interpose_s; 1] =
    [MI_INTERPOSE_FUN!(strndup, crate::mimalloc::mi_strndup)];

#[cfg(all(target_os = "macos", mi_osx_interpose))]
pub static _mi_interposes_10_15: [mi_interpose_s; 1] = [MI_INTERPOSE_FUN!(
    aligned_alloc,
    crate::mimalloc::mi_aligned_alloc
)];

#[cfg(all(target_os = "macos", mi_osx_interpose))]
#[used]
// SAFETY[AUDIT:raw-macro-macos-interpose-boundary]: macOS interpose tables preserve replacement/target symbol pairs for libc and C++ allocation APIs.
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
#[cfg_attr(not(feature = "c-abi"), unsafe(no_mangle))]
// SAFETY[AUDIT:raw-macro-allocation-query-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_malloc_size_checked(p: *mut c_void) -> usize {
    if !unsafe { crate::mimalloc::mi_is_in_heap_region(p.cast_const()) } {
        return 0;
    }
    // SAFETY[AUDIT:raw-macro-allocation-query-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_usable_size(p.cast_const()) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-crt-boundary]: Windows CRT allocation exports forward raw ABI calls to mimalloc ownership-preserving entrypoints.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _expand(block: *mut c_void, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_expand(block, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-crt-boundary]: Windows CRT allocation exports forward raw ABI calls to mimalloc ownership-preserving entrypoints.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _msize_base(block: *mut c_void) -> usize {
    unsafe { crate::mimalloc::mi_malloc_size(block) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-crt-boundary]: Windows CRT allocation exports forward raw ABI calls to mimalloc ownership-preserving entrypoints.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _msize(block: *mut c_void) -> usize {
    unsafe { crate::mimalloc::mi_malloc_size(block) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-crt-boundary]: Windows CRT allocation exports forward raw ABI calls to mimalloc ownership-preserving entrypoints.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _free_base(block: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(block) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-crt-boundary]: Windows CRT allocation exports forward raw ABI calls to mimalloc ownership-preserving entrypoints.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn free(block: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(block) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-crt-boundary]: Windows CRT allocation exports forward raw ABI calls to mimalloc ownership-preserving entrypoints.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_malloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-crt-boundary]: Windows CRT allocation exports forward raw ABI calls to mimalloc ownership-preserving entrypoints.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _malloc_base(size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_malloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-crt-boundary]: Windows CRT allocation exports forward raw ABI calls to mimalloc ownership-preserving entrypoints.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _realloc_base(block: *mut c_void, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_realloc(block, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-crt-boundary]: Windows CRT allocation exports forward raw ABI calls to mimalloc ownership-preserving entrypoints.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn realloc(block: *mut c_void, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_realloc(block, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-crt-boundary]: Windows CRT allocation exports forward raw ABI calls to mimalloc ownership-preserving entrypoints.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _calloc_base(count: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_calloc(count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-crt-boundary]: Windows CRT allocation exports forward raw ABI calls to mimalloc ownership-preserving entrypoints.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_calloc(count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-crt-boundary]: Windows CRT allocation exports forward raw ABI calls to mimalloc ownership-preserving entrypoints.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _recalloc_base(
    block: *mut c_void,
    count: usize,
    size: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-windows-crt-boundary]: Windows CRT allocation exports forward raw ABI calls to mimalloc ownership-preserving entrypoints.
    unsafe { crate::mimalloc::mi_recalloc(block, count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-crt-boundary]: Windows CRT allocation exports forward raw ABI calls to mimalloc ownership-preserving entrypoints.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _recalloc(block: *mut c_void, count: usize, size: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_recalloc(block, count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-aligned-boundary]: Windows aligned CRT exports preserve alignment, offset, and recalloc forwarding contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_free(block: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(block) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-aligned-boundary]: Windows aligned CRT exports preserve alignment, offset, and recalloc forwarding contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_malloc(size: usize, alignment: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_malloc_aligned(size, alignment) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-aligned-boundary]: Windows aligned CRT exports preserve alignment, offset, and recalloc forwarding contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_offset_malloc(
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-windows-aligned-boundary]: Windows aligned CRT exports preserve alignment, offset, and recalloc forwarding contracts.
    unsafe { crate::mimalloc::mi_malloc_aligned_at(size, alignment, offset) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-aligned-boundary]: Windows aligned CRT exports preserve alignment, offset, and recalloc forwarding contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_msize(
    block: *mut c_void,
    alignment: usize,
    offset: usize,
) -> usize {
    crate::internal::MI_UNUSED(&alignment);
    crate::internal::MI_UNUSED(&offset);
    // SAFETY[AUDIT:raw-macro-windows-aligned-boundary]: Windows aligned CRT exports preserve alignment, offset, and recalloc forwarding contracts.
    unsafe { crate::mimalloc::mi_malloc_size(block) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-aligned-boundary]: Windows aligned CRT exports preserve alignment, offset, and recalloc forwarding contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_offset_realloc(
    block: *mut c_void,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-windows-aligned-boundary]: Windows aligned CRT exports preserve alignment, offset, and recalloc forwarding contracts.
    unsafe { crate::mimalloc::mi_realloc_aligned_at(block, size, alignment, offset) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-aligned-boundary]: Windows aligned CRT exports preserve alignment, offset, and recalloc forwarding contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_offset_recalloc(
    block: *mut c_void,
    count: usize,
    size: usize,
    alignment: usize,
    offset: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-windows-aligned-boundary]: Windows aligned CRT exports preserve alignment, offset, and recalloc forwarding contracts.
    unsafe { crate::mimalloc::mi_recalloc_aligned_at(block, count, size, alignment, offset) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-aligned-boundary]: Windows aligned CRT exports preserve alignment, offset, and recalloc forwarding contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_realloc(
    block: *mut c_void,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-windows-aligned-boundary]: Windows aligned CRT exports preserve alignment, offset, and recalloc forwarding contracts.
    unsafe { crate::mimalloc::mi_realloc_aligned(block, size, alignment) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(windows)]
// SAFETY[AUDIT:raw-macro-windows-aligned-boundary]: Windows aligned CRT exports preserve alignment, offset, and recalloc forwarding contracts.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _aligned_recalloc(
    block: *mut c_void,
    count: usize,
    size: usize,
    alignment: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-windows-aligned-boundary]: Windows aligned CRT exports preserve alignment, offset, and recalloc forwarding contracts.
    unsafe { crate::mimalloc::mi_recalloc_aligned(block, count, size, alignment) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_c_malloc_fast)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "cxx-overrides"),
    unsafe(export_name = "_ZdlPv")
)]
// SAFETY[AUDIT:raw-macro-cxx-delete-boundary]: scalar C++ delete shims preserve Itanium delete symbols while forwarding to the matching free path.
pub unsafe extern "C" fn mi_cxx_delete(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_c_malloc_fast)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "cxx-overrides"),
    unsafe(export_name = "_ZdaPv")
)]
// SAFETY[AUDIT:raw-macro-cxx-array-delete-boundary]: array C++ delete shims preserve Itanium array-delete symbols while forwarding to the matching free path.
pub unsafe extern "C" fn mi_cxx_delete_array(p: *mut c_void) {
    unsafe { crate::mimalloc::mi_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_c_malloc_fast)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "cxx-overrides"),
    unsafe(export_name = "_ZdlPvm")
)]
// SAFETY[AUDIT:raw-macro-cxx-delete-boundary]: scalar C++ delete shims preserve Itanium delete symbols while forwarding to the matching free path.
pub unsafe extern "C" fn mi_cxx_delete_sized(p: *mut c_void, n: usize) {
    unsafe { crate::mimalloc::mi_free_size(p, n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_c_malloc_fast)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "cxx-overrides"),
    unsafe(export_name = "_ZdaPvm")
)]
// SAFETY[AUDIT:raw-macro-cxx-array-delete-boundary]: array C++ delete shims preserve Itanium array-delete symbols while forwarding to the matching free path.
pub unsafe extern "C" fn mi_cxx_delete_array_sized(p: *mut c_void, n: usize) {
    unsafe { crate::mimalloc::mi_free_size(p, n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "cxx-overrides"),
    unsafe(export_name = "_ZdlPvSt11align_val_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-aligned-delete-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
pub unsafe extern "C" fn mi_cxx_delete_aligned(p: *mut c_void, al: usize) {
    unsafe { crate::mimalloc::mi_free_aligned(p, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "cxx-overrides"),
    unsafe(export_name = "_ZdaPvSt11align_val_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-aligned-delete-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
pub unsafe extern "C" fn mi_cxx_delete_array_aligned(p: *mut c_void, al: usize) {
    unsafe { crate::mimalloc::mi_free_aligned(p, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_c_malloc_fast)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "cxx-overrides"),
    unsafe(export_name = "_ZdlPvmSt11align_val_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-aligned-delete-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
pub unsafe extern "C" fn mi_cxx_delete_sized_aligned(p: *mut c_void, n: usize, al: usize) {
    unsafe { crate::mimalloc::mi_free_size_aligned(p, n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_c_malloc_fast)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "cxx-overrides"),
    unsafe(export_name = "_ZdaPvmSt11align_val_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-aligned-delete-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
pub unsafe extern "C" fn mi_cxx_delete_array_sized_aligned(p: *mut c_void, n: usize, al: usize) {
    unsafe { crate::mimalloc::mi_free_size_aligned(p, n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "cxx-overrides"),
    unsafe(export_name = "_ZdlPvRKSt9nothrow_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-nothrow-boundary]: nothrow C++ shims consume the tag ABI and forward through null-returning allocation paths.
pub unsafe extern "C" fn mi_cxx_delete_nothrow(p: *mut c_void, tag: mi_nothrow_t) {
    crate::internal::MI_UNUSED(&tag);
    unsafe { crate::mimalloc::mi_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "cxx-overrides"),
    unsafe(export_name = "_ZdaPvRKSt9nothrow_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-nothrow-boundary]: nothrow C++ shims consume the tag ABI and forward through null-returning allocation paths.
pub unsafe extern "C" fn mi_cxx_delete_array_nothrow(p: *mut c_void, tag: mi_nothrow_t) {
    crate::internal::MI_UNUSED(&tag);
    unsafe { crate::mimalloc::mi_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "cxx-overrides"),
    unsafe(export_name = "_ZdlPvSt11align_val_tRKSt9nothrow_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-aligned-delete-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
pub unsafe extern "C" fn mi_cxx_delete_aligned_nothrow(
    p: *mut c_void,
    al: usize,
    tag: mi_nothrow_t,
) {
    crate::internal::MI_UNUSED(&tag);
    // SAFETY[AUDIT:raw-macro-cxx-aligned-delete-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
    unsafe { crate::mimalloc::mi_free_aligned(p, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(not(windows))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "cxx-overrides"),
    unsafe(export_name = "_ZdaPvSt11align_val_tRKSt9nothrow_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-aligned-delete-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
pub unsafe extern "C" fn mi_cxx_delete_array_aligned_nothrow(
    p: *mut c_void,
    al: usize,
    tag: mi_nothrow_t,
) {
    crate::internal::MI_UNUSED(&tag);
    // SAFETY[AUDIT:raw-macro-cxx-aligned-delete-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
    unsafe { crate::mimalloc::mi_free_aligned(p, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[cfg_attr(
    all(
        not(windows),
        not(mi_c_malloc_fast),
        not(mi_cxx_shim_linked),
        any(not(feature = "c-abi"), feature = "cxx-overrides")
    ),
    unsafe(export_name = "_Znwm")
)]
// SAFETY[AUDIT:raw-macro-cxx-new-boundary]: C++ new shims preserve pointer-width Itanium new symbols while forwarding to mi_new.
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
    all(
        not(windows),
        not(mi_c_malloc_fast),
        not(mi_cxx_shim_linked),
        any(not(feature = "c-abi"), feature = "cxx-overrides")
    ),
    unsafe(export_name = "_Znam")
)]
// SAFETY[AUDIT:raw-macro-cxx-new-boundary]: C++ new shims preserve pointer-width Itanium new symbols while forwarding to mi_new.
pub unsafe extern "C" fn mi_cxx_new_array_64(n: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new(n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[cfg_attr(
    all(
        not(windows),
        not(mi_cxx_shim_linked),
        any(not(feature = "c-abi"), feature = "cxx-overrides")
    ),
    unsafe(export_name = "_ZnwmRKSt9nothrow_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-nothrow-boundary]: nothrow C++ shims consume the tag ABI and forward through null-returning allocation paths.
pub unsafe extern "C" fn mi_cxx_new_64_nothrow(n: usize, tag: mi_nothrow_t) -> *mut c_void {
    crate::internal::MI_UNUSED(&tag);
    unsafe { crate::mimalloc::mi_new_nothrow(n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[cfg_attr(
    all(
        not(windows),
        not(mi_cxx_shim_linked),
        any(not(feature = "c-abi"), feature = "cxx-overrides")
    ),
    unsafe(export_name = "_ZnamRKSt9nothrow_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-nothrow-boundary]: nothrow C++ shims consume the tag ABI and forward through null-returning allocation paths.
pub unsafe extern "C" fn mi_cxx_new_array_64_nothrow(n: usize, tag: mi_nothrow_t) -> *mut c_void {
    crate::internal::MI_UNUSED(&tag);
    unsafe { crate::mimalloc::mi_new_nothrow(n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[cfg_attr(
    all(
        not(windows),
        not(mi_cxx_shim_linked),
        any(not(feature = "c-abi"), feature = "cxx-overrides")
    ),
    unsafe(export_name = "_ZnwmSt11align_val_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-aligned-new64-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
pub unsafe extern "C" fn mi_cxx_new_64_aligned(n: usize, al: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[cfg_attr(
    all(
        not(windows),
        not(mi_cxx_shim_linked),
        any(not(feature = "c-abi"), feature = "cxx-overrides")
    ),
    unsafe(export_name = "_ZnamSt11align_val_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-aligned-new64-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
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
    all(
        not(windows),
        not(mi_cxx_shim_linked),
        any(not(feature = "c-abi"), feature = "cxx-overrides")
    ),
    unsafe(export_name = "_ZnwmSt11align_val_tRKSt9nothrow_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-aligned-new64-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
pub unsafe extern "C" fn mi_cxx_new_64_aligned_nothrow(
    n: usize,
    al: usize,
    tag: mi_nothrow_t,
) -> *mut c_void {
    crate::internal::MI_UNUSED(&tag);
    // SAFETY[AUDIT:raw-macro-cxx-aligned-new64-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
    unsafe { crate::mimalloc::mi_new_aligned_nothrow(n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(any(
    target_pointer_width = "64",
    all(target_pointer_width = "32", target_os = "emscripten")
))]
#[cfg_attr(
    all(
        not(windows),
        not(mi_cxx_shim_linked),
        any(not(feature = "c-abi"), feature = "cxx-overrides")
    ),
    unsafe(export_name = "_ZnamSt11align_val_tRKSt9nothrow_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-aligned-new64-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
pub unsafe extern "C" fn mi_cxx_new_array_64_aligned_nothrow(
    n: usize,
    al: usize,
    tag: mi_nothrow_t,
) -> *mut c_void {
    crate::internal::MI_UNUSED(&tag);
    // SAFETY[AUDIT:raw-macro-cxx-aligned-new64-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
    unsafe { crate::mimalloc::mi_new_aligned_nothrow(n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(
    all(
        not(windows),
        not(mi_cxx_shim_linked),
        any(not(feature = "c-abi"), feature = "cxx-overrides")
    ),
    unsafe(export_name = "_Znwj")
)]
// SAFETY[AUDIT:raw-macro-cxx-new-boundary]: C++ new shims preserve pointer-width Itanium new symbols while forwarding to mi_new.
pub unsafe extern "C" fn mi_cxx_new_32(n: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new(n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(
    all(
        not(windows),
        not(mi_cxx_shim_linked),
        any(not(feature = "c-abi"), feature = "cxx-overrides")
    ),
    unsafe(export_name = "_Znaj")
)]
// SAFETY[AUDIT:raw-macro-cxx-new-boundary]: C++ new shims preserve pointer-width Itanium new symbols while forwarding to mi_new.
pub unsafe extern "C" fn mi_cxx_new_array_32(n: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new(n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(
    all(
        not(windows),
        not(mi_cxx_shim_linked),
        any(not(feature = "c-abi"), feature = "cxx-overrides")
    ),
    unsafe(export_name = "_ZnwjRKSt9nothrow_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-nothrow-boundary]: nothrow C++ shims consume the tag ABI and forward through null-returning allocation paths.
pub unsafe extern "C" fn mi_cxx_new_32_nothrow(n: usize, tag: mi_nothrow_t) -> *mut c_void {
    crate::internal::MI_UNUSED(&tag);
    unsafe { crate::mimalloc::mi_new_nothrow(n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(
    all(
        not(windows),
        not(mi_cxx_shim_linked),
        any(not(feature = "c-abi"), feature = "cxx-overrides")
    ),
    unsafe(export_name = "_ZnajRKSt9nothrow_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-nothrow-boundary]: nothrow C++ shims consume the tag ABI and forward through null-returning allocation paths.
pub unsafe extern "C" fn mi_cxx_new_array_32_nothrow(n: usize, tag: mi_nothrow_t) -> *mut c_void {
    crate::internal::MI_UNUSED(&tag);
    unsafe { crate::mimalloc::mi_new_nothrow(n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(
    all(
        not(windows),
        not(mi_cxx_shim_linked),
        any(not(feature = "c-abi"), feature = "cxx-overrides")
    ),
    unsafe(export_name = "_ZnwjSt11align_val_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-aligned-new32-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
pub unsafe extern "C" fn mi_cxx_new_32_aligned(n: usize, al: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(
    all(
        not(windows),
        not(mi_cxx_shim_linked),
        any(not(feature = "c-abi"), feature = "cxx-overrides")
    ),
    unsafe(export_name = "_ZnajSt11align_val_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-aligned-new32-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
pub unsafe extern "C" fn mi_cxx_new_array_32_aligned(n: usize, al: usize) -> *mut c_void {
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(
    all(
        not(windows),
        not(mi_cxx_shim_linked),
        any(not(feature = "c-abi"), feature = "cxx-overrides")
    ),
    unsafe(export_name = "_ZnwjSt11align_val_tRKSt9nothrow_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-aligned-new32-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
pub unsafe extern "C" fn mi_cxx_new_32_aligned_nothrow(
    n: usize,
    al: usize,
    tag: mi_nothrow_t,
) -> *mut c_void {
    crate::internal::MI_UNUSED(&tag);
    // SAFETY[AUDIT:raw-macro-cxx-aligned-new32-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
    unsafe { crate::mimalloc::mi_new_aligned_nothrow(n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_pointer_width = "32", not(target_os = "emscripten")))]
#[cfg_attr(
    all(
        not(windows),
        not(mi_cxx_shim_linked),
        any(not(feature = "c-abi"), feature = "cxx-overrides")
    ),
    unsafe(export_name = "_ZnajSt11align_val_tRKSt9nothrow_t")
)]
// SAFETY[AUDIT:raw-macro-cxx-aligned-new32-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
pub unsafe extern "C" fn mi_cxx_new_array_32_aligned_nothrow(
    n: usize,
    al: usize,
    tag: mi_nothrow_t,
) -> *mut c_void {
    crate::internal::MI_UNUSED(&tag);
    // SAFETY[AUDIT:raw-macro-cxx-aligned-new32-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
    unsafe { crate::mimalloc::mi_new_aligned_nothrow(n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_new_array_nothrow(n: usize, tag: mi_nothrow_t) -> *mut c_void {
    crate::internal::MI_UNUSED(&tag);
    // SAFETY[AUDIT:raw-macro-cxx-nothrow-boundary]: nothrow C++ shims consume the tag ABI and forward through null-returning allocation paths.
    unsafe { crate::mimalloc::mi_new_nothrow(n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_delete_sized(p: *mut c_void, n: usize) {
    // SAFETY[AUDIT:raw-macro-cxx-delete-boundary]: scalar C++ delete shims preserve Itanium delete symbols while forwarding to the matching free path.
    unsafe { crate::mimalloc::mi_free_size(p, n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_delete_array_sized(p: *mut c_void, n: usize) {
    // SAFETY[AUDIT:raw-macro-cxx-array-delete-boundary]: array C++ delete shims preserve Itanium array-delete symbols while forwarding to the matching free path.
    unsafe { crate::mimalloc::mi_free_size(p, n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_delete_array_aligned(p: *mut c_void, al: usize) {
    // SAFETY[AUDIT:raw-macro-cxx-aligned-operator-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
    unsafe { crate::mimalloc::mi_free_aligned(p, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_delete_sized_aligned(p: *mut c_void, n: usize, al: usize) {
    // SAFETY[AUDIT:raw-macro-cxx-aligned-operator-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
    unsafe { crate::mimalloc::mi_free_size_aligned(p, n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_delete_array_sized_aligned(p: *mut c_void, n: usize, al: usize) {
    // SAFETY[AUDIT:raw-macro-cxx-aligned-operator-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
    unsafe { crate::mimalloc::mi_free_size_aligned(p, n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_delete_aligned_nothrow(p: *mut c_void, al: usize, tag: mi_nothrow_t) {
    crate::internal::MI_UNUSED(&tag);
    // SAFETY[AUDIT:raw-macro-cxx-aligned-operator-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
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
    crate::internal::MI_UNUSED(&tag);
    // SAFETY[AUDIT:raw-macro-cxx-aligned-operator-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
    unsafe { crate::mimalloc::mi_free_aligned(p, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_new_aligned(n: usize, al: usize) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-cxx-aligned-operator-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
    unsafe { crate::mimalloc::mi_new_aligned(n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_operator_new_array_aligned(n: usize, al: usize) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-cxx-aligned-operator-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
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
    crate::internal::MI_UNUSED(&tag);
    // SAFETY[AUDIT:raw-macro-cxx-aligned-operator-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
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
    crate::internal::MI_UNUSED(&tag);
    // SAFETY[AUDIT:raw-macro-cxx-aligned-operator-boundary]: aligned C++ shims preserve align_val_t ABI variants while forwarding to aligned allocator exports.
    unsafe { crate::mimalloc::mi_new_aligned_nothrow(n, al) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
pub unsafe fn mi_unix_malloc(size: usize) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-bootstrap-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe {
        #[cfg(all(mi_sanitizer_runtime, target_family = "unix", not(mi_c_malloc_fast)))]
        {
            unix_bootstrap::malloc(size)
        }
        #[cfg(not(all(mi_sanitizer_runtime, target_family = "unix", not(mi_c_malloc_fast))))]
        {
            #[cfg(all(target_family = "unix", not(mi_c_malloc_fast)))]
            if crate::internal::_mi_preloading() {
                return unix_bootstrap::malloc(size);
            }
            crate::mimalloc::mi_malloc(size)
        }
    }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(
    not(windows),
    not(mi_c_malloc_fast),
    not(mi_sanitizer_runtime),
    not(miri)
))]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "malloc")
)]
// SAFETY[AUDIT:raw-macro-unix-core-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_unix_malloc_export(size: usize) -> *mut c_void {
    unsafe { mi_unix_malloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
pub unsafe fn mi_unix_calloc(count: usize, size: usize) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-bootstrap-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe {
        #[cfg(all(mi_sanitizer_runtime, target_family = "unix", not(mi_c_malloc_fast)))]
        {
            unix_bootstrap::calloc(count, size)
        }
        #[cfg(not(all(mi_sanitizer_runtime, target_family = "unix", not(mi_c_malloc_fast))))]
        {
            #[cfg(all(target_family = "unix", not(mi_c_malloc_fast)))]
            if crate::internal::_mi_preloading() {
                return unix_bootstrap::calloc(count, size);
            }
            crate::mimalloc::mi_calloc(count, size)
        }
    }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(
    not(windows),
    not(mi_c_malloc_fast),
    not(mi_sanitizer_runtime),
    not(miri)
))]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "calloc")
)]
// SAFETY[AUDIT:raw-macro-unix-core-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_unix_calloc_export(count: usize, size: usize) -> *mut c_void {
    unsafe { mi_unix_calloc(count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
pub unsafe fn mi_unix_realloc(p: *mut c_void, newsize: usize) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-bootstrap-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe {
        if p.is_null() {
            return mi_unix_malloc(newsize);
        }
        #[cfg(all(target_family = "unix", not(mi_c_malloc_fast)))]
        if unix_bootstrap::contains(p) {
            let oldsize = unix_bootstrap::usable_size(p);
            if newsize == 0 {
                let _ = unix_bootstrap::free(p);
                return core::ptr::null_mut();
            }
            let newp = if crate::internal::_mi_preloading() {
                unix_bootstrap::malloc(newsize)
            } else {
                crate::mimalloc::mi_malloc(newsize)
            };
            if !newp.is_null() {
                core::ptr::copy_nonoverlapping(
                    p.cast::<u8>(),
                    newp.cast::<u8>(),
                    core::cmp::min(oldsize, newsize),
                );
                let _ = unix_bootstrap::free(p);
            }
            return newp;
        }
        crate::mimalloc::mi_realloc(p, newsize)
    }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "realloc")
)]
// SAFETY[AUDIT:raw-macro-unix-core-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_unix_realloc_export(p: *mut c_void, newsize: usize) -> *mut c_void {
    unsafe { mi_unix_realloc(p, newsize) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
pub unsafe fn mi_unix_free(p: *mut c_void) {
    // SAFETY[AUDIT:raw-macro-bootstrap-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe {
        if p.is_null() {
            return;
        }
        #[cfg(all(mi_sanitizer_runtime, target_family = "unix", not(mi_c_malloc_fast)))]
        {
            let _ = unix_bootstrap::free(p);
            return;
        }
        #[cfg(all(
            not(mi_sanitizer_runtime),
            target_family = "unix",
            not(mi_c_malloc_fast)
        ))]
        if !unix_bootstrap::free(p) {
            crate::mimalloc::mi_free(p);
        }
        #[cfg(not(all(target_family = "unix", not(mi_c_malloc_fast))))]
        {
            crate::mimalloc::mi_free(p);
        }
    }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(
    not(windows),
    not(mi_c_malloc_fast),
    not(mi_sanitizer_runtime),
    not(miri)
))]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "free")
)]
// SAFETY[AUDIT:raw-macro-unix-core-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_unix_free_export(p: *mut c_void) {
    unsafe { mi_unix_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_strdup(s: *const c_char) -> *mut c_char {
    // SAFETY[AUDIT:raw-macro-unix-string-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_strdup(s) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(miri)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "strdup")
)]
// SAFETY[AUDIT:raw-macro-unix-string-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_unix_strdup_export(s: *const c_char) -> *mut c_char {
    unsafe { mi_unix_strdup(s) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_strndup(s: *const c_char, n: usize) -> *mut c_char {
    // SAFETY[AUDIT:raw-macro-unix-string-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_strndup(s, n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(miri)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "strndup")
)]
// SAFETY[AUDIT:raw-macro-unix-string-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_unix_strndup_export(s: *const c_char, n: usize) -> *mut c_char {
    unsafe { mi_unix_strndup(s, n) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
pub unsafe fn mi_unix_reallocf(p: *mut c_void, newsize: usize) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-unix-realloc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe {
        let newp = mi_unix_realloc(p, newsize);
        if newp.is_null() && !p.is_null() {
            mi_unix_free(p);
        }
        newp
    }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "reallocf")
)]
// SAFETY[AUDIT:raw-macro-unix-realloc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_unix_reallocf_export(p: *mut c_void, newsize: usize) -> *mut c_void {
    unsafe { mi_unix_reallocf(p, newsize) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
pub unsafe fn mi_unix_malloc_size(p: *const c_void) -> usize {
    // SAFETY[AUDIT:raw-macro-allocation-query-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe {
        #[cfg(all(target_family = "unix", not(mi_c_malloc_fast)))]
        if unix_bootstrap::contains(p) {
            return unix_bootstrap::usable_size(p);
        }
        crate::mimalloc::mi_usable_size(p)
    }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "malloc_size")
)]
// SAFETY[AUDIT:raw-macro-allocation-query-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_unix_malloc_size_export(p: *const c_void) -> usize {
    unsafe { mi_unix_malloc_size(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
pub unsafe fn mi_unix_malloc_usable_size(p: *const c_void) -> usize {
    // SAFETY[AUDIT:raw-macro-allocation-query-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { mi_unix_malloc_size(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_malloc_good_size(size: usize) -> usize {
    // SAFETY[AUDIT:raw-macro-allocation-query-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_malloc_good_size(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "malloc_good_size")
)]
// SAFETY[AUDIT:raw-macro-allocation-query-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_unix_malloc_good_size_export(size: usize) -> usize {
    unsafe { mi_unix_malloc_good_size(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "malloc_usable_size")
)]
// SAFETY[AUDIT:raw-macro-allocation-query-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_unix_malloc_usable_size_export(p: *const c_void) -> usize {
    unsafe { mi_unix_malloc_usable_size(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_valloc(size: usize) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-unix-page-aligned-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_valloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "valloc")
)]
// SAFETY[AUDIT:raw-macro-unix-page-aligned-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_unix_valloc_export(size: usize) -> *mut c_void {
    unsafe { mi_unix_valloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_vfree(p: *mut c_void) {
    // SAFETY[AUDIT:raw-macro-unix-page-aligned-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { mi_unix_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "vfree")
)]
// SAFETY[AUDIT:raw-macro-unix-page-aligned-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_unix_vfree_export(p: *mut c_void) {
    unsafe { mi_unix_vfree(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_posix_memalign(p: *mut *mut c_void, alignment: usize, size: usize) -> c_int {
    // SAFETY[AUDIT:raw-macro-unix-aligned-posix-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_posix_memalign(p, alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "posix_memalign")
)]
// SAFETY[AUDIT:raw-macro-unix-aligned-posix-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_unix_posix_memalign_export(
    p: *mut *mut c_void,
    alignment: usize,
    size: usize,
) -> c_int {
    // SAFETY[AUDIT:raw-macro-unix-aligned-posix-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { mi_unix_posix_memalign(p, alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_aligned_alloc(alignment: usize, size: usize) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-unix-aligned-posix-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_aligned_alloc(alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "aligned_alloc")
)]
// SAFETY[AUDIT:raw-macro-unix-aligned-posix-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_unix_aligned_alloc_export(
    alignment: usize,
    size: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-unix-aligned-posix-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { mi_unix_aligned_alloc(alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_cfree(p: *mut c_void) {
    // SAFETY[AUDIT:raw-macro-unix-cfree-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { mi_unix_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "cfree")
)]
// SAFETY[AUDIT:raw-macro-unix-cfree-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_unix_cfree_export(p: *mut c_void) {
    unsafe { mi_unix_cfree(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_pvalloc(size: usize) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-unix-page-aligned-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_pvalloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "pvalloc")
)]
// SAFETY[AUDIT:raw-macro-unix-page-aligned-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_unix_pvalloc_export(size: usize) -> *mut c_void {
    unsafe { mi_unix_pvalloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_memalign(alignment: usize, size: usize) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-unix-aligned-posix-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_memalign(alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "memalign")
)]
// SAFETY[AUDIT:raw-macro-unix-aligned-posix-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_unix_memalign_export(alignment: usize, size: usize) -> *mut c_void {
    unsafe { mi_unix_memalign(alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_aligned_malloc(size: usize, alignment: usize) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-unix-aligned-posix-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_malloc_aligned(size, alignment) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "_aligned_malloc")
)]
// SAFETY[AUDIT:raw-macro-unix-aligned-posix-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_unix_aligned_malloc_export(
    size: usize,
    alignment: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-unix-aligned-posix-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { mi_unix_aligned_malloc(size, alignment) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_reallocarray(p: *mut c_void, count: usize, size: usize) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-unix-product-realloc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe {
        match count.checked_mul(size) {
            Some(total) => mi_unix_realloc(p, total),
            None => core::ptr::null_mut(),
        }
    }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(not(windows), not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "reallocarray")
)]
// SAFETY[AUDIT:raw-macro-unix-product-realloc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_unix_reallocarray_export(
    p: *mut c_void,
    count: usize,
    size: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-unix-product-realloc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { mi_unix_reallocarray(p, count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_unix_reallocarr(ptrp: *mut *mut c_void, count: usize, size: usize) -> c_int {
    // SAFETY[AUDIT:raw-macro-unix-product-realloc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_reallocarr(ptrp, count, size) }
}

#[cfg(all(target_os = "linux", not(mi_sanitizer_runtime), not(miri)))]
// SAFETY[AUDIT:raw-macro-unix-product-realloc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
core::arch::global_asm!(
    ".weak reallocarr",
    ".type reallocarr,@function",
    ".set reallocarr, mi_reallocarr",
);

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_wasi_libc_malloc(size: usize) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-wasi-libc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_malloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_os = "wasi", not(mi_c_malloc_fast)))]
// SAFETY[AUDIT:raw-macro-wasi-libc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
#[unsafe(export_name = "__libc_malloc")]
pub unsafe extern "C" fn mi_wasi_libc_malloc_export(size: usize) -> *mut c_void {
    unsafe { mi_wasi_libc_malloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_wasi_libc_calloc(count: usize, size: usize) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-wasi-libc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_calloc(count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_os = "wasi", not(mi_c_malloc_fast)))]
// SAFETY[AUDIT:raw-macro-wasi-libc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
#[unsafe(export_name = "__libc_calloc")]
pub unsafe extern "C" fn mi_wasi_libc_calloc_export(count: usize, size: usize) -> *mut c_void {
    unsafe { mi_wasi_libc_calloc(count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_wasi_libc_realloc(p: *mut c_void, size: usize) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-wasi-libc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_realloc(p, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(target_os = "wasi")]
// SAFETY[AUDIT:raw-macro-wasi-libc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
#[unsafe(export_name = "__libc_realloc")]
pub unsafe extern "C" fn mi_wasi_libc_realloc_export(p: *mut c_void, size: usize) -> *mut c_void {
    unsafe { mi_wasi_libc_realloc(p, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_wasi_libc_free(p: *mut c_void) {
    // SAFETY[AUDIT:raw-macro-wasi-libc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_os = "wasi", not(mi_c_malloc_fast)))]
// SAFETY[AUDIT:raw-macro-wasi-libc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
#[unsafe(export_name = "__libc_free")]
pub unsafe extern "C" fn mi_wasi_libc_free_export(p: *mut c_void) {
    unsafe { mi_wasi_libc_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[inline]
pub unsafe fn mi_wasi_libc_memalign(alignment: usize, size: usize) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-wasi-libc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { crate::mimalloc::mi_memalign(alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(target_os = "wasi")]
// SAFETY[AUDIT:raw-macro-wasi-libc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
#[unsafe(export_name = "__libc_memalign")]
pub unsafe extern "C" fn mi_wasi_libc_memalign_export(
    alignment: usize,
    size: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-wasi-libc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { mi_wasi_libc_memalign(alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(
    target_os = "linux",
    not(mi_c_malloc_fast),
    not(mi_sanitizer_runtime),
    not(miri)
))]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "__libc_malloc")
)]
// SAFETY[AUDIT:raw-macro-linux-libc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_linux_libc_malloc_export(size: usize) -> *mut c_void {
    unsafe { mi_linux_libc_malloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(
    target_os = "linux",
    not(mi_c_malloc_fast),
    not(mi_sanitizer_runtime),
    not(miri)
))]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "__libc_calloc")
)]
// SAFETY[AUDIT:raw-macro-linux-libc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_linux_libc_calloc_export(count: usize, size: usize) -> *mut c_void {
    unsafe { mi_linux_libc_calloc(count, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_os = "linux", not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "__libc_realloc")
)]
// SAFETY[AUDIT:raw-macro-linux-libc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_linux_libc_realloc_export(p: *mut c_void, size: usize) -> *mut c_void {
    unsafe { mi_linux_libc_realloc(p, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(
    target_os = "linux",
    not(mi_c_malloc_fast),
    not(mi_sanitizer_runtime),
    not(miri)
))]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "__libc_free")
)]
// SAFETY[AUDIT:raw-macro-linux-libc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_linux_libc_free_export(p: *mut c_void) {
    unsafe { mi_linux_libc_free(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_os = "linux", not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(mi_sanitizer_runtime, sanitize(address = "off"))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "__libc_cfree")
)]
// SAFETY[AUDIT:raw-macro-linux-libc-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_linux_libc_cfree_export(p: *mut c_void) {
    unsafe { mi_linux_libc_cfree(p) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_os = "linux", not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "__libc_valloc")
)]
// SAFETY[AUDIT:raw-macro-linux-libc-aligned-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_linux_libc_valloc_export(size: usize) -> *mut c_void {
    unsafe { mi_linux_libc_valloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_os = "linux", not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "__libc_pvalloc")
)]
// SAFETY[AUDIT:raw-macro-linux-libc-aligned-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_linux_libc_pvalloc_export(size: usize) -> *mut c_void {
    unsafe { mi_linux_libc_pvalloc(size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_os = "linux", not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "__libc_memalign")
)]
// SAFETY[AUDIT:raw-macro-linux-libc-aligned-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_linux_libc_memalign_export(
    alignment: usize,
    size: usize,
) -> *mut c_void {
    // SAFETY[AUDIT:raw-macro-linux-libc-aligned-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { mi_linux_libc_memalign(alignment, size) }
}

/// # Safety
/// This compatibility wrapper has the same safety contract as the C/C++/libc allocation API it forwards to.
#[cfg(all(target_os = "linux", not(mi_sanitizer_runtime), not(miri)))]
#[cfg_attr(
    any(not(feature = "c-abi"), feature = "interpose"),
    unsafe(export_name = "__posix_memalign")
)]
// SAFETY[AUDIT:raw-macro-linux-libc-aligned-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
pub unsafe extern "C" fn mi_linux_posix_memalign_export(
    p: *mut *mut c_void,
    alignment: usize,
    size: usize,
) -> c_int {
    // SAFETY[AUDIT:raw-macro-linux-libc-aligned-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
    unsafe { mi_linux_posix_memalign(p, alignment, size) }
}

#[cfg(all(test, target_family = "unix", not(mi_c_malloc_fast)))]
mod tests {
    use super::*;

    #[test]
    fn unix_bootstrap_tracks_and_releases_allocations() {
        // SAFETY[AUDIT:raw-macro-bootstrap-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
        unsafe {
            let p = unix_bootstrap::malloc(17);
            assert!(!p.is_null());
            assert!(unix_bootstrap::contains(p));
            assert_eq!(unix_bootstrap::usable_size(p), 17);
            p.cast::<u8>().write(0xa5);
            assert!(unix_bootstrap::free(p));
            assert!(!unix_bootstrap::contains(p));
            assert!(!unix_bootstrap::free(p));
        }
    }

    #[test]
    fn unix_wrappers_can_realloc_and_free_bootstrap_allocations_after_init() {
        // SAFETY[AUDIT:raw-macro-bootstrap-boundary]: raw facade shims preserve C/C++/libc allocation ABI, interposition, bootstrap tracking, and forwarding contracts.
        unsafe {
            crate::init::mi_process_init();

            let p = unix_bootstrap::malloc(8);
            assert!(!p.is_null());
            for i in 0..8 {
                p.cast::<u8>().add(i).write(i as u8);
            }

            let q = mi_unix_realloc(p, 32);
            assert!(!q.is_null());
            assert!(!unix_bootstrap::contains(p));
            for i in 0..8 {
                assert_eq!(q.cast::<u8>().add(i).read(), i as u8);
            }
            mi_unix_free(q);
        }
    }
}
