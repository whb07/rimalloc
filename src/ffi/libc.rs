//! Small libc-like string and environment helpers.

// Retains upstream libc helper constants and fallback bit routines for target
// combinations that do not use the current optimized paths.
#![allow(dead_code, non_camel_case_types, non_upper_case_globals)]

use core::cmp::min;
use core::ffi::CStr;
use core::ffi::{c_char, c_int, c_void};
use core::fmt;
use core::ptr;

#[cfg(unix)]
unsafe extern "C" {
    fn vsnprintf(buf: *mut c_char, bufsize: usize, fmt: *const c_char, args: *mut c_void) -> c_int;
}

pub const MI_NEXTC: &str = "c = *in; if (c==0) break; in++;";

#[inline]
fn as_bytes_ptr(p: *const c_char) -> *const u8 {
    p as *const u8
}

#[inline]
fn as_bytes_mut_ptr(p: *mut c_char) -> *mut u8 {
    p as *mut u8
}

#[inline(never)]
pub(crate) extern "C" fn _mi_toupper(c: c_char) -> c_char {
    let b = c as u8;
    if b.is_ascii_lowercase() {
        (b - b'a' + b'A') as c_char
    } else {
        c
    }
}

/// # Safety
///
/// `s` and `t`, when non-null, must point to readable C strings for at least
/// `n` bytes or until their first nul byte.
#[inline(never)]
#[allow(clippy::many_single_char_names)]
pub(crate) unsafe extern "C" fn _mi_strnicmp(s: *const c_char, t: *const c_char, n: usize) -> i32 {
    if s.is_null() || t.is_null() || n == 0 {
        return 0;
    }
    let mut i = 0usize;
    while i < n {
        let a = unsafe { *as_bytes_ptr(s).add(i) };
        let b = unsafe { *as_bytes_ptr(t).add(i) };
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

/// # Safety
///
/// `dest` must be writable for `dest_size` bytes when non-null. `src`, when
/// non-null, must point to a readable nul-terminated C string.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_strlcpy(
    dest: *mut c_char,
    src: *const c_char,
    dest_size: usize,
) {
    if dest.is_null() || src.is_null() || dest_size == 0 {
        return;
    }
    let src_bytes = unsafe { CStr::from_ptr(src) }.to_bytes();
    let n = min(src_bytes.len(), dest_size - 1);
    unsafe { ptr::copy_nonoverlapping(src_bytes.as_ptr(), as_bytes_mut_ptr(dest), n) };
    unsafe { *as_bytes_mut_ptr(dest).add(n) = 0 };
}

/// # Safety
///
/// `dest` must be writable for `dest_size` bytes and contain a nul byte within
/// that range when non-null. `src`, when non-null, must point to a readable
/// nul-terminated C string.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_strlcat(
    dest: *mut c_char,
    src: *const c_char,
    dest_size: usize,
) {
    if dest.is_null() || src.is_null() || dest_size == 0 {
        return;
    }
    let mut len = 0usize;
    while len < dest_size {
        if unsafe { *as_bytes_ptr(dest).add(len) } == 0 {
            break;
        }
        len += 1;
    }
    if len >= dest_size {
        return;
    }
    unsafe {
        _mi_strlcpy(dest.add(len), src, dest_size - len);
    }
}

/// # Safety
///
/// `s`, when non-null, must be readable for `max_len` bytes or until its first
/// nul byte.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_strnlen(s: *const c_char, max_len: usize) -> usize {
    if s.is_null() {
        return 0;
    }
    let mut len = 0usize;
    while len < max_len && unsafe { *as_bytes_ptr(s).add(len) } != 0 {
        len += 1;
    }
    len
}

/// # Safety
///
/// `s` must point to a readable nul-terminated C string when non-null.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_strlen(s: *const c_char) -> usize {
    unsafe { _mi_strnlen(s, usize::MAX) }
}

/// # Safety
///
/// `s`, when non-null, must be readable for `max_len` bytes or until its first
/// nul byte. `pat`, when non-null, must point to a readable nul-terminated C
/// string.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_strnstr(
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
    let s_len = unsafe { _mi_strnlen(s, max_len) };
    let pat_len = unsafe { _mi_strlen(pat) };
    if pat_len == 0 || pat_len > s_len {
        return if pat_len == 0 { s } else { ptr::null_mut() };
    }
    let s_bytes = as_bytes_ptr(s);
    let pat_bytes = as_bytes_ptr(pat);
    let end = s_len - pat_len;
    let mut start = 0usize;
    while start <= end {
        let mut i = 0usize;
        while i < pat_len && unsafe { *s_bytes.add(start + i) == *pat_bytes.add(i) } {
            i += 1;
        }
        if i == pat_len {
            return unsafe { s.add(start) };
        }
        start += 1;
    }
    ptr::null_mut()
}

/// # Safety
///
/// `name` must point to a readable nul-terminated C string. `result` must be
/// writable for `result_size` bytes.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_getenv(
    name: *const c_char,
    result: *mut c_char,
    result_size: usize,
) -> bool {
    if name.is_null() || result.is_null() || result_size < 64 {
        return false;
    }
    unsafe { crate::prim::_mi_prim_getenv(name, result, result_size) }
}

