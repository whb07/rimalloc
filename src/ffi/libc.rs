//! Small libc-like string and environment helpers.

// Retains upstream libc helper constants and fallback bit routines for target
// combinations that do not use the current optimized paths.
#![allow(dead_code, non_camel_case_types, non_upper_case_globals)]

use core::cmp::min;
use core::ffi::CStr;
use core::ffi::{c_char, c_int, c_void};
use core::fmt;
use core::ptr;

use crate::bits::MI_SIZE_BITS;

// SAFETY: declares the Unix formatting primitive used
// only by `_mi_vsnprintf`, which preserves the caller-provided C varargs ABI.
#[cfg(unix)]
unsafe extern "C" {
    fn vsnprintf(buf: *mut c_char, bufsize: usize, fmt: *const c_char, args: *mut c_void) -> c_int;
}

pub const MI_NEXTC: &str = "c = *in; if (c==0) break; in++;";
pub const mi_mask_even_bits32: u32 = 0x5555_5555;
pub const mi_mask_even_nibbles32: u32 = 0x0F0F_0F0F;
pub const mi_mask_even_bits64: u64 = 0x5555_5555_5555_5555;
pub const mi_mask_even_nibbles64: u64 = 0x0F0F_0F0F_0F0F_0F0F;
#[cfg(target_pointer_width = "32")]
const MI_MASK_EVEN_PAIRS32: u32 = 0x3333_3333;
const MI_MASK_EVEN_PAIRS64: u64 = 0x3333_3333_3333_3333;

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
// SAFETY: the exported C-shaped helper inherits
// bounded readable C-string pointers from its callers.
#[inline(never)]
#[allow(clippy::many_single_char_names)]
pub(crate) unsafe extern "C" fn _mi_strnicmp(s: *const c_char, t: *const c_char, n: usize) -> i32 {
    if s.is_null() || t.is_null() || n == 0 {
        return 0;
    }
    let mut i = 0usize;
    while i < n {
        // SAFETY: `i < n`, and the helper contract
        // requires both strings to be readable until `n` or their first nul.
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
/// `s` and `t`, when non-null, must point to readable nul-terminated C strings.
// SAFETY: nullable equality handling is explicit; live
// inputs inherit the readable nul-terminated C-string contract.
#[inline(never)]
#[allow(clippy::many_single_char_names)]
pub(crate) unsafe extern "C" fn _mi_streq(s: *const c_char, t: *const c_char) -> bool {
    if s.is_null() && t.is_null() {
        return true;
    }
    if s.is_null() || t.is_null() {
        return false;
    }
    let mut i = 0usize;
    loop {
        // SAFETY: the function contract requires both
        // strings to be readable through their first nul byte.
        let a = unsafe { *as_bytes_ptr(s).add(i) };
        let b = unsafe { *as_bytes_ptr(t).add(i) };
        if a != b {
            return false;
        }
        if a == 0 {
            return true;
        }
        i += 1;
    }
}

/// # Safety
///
/// `dest` must be writable for `dest_size` bytes when non-null. `src`, when
/// non-null, must point to a readable nul-terminated C string.
// SAFETY: the C-shaped copy helper validates null and
// zero-capacity inputs before bounded destination writes.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_strlcpy(
    dest: *mut c_char,
    src: *const c_char,
    dest_size: usize,
) {
    if dest.is_null() || src.is_null() || dest_size == 0 {
        return;
    }
    // SAFETY: non-null `src` is required to point to a
    // readable nul-terminated C string.
    let src_bytes = unsafe { CStr::from_ptr(src) }.to_bytes();
    let n = min(src_bytes.len(), dest_size - 1);
    // SAFETY: `dest` is writable for `dest_size` bytes
    // by contract, `n` is at most `dest_size - 1`, and `src_bytes` came from
    // the non-null source C string.
    unsafe { ptr::copy_nonoverlapping(src_bytes.as_ptr(), as_bytes_mut_ptr(dest), n) };
    // SAFETY: `n < dest_size`, so the terminator write
    // stays in-bounds.
    unsafe { *as_bytes_mut_ptr(dest).add(n) = 0 };
}

/// # Safety
///
/// `dest` must be writable for `dest_size` bytes and contain a nul byte within
/// that range when non-null. `src`, when non-null, must point to a readable
/// nul-terminated C string.
// SAFETY: the C-shaped concatenation helper scans only
// within the caller-provided destination capacity before reusing `strlcpy`.
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
        // SAFETY: the loop bounds every destination
        // read by `dest_size`.
        if unsafe { *as_bytes_ptr(dest).add(len) } == 0 {
            break;
        }
        len += 1;
    }
    if len >= dest_size {
        return;
    }
    // SAFETY: `len < dest_size`, so the suffix pointer
    // and remaining capacity describe the writable tail of `dest`.
    unsafe {
        _mi_strlcpy(dest.add(len), src, dest_size - len);
    }
}

