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
    ffi::c_void,
    ptr::{null, write_bytes},
};

pub const ETW_INLINE: () = ();
pub const MCGEN_USE_KERNEL_MODE_APIS: usize = 0;
pub const MCGEN_HAVE_EVENTSETINFORMATION: usize = 2;
pub const MCGEN_EVENTUNREGISTER_EtwUnregister: &str = "EtwUnregister";
pub const MCGEN_EVENTUNREGISTER_EventUnregister: &str = "EventUnregister";
pub const MCGEN_EVENTUNREGISTER: &str = MCGEN_EVENTUNREGISTER_EventUnregister;
pub const MCGEN_EVENTSETINFORMATION_EtwSetInformation: &str = "EtwSetInformation";
pub const MCGEN_EVENTSETINFORMATION_EventSetInformation: &str = "EventSetInformation";
pub const MCGEN_EVENTSETINFORMATION: &str = MCGEN_EVENTSETINFORMATION_EventSetInformation;
pub const MCGEN_EVENTWRITETRANSFER_EtwWriteTransfer: &str = "EtwWriteTransfer";
pub const MCGEN_EVENTWRITETRANSFER_EventWriteTransfer: &str = "EventWriteTransfer";
pub const MCGEN_EVENTWRITETRANSFER: &str = MCGEN_EVENTWRITETRANSFER_EventWriteTransfer;
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
pub const EVENT_CONTROL_CODE_DISABLE_PROVIDER: ULONG = 0;
pub const EVENT_CONTROL_CODE_ENABLE_PROVIDER: ULONG = 1;

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
pub unsafe fn McGenControlCallbackV2(
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
