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

use core::ffi::c_void;

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

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct tagMcGenContext_microsoft_windows_mimalloc {
    pub Context: MCGEN_TRACE_CONTEXT,
    pub EnableBits: [u32; 1],
}

pub type McGenContext_microsoft_windows_mimalloc = tagMcGenContext_microsoft_windows_mimalloc;