/// # Safety
///
/// `s`, when non-null, must be readable for `max_len` bytes or until its first
/// nul byte.
// SAFETY: null is handled explicitly; otherwise reads
// are bounded by `max_len`.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_strnlen(s: *const c_char, max_len: usize) -> usize {
    if s.is_null() {
        return 0;
    }
    let mut len = 0usize;
    // SAFETY: each byte read is within the caller's
    // `max_len` readable prefix or stops at the first nul.
    while len < max_len && unsafe { *as_bytes_ptr(s).add(len) } != 0 {
        len += 1;
    }
    len
}

/// # Safety
///
/// `s` must point to a readable nul-terminated C string when non-null.
// SAFETY: delegates to the bounded scanner with the
// C `strlen` contract supplied by callers.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_strlen(s: *const c_char) -> usize {
    // SAFETY: `_mi_strlen` inherits the readable
    // nul-terminated string contract and maps it to an unbounded C-style scan.
    unsafe { _mi_strnlen(s, usize::MAX) }
}

/// # Safety
///
/// `s`, when non-null, must be readable for `max_len` bytes or until its first
/// nul byte. `pat`, when non-null, must point to a readable nul-terminated C
/// string.
// SAFETY: the search helper treats nulls explicitly and
// bounds all haystack reads by `max_len`.
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
    // SAFETY: both scans use the caller-provided
    // readable C-string contracts.
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
        // SAFETY: `start <= end` keeps haystack reads
        // within `s_len`, and `i < pat_len` bounds pattern reads.
        while i < pat_len && unsafe { *s_bytes.add(start + i) == *pat_bytes.add(i) } {
            i += 1;
        }
        if i == pat_len {
            // SAFETY: the matching offset is within the
            // bounded haystack slice.
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
// SAFETY: validates nullable output buffers before
// forwarding to the platform primitive environment reader.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_getenv(
    name: *const c_char,
    result: *mut c_char,
    result_size: usize,
) -> bool {
    if name.is_null() || result.is_null() || result_size < 64 {
        return false;
    }
    // SAFETY: after local validation the platform
    // primitive receives the same readable name and writable result contract.
    unsafe { crate::prim::_mi_prim_getenv(name, result, result_size) }
}

#[inline]
fn mi_outc(c: c_char, out: &mut *mut c_char, end: *mut c_char) {
    let p = *out;
    if (p as usize) >= (end as usize) {
        return;
    }
    // SAFETY: `p < end` leaves room for exactly one
    // formatting byte in the caller-owned output range.
    unsafe {
        *p = c;
    }
    // SAFETY: advancing by one stays at or before the
    // end pointer after the checked byte write.
    *out = unsafe { p.add(1) };
}

