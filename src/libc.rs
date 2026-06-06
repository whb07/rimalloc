//! Small libc-like string and environment helpers.

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    clippy::missing_safety_doc,
    unsafe_op_in_unsafe_fn
)]

use core::ffi::c_char;
use core::ptr;
use std::cmp::min;
use std::ffi::CStr;
use std::fmt;

#[inline]
fn as_bytes_ptr(p: *const c_char) -> *const u8 {
    p as *const u8
}

#[inline]
fn as_bytes_mut_ptr(p: *mut c_char) -> *mut u8 {
    p as *mut u8
}

#[inline]
pub extern "C" fn _mi_toupper(c: c_char) -> c_char {
    let b = c as u8;
    if b.is_ascii_lowercase() {
        (b - b'a' + b'A') as c_char
    } else {
        c
    }
}

#[inline]
pub unsafe extern "C" fn _mi_strnicmp(s: *const c_char, t: *const c_char, n: usize) -> i32 {
    if s.is_null() || t.is_null() || n == 0 {
        return 0;
    }
    let mut i = 0usize;
    while i < n {
        let a = *as_bytes_ptr(s).add(i);
        let b = *as_bytes_ptr(t).add(i);
        if _mi_toupper(a as c_char) != _mi_toupper(b as c_char) {
            return (a as i32) - (b as i32);
        }
        if a == 0 || b == 0 {
            return 0;
        }
        i += 1;
    }
    0
}

#[inline]
pub unsafe extern "C" fn _mi_streq(s: *const c_char, t: *const c_char) -> bool {
    if s.is_null() && t.is_null() {
        return true;
    }
    if s.is_null() || t.is_null() {
        return false;
    }
    let mut i = 0usize;
    loop {
        let a = *as_bytes_ptr(s).add(i);
        let b = *as_bytes_ptr(t).add(i);
        if a != b {
            return false;
        }
        if a == 0 {
            return true;
        }
        i += 1;
    }
}

#[inline]
pub unsafe extern "C" fn _mi_strlcpy(dest: *mut c_char, src: *const c_char, dest_size: usize) {
    if dest.is_null() || src.is_null() || dest_size == 0 {
        return;
    }
    let src_bytes = CStr::from_ptr(src).to_bytes();
    let n = min(src_bytes.len(), dest_size - 1);
    ptr::copy_nonoverlapping(src_bytes.as_ptr(), as_bytes_mut_ptr(dest), n);
    *as_bytes_mut_ptr(dest).add(n) = 0;
}

#[inline]
pub unsafe extern "C" fn _mi_strlcat(dest: *mut c_char, src: *const c_char, dest_size: usize) {
    if dest.is_null() || src.is_null() || dest_size == 0 {
        return;
    }
    let mut len = 0usize;
    while len < dest_size {
        if *as_bytes_ptr(dest).add(len) == 0 {
            break;
        }
        len += 1;
    }
    if len >= dest_size {
        return;
    }
    _mi_strlcpy(dest.add(len), src, dest_size - len);
}

#[inline]
pub unsafe extern "C" fn _mi_strnlen(s: *const c_char, max_len: usize) -> usize {
    if s.is_null() {
        return 0;
    }
    let mut len = 0usize;
    while len < max_len && *as_bytes_ptr(s).add(len) != 0 {
        len += 1;
    }
    len
}

#[inline]
pub unsafe extern "C" fn _mi_strlen(s: *const c_char) -> usize {
    _mi_strnlen(s, usize::MAX)
}

#[inline]
pub unsafe extern "C" fn _mi_strnstr(
    s: *mut c_char,
    max_len: usize,
    pat: *const c_char,
) -> *mut c_char {
    if s.is_null() {
        return ptr::null_mut();
    }
    if pat.is_null() {
        return s;
    }
    let s_len = _mi_strnlen(s, max_len);
    let pat_len = _mi_strlen(pat);
    if pat_len == 0 || pat_len > s_len {
        return if pat_len == 0 { s } else { ptr::null_mut() };
    }
    let s_bytes = as_bytes_ptr(s);
    let pat_bytes = as_bytes_ptr(pat);
    let end = s_len - pat_len;
    let mut start = 0usize;
    while start <= end {
        let mut i = 0usize;
        while i < pat_len && *s_bytes.add(start + i) == *pat_bytes.add(i) {
            i += 1;
        }
        if i == pat_len {
            return s.add(start);
        }
        start += 1;
    }
    ptr::null_mut()
}

#[inline]
pub unsafe extern "C" fn _mi_getenv(
    name: *const c_char,
    result: *mut c_char,
    result_size: usize,
) -> bool {
    if name.is_null() || result.is_null() || result_size < 64 {
        return false;
    }
    let name = match CStr::from_ptr(name).to_str() {
        Ok(name) => name,
        Err(_) => return false,
    };
    let value = match std::env::var_os(name).and_then(|v| v.into_string().ok()) {
        Some(value) => value,
        None => return false,
    };
    let bytes = value.as_bytes();
    if bytes.len() >= result_size {
        return false;
    }
    ptr::copy_nonoverlapping(bytes.as_ptr(), as_bytes_mut_ptr(result), bytes.len());
    *as_bytes_mut_ptr(result).add(bytes.len()) = 0;
    true
}

#[allow(dead_code)]
pub(crate) fn _mi_vsnprintf(buf: *mut c_char, bufsize: usize, args: fmt::Arguments<'_>) -> i32 {
    if buf.is_null() || bufsize == 0 {
        return 0;
    }

    let mut rendered = String::new();
    if fmt::write(&mut rendered, args).is_err() {
        rendered.clear();
    }

    let bytes = rendered.as_bytes();
    let n = min(bytes.len(), bufsize - 1);
    // SAFETY: `buf` is valid for `bufsize` bytes by caller contract, and `n < bufsize`.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), as_bytes_mut_ptr(buf), n);
        *as_bytes_mut_ptr(buf).add(n) = 0;
    }
    n as i32
}

#[allow(dead_code)]
pub(crate) fn _mi_snprintf(buf: *mut c_char, buflen: usize, args: fmt::Arguments<'_>) -> i32 {
    _mi_vsnprintf(buf, buflen, args)
}
