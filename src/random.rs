//! Random number generation

#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    clippy::missing_const_for_fn,
    clippy::missing_safety_doc,
    unsafe_op_in_unsafe_fn
)]

use std::sync::OnceLock;
use std::time::Instant;

use crate::bits::mi_rotl32;
use crate::types::{mi_msecs_t, mi_random_ctx_t};

const MI_CHACHA_ROUNDS: usize = 20;

#[inline]
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
fn chacha_split(ctx: &mut mi_random_ctx_t, nonce: u64, ctx_new: &mut mi_random_ctx_t) {
    *ctx_new = mi_random_ctx_t::default();
    ctx_new.weak = ctx.weak;
    ctx_new.input.copy_from_slice(&ctx.input);
    ctx_new.input[12] = 0;
    ctx_new.input[13] = 0;
    ctx_new.input[14] = nonce as u32;
    ctx_new.input[15] = (nonce >> 32) as u32;
    crate::mi_debug_assert!(
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
    static START: OnceLock<Instant> = OnceLock::new();
    let start = START.get_or_init(Instant::now);
    start.elapsed().as_millis() as mi_msecs_t
}

#[inline]
fn _mi_prim_random_buf(buf: *mut u8, buf_len: usize) -> bool {
    if buf_len == 0 {
        return true;
    }
    crate::mi_debug_assert!(!buf.is_null());
    #[cfg(unix)]
    {
        unsafe { crate::prim::unix::_mi_prim_random_buf(buf.cast(), buf_len) }
    }
    #[cfg(not(unix))]
    {
        let _ = buf;
        let _ = buf_len;
        false
    }
}

#[inline]
fn mi_random_init_ex(ctx: &mut mi_random_ctx_t, use_weak: bool) {
    let mut key = [0u8; 32];
    if use_weak || !_mi_prim_random_buf(key.as_mut_ptr(), key.len()) {
        if !use_weak {
            crate::options::_mi_warning_message_fmt(format_args!(
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
        ctx.weak = true;
    } else {
        ctx.weak = false;
    }
    let nonce = ctx as *mut mi_random_ctx_t as usize as u64;
    chacha_init(ctx, &key, nonce);
}

pub(crate) unsafe extern "C" fn _mi_random_split(
    ctx: *mut mi_random_ctx_t,
    ctx_new: *mut mi_random_ctx_t,
) {
    let ctx = &mut *ctx;
    let ctx_new = &mut *ctx_new;
    crate::mi_debug_assert!(mi_random_is_initialized(ctx));
    crate::mi_debug_assert!(!std::ptr::eq(ctx, ctx_new));
    chacha_split(
        ctx,
        ctx_new as *mut mi_random_ctx_t as usize as u64,
        ctx_new,
    );
}

pub(crate) unsafe extern "C" fn _mi_random_next(ctx: *mut mi_random_ctx_t) -> usize {
    let ctx = &mut *ctx;
    crate::mi_debug_assert!(mi_random_is_initialized(ctx));
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

#[inline]
fn _mi_os_random_weak(extra_seed: usize) -> usize {
    let mut x = (_mi_os_random_weak as usize) ^ extra_seed;
    x ^= _mi_prim_clock_now() as usize;
    let max = ((x ^ (x >> 17)) & 0x0F) + 1;
    let mut i = 0usize;
    while i < max || x == 0 {
        x = _mi_random_shuffle(x);
        i += 1;
    }
    crate::mi_debug_assert!(x != 0);
    x
}

pub(crate) unsafe extern "C" fn _mi_random_init(ctx: *mut mi_random_ctx_t) {
    mi_random_init_ex(&mut *ctx, false);
}

#[inline]
pub(crate) unsafe extern "C" fn _mi_random_init_weak(ctx: *mut mi_random_ctx_t) {
    mi_random_init_ex(&mut *ctx, true);
}

#[inline]
pub(crate) unsafe extern "C" fn _mi_random_reinit_if_weak(ctx: *mut mi_random_ctx_t) {
    if (*ctx).weak {
        _mi_random_init(ctx);
    }
}