// SAFETY: number formatting receives a caller-owned
// output range and only writes through `mi_outc`.
#[inline(never)]
unsafe fn mi_out_num(
    mut x: u128,
    base: usize,
    prefix: c_char,
    out: &mut *mut c_char,
    end: *mut c_char,
) {
    if x == 0 || base == 0 || base > 16 {
        if prefix != 0 {
            mi_outc(prefix, out, end);
        }
        mi_outc(b'0' as c_char, out, end);
        return;
    }

    let start = *out;
    while x > 0 {
        let digit = (x % base as u128) as u8;
        let c = if digit <= 9 {
            b'0' + digit
        } else {
            b'A' + digit - 10
        };
        mi_outc(c as c_char, out, end);
        x /= base as u128;
    }
    if prefix != 0 {
        mi_outc(prefix, out, end);
    }
    // SAFETY: both pointers are into the same output
    // range produced by repeated `mi_outc` writes.
    let len = unsafe { (*out).offset_from(start) as usize };
    for i in 0..(len / 2) {
        // SAFETY: `i` and `len - i - 1` are in the
        // initialized digit range `[start, start + len)`.
        unsafe {
            let c = *start.add(len - i - 1);
            *start.add(len - i - 1) = *start.add(i);
            *start.add(i) = c;
        }
    }
}

#[inline]
#[cfg(target_pointer_width = "32")]
fn mi_byte_sum32(mut x: u32) -> usize {
    x += x << 8;
    x += x << 16;
    (x >> 24) as usize
}

#[inline]
fn mi_byte_sum64(mut x: u64) -> usize {
    x += x << 8;
    x += x << 16;
    x += x << 32;
    (x >> 56) as usize
}

#[inline]
fn mi_ctz_generic32(x: u32) -> usize {
    const DEBRUIJN: [u8; 32] = [
        0, 1, 28, 2, 29, 14, 24, 3, 30, 22, 20, 15, 25, 17, 4, 8, 31, 27, 13, 23, 21, 19, 16, 7,
        26, 12, 18, 6, 11, 5, 10, 9,
    ];
    if x == 0 {
        32
    } else {
        DEBRUIJN[((x & x.wrapping_neg()).wrapping_mul(0x077C_B531)) as usize >> 27] as usize
    }
}

#[inline]
fn mi_clz_generic32(mut x: u32) -> usize {
    const DEBRUIJN: [u8; 32] = [
        31, 22, 30, 21, 18, 10, 29, 2, 20, 17, 15, 13, 9, 6, 28, 1, 23, 19, 11, 3, 16, 14, 7, 24,
        12, 4, 8, 25, 5, 26, 27, 0,
    ];
    if x == 0 {
        return 32;
    }
    x |= x >> 1;
    x |= x >> 2;
    x |= x >> 4;
    x |= x >> 8;
    x |= x >> 16;
    DEBRUIJN[(x.wrapping_mul(0x07C4_ACDD)) as usize >> 27] as usize
}

#[inline]
#[cfg(target_pointer_width = "32")]
fn mi_popcount_generic32(mut x: u32) -> usize {
    x = x - ((x >> 1) & mi_mask_even_bits32);
    x = (x & MI_MASK_EVEN_PAIRS32) + ((x >> 2) & MI_MASK_EVEN_PAIRS32);
    x = (x + (x >> 4)) & mi_mask_even_nibbles32;
    mi_byte_sum32(x)
}

#[inline]
fn mi_popcount_generic64(mut x: u64) -> usize {
    x = x - ((x >> 1) & mi_mask_even_bits64);
    x = (x & MI_MASK_EVEN_PAIRS64) + ((x >> 2) & MI_MASK_EVEN_PAIRS64);
    x = (x + (x >> 4)) & mi_mask_even_nibbles64;
    mi_byte_sum64(x)
}

#[inline]
pub extern "C" fn _mi_ctz_generic(x: usize) -> usize {
    if x == 0 {
        return MI_SIZE_BITS;
    }
    #[cfg(target_pointer_width = "32")]
    {
        mi_ctz_generic32(x as u32)
    }
    #[cfg(target_pointer_width = "64")]
    {
        let lo = x as u32;
        if lo != 0 {
            mi_ctz_generic32(lo)
        } else {
            32 + mi_ctz_generic32((x >> 32) as u32)
        }
    }
}

#[inline]
pub extern "C" fn _mi_clz_generic(x: usize) -> usize {
    if x == 0 {
        return MI_SIZE_BITS;
    }
    #[cfg(target_pointer_width = "32")]
    {
        mi_clz_generic32(x as u32)
    }
    #[cfg(target_pointer_width = "64")]
    {
        let hi = (x >> 32) as u32;
        if hi != 0 {
            mi_clz_generic32(hi)
        } else {
            32 + mi_clz_generic32(x as u32)
        }
    }
}

