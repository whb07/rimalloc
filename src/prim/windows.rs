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
    ffi::{CStr, c_char, c_void},
    ptr::{copy_nonoverlapping, null, null_mut, write_bytes},
    sync::atomic::{AtomicI64, Ordering},
};

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
pub const MI_WIN_INIT_USE_TLS_DLLMAIN: usize = 0;
pub const MI_PRIM_HAS_PROCESS_ATTACH: usize = 1;
pub const MI_PRIM_HAS_ALLOCATOR_INIT: usize = 0;
pub const MI_MEM_EXTENDED_PARAMETER_NONPAGED_HUGE: u32 = 0x0000_0010;
pub const DECLSPEC_ALIGN: usize = 8;

pub type BYTE = u8;
pub type BOOLEAN = u8;
pub type BOOL = i32;
pub type DWORD64 = u64;
pub type HINSTANCE = *mut c_void;
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
pub type SIZE_T = usize;
pub type HANDLE = *mut c_void;
pub type DWORD = u32;
pub type PUCHAR = *mut UCHAR;
pub type PULONG = *mut ULONG;
pub type PULONGLONG = *mut u64;
pub type PUSHORT = *mut USHORT;
pub type PGROUP_AFFINITY = *mut c_void;
pub type PROCESS_MEMORY_COUNTERS = c_void;
pub type PPROCESS_MEMORY_COUNTERS = *mut PROCESS_MEMORY_COUNTERS;
pub type REGHANDLE = TRACEHANDLE;
pub type PREGHANDLE = *mut REGHANDLE;
pub type LPCGUID = *const GUID;
pub type PEVENT_FILTER_DESCRIPTOR = *const EVENT_FILTER_DESCRIPTOR;

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
pub const MEM_COMMIT: DWORD = 0x0000_1000;
pub const MEM_DECOMMIT: DWORD = 0x0000_4000;
pub const MEM_RESET: DWORD = 0x0008_0000;
pub const PAGE_NOACCESS: DWORD = 0x0000_0001;
pub const PAGE_READWRITE: DWORD = 0x0000_0004;
pub const EVENT_CONTROL_CODE_DISABLE_PROVIDER: ULONG = 0;
pub const EVENT_CONTROL_CODE_ENABLE_PROVIDER: ULONG = 1;
pub const FLS_OUT_OF_INDEXES: DWORD = u32::MAX;

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
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LARGE_INTEGER {
    pub QuadPart: i64,
}

static MI_MSECS_FREQ: AtomicI64 = AtomicI64::new(0);
static mut mi_fls_key: DWORD = FLS_OUT_OF_INDEXES;

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
unsafe fn mi_win_main(module: PVOID, reason: DWORD, reserved: PVOID) {
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
pub unsafe extern "C" fn _mi_prim_thread_init_auto_done() {}

#[inline]
pub unsafe extern "C" fn _mi_prim_thread_done_auto_done() {}

#[inline]
pub unsafe extern "system" fn mi_dll_main_raw(
    module: PVOID,
    reason: DWORD,
    reserved: PVOID,
) -> BOOL {
    unsafe { mi_win_main(module, reason, reserved) };
    TRUE as BOOL
}

#[inline]
pub unsafe extern "C" fn _mi_allocator_done() {}

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
