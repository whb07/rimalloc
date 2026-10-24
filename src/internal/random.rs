//! Random number generation

#![allow(non_camel_case_types, non_upper_case_globals)]

use crate::bits::mi_rotl32;
use crate::types::{mi_msecs_t, mi_random_ctx_t};

const MI_CHACHA_ROUNDS: usize = 20;

#[inline]
#[allow(clippy::many_single_char_names)]
fn qround(x: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    x[a] = x[a].wrapping_add(x[b]);
    x[d] = mi_rotl32(x[d] ^ x[a], 16);
    x[c] = x[c].wrapping_add(x[d]);
    x[b] = mi_rotl32(x[b] ^ x[c], 12);
    x[a] = x[a].wrapping_add(x[b]);
    x[d] = mi_rotl32(x[d] ^ x[a], 8);
    x[c] = x[c].wrapping_add(x[d]);
    x[b] = mi_rotl32(x[b] ^ x[c], 7);
}

#[inline]
fn read32(p: &[u8], idx32: usize) -> u32 {
    let i = 4 * idx32;
    u32::from_le_bytes([p[i], p[i + 1], p[i + 2], p[i + 3]])
}

#[inline]
fn mi_random_is_initialized(ctx: &mi_random_ctx_t) -> bool {
    ctx.input[0] != 0
}

#[inline]
fn chacha_block(ctx: &mut mi_random_ctx_t) {
    let mut x = ctx.input;
    for _ in 0..(MI_CHACHA_ROUNDS / 2) {
        qround(&mut x, 0, 4, 8, 12);
        qround(&mut x, 1, 5, 9, 13);
        qround(&mut x, 2, 6, 10, 14);
        qround(&mut x, 3, 7, 11, 15);
        qround(&mut x, 0, 5, 10, 15);
        qround(&mut x, 1, 6, 11, 12);
        qround(&mut x, 2, 7, 8, 13);
        qround(&mut x, 3, 4, 9, 14);
    }

    for (i, value) in x.iter().enumerate() {
        ctx.output[i] = (*value).wrapping_add(ctx.input[i]);
    }
    ctx.output_available = 16;

    ctx.input[12] = ctx.input[12].wrapping_add(1);
    if ctx.input[12] == 0 {
        ctx.input[13] = ctx.input[13].wrapping_add(1);
        if ctx.input[13] == 0 {
            ctx.input[14] = ctx.input[14].wrapping_add(1);
        }
    }
}

#[inline]
fn chacha_next32(ctx: &mut mi_random_ctx_t) -> u32 {
    if ctx.output_available <= 0 {
        chacha_block(ctx);
    }
    let idx = 16 - (ctx.output_available as usize);
    let x = ctx.output[idx];
    ctx.output[idx] = 0;
    ctx.output_available -= 1;
    x
}

#[inline]
fn chacha_init(ctx: &mut mi_random_ctx_t, key: &[u8; 32], nonce: u64) {
    ctx.output_available = 0;
    ctx.output.fill(0);
    let sigma = b"expand 32-byte k";
    for i in 0..4 {
        ctx.input[i] = read32(sigma, i);
    }
    for i in 0..8 {
        ctx.input[i + 4] = read32(key, i);
    }
    ctx.input[12] = 0;
    ctx.input[13] = 0;
    ctx.input[14] = nonce as u32;
    ctx.input[15] = (nonce >> 32) as u32;
}

#[inline]
fn chacha_split(ctx: &mi_random_ctx_t, nonce: u64, ctx_new: &mut mi_random_ctx_t) {
    *ctx_new = mi_random_ctx_t::default();
    ctx_new.weak = ctx.weak;
    ctx_new.input.copy_from_slice(&ctx.input);
    ctx_new.input[12] = 0;
    ctx_new.input[13] = 0;
    ctx_new.input[14] = nonce as u32;
    ctx_new.input[15] = (nonce >> 32) as u32;
    crate::internal::mi_debug_assert!(
        ctx.input[14] != ctx_new.input[14] || ctx.input[15] != ctx_new.input[15]
    );
    chacha_block(ctx_new);
}

#[inline]
pub(crate) fn _mi_random_shuffle(mut x: usize) -> usize {
    if x == 0 {
        x = 17;
    }
    #[cfg(target_pointer_width = "64")]
    {
        x ^= x >> 30;
        x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        x ^= x >> 27;
        x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
        x ^= x >> 31;
    }
    #[cfg(target_pointer_width = "32")]
    {
        x ^= x >> 16;
        x = x.wrapping_mul(0x7feb_352d);
        x ^= x >> 15;
        x = x.wrapping_mul(0x846c_a68b);
        x ^= x >> 16;
    }
    x
}

#[inline]
fn _mi_prim_clock_now() -> mi_msecs_t {
    unsafe { crate::prim::_mi_prim_clock_now() }
}