#[inline(never)]
pub(crate) extern "C" fn _mi_popcount_generic(x: usize) -> usize {
    if x <= 1 {
        return x;
    }
    if !x == 0 {
        return MI_SIZE_BITS;
    }
    #[cfg(target_pointer_width = "32")]
    {
        mi_popcount_generic32(x as u32)
    }
    #[cfg(target_pointer_width = "64")]
    {
        mi_popcount_generic64(x as u64)
    }
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
                // SAFETY: `ptr` is valid for `cap`
                // bytes by construction, and `n` is clamped to the remaining
                // space.
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
// SAFETY: the C-shaped formatting helper validates the
// output buffer and format pointer before delegating to platform formatting.
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
    // SAFETY: the caller-provided buffer is writable
    // for `bufsize`, so pre-clearing the last byte is in-bounds.
    unsafe {
        *as_bytes_mut_ptr(buf).add(bufsize - 1) = 0;
    }
    #[cfg(unix)]
    {
        // SAFETY: Unix builds forward the caller's
        // valid C format string and varargs object to `vsnprintf`.
        let written = unsafe { vsnprintf(buf, bufsize, fmt, args) };
        // SAFETY: the output buffer remains writable
        // for `bufsize` after the C call; the trailing nul is enforced.
        unsafe {
            *as_bytes_mut_ptr(buf).add(bufsize - 1) = 0;
        }
        written
    }
    #[cfg(not(unix))]
    {
        let _ = args;
        // SAFETY: non-Unix fallback reads the
        // validated format C string and writes through the bounded Rust helper.
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
        // SAFETY: test buffers passed here are
        // explicitly nul-terminated by the helper under test.
        unsafe { CStr::from_ptr(buf.as_ptr()).to_bytes() }
    }

    #[test]
    fn string_helpers_handle_null_truncation_and_search_edges() {
        let mut copied = [b'X' as c_char; 5];
        // SAFETY: `copied` is writable and the source
        // is a static C string.
        unsafe { _mi_strlcpy(copied.as_mut_ptr(), c"abcdef".as_ptr(), copied.len()) };
        assert_eq!(c_bytes(&copied), b"abcd");
        assert_eq!(copied[4], 0);

        let mut unchanged = [b'X' as c_char; 4];
        // SAFETY: null inputs are valid edge cases for
        // these compatibility helpers.
        unsafe {
            _mi_strlcpy(unchanged.as_mut_ptr(), ptr::null(), unchanged.len());
            _mi_strlcat(ptr::null_mut(), c"x".as_ptr(), 0);
        }
        assert_eq!(unchanged, [b'X' as c_char; 4]);

        let mut concatenated = [0 as c_char; 6];
        // SAFETY: `concatenated` is writable and both
        // inputs are static C strings.
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
        // SAFETY: `bounded` is readable and contains a
        // nul terminator.
        unsafe {
            assert_eq!(_mi_strnlen(bounded.as_ptr(), 2), 2);
            assert_eq!(_mi_strnlen(bounded.as_ptr(), bounded.len()), 3);
            assert_eq!(_mi_strlen(bounded.as_ptr()), 3);
            assert_eq!(_mi_strnlen(ptr::null(), bounded.len()), 0);
        }

        let mut haystack = [b'a' as c_char, b'b' as c_char, b'c' as c_char, 0];
        // SAFETY: `haystack` and the pattern are
        // readable C strings within the bounded search length.
        let found = unsafe { _mi_strnstr(haystack.as_mut_ptr(), haystack.len(), c"bc".as_ptr()) };
        assert!(!found.is_null());
        // SAFETY: `found` came from inside `haystack`.
        let offset = unsafe { found.offset_from(haystack.as_mut_ptr()) };
        assert_eq!(offset, 1);

        // SAFETY: a null pattern is a valid edge case
        // and returns the haystack pointer.
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