pub(crate) fn mi_vsnprintf_args(buf: &mut [c_char], args: fmt::Arguments<'_>) -> i32 {
    if buf.is_empty() {
        return 0;
    }

    struct FixedBuffer {
        ptr: *mut u8,
        cap: usize,
        used: usize,
    }

    impl fmt::Write for FixedBuffer {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            let available = self.cap.saturating_sub(self.used);
            let n = min(s.len(), available);
            if n > 0 {
                unsafe {
                    ptr::copy_nonoverlapping(s.as_ptr(), self.ptr.add(self.used), n);
                }
                self.used += n;
            }
            Ok(())
        }
    }

    let mut out = FixedBuffer {
        ptr: as_bytes_mut_ptr(buf.as_mut_ptr()),
        cap: buf.len() - 1,
        used: 0,
    };
    let _ = fmt::write(&mut out, args);

    // SAFETY: `out.ptr` came from `buf`, which remains exclusively borrowed,
    // and `out.used <= buf.len() - 1`.
    unsafe {
        *out.ptr.add(out.used) = 0;
    }
    out.used as i32
}

/// # Safety
///
/// `buf` must be writable for `bufsize` bytes. `fmt` must point to a readable
/// nul-terminated C format string, and `args` must be a valid varargs object
/// for that format on platforms that forward to C `vsnprintf`.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_vsnprintf(
    buf: *mut c_char,
    bufsize: usize,
    fmt: *const c_char,
    args: *mut c_void,
) -> c_int {
    if buf.is_null() || bufsize == 0 || fmt.is_null() {
        return 0;
    }
    unsafe {
        *as_bytes_mut_ptr(buf).add(bufsize - 1) = 0;
    }
    #[cfg(unix)]
    {
        let written = unsafe { vsnprintf(buf, bufsize, fmt, args) };
        unsafe {
            *as_bytes_mut_ptr(buf).add(bufsize - 1) = 0;
        }
        written
    }
    #[cfg(not(unix))]
    {
        let _ = args;
        let fmt_str = unsafe { CStr::from_ptr(fmt) }.to_string_lossy();
        // SAFETY: the raw boundary checked that `buf` is nonnull and the caller
        // guarantees it is writable for `bufsize` elements for this call.
        mi_vsnprintf_args(
            unsafe { core::slice::from_raw_parts_mut(buf, bufsize) },
            format_args!("{}", fmt_str),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ptr;

    fn c_bytes(buf: &[c_char]) -> &[u8] {
        unsafe { CStr::from_ptr(buf.as_ptr()).to_bytes() }
    }

    #[test]
    fn string_helpers_handle_null_truncation_and_search_edges() {
        let mut copied = [b'X' as c_char; 5];
        unsafe { _mi_strlcpy(copied.as_mut_ptr(), c"abcdef".as_ptr(), copied.len()) };
        assert_eq!(c_bytes(&copied), b"abcd");
        assert_eq!(copied[4], 0);

        let mut unchanged = [b'X' as c_char; 4];
        unsafe {
            _mi_strlcpy(unchanged.as_mut_ptr(), ptr::null(), unchanged.len());
            _mi_strlcat(ptr::null_mut(), c"x".as_ptr(), 0);
        }
        assert_eq!(unchanged, [b'X' as c_char; 4]);

        let mut concatenated = [0 as c_char; 6];
        unsafe {
            _mi_strlcpy(
                concatenated.as_mut_ptr(),
                c"hi".as_ptr(),
                concatenated.len(),
            );
            _mi_strlcat(
                concatenated.as_mut_ptr(),
                c"there".as_ptr(),
                concatenated.len(),
            );
        }
        assert_eq!(c_bytes(&concatenated), b"hithe");
        assert_eq!(concatenated[5], 0);

        let bounded = [
            b'a' as c_char,
            b'b' as c_char,
            b'c' as c_char,
            0,
            b'z' as c_char,
        ];
        unsafe {
            assert_eq!(_mi_strnlen(bounded.as_ptr(), 2), 2);
            assert_eq!(_mi_strnlen(bounded.as_ptr(), bounded.len()), 3);
            assert_eq!(_mi_strlen(bounded.as_ptr()), 3);
            assert_eq!(_mi_strnlen(ptr::null(), bounded.len()), 0);
        }

        let mut haystack = [b'a' as c_char, b'b' as c_char, b'c' as c_char, 0];
        let found = unsafe { _mi_strnstr(haystack.as_mut_ptr(), haystack.len(), c"bc".as_ptr()) };
        assert!(!found.is_null());
        let offset = unsafe { found.offset_from(haystack.as_mut_ptr()) };
        assert_eq!(offset, 1);

        let found = unsafe { _mi_strnstr(haystack.as_mut_ptr(), haystack.len(), ptr::null()) };
        assert_eq!(found, haystack.as_mut_ptr());
    }

    #[test]
    fn fixed_buffer_formatting_truncates_and_always_terminates() {
        let mut truncated = [b'X' as c_char; 5];
        let written = mi_vsnprintf_args(&mut truncated, format_args!("abcdef"));
        assert_eq!(written, 4);
        assert_eq!(c_bytes(&truncated), b"abcd");
        assert_eq!(truncated[4], 0);

        let mut one_byte = [b'X' as c_char; 1];
        let written = mi_vsnprintf_args(&mut one_byte, format_args!("abcdef"));
        assert_eq!(written, 0);
        assert_eq!(one_byte[0], 0);

        let written = mi_vsnprintf_args(&mut [], format_args!("ignored"));
        assert_eq!(written, 0);

        // SAFETY: a null output buffer is an explicitly supported raw-boundary
        // case and returns before either the format or varargs pointers are read.
        let written =
            unsafe { _mi_vsnprintf(ptr::null_mut(), 8, c"ignored".as_ptr(), ptr::null_mut()) };
        assert_eq!(written, 0);
    }
}
