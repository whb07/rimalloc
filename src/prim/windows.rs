//! Windows primitive-layer ETW header surface.
//!
//! Bounded mechanical translation of the requested macro/type slice from:
//! - `src/prim/windows/etw.h`

#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code,
    unused_macros
)]

use core::{
    ffi::{CStr, c_char, c_int, c_void},
    mem::zeroed,
    ptr::{copy_nonoverlapping, null, null_mut, write_bytes},
    sync::atomic::{AtomicI64, AtomicUsize, Ordering},
};
use std::sync::Once;

use crate::bits::{MI_KiB, MI_MiB, MI_SIZE_BITS, mi_clz};
use crate::init::_mi_preloading;
use crate::internal::{_mi_os_canuse_large_page, _mi_os_get_aligned_hint, _mi_os_page_size};
use crate::options::{
    _mi_verbose_message, _mi_warning_message_fmt, mi_option_get_clamp, mi_option_is_enabled,
    mi_option_t,
};
use crate::prim::{mi_os_mem_config_t, mi_process_info_t};
use crate::types::{mi_msecs_t, mi_theap_t};

pub const ETW_INLINE: () = ();
pub const MCGEN_USE_KERNEL_MODE_APIS: usize = 0;
pub const MCGEN_HAVE_EVENTSETINFORMATION_direct: usize = 1;
pub const MCGEN_HAVE_EVENTSETINFORMATION_dynamic_lookup: usize = 2;
pub const MCGEN_HAVE_EVENTSETINFORMATION: usize = MCGEN_HAVE_EVENTSETINFORMATION_dynamic_lookup;
pub const MCGEN_EVENTUNREGISTER_EtwUnregister: &str = "EtwUnregister";
pub const MCGEN_EVENTUNREGISTER_EventUnregister: &str = "EventUnregister";
pub const MCGEN_EVENTUNREGISTER: &str = MCGEN_EVENTUNREGISTER_EventUnregister;
pub const MCGEN_EVENTSETINFORMATION_EtwSetInformation: &str = "EtwSetInformation";
pub const MCGEN_EVENTSETINFORMATION_EventSetInformation: &str = "EventSetInformation";
pub const MCGEN_EVENTSETINFORMATION: &str = MCGEN_EVENTSETINFORMATION_EventSetInformation;
pub const MCGEN_EVENTWRITETRANSFER_EtwWriteTransfer: &str = "EtwWriteTransfer";
pub const MCGEN_EVENTWRITETRANSFER_EventWriteTransfer: &str = "EventWriteTransfer";
pub const MCGEN_EVENTWRITETRANSFER: &str = MCGEN_EVENTWRITETRANSFER_EventWriteTransfer;
pub const MI_HAS_CONSOLE_IO: usize = 1;
pub const MI_WIN_INIT_USE_CRT_TLS: usize = 1;
pub const MI_WIN_INIT_USE_FLS: usize = 0;
pub const MI_WIN_INIT_USE_TLS_DLLMAIN: usize = 0;
pub const MI_PRIM_HAS_PROCESS_ATTACH: usize = 1;
pub const MI_PRIM_HAS_ALLOCATOR_INIT: usize = 1;
pub const MI_MEM_EXTENDED_PARAMETER_NONPAGED_HUGE: u32 = 0x0000_0010;
pub const DECLSPEC_ALIGN: usize = 8;

