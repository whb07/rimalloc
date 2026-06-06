//! Primitive OS / platform layer

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    unsafe_op_in_unsafe_fn,
    clippy::missing_safety_doc
)]

use core::ffi::c_char;
use std::ffi::CStr;
use std::io::{self, Write};

use crate::types::mi_process_info_t;

#[inline]
pub unsafe extern "C" fn _mi_prim_out_stderr(msg: *const c_char) {
    if msg.is_null() {
        return;
    }
    let bytes = CStr::from_ptr(msg).to_bytes();
    let mut stderr = io::stderr().lock();
    let _ = stderr.write_all(bytes);
    let _ = stderr.flush();
}

#[inline]
pub unsafe extern "C" fn _mi_prim_process_info(pinfo: *mut mi_process_info_t) {
    if pinfo.is_null() {
        return;
    }
    // Leave any values already filled by higher layers intact; only provide the
    // primitive defaults that are currently available in the Rust port.
    unsafe {
        (*pinfo).utime = 0;
        (*pinfo).stime = 0;
        (*pinfo).page_faults = 0;
    }
}