#[inline]
fn _mi_prim_random_buf(buf: &mut [u8]) -> bool {
    if buf.is_empty() {
        return true;
    }
    #[cfg(unix)]
    {
        // SAFETY: the mutable slice provides live, uniquely writable backing
        // for exactly `buf.len()` bytes through this synchronous primitive call.
        unsafe { crate::prim::unix::_mi_prim_random_buf(buf.as_mut_ptr().cast(), buf.len()) }
    }
    #[cfg(not(unix))]
    {
        let _ = buf;
        false
    }
}

#[inline]
/// # Safety
/// `ctx` must remain live and writable through entropy acquisition and any
/// diagnostic callback, and exclusively writable during final initialization.
unsafe fn mi_random_init_ex(ctx: *mut mi_random_ctx_t, use_weak: bool) {
    let mut key = [0u8; 32];
    let weak = use_weak || !_mi_prim_random_buf(&mut key);
    if weak {
        if !use_weak {
            crate::allocator::options::_mi_warning_message_fmt(format_args!(
                "unable to use secure randomness\n"
            ));
        }
        let mut x = _mi_os_random_weak(0);
        for i in (0..32).step_by(4) {
            x = _mi_random_shuffle(x);
            key[i] = x as u8;
            key[i + 1] = (x >> 8) as u8;
            key[i + 2] = (x >> 16) as u8;
            key[i + 3] = (x >> 24) as u8;
            x = x.wrapping_add(1);
        }
    }
    // Entropy acquisition and warning callbacks finish before borrowing the
    // destination, so an allocating callback cannot overlap that borrow.
    let ctx = unsafe { &mut *ctx };
    ctx.weak = weak;
    let nonce = ctx as *mut mi_random_ctx_t as usize as u64;
    chacha_init(ctx, &key, nonce);
}

#[inline(never)]
pub(crate) extern "C" fn _mi_random_split(ctx: &mi_random_ctx_t, ctx_new: &mut mi_random_ctx_t) {
    crate::internal::mi_debug_assert!(mi_random_is_initialized(ctx));
    crate::internal::mi_debug_assert!(!core::ptr::eq(ctx, ctx_new));
    chacha_split(
        ctx,
        ctx_new as *mut mi_random_ctx_t as usize as u64,
        ctx_new,
    );
}

#[inline(never)]
pub(crate) extern "C" fn _mi_random_next(ctx: &mut mi_random_ctx_t) -> usize {
    crate::internal::mi_debug_assert!(mi_random_is_initialized(ctx));
    loop {
        #[cfg(target_pointer_width = "32")]
        let r = chacha_next32(ctx) as usize;
        #[cfg(target_pointer_width = "64")]
        let r = ((chacha_next32(ctx) as usize) << 32) | (chacha_next32(ctx) as usize);
        if r != 0 {
            return r;
        }
    }
}

#[inline(never)]
pub(crate) extern "C" fn _mi_os_random_weak(extra_seed: usize) -> usize {
    let mut x = (_mi_os_random_weak as *const () as usize) ^ extra_seed;
    x ^= _mi_prim_clock_now() as usize;
    let max = ((x ^ (x >> 17)) & 0x0F) + 1;
    let mut i = 0usize;
    while i < max || x == 0 {
        x = _mi_random_shuffle(x);
        i += 1;
    }
    crate::internal::mi_debug_assert!(x != 0);
    x
}

/// # Safety
///
/// `ctx` must point to a writable random context.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_random_init(ctx: *mut mi_random_ctx_t) {
    unsafe { mi_random_init_ex(ctx, false) };
}

/// # Safety
///
/// `ctx` must point to a writable random context.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_random_init_weak(ctx: *mut mi_random_ctx_t) {
    unsafe { mi_random_init_ex(ctx, true) };
}

/// # Safety
///
/// `ctx` must point to a writable random context that was previously
/// initialized by the random initialization routines.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_random_reinit_if_weak(ctx: *mut mi_random_ctx_t) {
    if unsafe { (*ctx).weak } {
        unsafe { mi_random_init_ex(ctx, false) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrowed_context_split_preserves_parent_and_uses_child_nonce() {
        let mut parent = mi_random_ctx_t::default();
        chacha_init(&mut parent, &[0x5A; 32], 7);
        let original = parent;
        let mut child = mi_random_ctx_t::default();
        _mi_random_split(&parent, &mut child);
        assert_eq!(parent, original);
        assert_eq!(child.weak, parent.weak);
        assert_eq!(&child.input[..12], &parent.input[..12]);
        assert_ne!(&child.input[14..], &parent.input[14..]);
        let mut same_child = child;
        assert_eq!(
            _mi_random_next(&mut child),
            _mi_random_next(&mut same_child)
        );
    }

    #[test]
    fn primitive_random_buffer_accepts_empty_slice_without_platform_access() {
        let mut empty = [];
        assert!(_mi_prim_random_buf(&mut empty));
    }
}