pub type BYTE = u8;
pub type BOOLEAN = u8;
pub type BOOL = i32;
pub type DWORD64 = u64;
pub type HINSTANCE = *mut c_void;
pub type HMODULE = *mut c_void;
pub type LPCSTR = *const c_char;
pub type LONG = i32;
pub type UCHAR = u8;
pub type ULONG = u32;
pub type ULONGLONG = u64;
pub type ULONG_PTR = usize;
pub type USHORT = u16;
pub type UINT_PTR = usize;
pub type WORD = u16;
pub type VOID = c_void;
pub type PVOID = *mut c_void;
pub type LPVOID = PVOID;
pub type SIZE_T = usize;
pub type HANDLE = *mut c_void;
pub type DWORD = u32;
pub type PUCHAR = *mut UCHAR;
pub type PULONG = *mut ULONG;
pub type PULONGLONG = *mut u64;
pub type PUSHORT = *mut USHORT;
pub type PGROUP_AFFINITY = *mut GROUP_AFFINITY;
pub type REGHANDLE = TRACEHANDLE;
pub type PREGHANDLE = *mut REGHANDLE;
pub type LPCGUID = *const GUID;
pub type PEVENT_FILTER_DESCRIPTOR = *const EVENT_FILTER_DESCRIPTOR;
pub type FARPROC = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GROUP_AFFINITY {
    pub Mask: ULONG_PTR,
    pub Group: WORD,
    pub Reserved: [WORD; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SYSTEM_INFO_u_s {
    pub wProcessorArchitecture: WORD,
    pub wReserved: WORD,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union SYSTEM_INFO_u {
    pub dwOemId: DWORD,
    pub s: SYSTEM_INFO_u_s,
}

impl Default for SYSTEM_INFO_u {
    fn default() -> Self {
        Self { dwOemId: 0 }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SYSTEM_INFO {
    pub u: SYSTEM_INFO_u,
    pub dwPageSize: DWORD,
    pub lpMinimumApplicationAddress: LPVOID,
    pub lpMaximumApplicationAddress: LPVOID,
    pub dwActiveProcessorMask: ULONG_PTR,
    pub dwNumberOfProcessors: DWORD,
    pub dwProcessorType: DWORD,
    pub dwAllocationGranularity: DWORD,
    pub wProcessorLevel: WORD,
    pub wProcessorRevision: WORD,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct OSVERSIONINFOW {
    pub dwOSVersionInfoSize: DWORD,
    pub dwMajorVersion: DWORD,
    pub dwMinorVersion: DWORD,
    pub dwBuildNumber: DWORD,
    pub dwPlatformId: DWORD,
    pub szCSDVersion: [u16; 128],
}

impl Default for OSVERSIONINFOW {
    fn default() -> Self {
        Self {
            dwOSVersionInfoSize: 0,
            dwMajorVersion: 0,
            dwMinorVersion: 0,
            dwBuildNumber: 0,
            dwPlatformId: 0,
            szCSDVersion: [0; 128],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LUID {
    pub LowPart: DWORD,
    pub HighPart: LONG,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LUID_AND_ATTRIBUTES {
    pub Luid: LUID,
    pub Attributes: DWORD,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TOKEN_PRIVILEGES {
    pub PrivilegeCount: DWORD,
    pub Privileges: [LUID_AND_ATTRIBUTES; 1],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MEMORY_BASIC_INFORMATION {
    pub BaseAddress: PVOID,
    pub AllocationBase: PVOID,
    pub AllocationProtect: DWORD,
    pub RegionSize: SIZE_T,
    pub State: DWORD,
    pub Protect: DWORD,
    pub Type: DWORD,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PROCESS_MEMORY_COUNTERS {
    pub cb: DWORD,
    pub PageFaultCount: DWORD,
    pub PeakWorkingSetSize: SIZE_T,
    pub WorkingSetSize: SIZE_T,
    pub QuotaPeakPagedPoolUsage: SIZE_T,
    pub QuotaPagedPoolUsage: SIZE_T,
    pub QuotaPeakNonPagedPoolUsage: SIZE_T,
    pub QuotaNonPagedPoolUsage: SIZE_T,
    pub PagefileUsage: SIZE_T,
    pub PeakPagefileUsage: SIZE_T,
}

pub type PPROCESS_MEMORY_COUNTERS = *mut PROCESS_MEMORY_COUNTERS;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct COORD {
    pub X: i16,
    pub Y: i16,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SMALL_RECT {
    pub Left: i16,
    pub Top: i16,
    pub Right: i16,
    pub Bottom: i16,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CONSOLE_SCREEN_BUFFER_INFO {
    pub dwSize: COORD,
    pub dwCursorPosition: COORD,
    pub wAttributes: WORD,
    pub srWindow: SMALL_RECT,
    pub dwMaximumWindowSize: COORD,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IMAGE_DOS_HEADER {
    pub _padding: [u8; 60],
    pub e_lfanew: i32,
}

impl Default for IMAGE_DOS_HEADER {
    fn default() -> Self {
        Self {
            _padding: [0; 60],
            e_lfanew: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IMAGE_FILE_HEADER {
    pub Machine: WORD,
    pub NumberOfSections: WORD,
    pub TimeDateStamp: DWORD,
    pub PointerToSymbolTable: DWORD,
    pub NumberOfSymbols: DWORD,
    pub SizeOfOptionalHeader: WORD,
    pub Characteristics: WORD,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IMAGE_NT_HEADERS {
    pub Signature: DWORD,
    pub FileHeader: IMAGE_FILE_HEADER,
}

pub const TRUE: BOOLEAN = 1;
pub const FALSE: BOOLEAN = 0;
pub const DLL_PROCESS_DETACH: DWORD = 0;
pub const DLL_PROCESS_ATTACH: DWORD = 1;
pub const DLL_THREAD_ATTACH: DWORD = 2;
pub const DLL_THREAD_DETACH: DWORD = 3;
pub const ERROR_COMMITMENT_MINIMUM: DWORD = 635;
pub const ERROR_COMMITMENT_LIMIT: DWORD = 1455;
pub const ERROR_PAGEFILE_QUOTA: DWORD = 1454;
pub const ERROR_NOT_ENOUGH_MEMORY: DWORD = 8;
pub const ERROR_INVALID_ADDRESS: DWORD = 487;
pub const ERROR_SUCCESS: DWORD = 0;
pub const STD_ERROR_HANDLE: DWORD = (-12i32) as DWORD;
pub const MEM_COMMIT: DWORD = 0x0000_1000;
pub const MEM_DECOMMIT: DWORD = 0x0000_4000;
pub const MEM_RELEASE: DWORD = 0x0000_8000;
pub const MEM_RESERVE: DWORD = 0x0000_2000;
pub const MEM_RESET: DWORD = 0x0008_0000;
pub const MEM_LARGE_PAGES: DWORD = 0x2000_0000;
pub const PAGE_NOACCESS: DWORD = 0x0000_0001;
pub const PAGE_READWRITE: DWORD = 0x0000_0004;
pub const EVENT_CONTROL_CODE_DISABLE_PROVIDER: ULONG = 0;
pub const EVENT_CONTROL_CODE_ENABLE_PROVIDER: ULONG = 1;
pub const FLS_OUT_OF_INDEXES: DWORD = u32::MAX;
pub const TOKEN_ADJUST_PRIVILEGES: DWORD = 0x0020;
pub const TOKEN_QUERY: DWORD = 0x0008;
pub const SE_PRIVILEGE_ENABLED: DWORD = 0x0000_0002;
pub const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: DWORD = 0x0000_0004;
pub const GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT: DWORD = 0x0000_0002;
pub const IMAGE_FILE_DLL: WORD = 0x2000;
pub const INVALID_HANDLE_VALUE: HANDLE = usize::MAX as HANDLE;

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MI_MEM_EXTENDED_PARAMETER_TYPE_E {
    MiMemExtendedParameterInvalidType = 0,
    MiMemExtendedParameterAddressRequirements = 1,
    MiMemExtendedParameterNumaNode = 2,
    MiMemExtendedParameterPartitionHandle = 3,
    MiMemExtendedParameterUserPhysicalHandle = 4,
    MiMemExtendedParameterAttributeFlags = 5,
    MiMemExtendedParameterMax = 6,
}

pub type MI_MEM_EXTENDED_PARAMETER_TYPE = MI_MEM_EXTENDED_PARAMETER_TYPE_E;

// The original C uses a pair of bitfields packed into one 64-bit word.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MI_MEM_EXTENDED_PARAMETER_TYPE_BITS {
    pub bits: DWORD64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union MI_MEM_EXTENDED_PARAMETER_ARG {
    pub ULong64: DWORD64,
    pub Pointer: PVOID,
    pub Size: SIZE_T,
    pub Handle: HANDLE,
    pub ULong: DWORD,
}

impl Default for MI_MEM_EXTENDED_PARAMETER_ARG {
    fn default() -> Self {
        Self { ULong64: 0 }
    }
}

#[repr(C, align(8))]
#[derive(Clone, Copy, Default)]
pub struct MI_MEM_EXTENDED_PARAMETER_S {
    pub Type: MI_MEM_EXTENDED_PARAMETER_TYPE_BITS,
    pub Arg: MI_MEM_EXTENDED_PARAMETER_ARG,
}

pub type MI_MEM_EXTENDED_PARAMETER = MI_MEM_EXTENDED_PARAMETER_S;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MI_MEM_ADDRESS_REQUIREMENTS_S {
    pub LowestStartingAddress: PVOID,
    pub HighestEndingAddress: PVOID,
    pub Alignment: SIZE_T,
}

pub type MI_MEM_ADDRESS_REQUIREMENTS = MI_MEM_ADDRESS_REQUIREMENTS_S;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MI_PROCESSOR_NUMBER_S {
    pub Group: WORD,
    pub Number: BYTE,
    pub Reserved: BYTE,
}

pub type MI_PROCESSOR_NUMBER = MI_PROCESSOR_NUMBER_S;

pub static mut win_major_version: DWORD = 6;
pub static mut win_minor_version: DWORD = 0;
static mut win_allocation_granularity: usize = 64 * MI_KiB;

pub type PVirtualAlloc2 = Option<
    unsafe extern "system" fn(
        HANDLE,
        PVOID,
        SIZE_T,
        ULONG,
        ULONG,
        *mut MI_MEM_EXTENDED_PARAMETER,
        ULONG,
    ) -> PVOID,
>;

pub type PNtAllocateVirtualMemoryEx = Option<
    unsafe extern "system" fn(
        HANDLE,
        *mut PVOID,
        *mut SIZE_T,
        ULONG,
        ULONG,
        *mut MI_MEM_EXTENDED_PARAMETER,
        ULONG,
    ) -> LONG,
>;

pub static mut pVirtualAlloc2: PVirtualAlloc2 = None;
pub static mut pNtAllocateVirtualMemoryEx: PNtAllocateVirtualMemoryEx = None;

pub type PGetCurrentProcessorNumberEx = Option<unsafe extern "system" fn(*mut MI_PROCESSOR_NUMBER)>;
pub type PGetNumaProcessorNodeEx =
    Option<unsafe extern "system" fn(*mut MI_PROCESSOR_NUMBER, PUSHORT) -> BOOL>;
pub type PGetNumaNodeProcessorMaskEx =
    Option<unsafe extern "system" fn(USHORT, PGROUP_AFFINITY) -> BOOL>;
pub type PGetNumaProcessorNode = Option<unsafe extern "system" fn(UCHAR, PUCHAR) -> BOOL>;
pub type PGetNumaNodeProcessorMask = Option<unsafe extern "system" fn(UCHAR, PULONGLONG) -> BOOL>;
pub type PGetNumaHighestNodeNumber = Option<unsafe extern "system" fn(PULONG) -> BOOL>;

pub static mut pGetCurrentProcessorNumberEx: PGetCurrentProcessorNumberEx = None;
pub static mut pGetNumaProcessorNodeEx: PGetNumaProcessorNodeEx = None;
pub static mut pGetNumaNodeProcessorMaskEx: PGetNumaNodeProcessorMaskEx = None;
pub static mut pGetNumaProcessorNode: PGetNumaProcessorNode = None;
pub static mut pGetNumaNodeProcessorMask: PGetNumaNodeProcessorMask = None;
pub static mut pGetNumaHighestNodeNumber: PGetNumaHighestNodeNumber = None;

pub type PGetLargePageMinimum = Option<unsafe extern "system" fn() -> SIZE_T>;
pub type PGetPhysicallyInstalledSystemMemory =
    Option<unsafe extern "system" fn(*mut ULONGLONG) -> BOOL>;
pub type PGetVersionExW = Option<unsafe extern "system" fn(*mut OSVERSIONINFOW) -> BOOL>;

pub static mut pGetLargePageMinimum: PGetLargePageMinimum = None;

pub const RtlGenRandom_SystemFunction036: &str = "SystemFunction036";
pub const RtlGenRandom: &str = RtlGenRandom_SystemFunction036;
pub const BCRYPT_USE_SYSTEM_PREFERRED_RNG: ULONG = 0x0000_0002;

unsafe extern "system" {
    pub fn SystemFunction036(RandomBuffer: PVOID, RandomBufferLength: ULONG) -> BOOLEAN;
}

pub type PGetProcessMemoryInfo =
    Option<unsafe extern "system" fn(HANDLE, PPROCESS_MEMORY_COUNTERS, DWORD) -> BOOL>;
pub type PBCryptGenRandom = Option<unsafe extern "system" fn(HANDLE, PUCHAR, ULONG, ULONG) -> LONG>;

pub static mut pGetProcessMemoryInfo: PGetProcessMemoryInfo = None;
pub static mut pBCryptGenRandom: PBCryptGenRandom = None;

pub type PFLS_CALLBACK_FUNCTION = Option<unsafe extern "system" fn(PVOID)>;

unsafe extern "system" {
    pub fn FlsAlloc(lpCallback: PFLS_CALLBACK_FUNCTION) -> DWORD;
    pub fn FlsGetValue(dwFlsIndex: DWORD) -> PVOID;
    pub fn FlsSetValue(dwFlsIndex: DWORD, lpFlsData: PVOID) -> BOOL;
    pub fn FlsFree(dwFlsIndex: DWORD) -> BOOL;
    pub fn GetLastError() -> DWORD;
    pub fn QueryPerformanceFrequency(lpFrequency: *mut LARGE_INTEGER) -> BOOL;
    pub fn VirtualAlloc(
        lpAddress: PVOID,
        dwSize: SIZE_T,
        flAllocationType: DWORD,
        flProtect: DWORD,
    ) -> PVOID;
    pub fn VirtualFree(lpAddress: PVOID, dwSize: SIZE_T, dwFreeType: DWORD) -> BOOL;
    pub fn VirtualProtect(
        lpAddress: PVOID,
        dwSize: SIZE_T,
        flNewProtect: DWORD,
        lpflOldProtect: *mut DWORD,
    ) -> BOOL;
    pub fn VirtualQuery(
        lpAddress: *const c_void,
        lpBuffer: *mut MEMORY_BASIC_INFORMATION,
        dwLength: SIZE_T,
    ) -> SIZE_T;
    pub fn GetCurrentProcess() -> HANDLE;
    pub fn GetProcessTimes(
        hProcess: HANDLE,
        lpCreationTime: *mut FILETIME,
        lpExitTime: *mut FILETIME,
        lpKernelTime: *mut FILETIME,
        lpUserTime: *mut FILETIME,
    ) -> BOOL;
    pub fn OpenProcessToken(
        ProcessHandle: HANDLE,
        DesiredAccess: DWORD,
        TokenHandle: *mut HANDLE,
    ) -> BOOL;
    pub fn LookupPrivilegeValueA(lpSystemName: LPCSTR, lpName: LPCSTR, lpLuid: *mut LUID) -> BOOL;
    pub fn AdjustTokenPrivileges(
        TokenHandle: HANDLE,
        DisableAllPrivileges: BOOL,
        NewState: *const TOKEN_PRIVILEGES,
        BufferLength: DWORD,
        PreviousState: *mut TOKEN_PRIVILEGES,
        ReturnLength: *mut DWORD,
    ) -> BOOL;
    pub fn CloseHandle(hObject: HANDLE) -> BOOL;
    pub fn GetModuleHandleExA(dwFlags: DWORD, lpModuleName: LPCSTR, phModule: *mut HMODULE)
    -> BOOL;
    pub fn GetCurrentProcessorNumber() -> DWORD;
    pub fn SwitchToThread() -> BOOL;
    pub fn GetSystemInfo(lpSystemInfo: *mut SYSTEM_INFO);
    pub fn GetStdHandle(nStdHandle: DWORD) -> HANDLE;
    pub fn GetConsoleScreenBufferInfo(
        hConsoleOutput: HANDLE,
        lpConsoleScreenBufferInfo: *mut CONSOLE_SCREEN_BUFFER_INFO,
    ) -> BOOL;
    pub fn WriteConsoleA(
        hConsoleOutput: HANDLE,
        lpBuffer: *const c_void,
        nNumberOfCharsToWrite: DWORD,
        lpNumberOfCharsWritten: *mut DWORD,
        lpReserved: LPVOID,
    ) -> BOOL;
    pub fn WriteFile(
        hFile: HANDLE,
        lpBuffer: *const c_void,
        nNumberOfBytesToWrite: DWORD,
        lpNumberOfBytesWritten: *mut DWORD,
        lpOverlapped: LPVOID,
    ) -> BOOL;
    pub fn LoadLibraryA(lpLibFileName: LPCSTR) -> HINSTANCE;
    pub fn GetProcAddress(hModule: HMODULE, lpProcName: LPCSTR) -> FARPROC;
    pub fn FreeLibrary(hLibModule: HMODULE) -> BOOL;
    pub fn Sleep(dwMilliseconds: DWORD);
}

unsafe extern "C" {
    fn atexit(func: Option<unsafe extern "C" fn()>) -> c_int;
}

unsafe extern "system" {
    fn NtCurrentTeb() -> *mut c_void;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LARGE_INTEGER {
    pub QuadPart: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FILETIME {
    pub dwLowDateTime: DWORD,
    pub dwHighDateTime: DWORD,
}

static MI_MSECS_FREQ: AtomicI64 = AtomicI64::new(0);
static MI_HUGE_PAGES_AVAILABLE: AtomicUsize = AtomicUsize::new(1);
static WIN_LARGE_OS_PAGES_INIT: Once = Once::new();
static PROCESS_INFO_INIT: Once = Once::new();
static STDERR_HANDLE_INIT: Once = Once::new();
static mut mi_fls_key: DWORD = FLS_OUT_OF_INDEXES;
static mut win_large_os_pages_enabled: bool = false;
static mut win_large_os_page_size: usize = 0;
static mut mi_stderr_handle: HANDLE = INVALID_HANDLE_VALUE;
static mut mi_stderr_is_console: bool = false;

#[inline]
fn mi_getenv_value(name: &str) -> Option<String> {
    if let Some(value) = std::env::var_os(name) {
        return Some(value.to_string_lossy().into_owned());
    }
    std::env::vars_os().find_map(|(key, value)| {
        key.to_str()
            .filter(|candidate| candidate.eq_ignore_ascii_case(name))
            .map(|_| value.to_string_lossy().into_owned())
    })
}

#[inline]
pub fn win_is_out_of_memory_error(err: DWORD) -> bool {
    matches!(
        err,
        ERROR_COMMITMENT_MINIMUM
            | ERROR_COMMITMENT_LIMIT
            | ERROR_PAGEFILE_QUOTA
            | ERROR_NOT_ENOUGH_MEMORY
    )
}

#[inline]
pub fn mi_to_msecs(t: LARGE_INTEGER) -> mi_msecs_t {
    let mut freq = MI_MSECS_FREQ.load(Ordering::Acquire);
    if freq == 0 {
        let mut raw_freq = LARGE_INTEGER::default();
        let measured = unsafe {
            if QueryPerformanceFrequency(&mut raw_freq) != FALSE as BOOL {
                raw_freq.QuadPart / 1000
            } else {
                1
            }
        };
        freq = measured.max(1);
        MI_MSECS_FREQ.store(freq, Ordering::Release);
    }
    t.QuadPart / freq
}

#[inline]
pub unsafe fn filetime_msecs(ftime: *const FILETIME) -> mi_msecs_t {
    crate::mi_assert_internal!(!ftime.is_null());
    let ftime = unsafe { &*ftime };
    ((((ftime.dwHighDateTime as u64) << 32) | (ftime.dwLowDateTime as u64)) / 10_000) as mi_msecs_t
}

#[inline]
pub unsafe extern "system" fn _mi_prim_thread_yield() {
    unsafe {
        SwitchToThread();
    }
}

#[inline]
unsafe extern "system" fn mi_win_main(module: PVOID, reason: DWORD, reserved: LPVOID) {
    let _ = module;
    let _ = reserved;
    match reason {
        DLL_PROCESS_ATTACH => unsafe { crate::internal::_mi_auto_process_init() },
        DLL_PROCESS_DETACH => unsafe { crate::internal::_mi_auto_process_done() },
        DLL_THREAD_DETACH => {
            if !unsafe { crate::prim::_mi_is_redirected() } {
                unsafe { crate::internal::_mi_thread_done(null_mut()) };
            }
        }
        _ => {}
    }
}

#[inline]
unsafe fn win_enable_large_os_pages(large_page_size: *mut usize) -> bool {
    WIN_LARGE_OS_PAGES_INIT.call_once(|| {
        let mut size = 0usize;
        let ok = unsafe { win_enable_large_os_pages_once(&mut size) };
        unsafe {
            win_large_os_pages_enabled = ok;
            if ok {
                win_large_os_page_size = size;
            }
        }
    });
    if !large_page_size.is_null() {
        unsafe {
            *large_page_size = win_large_os_page_size;
        }
    }
    unsafe { win_large_os_pages_enabled }
}

#[inline]
unsafe fn mi_get_proc<T>(module: HMODULE, name: LPCSTR) -> Option<T>
where
    T: Copy,
{
    let proc = unsafe { GetProcAddress(module, name) };
    if proc.is_null() {
        None
    } else {
        Some(unsafe { core::mem::transmute_copy::<FARPROC, T>(&proc) })
    }
}

#[inline]
unsafe extern "C" fn mi_current_module_is_dll() -> bool {
    let mut module: HMODULE = null_mut();
    let ok = unsafe {
        GetModuleHandleExA(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            mi_current_module_is_dll as *const () as LPCSTR,
            &mut module,
        )
    } != FALSE as BOOL;
    ok && unsafe { mi_module_is_dll(module.cast::<c_void>()) }
}

#[inline]
unsafe extern "C" fn mi_crt_done() {
    unsafe {
        mi_win_main(null_mut(), DLL_PROCESS_DETACH, null_mut());
    }
}

#[inline]
unsafe extern "C" fn mi_crt_init() -> c_int {
    if unsafe { mi_current_module_is_dll() } {
        unsafe {
            atexit(Some(mi_crt_done));
        }
    }
    0
}

#[inline]
pub unsafe extern "C" fn _mi_prim_getenv(
    name: *const c_char,
    result: *mut c_char,
    result_size: usize,
) -> bool {
    if result.is_null() || result_size == 0 {
        return false;
    }
    unsafe { *result = 0 };
    if name.is_null() {
        return false;
    }
    let key = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    let Some(value) = mi_getenv_value(&key) else {
        return false;
    };
    let bytes = value.as_bytes();
    if bytes.len() >= result_size {
        return false;
    }
    unsafe {
        copy_nonoverlapping(bytes.as_ptr().cast::<c_char>(), result, bytes.len());
        *result.add(bytes.len()) = 0;
    }
    true
}

#[inline]
pub unsafe extern "C" fn _mi_prim_mem_init(config: *mut mi_os_mem_config_t) {
    if config.is_null() {
        return;
    }

    (*config).has_overcommit = false;
    (*config).has_partial_free = false;
    (*config).has_virtual_reserve = true;

    let mut si = SYSTEM_INFO::default();
    unsafe {
        GetSystemInfo(&mut si);
    }
    if si.dwPageSize > 0 {
        (*config).page_size = si.dwPageSize as usize;
    }
    if si.dwAllocationGranularity > 0 {
        (*config).alloc_granularity = si.dwAllocationGranularity as usize;
        unsafe {
            win_allocation_granularity = si.dwAllocationGranularity as usize;
        }
    }
    if (si.lpMaximumApplicationAddress as usize) > 0 {
        let vbits = MI_SIZE_BITS - mi_clz(si.lpMaximumApplicationAddress as usize);
        (*config).virtual_address_bits = vbits;
    }

    let mut h_dll = unsafe { LoadLibraryA(c"kernelbase.dll".as_ptr()) };
    if !h_dll.is_null() {
        unsafe {
            pVirtualAlloc2 = mi_get_proc(h_dll, c"VirtualAlloc2FromApp".as_ptr());
            let virtual_alloc2 = pVirtualAlloc2;
            if virtual_alloc2.is_none() {
                pVirtualAlloc2 = mi_get_proc(h_dll, c"VirtualAlloc2".as_ptr());
            }
            FreeLibrary(h_dll);
        }
    }

    h_dll = unsafe { LoadLibraryA(c"ntdll.dll".as_ptr()) };
    if !h_dll.is_null() {
        unsafe {
            pNtAllocateVirtualMemoryEx = mi_get_proc(h_dll, c"NtAllocateVirtualMemoryEx".as_ptr());
            FreeLibrary(h_dll);
        }
    }

    h_dll = unsafe { LoadLibraryA(c"kernel32.dll".as_ptr()) };
    if !h_dll.is_null() {
        unsafe {
            pGetCurrentProcessorNumberEx =
                mi_get_proc(h_dll, c"GetCurrentProcessorNumberEx".as_ptr());
            pGetNumaProcessorNodeEx = mi_get_proc(h_dll, c"GetNumaProcessorNodeEx".as_ptr());
            pGetNumaNodeProcessorMaskEx =
                mi_get_proc(h_dll, c"GetNumaNodeProcessorMaskEx".as_ptr());
            pGetNumaProcessorNode = mi_get_proc(h_dll, c"GetNumaProcessorNode".as_ptr());
            pGetNumaNodeProcessorMask = mi_get_proc(h_dll, c"GetNumaNodeProcessorMask".as_ptr());
            pGetNumaHighestNodeNumber = mi_get_proc(h_dll, c"GetNumaHighestNodeNumber".as_ptr());
            pGetLargePageMinimum = mi_get_proc(h_dll, c"GetLargePageMinimum".as_ptr());

            let p_get_physically_installed_system_memory: PGetPhysicallyInstalledSystemMemory =
                mi_get_proc(h_dll, c"GetPhysicallyInstalledSystemMemory".as_ptr());
            if let Some(get_physical_memory) = p_get_physically_installed_system_memory {
                let mut mem_in_kib: ULONGLONG = 0;
                if get_physical_memory(&mut mem_in_kib) != FALSE as BOOL
                    && mem_in_kib > 0
                    && let Ok(mem) = usize::try_from(mem_in_kib)
                {
                    (*config).physical_memory_in_kib = mem;
                }
            }

            let p_get_version_ex_w: PGetVersionExW = mi_get_proc(h_dll, c"GetVersionExW".as_ptr());
            if let Some(get_version_ex_w) = p_get_version_ex_w {
                let mut version = OSVERSIONINFOW {
                    dwOSVersionInfoSize: core::mem::size_of::<OSVERSIONINFOW>() as DWORD,
                    ..OSVERSIONINFOW::default()
                };
                if get_version_ex_w(&mut version) != FALSE as BOOL {
                    win_major_version = version.dwMajorVersion;
                    win_minor_version = version.dwMinorVersion;
                }
            }
            FreeLibrary(h_dll);
        }
    }

    if mi_option_is_enabled(mi_option_t::mi_option_allow_large_os_pages)
        || mi_option_is_enabled(mi_option_t::mi_option_reserve_huge_os_pages)
    {
        unsafe {
            win_enable_large_os_pages(&mut (*config).large_page_size);
        }
    }
}

#[inline]
unsafe fn mi_module_is_dll(mod_: PVOID) -> bool {
    if mod_.is_null() {
        return false;
    }
    let image_dos_header = mod_.cast::<IMAGE_DOS_HEADER>();
    let image_nt_headers =
        mod_.cast::<u8>().add((*image_dos_header).e_lfanew as usize) as *const IMAGE_NT_HEADERS;
    ((*image_nt_headers).FileHeader.Characteristics & IMAGE_FILE_DLL) == IMAGE_FILE_DLL
}

#[inline]
pub unsafe extern "system" fn mi_tls_attach(module: PVOID, reason: DWORD, reserved: LPVOID) {
    if reason == DLL_THREAD_ATTACH || reason == DLL_PROCESS_ATTACH {
        unsafe { mi_win_main(module, reason, reserved) };
    }
}

#[inline]
pub unsafe extern "system" fn mi_win_main_detach(module: PVOID, reason: DWORD, reserved: LPVOID) {
    if reason == DLL_PROCESS_DETACH || reason == DLL_THREAD_DETACH {
        unsafe { mi_win_main(module, reason, reserved) };
    }
}

#[inline]
pub unsafe extern "system" fn mi_tls_detach(module: PVOID, reason: DWORD, reserved: LPVOID) {
    if reason == DLL_THREAD_DETACH
        || (reason == DLL_PROCESS_DETACH && !unsafe { mi_module_is_dll(module) })
    {
        unsafe { mi_win_main(module, reason, reserved) };
    }
}

#[inline]
pub unsafe extern "system" fn mi_fls_done(value: PVOID) {
    let theap = value.cast::<mi_theap_t>();
    if !theap.is_null() {
        unsafe { crate::internal::_mi_thread_done(theap) };
        if unsafe { mi_fls_key } != FLS_OUT_OF_INDEXES {
            unsafe {
                FlsSetValue(mi_fls_key, null_mut());
            }
        }
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_thread_init_auto_done() {
    if MI_WIN_INIT_USE_FLS != 0 {
        unsafe {
            mi_fls_key = FlsAlloc(Some(mi_fls_done));
        }
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_thread_done_auto_done() {
    if MI_WIN_INIT_USE_FLS != 0 && unsafe { mi_fls_key } != FLS_OUT_OF_INDEXES {
        unsafe {
            FlsFree(mi_fls_key);
            mi_fls_key = FLS_OUT_OF_INDEXES;
        }
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_thread_associate_default_theap(theap: *mut mi_theap_t) {
    if MI_WIN_INIT_USE_FLS != 0 {
        crate::mi_assert_internal!(unsafe { mi_fls_key } != FLS_OUT_OF_INDEXES);
        unsafe {
            FlsSetValue(mi_fls_key, theap.cast::<c_void>());
        }
    } else {
        let _ = theap;
    }
}

#[inline]
pub unsafe extern "system" fn mi_win_main_attach(module: PVOID, reason: DWORD, reserved: LPVOID) {
    if reason == DLL_PROCESS_ATTACH || reason == DLL_THREAD_ATTACH {
        unsafe { mi_win_main(module, reason, reserved) };
    }
}

#[inline]
pub unsafe extern "system" fn mi_dll_main_raw(
    module: PVOID,
    reason: DWORD,
    reserved: PVOID,
) -> BOOL {
    unsafe { mi_win_main(module, reason, reserved) };
    TRUE as BOOL
}

#[allow(dead_code)]
#[inline]
unsafe extern "system" fn DllMain(inst: HINSTANCE, reason: DWORD, reserved: LPVOID) -> BOOL {
    unsafe { mi_win_main(inst.cast::<c_void>(), reason, reserved) };
    TRUE as BOOL
}

#[allow(dead_code)]
#[inline]
unsafe extern "C" fn mi_process_attach() -> c_int {
    unsafe {
        mi_win_main(null_mut(), DLL_PROCESS_ATTACH, null_mut());
        atexit(Some(crate::init::_mi_auto_process_done));
    }
    0
}

#[inline]
pub unsafe extern "C" fn _mi_allocator_done() {}

#[inline]
pub unsafe extern "C" fn _mi_is_redirected() -> bool {
    false
}

#[inline]
pub unsafe extern "C" fn _mi_allocator_init(message: *mut *const c_char) -> bool {
    if !message.is_null() {
        unsafe {
            *message = null();
        }
    }
    true
}

#[inline]
pub unsafe extern "C" fn _mi_prim_commit(
    addr: *mut c_void,
    size: usize,
    is_zero: *mut bool,
) -> i32 {
    if !is_zero.is_null() {
        *is_zero = false;
    }
    let p = unsafe { VirtualAlloc(addr, size, MEM_COMMIT, PAGE_READWRITE) };
    if p.is_null() {
        unsafe { GetLastError() as i32 }
    } else {
        0
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_decommit(
    addr: *mut c_void,
    size: usize,
    needs_recommit: *mut bool,
) -> i32 {
    let ok = unsafe { VirtualFree(addr, size, MEM_DECOMMIT) };
    if !needs_recommit.is_null() {
        *needs_recommit = true;
    }
    if ok != FALSE as BOOL {
        0
    } else {
        unsafe { GetLastError() as i32 }
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_free(addr: *mut c_void, _size: usize) -> i32 {
    let mut errcode: DWORD = 0;
    let mut err = unsafe { VirtualFree(addr, 0, MEM_RELEASE) == FALSE as BOOL };
    if err {
        errcode = unsafe { GetLastError() };
    }
    if errcode == ERROR_INVALID_ADDRESS {
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { zeroed() };
        unsafe {
            VirtualQuery(
                addr.cast::<c_void>(),
                &mut info,
                core::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            );
        }
        if info.AllocationBase < addr
            && (addr as usize).saturating_sub(info.AllocationBase as usize) < 4 * MI_MiB
        {
            errcode = 0;
            err = unsafe { VirtualFree(info.AllocationBase, 0, MEM_RELEASE) == FALSE as BOOL };
            if err {
                errcode = unsafe { GetLastError() };
            }
        }
    }
    errcode as i32
}

#[inline]
pub unsafe extern "C" fn _mi_prim_reset(addr: *mut c_void, size: usize) -> i32 {
    let p = unsafe { VirtualAlloc(addr, size, MEM_RESET, PAGE_READWRITE) };
    crate::mi_assert_internal!(p == addr);
    if !p.is_null() {
        0
    } else {
        unsafe { GetLastError() as i32 }
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_reuse(addr: *mut c_void, size: usize) -> i32 {
    let _ = addr;
    let _ = size;
    0
}

#[inline]
pub unsafe extern "C" fn _mi_prim_process_info(pinfo: *mut mi_process_info_t) {
    if pinfo.is_null() {
        return;
    }

    let mut ct = FILETIME::default();
    let mut ut = FILETIME::default();
    let mut st = FILETIME::default();
    let mut et = FILETIME::default();
    unsafe {
        GetProcessTimes(GetCurrentProcess(), &mut ct, &mut et, &mut st, &mut ut);
    }
    unsafe {
        (*pinfo).utime = filetime_msecs(&ut);
        (*pinfo).stime = filetime_msecs(&st);
    }

    PROCESS_INFO_INIT.call_once(|| unsafe {
        let h_dll = LoadLibraryA(c"psapi.dll".as_ptr());
        if !h_dll.is_null() {
            pGetProcessMemoryInfo = mi_get_proc(h_dll, c"GetProcessMemoryInfo".as_ptr());
        }
    });

    let mut info = PROCESS_MEMORY_COUNTERS {
        cb: core::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as DWORD,
        ..PROCESS_MEMORY_COUNTERS::default()
    };
    if let Some(get_process_memory_info) = unsafe { pGetProcessMemoryInfo } {
        unsafe {
            get_process_memory_info(GetCurrentProcess(), &mut info, info.cb);
        }
    }
    unsafe {
        (*pinfo).current_rss = info.WorkingSetSize;
        (*pinfo).peak_rss = info.PeakWorkingSetSize;
        (*pinfo).current_commit = info.PagefileUsage;
        (*pinfo).peak_commit = info.PeakPagefileUsage;
        (*pinfo).page_faults = info.PageFaultCount as usize;
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_out_stderr(msg: *const c_char) {
    if msg.is_null() || _mi_preloading() {
        return;
    }
    let bytes = unsafe { CStr::from_ptr(msg) }.to_bytes();
    let Ok(len) = DWORD::try_from(bytes.len()) else {
        return;
    };
    if len == 0 {
        return;
    }

    STDERR_HANDLE_INIT.call_once(|| unsafe {
        mi_stderr_handle = GetStdHandle(STD_ERROR_HANDLE);
        if mi_stderr_handle != INVALID_HANDLE_VALUE {
            let mut sbi = CONSOLE_SCREEN_BUFFER_INFO::default();
            mi_stderr_is_console =
                GetConsoleScreenBufferInfo(mi_stderr_handle, &mut sbi) != FALSE as BOOL;
        }
    });

    let mut written: DWORD = 0;
    unsafe {
        if mi_stderr_is_console {
            let _ = WriteConsoleA(
                mi_stderr_handle,
                bytes.as_ptr().cast::<c_void>(),
                len,
                &mut written,
                null_mut(),
            );
        } else if mi_stderr_handle != INVALID_HANDLE_VALUE {
            let _ = WriteFile(
                mi_stderr_handle,
                bytes.as_ptr().cast::<c_void>(),
                len,
                &mut written,
                null_mut(),
            );
        } else {
            let _ = std::io::Write::write_all(&mut std::io::stderr().lock(), bytes);
        }
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_protect(addr: *mut c_void, size: usize, protect: bool) -> i32 {
    let mut oldprotect: DWORD = 0;
    let ok = unsafe {
        VirtualProtect(
            addr,
            size,
            if protect {
                PAGE_NOACCESS
            } else {
                PAGE_READWRITE
            },
            &mut oldprotect,
        )
    };
    if ok != FALSE as BOOL {
        0
    } else {
        unsafe { GetLastError() as i32 }
    }
}

#[inline]
unsafe fn win_enable_large_os_pages_once(large_page_size: *mut usize) -> bool {
    if unsafe { pGetLargePageMinimum }.is_none() {
        return false;
    }
    let mut err: DWORD = 0;
    let mut token: HANDLE = null_mut();
    let ok = unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY,
            &mut token,
        )
    } != FALSE as BOOL;
    if ok {
        let mut tp = TOKEN_PRIVILEGES::default();
        let looked_up = unsafe {
            LookupPrivilegeValueA(
                null(),
                c"SeLockMemoryPrivilege".as_ptr(),
                &mut tp.Privileges[0].Luid,
            )
        } != FALSE as BOOL;
        if looked_up {
            tp.PrivilegeCount = 1;
            tp.Privileges[0].Attributes = SE_PRIVILEGE_ENABLED;
            let adjusted = unsafe {
                AdjustTokenPrivileges(token, FALSE as BOOL, &tp, 0, null_mut(), null_mut())
            } != FALSE as BOOL;
            if adjusted {
                err = unsafe { GetLastError() };
                if err == ERROR_SUCCESS {
                    if !large_page_size.is_null()
                        && let Some(get_large_page_minimum) = unsafe { pGetLargePageMinimum }
                    {
                        unsafe {
                            *large_page_size = get_large_page_minimum();
                        }
                    }
                    unsafe {
                        CloseHandle(token);
                    }
                    return true;
                }
            }
        }
        unsafe {
            CloseHandle(token);
        }
    }
    if err == 0 {
        err = unsafe { GetLastError() };
    }
    _mi_warning_message_fmt(format_args!(
        "cannot enable large OS page support, error {}\n",
        err
    ));
    false
}

#[inline]
unsafe fn win_virtual_alloc_prim_once(
    addr: *mut c_void,
    size: usize,
    try_alignment: usize,
    flags: DWORD,
) -> *mut c_void {
    if MI_SIZE_BITS >= 64 && addr.is_null() {
        let hint = unsafe { _mi_os_get_aligned_hint(try_alignment, size) };
        if !hint.is_null() {
            let p = unsafe { VirtualAlloc(hint, size, flags, PAGE_READWRITE) };
            if !p.is_null() {
                return p;
            }
            _mi_verbose_message(format_args!(
                "warning: unable to allocate hinted aligned OS memory ({} bytes, error code: 0x{:x}, address: {:p}, alignment: {}, flags: 0x{:x})\n",
                size,
                unsafe { GetLastError() },
                hint,
                try_alignment,
                flags
            ));
        }
    }

    if addr.is_null()
        && try_alignment > unsafe { win_allocation_granularity }
        && try_alignment.is_multiple_of(unsafe { _mi_os_page_size() })
        && let Some(virtual_alloc2) = unsafe { pVirtualAlloc2 }
    {
        let mut reqs = MI_MEM_ADDRESS_REQUIREMENTS {
            Alignment: try_alignment,
            ..MI_MEM_ADDRESS_REQUIREMENTS::default()
        };
        let mut param = MI_MEM_EXTENDED_PARAMETER::default();
        param.Type.bits =
            MI_MEM_EXTENDED_PARAMETER_TYPE_E::MiMemExtendedParameterAddressRequirements as DWORD64;
        param.Arg.Pointer = (&mut reqs as *mut MI_MEM_ADDRESS_REQUIREMENTS).cast::<c_void>();
        let p = unsafe {
            virtual_alloc2(
                GetCurrentProcess(),
                addr,
                size,
                flags,
                PAGE_READWRITE,
                &mut param,
                1,
            )
        };
        if !p.is_null() {
            return p;
        }
        _mi_warning_message_fmt(format_args!(
            "unable to allocate aligned OS memory (0x{:x} bytes, error code: 0x{:x}, address: {:p}, alignment: 0x{:x}, flags: 0x{:x})\n",
            size,
            unsafe { GetLastError() },
            addr,
            try_alignment,
            flags
        ));
    }

    unsafe { VirtualAlloc(addr, size, flags, PAGE_READWRITE) }
}

#[inline]
unsafe fn win_virtual_alloc_prim(
    addr: *mut c_void,
    size: usize,
    try_alignment: usize,
    flags: DWORD,
) -> *mut c_void {
    let mut max_retry_msecs = mi_option_get_clamp(mi_option_t::mi_option_retry_on_oom, 0, 2000);
    if max_retry_msecs == 1 {
        max_retry_msecs = 100;
    }
    let mut tries = 1u32;
    loop {
        let p = unsafe { win_virtual_alloc_prim_once(addr, size, try_alignment, flags) };
        if !p.is_null() {
            return p;
        }
        let err = unsafe { GetLastError() };
        if max_retry_msecs > 0
            && try_alignment <= 8 * MI_MiB
            && (flags & MEM_COMMIT) != 0
            && (flags & MEM_LARGE_PAGES) == 0
            && win_is_out_of_memory_error(err)
            && tries <= 10
        {
            _mi_warning_message_fmt(format_args!(
                "out-of-memory on OS allocation, try again... (attempt {}, 0x{:x} bytes, error code: 0x{:x}, address: {:p}, alignment: 0x{:x}, flags: 0x{:x})\n",
                tries, size, err, addr, try_alignment, flags
            ));
            let mut sleep_msecs = (tries as i64) * 40;
            if sleep_msecs > max_retry_msecs {
                sleep_msecs = max_retry_msecs;
            }
            max_retry_msecs -= sleep_msecs;
            unsafe {
                Sleep(sleep_msecs as DWORD);
            }
            tries += 1;
            continue;
        }
        return null_mut();
    }
}

#[inline]
unsafe fn win_virtual_alloc(
    addr: *mut c_void,
    size: usize,
    try_alignment: usize,
    flags: DWORD,
    large_only: bool,
    allow_large: bool,
    is_large: *mut bool,
) -> *mut c_void {
    crate::mi_assert_internal!(!large_only || allow_large);
    static LARGE_PAGE_TRY_OK: AtomicUsize = AtomicUsize::new(0);

    let mut p = null_mut();
    if (large_only
        || (unsafe { _mi_os_canuse_large_page(size, try_alignment) }
            && mi_option_is_enabled(mi_option_t::mi_option_allow_large_os_pages)))
        && allow_large
        && (flags & MEM_COMMIT) != 0
        && (flags & MEM_RESERVE) != 0
    {
        let try_ok = LARGE_PAGE_TRY_OK.load(Ordering::Acquire);
        if !large_only && try_ok > 0 {
            let _ = LARGE_PAGE_TRY_OK.compare_exchange(
                try_ok,
                try_ok - 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        } else {
            if !is_large.is_null() {
                unsafe {
                    *is_large = true;
                }
            }
            p = unsafe {
                win_virtual_alloc_prim_once(addr, size, try_alignment, flags | MEM_LARGE_PAGES)
            };
            if large_only {
                return p;
            }
            if p.is_null() {
                LARGE_PAGE_TRY_OK.store(10, Ordering::Release);
            }
        }
    }

    if p.is_null() {
        if !is_large.is_null() {
            unsafe {
                *is_large = (flags & MEM_LARGE_PAGES) != 0;
            }
        }
        p = unsafe { win_virtual_alloc_prim(addr, size, try_alignment, flags) };
    }

    p
}

#[inline]
unsafe fn _mi_prim_alloc_huge_os_pagesx(
    hint_addr: *mut c_void,
    size: usize,
    numa_node: i32,
) -> *mut c_void {
    let flags = MEM_LARGE_PAGES | MEM_COMMIT | MEM_RESERVE;
    unsafe {
        win_enable_large_os_pages(null_mut());
    }

    let mut params = [MI_MEM_EXTENDED_PARAMETER::default(); 3];
    if let Some(nt_allocate_virtual_memory_ex) = unsafe { pNtAllocateVirtualMemoryEx }
        && MI_HUGE_PAGES_AVAILABLE.load(Ordering::Acquire) != 0
    {
        params[0].Type.bits =
            MI_MEM_EXTENDED_PARAMETER_TYPE_E::MiMemExtendedParameterAttributeFlags as DWORD64;
        params[0].Arg.ULong64 = MI_MEM_EXTENDED_PARAMETER_NONPAGED_HUGE as u64;
        let mut param_count: ULONG = 1;
        if numa_node >= 0 {
            param_count += 1;
            params[1].Type.bits =
                MI_MEM_EXTENDED_PARAMETER_TYPE_E::MiMemExtendedParameterNumaNode as DWORD64;
            params[1].Arg.ULong = numa_node as DWORD;
        }
        let mut psize = size;
        let mut base = hint_addr;
        let err = unsafe {
            nt_allocate_virtual_memory_ex(
                GetCurrentProcess(),
                &mut base,
                &mut psize,
                flags,
                PAGE_READWRITE,
                params.as_mut_ptr(),
                param_count,
            )
        };
        if err == 0 && !base.is_null() {
            return base;
        }
        MI_HUGE_PAGES_AVAILABLE.store(0, Ordering::Release);
        _mi_warning_message_fmt(format_args!(
            "unable to allocate using huge (1GiB) pages, trying large (2MiB) pages instead (status 0x{:x})\n",
            err
        ));
    }
    if numa_node >= 0
        && let Some(virtual_alloc2) = unsafe { pVirtualAlloc2 }
    {
        params[0].Type.bits =
            MI_MEM_EXTENDED_PARAMETER_TYPE_E::MiMemExtendedParameterNumaNode as DWORD64;
        params[0].Arg.ULong = numa_node as DWORD;
        return unsafe {
            virtual_alloc2(
                GetCurrentProcess(),
                hint_addr,
                size,
                flags,
                PAGE_READWRITE,
                params.as_mut_ptr(),
                1,
            )
        };
    }
    unsafe { VirtualAlloc(hint_addr, size, flags, PAGE_READWRITE) }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_alloc_huge_os_pages(
    hint_addr: *mut c_void,
    size: usize,
    numa_node: i32,
    is_zero: *mut bool,
    addr: *mut *mut c_void,
) -> i32 {
    if !is_zero.is_null() {
        unsafe {
            *is_zero = true;
        }
    }
    if addr.is_null() {
        return unsafe { GetLastError() as i32 };
    }
    unsafe {
        *addr = _mi_prim_alloc_huge_os_pagesx(hint_addr, size, numa_node);
        if !(*addr).is_null() {
            0
        } else {
            GetLastError() as i32
        }
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_alloc(
    hint_addr: *mut c_void,
    size: usize,
    try_alignment: usize,
    commit: bool,
    allow_large: bool,
    is_large: *mut bool,
    is_zero: *mut bool,
    addr: *mut *mut c_void,
) -> i32 {
    crate::mi_assert_internal!(size > 0 && size.is_multiple_of(unsafe { _mi_os_page_size() }));
    crate::mi_assert_internal!(commit || !allow_large);
    crate::mi_assert_internal!(try_alignment > 0);
    if !is_zero.is_null() {
        unsafe {
            *is_zero = true;
        }
    }
    let mut flags = MEM_RESERVE;
    if commit {
        flags |= MEM_COMMIT;
    }
    let p = unsafe {
        win_virtual_alloc(
            hint_addr,
            size,
            try_alignment,
            flags,
            false,
            allow_large,
            is_large,
        )
    };
    if !addr.is_null() {
        unsafe {
            *addr = p;
        }
    }
    if !p.is_null() {
        0
    } else {
        unsafe { GetLastError() as i32 }
    }
}

#[inline]
pub unsafe extern "C" fn _mi_prim_numa_node() -> usize {
    let mut numa_node: USHORT = 0;
    if let (Some(get_current_processor_number_ex), Some(get_numa_processor_node_ex)) =
        (unsafe { pGetCurrentProcessorNumberEx }, unsafe {
            pGetNumaProcessorNodeEx
        })
    {
        let mut pnum = MI_PROCESSOR_NUMBER::default();
        unsafe {
            get_current_processor_number_ex(&mut pnum);
        }
        let mut nnode: USHORT = 0;
        if unsafe { get_numa_processor_node_ex(&mut pnum, &mut nnode) } != FALSE as BOOL {
            numa_node = nnode;
        }
    } else if let Some(get_numa_processor_node) = unsafe { pGetNumaProcessorNode } {
        let pnum = unsafe { GetCurrentProcessorNumber() };
        let mut nnode: UCHAR = 0;
        if unsafe { get_numa_processor_node(pnum as UCHAR, &mut nnode) } != FALSE as BOOL {
            numa_node = nnode as USHORT;
        }
    }
    numa_node as usize
}

#[inline]
pub unsafe extern "C" fn _mi_prim_numa_node_count() -> usize {
    let mut numa_max: ULONG = 0;
    if let Some(get_numa_highest_node_number) = unsafe { pGetNumaHighestNodeNumber } {
        unsafe {
            get_numa_highest_node_number(&mut numa_max);
        }
    }
    while numa_max > 0 {
        if let Some(get_numa_node_processor_mask_ex) = unsafe { pGetNumaNodeProcessorMaskEx } {
            let mut affinity = GROUP_AFFINITY::default();
            if unsafe { get_numa_node_processor_mask_ex(numa_max as USHORT, &mut affinity) }
                != FALSE as BOOL
                && affinity.Mask != 0
            {
                break;
            }
        } else if let Some(get_numa_node_processor_mask) = unsafe { pGetNumaNodeProcessorMask } {
            let mut mask: ULONGLONG = 0;
            if unsafe { get_numa_node_processor_mask(numa_max as UCHAR, &mut mask) }
                != FALSE as BOOL
                && mask != 0
            {
                break;
            }
        }
        numa_max -= 1;
    }
    (numa_max as usize) + 1
}

#[inline]
pub unsafe extern "C" fn _mi_prim_thread_is_in_threadpool() -> bool {
    #[cfg(any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64"))]
    {
        if unsafe { win_major_version } >= 6 {
            let teb = unsafe { NtCurrentTeb() } as *mut u8;
            if !teb.is_null() {
                let pool_data = unsafe {
                    *(teb.add(if MI_SIZE_BITS == 32 { 0x0F90 } else { 0x1778 })
                        as *const *mut c_void)
                };
                return !pool_data.is_null();
            }
        }
    }
    false
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EVENT_FILTER_DESCRIPTOR {
    pub ptr: u64,
    pub size: u32,
    pub type_: u32,
}

pub type TRACEHANDLE = usize;
pub type PMCGEN_TRACE_CONTEXT = *mut MCGEN_TRACE_CONTEXT;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EVENT_DESCRIPTOR {
    pub Id: USHORT,
    pub Version: UCHAR,
    pub Channel: UCHAR,
    pub Level: UCHAR,
    pub Opcode: UCHAR,
    pub Task: USHORT,
    pub Keyword: ULONGLONG,
}

pub type PCEVENT_DESCRIPTOR = *const EVENT_DESCRIPTOR;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EVENT_DATA_DESCRIPTOR {
    pub Ptr: ULONG_PTR,
    pub Size: ULONG,
    pub Reserved: ULONG,
}

pub type PEVENT_DATA_DESCRIPTOR = *mut EVENT_DATA_DESCRIPTOR;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MCGEN_TRACE_CONTEXT {
    pub RegistrationHandle: TRACEHANDLE,
    pub Logger: TRACEHANDLE,
    pub MatchAnyKeyword: u64,
    pub MatchAllKeyword: u64,
    pub Flags: u32,
    pub IsEnabled: u32,
    pub Level: u8,
    pub Reserve: u8,
    pub EnableBitsCount: u16,
    pub EnableBitMask: *mut u32,
    pub EnableKeyWords: *const u64,
    pub EnableLevel: *const u8,
}

pub type PENABLECALLBACK = Option<
    unsafe extern "system" fn(
        source_id: *const GUID,
        is_enabled: u32,
        level: u8,
        match_any_keyword: u64,
        match_all_keyword: u64,
        filter_data: *const EVENT_FILTER_DESCRIPTOR,
        callback_context: *mut c_void,
    ),
>;

pub type _mcgen_PENABLECALLBACK = PENABLECALLBACK;
pub const MCGEN_LEVEL_KEYWORD_ENABLED_DEF: () = ();
pub const MCGEN_EVENT_ENABLED_DEF: () = ();
pub const MCGEN_CONTROL_CALLBACK: () = ();
pub const McGenEventWrite_def: () = ();
pub const McGenEventRegisterUnregister: () = ();

// These C helpers paste identifiers. In Rust, this bounded header-only pass keeps
// them as compile-time string-building macros until concrete call sites are ported.
macro_rules! _mcgen_PASTE2 {
    ($a:tt, $b:tt) => {
        concat!(stringify!($a), stringify!($b))
    };
}

macro_rules! _mcgen_PASTE3 {
    ($a:tt, $b:tt, $c:tt) => {
        concat!(stringify!($a), stringify!($b), "_", stringify!($c))
    };
}

#[repr(C)]
pub struct MCGEN_EVENTREGISTER_definition_must_be_an_unqualified_identifier_EventRegister {
    _private: [u8; 0],
}

pub type MCGEN_EVENTREGISTER_must_not_be_redefined_between_headers =
    MCGEN_EVENTREGISTER_definition_must_be_an_unqualified_identifier_EventRegister;

#[repr(C)]
pub struct MCGEN_EVENTUNREGISTER_definition_must_be_an_unqualified_identifier_EventUnregister {
    _private: [u8; 0],
}

pub type MCGEN_EVENTUNREGISTER_must_not_be_redefined_between_headers =
    MCGEN_EVENTUNREGISTER_definition_must_be_an_unqualified_identifier_EventUnregister;

#[repr(C)]
pub struct MCGEN_EVENTSETINFORMATION_definition_must_be_an_unqualified_identifier_EventSetInformation
{
    _private: [u8; 0],
}

pub type MCGEN_EVENTSETINFORMATION_must_not_be_redefined_between_headers =
    MCGEN_EVENTSETINFORMATION_definition_must_be_an_unqualified_identifier_EventSetInformation;

pub type MCGEN_EVENTREGISTER_must_not_be_a_functionLike_macro_MCGEN_EVENTREGISTER = c_void;
pub type MCGEN_EVENTUNREGISTER_must_not_be_a_functionLike_macro_MCGEN_EVENTUNREGISTER = c_void;
pub type MCGEN_EVENTSETINFORMATION_must_not_be_a_functionLike_macro_MCGEN_EVENTSETINFORMATION =
    c_void;
pub type MCGEN_EVENTWRITETRANSFER_must_not_be_a_functionLike_macro_MCGEN_EVENTWRITETRANSFER =
    c_void;

unsafe extern "system" {
    pub fn EventWriteTransfer(
        RegHandle: REGHANDLE,
        EventDescriptor: PCEVENT_DESCRIPTOR,
        ActivityId: LPCGUID,
        RelatedActivityId: LPCGUID,
        UserDataCount: ULONG,
        UserData: PEVENT_DATA_DESCRIPTOR,
    ) -> ULONG;

    pub fn EventRegister(
        ProviderId: LPCGUID,
        EnableCallback: _mcgen_PENABLECALLBACK,
        CallbackContext: PVOID,
        RegHandle: PREGHANDLE,
    ) -> ULONG;

    pub fn EventUnregister(RegHandle: REGHANDLE) -> ULONG;
}

#[inline]
pub unsafe fn McGenLevelKeywordEnabled(
    enable_info: *const MCGEN_TRACE_CONTEXT,
    level: UCHAR,
    keyword: ULONGLONG,
) -> BOOLEAN {
    if enable_info.is_null() {
        return FALSE;
    }
    let enable_info = &*enable_info;
    if (level <= enable_info.Level || enable_info.Level == 0)
        && (keyword == 0
            || ((keyword & enable_info.MatchAnyKeyword) != 0
                && (keyword & enable_info.MatchAllKeyword) == enable_info.MatchAllKeyword))
    {
        return TRUE;
    }
    FALSE
}

#[inline]
pub unsafe fn McGenEventEnabled(
    enable_info: *const MCGEN_TRACE_CONTEXT,
    event_descriptor: PCEVENT_DESCRIPTOR,
) -> BOOLEAN {
    if event_descriptor.is_null() {
        return FALSE;
    }
    McGenLevelKeywordEnabled(
        enable_info,
        (*event_descriptor).Level,
        (*event_descriptor).Keyword,
    )
}

#[inline]
pub unsafe extern "system" fn McGenControlCallbackV2(
    _source_id: LPCGUID,
    control_code: ULONG,
    level: UCHAR,
    match_any_keyword: ULONGLONG,
    match_all_keyword: ULONGLONG,
    _filter_data: PEVENT_FILTER_DESCRIPTOR,
    callback_context: PVOID,
) {
    let ctx = callback_context as PMCGEN_TRACE_CONTEXT;
    if ctx.is_null() {
        return;
    }
    let ctx_ref = &mut *ctx;
    match control_code {
        EVENT_CONTROL_CODE_ENABLE_PROVIDER => {
            ctx_ref.Level = level;
            ctx_ref.MatchAnyKeyword = match_any_keyword;
            ctx_ref.MatchAllKeyword = match_all_keyword;
            ctx_ref.IsEnabled = EVENT_CONTROL_CODE_ENABLE_PROVIDER;
            if !ctx_ref.EnableBitMask.is_null()
                && !ctx_ref.EnableKeyWords.is_null()
                && !ctx_ref.EnableLevel.is_null()
            {
                for ix in 0..ctx_ref.EnableBitsCount as usize {
                    let enabled = McGenLevelKeywordEnabled(
                        ctx as *const MCGEN_TRACE_CONTEXT,
                        *ctx_ref.EnableLevel.add(ix),
                        *ctx_ref.EnableKeyWords.add(ix),
                    ) != FALSE;
                    let bit_mask = &mut *ctx_ref.EnableBitMask.add(ix >> 5);
                    if enabled {
                        *bit_mask |= 1u32 << (ix % 32);
                    } else {
                        *bit_mask &= !(1u32 << (ix % 32));
                    }
                }
            }
        }
        EVENT_CONTROL_CODE_DISABLE_PROVIDER => {
            ctx_ref.IsEnabled = EVENT_CONTROL_CODE_DISABLE_PROVIDER;
            ctx_ref.Level = 0;
            ctx_ref.MatchAnyKeyword = 0;
            ctx_ref.MatchAllKeyword = 0;
            if ctx_ref.EnableBitsCount > 0 && !ctx_ref.EnableBitMask.is_null() {
                let word_count = (((ctx_ref.EnableBitsCount as usize) - 1) / 32) + 1;
                write_bytes(ctx_ref.EnableBitMask, 0, word_count);
            }
        }
        _ => {}
    }
}

#[inline]
pub unsafe fn McGenEventWrite(
    context: PMCGEN_TRACE_CONTEXT,
    descriptor: PCEVENT_DESCRIPTOR,
    activity_id: LPCGUID,
    event_data_count: ULONG,
    event_data: PEVENT_DATA_DESCRIPTOR,
) -> ULONG {
    if context.is_null() || descriptor.is_null() || event_data.is_null() || event_data_count == 0 {
        return 0;
    }
    let traits = (*context).Logger as UINT_PTR as *const USHORT;
    let first = &mut *event_data;
    if traits.is_null() {
        first.Ptr = 0;
        first.Size = 0;
        first.Reserved = 0;
    } else {
        first.Ptr = traits as ULONG_PTR;
        first.Size = *traits as ULONG;
        first.Reserved = 2;
    }
    EventWriteTransfer(
        (*context).RegistrationHandle,
        descriptor,
        activity_id,
        null(),
        event_data_count,
        event_data,
    )
}

#[inline]
pub unsafe fn McGenEventRegister(
    provider_id: LPCGUID,
    enable_callback: _mcgen_PENABLECALLBACK,
    callback_context: PVOID,
    reg_handle: PREGHANDLE,
) -> ULONG {
    if reg_handle.is_null() || *reg_handle != 0 {
        return 0;
    }
    EventRegister(provider_id, enable_callback, callback_context, reg_handle)
}

#[inline]
pub unsafe fn McGenEventUnregister(reg_handle: PREGHANDLE) -> ULONG {
    if reg_handle.is_null() || *reg_handle == 0 {
        return 0;
    }
    let error = EventUnregister(*reg_handle);
    *reg_handle = 0;
    error
}

macro_rules! McGenEventWrite {
    ($($arg:expr),* $(,)?) => {
        $crate::prim::windows::McGenEventWrite($($arg),*)
    };
}

macro_rules! McGenEventRegister {
    ($($arg:expr),* $(,)?) => {
        $crate::prim::windows::McGenEventRegister($($arg),*)
    };
}

macro_rules! McGenEventUnregister {
    ($($arg:expr),* $(,)?) => {
        $crate::prim::windows::McGenEventUnregister($($arg),*)
    };
}

macro_rules! MCGEN_EVENT_ENABLED {
    ($event_name:ident) => {
        unsafe {
            $crate::prim::windows::McGenEventEnabled(
                core::ptr::addr_of!($crate::prim::windows::ETW_MI_Provider_Context),
                &$crate::prim::windows::$event_name,
            ) != $crate::prim::windows::FALSE
        }
    };
}

#[inline]
pub unsafe fn _mcgen_EVENT_BIT_SET(enable_bits: *const ULONG, bit_position: usize) -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        let enable_bits = enable_bits as *const u8;
        ((*enable_bits.add(bit_position >> 3)) & (1u8 << (bit_position & 7))) != 0
    }
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    {
        ((*enable_bits.add(bit_position >> 5)) & (1u32 << (bit_position & 31))) != 0
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct tagMcGenContext_microsoft_windows_mimalloc {
    pub Context: MCGEN_TRACE_CONTEXT,
    pub EnableBits: [u32; 1],
}

pub type McGenContext_microsoft_windows_mimalloc = tagMcGenContext_microsoft_windows_mimalloc;

pub const ETW_MI_Provider: GUID = GUID {
    data1: 0x138f4dbb,
    data2: 0xee04,
    data3: 0x4899,
    data4: [0xaa, 0x0a, 0x57, 0x2a, 0xd4, 0x47, 0x57, 0x79],
};

pub const ETW_MI_Provider_Traits: *const USHORT = null();

pub const ETW_MI_ALLOC_value: USHORT = 0x64;
pub const ETW_MI_FREE_value: USHORT = 0x65;

pub const ETW_MI_ALLOC: EVENT_DESCRIPTOR = EVENT_DESCRIPTOR {
    Id: ETW_MI_ALLOC_value,
    Version: 0x1,
    Channel: 0x0,
    Level: 0x4,
    Opcode: 0x0,
    Task: 0x0,
    Keyword: 0x0,
};

pub const ETW_MI_FREE: EVENT_DESCRIPTOR = EVENT_DESCRIPTOR {
    Id: ETW_MI_FREE_value,
    Version: 0x1,
    Channel: 0x0,
    Level: 0x4,
    Opcode: 0x0,
    Task: 0x0,
    Keyword: 0x0,
};

pub static mut microsoft_windows_mimallocEnableBits: [ULONG; 1] = [0; 1];
pub static microsoft_windows_mimallocKeywords: [ULONGLONG; 1] = [0x0];
pub static microsoft_windows_mimallocLevels: [UCHAR; 1] = [0x4];

// Rust counterpart of the header's `__declspec(selectany)` provider context.
pub static mut ETW_MI_Provider_Context: MCGEN_TRACE_CONTEXT = MCGEN_TRACE_CONTEXT {
    RegistrationHandle: 0,
    Logger: 0,
    MatchAnyKeyword: 0,
    MatchAllKeyword: 0,
    Flags: 0,
    IsEnabled: 0,
    Level: 0,
    Reserve: 0,
    EnableBitsCount: 1,
    EnableBitMask: (&raw mut microsoft_windows_mimallocEnableBits).cast::<ULONG>(),
    EnableKeyWords: (&raw const microsoft_windows_mimallocKeywords).cast::<ULONGLONG>(),
    EnableLevel: (&raw const microsoft_windows_mimallocLevels).cast::<UCHAR>(),
};

#[inline]
pub unsafe fn microsoft_windows_mimallocHandle() -> PREGHANDLE {
    &raw mut ETW_MI_Provider_Context.RegistrationHandle
}

pub const ETW_MI_Provider_EventWriteActivity: ULONG = 0;

#[inline]
pub unsafe fn microsoft_windows_mimallocHandle_ForContext(
    pContext: *mut McGenContext_microsoft_windows_mimalloc,
) -> PREGHANDLE {
    &raw mut (*pContext).Context.RegistrationHandle
}

#[inline]
pub unsafe fn _mcgen_CheckContextType_microsoft_windows_mimalloc(
    pContext: *mut McGenContext_microsoft_windows_mimalloc,
) -> *mut McGenContext_microsoft_windows_mimalloc {
    pContext
}

#[inline]
pub unsafe fn _mcgen_RegisterForContext_microsoft_windows_mimalloc_EventRegister(
    provider_id: LPCGUID,
    pContext: *mut McGenContext_microsoft_windows_mimalloc,
) -> ULONG {
    if pContext.is_null() {
        return 0;
    }
    let context = &mut *pContext;
    write_bytes(
        context as *mut McGenContext_microsoft_windows_mimalloc,
        0,
        1,
    );
    context.Context.Logger = ETW_MI_Provider_Traits as ULONG_PTR;
    context.Context.EnableBitsCount = 1;
    context.Context.EnableBitMask = context.EnableBits.as_mut_ptr();
    context.Context.EnableKeyWords =
        (&raw const microsoft_windows_mimallocKeywords).cast::<ULONGLONG>();
    context.Context.EnableLevel = (&raw const microsoft_windows_mimallocLevels).cast::<UCHAR>();
    McGenEventRegister(
        provider_id,
        Some(McGenControlCallbackV2),
        (&raw mut context.Context).cast::<c_void>(),
        &raw mut context.Context.RegistrationHandle,
    )
}

#[inline]
pub unsafe fn EventRegistermicrosoft_windows_mimalloc() -> ULONG {
    McGenEventRegister(
        &ETW_MI_Provider,
        Some(McGenControlCallbackV2),
        (&raw mut ETW_MI_Provider_Context).cast::<c_void>(),
        microsoft_windows_mimallocHandle(),
    )
}

#[inline]
pub unsafe fn EventRegisterByGuidmicrosoft_windows_mimalloc(Guid: LPCGUID) -> ULONG {
    McGenEventRegister(
        Guid,
        Some(McGenControlCallbackV2),
        (&raw mut ETW_MI_Provider_Context).cast::<c_void>(),
        microsoft_windows_mimallocHandle(),
    )
}

macro_rules! EventRegistermicrosoft_windows_mimalloc_ForContext {
    ($pContext:expr $(,)?) => {
        $crate::prim::windows::_mcgen_RegisterForContext_microsoft_windows_mimalloc_EventRegister(
            core::ptr::addr_of!($crate::prim::windows::ETW_MI_Provider),
            $pContext,
        )
    };
}

macro_rules! EventUnregistermicrosoft_windows_mimalloc {
    () => {
        $crate::prim::windows::McGenEventUnregister(
            $crate::prim::windows::microsoft_windows_mimallocHandle(),
        )
    };
}

macro_rules! EventUnregistermicrosoft_windows_mimalloc_ForContext {
    ($pContext:expr $(,)?) => {
        $crate::prim::windows::McGenEventUnregister(core::ptr::addr_of_mut!(
            (*$pContext).Context.RegistrationHandle
        ))
    };
}

pub const McTemplateU0xx_def: () = ();
pub const McTemplateU0xx_ARGCOUNT: usize = 2;

#[inline]
pub unsafe fn McTemplateU0xx_EventWriteTransfer(
    Context: PMCGEN_TRACE_CONTEXT,
    Descriptor: PCEVENT_DESCRIPTOR,
    Arg0: u64,
    Arg1: u64,
) -> ULONG {
    let mut EventData = [EVENT_DATA_DESCRIPTOR::default(); McTemplateU0xx_ARGCOUNT + 1];

    EventData[1].Ptr = (&raw const Arg0).cast::<c_void>() as ULONG_PTR;
    EventData[1].Size = core::mem::size_of::<u64>() as ULONG;
    EventData[1].Reserved = 0;

    EventData[2].Ptr = (&raw const Arg1).cast::<c_void>() as ULONG_PTR;
    EventData[2].Size = core::mem::size_of::<u64>() as ULONG;
    EventData[2].Reserved = 0;

    McGenEventWrite(
        Context,
        Descriptor,
        null(),
        (McTemplateU0xx_ARGCOUNT + 1) as ULONG,
        EventData.as_mut_ptr(),
    )
}

macro_rules! _mcgen_TEMPLATE_FOR_ETW_MI_ALLOC {
    ($($arg:expr),* $(,)?) => {
        $crate::prim::windows::McTemplateU0xx_EventWriteTransfer($($arg),*)
    };
}

macro_rules! _mcgen_TEMPLATE_FOR_ETW_MI_FREE {
    ($($arg:expr),* $(,)?) => {
        $crate::prim::windows::McTemplateU0xx_EventWriteTransfer($($arg),*)
    };
}

#[inline]
pub unsafe fn EventEnabledETW_MI_ALLOC() -> bool {
    _mcgen_EVENT_BIT_SET(
        (&raw const microsoft_windows_mimallocEnableBits).cast::<ULONG>(),
        0,
    )
}

#[inline]
pub unsafe fn EventEnabledETW_MI_ALLOC_ForContext(
    pContext: *mut McGenContext_microsoft_windows_mimalloc,
) -> bool {
    let pContext = _mcgen_CheckContextType_microsoft_windows_mimalloc(pContext);
    !pContext.is_null()
        && _mcgen_EVENT_BIT_SET((&raw const (*pContext).EnableBits).cast::<ULONG>(), 0)
}

#[inline]
pub unsafe fn EventEnabledETW_MI_FREE() -> bool {
    _mcgen_EVENT_BIT_SET(
        (&raw const microsoft_windows_mimallocEnableBits).cast::<ULONG>(),
        0,
    )
}

#[inline]
pub unsafe fn EventEnabledETW_MI_FREE_ForContext(
    pContext: *mut McGenContext_microsoft_windows_mimalloc,
) -> bool {
    let pContext = _mcgen_CheckContextType_microsoft_windows_mimalloc(pContext);
    !pContext.is_null()
        && _mcgen_EVENT_BIT_SET((&raw const (*pContext).EnableBits).cast::<ULONG>(), 0)
}

#[inline]
pub unsafe fn EventWriteETW_MI_ALLOC(Address: u64, Size: u64) -> ULONG {
    if MCGEN_EVENT_ENABLED!(ETW_MI_ALLOC) {
        _mcgen_TEMPLATE_FOR_ETW_MI_ALLOC!(
            &raw mut ETW_MI_Provider_Context,
            &ETW_MI_ALLOC,
            Address,
            Size
        )
    } else {
        0
    }
}

#[inline]
pub unsafe fn EventWriteETW_MI_ALLOC_ForContext(
    pContext: *mut McGenContext_microsoft_windows_mimalloc,
    Address: u64,
    Size: u64,
) -> ULONG {
    let pContext = _mcgen_CheckContextType_microsoft_windows_mimalloc(pContext);
    if !pContext.is_null() && EventEnabledETW_MI_ALLOC_ForContext(pContext) {
        _mcgen_TEMPLATE_FOR_ETW_MI_ALLOC!(
            &raw mut (*pContext).Context,
            &ETW_MI_ALLOC,
            Address,
            Size
        )
    } else {
        0
    }
}

#[inline]
pub unsafe fn EventWriteETW_MI_FREE(Address: u64, Size: u64) -> ULONG {
    if MCGEN_EVENT_ENABLED!(ETW_MI_FREE) {
        _mcgen_TEMPLATE_FOR_ETW_MI_FREE!(
            &raw mut ETW_MI_Provider_Context,
            &ETW_MI_FREE,
            Address,
            Size
        )
    } else {
        0
    }
}

#[inline]
pub unsafe fn EventWriteETW_MI_FREE_ForContext(
    pContext: *mut McGenContext_microsoft_windows_mimalloc,
    Address: u64,
    Size: u64,
) -> ULONG {
    let pContext = _mcgen_CheckContextType_microsoft_windows_mimalloc(pContext);
    if !pContext.is_null() && EventEnabledETW_MI_FREE_ForContext(pContext) {
        _mcgen_TEMPLATE_FOR_ETW_MI_FREE!(&raw mut (*pContext).Context, &ETW_MI_FREE, Address, Size)
    } else {
        0
    }
}
