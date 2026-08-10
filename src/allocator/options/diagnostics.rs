//! Bounded diagnostic formatting and callback registration/invocation.

#[cfg(any(
    not(target_os = "linux"),
    all(feature = "std", not(feature = "tls-shim"))
))]
use core::cell::Cell;
use core::cell::UnsafeCell;
use core::ffi::{CStr, c_char, c_int, c_long, c_void};
use core::fmt;
use core::fmt::Write as _;
use core::ptr;
use core::ptr::{NonNull, addr_of_mut};
use core::sync::atomic::{AtomicIsize, AtomicPtr, AtomicUsize, Ordering};

use crate::atomic::{mi_lock_acquire, mi_lock_release, mi_lock_s, mi_lock_t};
use crate::core_types::{
    Errno, ErrorCallback, ErrorCallbackArg, MiLockGuard, MiLockPtr, OutputCallback,
    OutputCallbackArg,
};
use crate::init::{_mi_is_main_thread, _mi_preloading, _mi_thread_id};
use crate::internal::MI_DEBUG;
use crate::libc::_mi_strnlen;
use crate::prim::_mi_prim_out_stderr;
use crate::types::{MI_MALLOC_VERSION, mi_error_fun, mi_output_fun};

use super::{mi_option_t, storage};

const MI_MAX_DELAY_OUTPUT: usize = 16 * 1024;

// SAFETY: assertion failure shims read caller C strings, emit diagnostics, and abort.
unsafe extern "C" {
    #[link_name = "abort"]
    fn c_abort() -> !;
}

// SAFETY: C stdout/stderr symbol comparisons stay behind platform-gated libc file handles.
#[cfg(all(target_os = "linux", not(target_env = "musl")))]
unsafe extern "C" {
    #[link_name = "stdout"]
    static mut C_STDOUT: *mut c_void;
    #[link_name = "stderr"]
    static mut C_STDERR: *mut c_void;
}

#[cfg(any(
    not(target_os = "linux"),
    all(feature = "std", not(feature = "tls-shim"))
))]
thread_local! {
    static recurse: Cell<bool> = const { Cell::new(false) };
}

// SAFETY: diagnostic recursion guards use TLS or thread-local state before callback dispatch.
#[cfg(all(target_os = "linux", any(feature = "tls-shim", not(feature = "std"))))]
unsafe extern "C" {
    fn _mi_tls_recurse_get() -> c_int;
    fn _mi_tls_recurse_set(recurse: c_int);
}

struct DelayedOutputBuffer(UnsafeCell<[u8; MI_MAX_DELAY_OUTPUT + 1]>);

// SAFETY: delayed output buffering uses a bounded static buffer and mimalloc lock discipline.
unsafe impl Sync for DelayedOutputBuffer {}

static out_buf: DelayedOutputBuffer =
    DelayedOutputBuffer(UnsafeCell::new([0; MI_MAX_DELAY_OUTPUT + 1]));
static out_len: AtomicUsize = AtomicUsize::new(0);
static mut out_buf_lock: mi_lock_t = mi_lock_s::new();
static mi_out_default: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static mi_out_arg: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static mi_max_error_count: AtomicIsize = AtomicIsize::new(16);
static mi_max_warning_count: AtomicIsize = AtomicIsize::new(16);
static error_count: AtomicUsize = AtomicUsize::new(0);
static warning_count: AtomicUsize = AtomicUsize::new(0);

fn option_get(option: mi_option_t) -> c_long {
    storage::option_get(option, _mi_warning_message_fmt)
}

fn option_is_enabled(option: mi_option_t) -> bool {
    storage::option_is_enabled(option, _mi_warning_message_fmt)
}

struct LockedErrorHandler(UnsafeCell<ErrorCallback>);

// SAFETY: allocation and error failures invoke registered callbacks or the default abort policy.
unsafe impl Sync for LockedErrorHandler {}

static mut mi_error_handler_lock: mi_lock_t = mi_lock_s::new();
static mi_error_handler: LockedErrorHandler =
    LockedErrorHandler(UnsafeCell::new(ErrorCallback::none()));
static mi_error_arg: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

#[inline]
fn error_handler_load() -> ErrorCallback {
    // SAFETY: allocation and error failures invoke registered callbacks or the default abort policy.
    unsafe {
        let lock =
            MiLockPtr::from_nonnull(NonNull::new_unchecked(addr_of_mut!(mi_error_handler_lock)));
        let _lock = MiLockGuard::acquire(lock);
        *mi_error_handler.0.get()
    }
}

#[inline]
fn error_handler_store(fun: ErrorCallback) {
    // SAFETY: allocation and error failures invoke registered callbacks or the default abort policy.
    unsafe {
        let lock =
            MiLockPtr::from_nonnull(NonNull::new_unchecked(addr_of_mut!(mi_error_handler_lock)));
        let _lock = MiLockGuard::acquire(lock);
        *mi_error_handler.0.get() = fun;
    }
}

#[inline]
pub(crate) fn registered_error_callback() -> (Option<mi_error_fun>, *mut c_void) {
    (
        error_handler_load().raw(),
        mi_error_arg.load(Ordering::Acquire),
    )
}

#[inline]
pub(crate) fn restore_error_callback(snapshot: (Option<mi_error_fun>, *mut c_void)) {
    let (fun, arg) = snapshot;
    error_handler_store(ErrorCallback::new(fun));
    mi_error_arg.store(arg, Ordering::Release);
}

#[inline]
fn mi_output_is_c_file(out: mi_output_fun) -> bool {
    let p = OutputCallback::new(Some(out)).storage_ptr();
    #[cfg(all(target_os = "linux", not(target_env = "musl")))]
    {
        // SAFETY: C stdout/stderr symbol comparisons stay behind platform-gated libc file handles.
        p == unsafe { C_STDOUT } || p == unsafe { C_STDERR }
    }
    #[cfg(not(all(target_os = "linux", not(target_env = "musl"))))]
    {
        let _ = p;
        false
    }
}

// SAFETY: stderr output validates C strings before forwarding to the primitive backend.
unsafe extern "C" fn mi_out_stderr(msg: *const c_char, arg: *mut c_void) {
    let _ = arg;
    // SAFETY: stderr output validates C strings before forwarding to the primitive backend.
    if !msg.is_null() && unsafe { *msg } != 0 {
        // SAFETY: stderr output validates C strings before forwarding to the primitive backend.
        unsafe { _mi_prim_out_stderr(msg) };
    }
}

// SAFETY: delayed output buffering uses a bounded static buffer and mimalloc lock discipline.
unsafe extern "C" fn mi_out_buf(msg: *const c_char, arg: *mut c_void) {
    let _ = arg;
    if msg.is_null() || out_len.load(Ordering::Acquire) >= MI_MAX_DELAY_OUTPUT {
        return;
    }

    // SAFETY: delayed output buffering uses a bounded static buffer and mimalloc lock discipline.
    let msg_bytes = unsafe { CStr::from_ptr(msg).to_bytes() };
    let mut n = msg_bytes.len();
    if n == 0 || n >= MI_MAX_DELAY_OUTPUT {
        return;
    }

    // Manual lock handling keeps this translated callback buffer path close to
    // upstream and makes the callback-holding lifetime explicit.
    // SAFETY: delayed output buffering uses a bounded static buffer and mimalloc lock discipline.
    unsafe { mi_lock_acquire(addr_of_mut!(out_buf_lock)) };
    let start = out_len.fetch_add(n, Ordering::AcqRel);
    if start < MI_MAX_DELAY_OUTPUT {
        if start + n >= MI_MAX_DELAY_OUTPUT {
            n = MI_MAX_DELAY_OUTPUT - start - 1;
        }
        // SAFETY: delayed output buffering uses a bounded static buffer and mimalloc lock discipline.
        let buf = unsafe { &mut *out_buf.0.get() };
        buf[start..start + n].copy_from_slice(&msg_bytes[..n]);
    }
    // SAFETY: delayed output buffering uses a bounded static buffer and mimalloc lock discipline.
    unsafe { mi_lock_release(addr_of_mut!(out_buf_lock)) };
}

fn mi_out_buf_flush(out: OutputCallback, no_more_buf: bool, arg: OutputCallbackArg) {
    // Intentionally holds the output buffer lock while invoking `out`, matching
    // the existing delayed-output flush semantics.
    // SAFETY: delayed output buffering uses a bounded static buffer and mimalloc lock discipline.
    unsafe { mi_lock_acquire(addr_of_mut!(out_buf_lock)) };
    let mut count = out_len.fetch_add(
        if no_more_buf { MI_MAX_DELAY_OUTPUT } else { 1 },
        Ordering::AcqRel,
    );
    if count > MI_MAX_DELAY_OUTPUT {
        count = MI_MAX_DELAY_OUTPUT;
    }
    // SAFETY: delayed output buffering uses a bounded static buffer and mimalloc lock discipline.
    let buf = unsafe { &mut *out_buf.0.get() };
    buf[count] = 0;
    // SAFETY: delayed output buffering uses a bounded static buffer and mimalloc lock discipline.
    unsafe { out.invoke(buf.as_ptr().cast::<c_char>(), arg) };
    if !no_more_buf {
        buf[count] = b'\n';
    }
    // SAFETY: delayed output buffering uses a bounded static buffer and mimalloc lock discipline.
    unsafe { mi_lock_release(addr_of_mut!(out_buf_lock)) };
}

// SAFETY: delayed output buffering uses a bounded static buffer and mimalloc lock discipline.
unsafe extern "C" fn mi_out_buf_stderr(msg: *const c_char, arg: *mut c_void) {
    // SAFETY: stderr output validates C strings before forwarding to the primitive backend.
    unsafe { mi_out_stderr(msg, arg) };
    // SAFETY: delayed output buffering uses a bounded static buffer and mimalloc lock discipline.
    unsafe { mi_out_buf(msg, arg) };
}

fn mi_out_get_default(parg: Option<&mut OutputCallbackArg>) -> OutputCallback {
    if let Some(arg) = parg {
        *arg = OutputCallbackArg::from_raw(mi_out_arg.load(Ordering::Acquire));
    }
    let callback =
        // SAFETY: registered output callbacks are loaded, stored, and invoked through typed callback wrappers.
        unsafe { OutputCallback::from_storage_ptr(mi_out_default.load(Ordering::Acquire)) };
    OutputCallback::new(callback.raw().or(Some(mi_out_buf)))
}

fn mi_add_stderr_output() {
    if !mi_out_default.load(Ordering::Acquire).is_null() {
        return;
    }
    mi_out_buf_flush(
        OutputCallback::new(Some(mi_out_stderr)),
        false,
        OutputCallbackArg::null(),
    );
    mi_out_default.store(
        OutputCallback::new(Some(mi_out_buf_stderr)).storage_ptr(),
        Ordering::Release,
    );
    mi_out_arg.store(ptr::null_mut(), Ordering::Release);
}

#[inline(never)]
fn mi_recurse_enter_prim() -> bool {
    #[cfg(all(target_os = "linux", any(feature = "tls-shim", not(feature = "std"))))]
    // SAFETY: diagnostic recursion guards use TLS or thread-local state before callback dispatch.
    unsafe {
        if _mi_tls_recurse_get() != 0 {
            false
        } else {
            _mi_tls_recurse_set(1);
            true
        }
    }
    #[cfg(any(
        not(target_os = "linux"),
        all(target_os = "linux", feature = "std", not(feature = "tls-shim"))
    ))]
    {
        recurse.with(|slot| {
            if slot.get() {
                false
            } else {
                slot.set(true);
                true
            }
        })
    }
}

#[used]
static MI_RECURSE_ENTER_PRIM_KEEP: fn() -> bool = mi_recurse_enter_prim;

#[inline]
fn mi_recurse_enter() -> bool {
    if cfg!(any(target_vendor = "apple", target_os = "android")) && _mi_preloading() {
        return false;
    }
    mi_recurse_enter_prim()
}

#[inline(never)]
fn mi_recurse_exit_prim() {
    #[cfg(all(target_os = "linux", any(feature = "tls-shim", not(feature = "std"))))]
    // SAFETY: diagnostic recursion guards use TLS or thread-local state before callback dispatch.
    unsafe {
        _mi_tls_recurse_set(0);
    }
    #[cfg(any(
        not(target_os = "linux"),
        all(target_os = "linux", feature = "std", not(feature = "tls-shim"))
    ))]
    {
        recurse.with(|slot| slot.set(false));
    }
}

#[used]
static MI_RECURSE_EXIT_PRIM_KEEP: fn() = mi_recurse_exit_prim;

#[inline]
fn mi_recurse_exit() {
    if cfg!(any(target_vendor = "apple", target_os = "android")) && _mi_preloading() {
        return;
    }
    mi_recurse_exit_prim();
}

/// # Safety
///
/// `prefix` and `message`, when non-null, must point to valid nul-terminated
/// C strings. If `out` is supplied, it must be callable with `arg`.
// SAFETY: registered output callbacks are loaded, stored, and invoked through typed callback wrappers.
#[inline(never)]
pub(crate) unsafe extern "C" fn _mi_fputs(
    out: Option<mi_output_fun>,
    arg: *mut c_void,
    prefix: *const c_char,
    message: *const c_char,
) {
    let use_default = match out {
        None => true,
        Some(out) => mi_output_is_c_file(out),
    };
    if use_default {
        if !mi_recurse_enter() {
            return;
        }
        let mut default_arg = OutputCallbackArg::null();
        let default_out = mi_out_get_default(Some(&mut default_arg));
        if !prefix.is_null() {
            // SAFETY: registered output callbacks are loaded, stored, and invoked through typed callback wrappers.
            unsafe { default_out.invoke(prefix, default_arg) };
        }
        // SAFETY: registered output callbacks are loaded, stored, and invoked through typed callback wrappers.
        unsafe { default_out.invoke(message, default_arg) };
        mi_recurse_exit();
    } else if let Some(out) = out {
        let out = OutputCallback::new(Some(out));
        let arg = OutputCallbackArg::from_raw(arg);
        if !prefix.is_null() {
            // SAFETY: registered output callbacks are loaded, stored, and invoked through typed callback wrappers.
            unsafe { out.invoke(prefix, arg) };
        }
        // SAFETY: registered output callbacks are loaded, stored, and invoked through typed callback wrappers.
        unsafe { out.invoke(message, arg) };
    }
}

pub(crate) fn _mi_fprintf(out: Option<mi_output_fun>, arg: *mut c_void, args: fmt::Arguments<'_>) {
    let mut text = StackMessage::new();
    if fmt::write(&mut text, args).is_ok() {
        // SAFETY: formatted diagnostic output uses bounded stack buffers and checked C-string prefixes.
        unsafe { _mi_fputs(out, arg, ptr::null(), text.as_c_ptr()) };
    }
}

struct StackMessage {
    bytes: [u8; 1024],
    len: usize,
}

impl StackMessage {
    fn new() -> Self {
        Self {
            bytes: [0; 1024],
            len: 0,
        }
    }

    fn as_c_ptr(&mut self) -> *const c_char {
        self.bytes[self.len] = 0;
        self.bytes.as_ptr().cast()
    }

    fn write_bytes(&mut self, bytes: &[u8]) {
        let capacity = self.bytes.len() - 1;
        if self.len >= capacity {
            return;
        }
        let available = capacity - self.len;
        let copy = bytes.len().min(available);
        self.bytes[self.len..self.len + copy].copy_from_slice(&bytes[..copy]);
        self.len += copy;
    }
}

impl fmt::Write for StackMessage {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_bytes(s.as_bytes());
        Ok(())
    }
}

fn mi_vfprintf(
    out: Option<mi_output_fun>,
    arg: *mut c_void,
    prefix: *const c_char,
    args: fmt::Arguments<'_>,
) {
    let mut text = StackMessage::new();
    if !mi_recurse_enter() {
        return;
    }
    let wrote = text.write_fmt(args).is_ok();
    mi_recurse_exit();
    if !wrote {
        return;
    }
    // SAFETY: formatted diagnostic output uses bounded stack buffers and checked C-string prefixes.
    unsafe { _mi_fputs(out, arg, prefix, text.as_c_ptr()) };
}

fn mi_vfprintf_thread(
    out: Option<mi_output_fun>,
    arg: *mut c_void,
    prefix: *const c_char,
    args: fmt::Arguments<'_>,
) {
    if !prefix.is_null() {
        // SAFETY: formatted diagnostic output uses bounded stack buffers and checked C-string prefixes.
        let prefix_len = unsafe { _mi_strnlen(prefix, 33) };
        if prefix_len <= 32 && !_mi_is_main_thread() {
            let mut tprefix = StackMessage::new();
            let prefix_bytes =
                // SAFETY: formatted diagnostic output uses bounded stack buffers and checked C-string prefixes.
                unsafe { core::slice::from_raw_parts(prefix.cast::<u8>(), prefix_len) };
            tprefix.write_bytes(prefix_bytes);
            let _ = write!(tprefix, "thread 0x{:x}: ", _mi_thread_id() as usize);
            mi_vfprintf(out, arg, tprefix.as_c_ptr(), args);
            return;
        }
    }
    mi_vfprintf(out, arg, prefix, args);
}

fn mi_vfputs_thread(
    out: Option<mi_output_fun>,
    arg: *mut c_void,
    prefix: *const c_char,
    message: *const c_char,
) {
    if message.is_null() {
        return;
    }
    if !prefix.is_null() {
        // SAFETY: formatted diagnostic output uses bounded stack buffers and checked C-string prefixes.
        let prefix_len = unsafe { _mi_strnlen(prefix, 33) };
        if prefix_len <= 32 && !_mi_is_main_thread() {
            let mut tprefix = StackMessage::new();
            let prefix_bytes =
                // SAFETY: formatted diagnostic output uses bounded stack buffers and checked C-string prefixes.
                unsafe { core::slice::from_raw_parts(prefix.cast::<u8>(), prefix_len) };
            tprefix.write_bytes(prefix_bytes);
            let _ = write!(tprefix, "thread 0x{:x}: ", _mi_thread_id() as usize);
            // SAFETY: formatted diagnostic output uses bounded stack buffers and checked C-string prefixes.
            unsafe { _mi_fputs(out, arg, tprefix.as_c_ptr(), message) };
            return;
        }
    }
    // SAFETY: formatted diagnostic output uses bounded stack buffers and checked C-string prefixes.
    unsafe { _mi_fputs(out, arg, prefix, message) };
}

#[inline(never)]
pub(crate) fn _mi_raw_message(args: fmt::Arguments<'_>) {
    mi_vfprintf(None, ptr::null_mut(), ptr::null(), args);
}

#[inline(always)]
pub(super) fn message_impl(args: fmt::Arguments<'_>) {
    mi_vfprintf_thread(None, ptr::null_mut(), c"mimalloc: ".as_ptr(), args);
}

#[used]
static MI_RAW_MESSAGE_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_raw_message;

#[inline(never)]
pub(crate) fn _mi_trace_message(args: fmt::Arguments<'_>) {
    if option_get(mi_option_t::mi_option_verbose) <= 1 {
        return;
    }
    mi_vfprintf_thread(None, ptr::null_mut(), c"mimalloc: ".as_ptr(), args);
}

#[inline(never)]
pub(crate) fn _mi_verbose_message(args: fmt::Arguments<'_>) {
    if !option_is_enabled(mi_option_t::mi_option_verbose) {
        return;
    }
    mi_vfprintf(None, ptr::null_mut(), c"mimalloc: ".as_ptr(), args);
}

#[used]
static MI_TRACE_MESSAGE_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_trace_message;
#[used]
static MI_VERBOSE_MESSAGE_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_verbose_message;

fn mi_show_error_message(args: fmt::Arguments<'_>) {
    if !option_is_enabled(mi_option_t::mi_option_verbose) {
        if !option_is_enabled(mi_option_t::mi_option_show_errors) {
            return;
        }
        let max_errors = mi_max_error_count.load(Ordering::Acquire);
        let count = error_count.fetch_add(1, Ordering::AcqRel);
        if max_errors >= 0 && (count as isize) > max_errors {
            return;
        }
    }
    mi_vfprintf_thread(None, ptr::null_mut(), c"mimalloc: error: ".as_ptr(), args);
}

fn mi_show_error_message_raw(message: *const c_char) {
    if !option_is_enabled(mi_option_t::mi_option_verbose) {
        if !option_is_enabled(mi_option_t::mi_option_show_errors) {
            return;
        }
        let max_errors = mi_max_error_count.load(Ordering::Acquire);
        let count = error_count.fetch_add(1, Ordering::AcqRel);
        if max_errors >= 0 && (count as isize) > max_errors {
            return;
        }
    }
    mi_vfputs_thread(
        None,
        ptr::null_mut(),
        c"mimalloc: error: ".as_ptr(),
        message,
    );
}

#[inline(never)]
pub(crate) fn _mi_warning_message_fmt(args: fmt::Arguments<'_>) {
    if !option_is_enabled(mi_option_t::mi_option_verbose) {
        if !option_is_enabled(mi_option_t::mi_option_show_errors) {
            return;
        }
        let max_warnings = mi_max_warning_count.load(Ordering::Acquire);
        let count = warning_count.fetch_add(1, Ordering::AcqRel);
        if max_warnings >= 0 && (count as isize) > max_warnings {
            return;
        }
    }
    mi_vfprintf_thread(None, ptr::null_mut(), c"mimalloc: warning: ".as_ptr(), args);
}

#[inline(never)]
pub(super) unsafe fn warning_message(fmt: *const c_char) {
    if !option_is_enabled(mi_option_t::mi_option_verbose) {
        if !option_is_enabled(mi_option_t::mi_option_show_errors) {
            return;
        }
        let max_warnings = mi_max_warning_count.load(Ordering::Acquire);
        let count = warning_count.fetch_add(1, Ordering::AcqRel);
        if max_warnings >= 0 && (count as isize) > max_warnings {
            return;
        }
    }
    mi_vfputs_thread(None, ptr::null_mut(), c"mimalloc: warning: ".as_ptr(), fmt);
}

#[inline(never)]
pub(crate) fn _mi_error_message_fmt(err: Errno, args: fmt::Arguments<'_>) {
    mi_show_error_message(args);
    let handler = error_handler_load();
    let arg = ErrorCallbackArg::from_raw(mi_error_arg.load(Ordering::Acquire));
    if handler.is_some() {
        // SAFETY: allocation and error failures invoke registered callbacks or the default abort policy.
        unsafe { handler.invoke(err.get(), arg) };
    } else {
        mi_error_default(err);
    }
}

#[inline(never)]
pub(super) unsafe fn error_message(err: c_int, fmt: *const c_char) {
    let err = Errno::new(err);
    mi_show_error_message_raw(fmt);
    let handler = error_handler_load();
    let arg = ErrorCallbackArg::from_raw(mi_error_arg.load(Ordering::Acquire));
    if handler.is_some() {
        // SAFETY: allocation and error failures invoke registered callbacks or the default abort policy.
        unsafe { handler.invoke(err.get(), arg) };
    } else {
        mi_error_default(err);
    }
}

#[used]
static MI_WARNING_MESSAGE_FMT_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_warning_message_fmt;
#[used]
static MI_ERROR_MESSAGE_FMT_KEEP: for<'a> fn(Errno, fmt::Arguments<'a>) = _mi_error_message_fmt;

pub(super) fn mi_error_default(err: Errno) {
    if err == Errno::EFAULT && (MI_DEBUG > 0 || cfg!(feature = "secure")) {
        // SAFETY: allocation and error failures invoke registered callbacks or the default abort policy.
        unsafe { c_abort() };
    }
}

// SAFETY: registered output callbacks are loaded, stored, and invoked through typed callback wrappers.
pub(super) fn mi_register_output(out: Option<mi_output_fun>, arg: *mut c_void) {
    let callback = OutputCallback::new(Some(out.unwrap_or(mi_out_stderr)));
    let arg = OutputCallbackArg::from_raw(arg);
    mi_out_default.store(callback.storage_ptr(), Ordering::Release);
    mi_out_arg.store(arg.as_ptr(), Ordering::Release);
    if let Some(out) = out {
        mi_out_buf_flush(OutputCallback::new(Some(out)), true, arg);
    }
}

// SAFETY: allocation and error failures invoke registered callbacks or the default abort policy.
pub(super) fn mi_register_error(fun: Option<mi_error_fun>, arg: *mut c_void) {
    let callback = ErrorCallback::new(fun);
    let arg = ErrorCallbackArg::from_raw(arg);
    error_handler_store(callback);
    mi_error_arg.store(arg.as_ptr(), Ordering::Release);
}

// SAFETY: option printing formats static option names and values through the registered output path.
pub(super) fn mi_options_print_out(out: Option<mi_output_fun>, arg: *mut c_void) {
    let (out, arg) = if let Some(out) = out {
        (out, arg)
    } else {
        let mut default_arg = OutputCallbackArg::null();
        let default_out = mi_out_get_default(Some(&mut default_arg));
        (
            default_out.raw().unwrap_or(mi_out_buf),
            default_arg.as_ptr(),
        )
    };

    let vermajor = MI_MALLOC_VERSION / 10000;
    let verminor = (MI_MALLOC_VERSION % 10000) / 100;
    let verpatch = MI_MALLOC_VERSION % 100;
    _mi_fprintf(
        Some(out),
        arg,
        format_args!("v{vermajor}.{verminor}.{verpatch} (Rust translation)\n"),
    );

    for i in 0..storage::option_count() {
        let option = storage::option_at(i);
        let value = option_get(option);
        let desc = storage::option_descriptor(i);
        // SAFETY: option names are static ASCII strings from the translated option table.
        // SAFETY: option printing formats static option names and values through the registered output path.
        let name = unsafe { core::str::from_utf8_unchecked(CStr::from_ptr(desc.name).to_bytes()) };
        _mi_fprintf(
            Some(out),
            arg,
            format_args!(
                "option '{name}': {value} {}\n",
                if storage::option_uses_kib(option) {
                    "KiB"
                } else {
                    ""
                }
            ),
        );
    }

    _mi_fprintf(
        Some(out),
        arg,
        format_args!(
            "debug level : {}\n",
            if cfg!(debug_assertions) { 2 } else { 0 }
        ),
    );
    _mi_fprintf(Some(out), arg, format_args!("secure level: 0\n"));
    _mi_fprintf(Some(out), arg, format_args!("mem tracking: false\n"));
    if option_get(mi_option_t::mi_option_guarded_sample_rate) != 0 {
        _mi_fprintf(Some(out), arg, format_args!("guarded build: enabled\n"));
    } else {
        _mi_fprintf(Some(out), arg, format_args!("guarded build: disabled\n"));
    }
}

// SAFETY: option printing formats static option names and values through the registered output path.
pub(super) fn mi_options_print() {
    mi_options_print_out(None, ptr::null_mut());
}

/// # Safety
///
/// `assertion`, `fname`, and `func`, when non-null, must point to valid
/// nul-terminated C strings. This function aborts and does not return.
// SAFETY: assertion failure shims read caller C strings, emit diagnostics, and abort.
#[cold]
pub(super) unsafe fn mi_assert_fail(
    assertion: *const c_char,
    fname: *const c_char,
    line: u32,
    func: *const c_char,
) -> ! {
    let assertion = if assertion.is_null() {
        ""
    } else {
        // SAFETY: assertion failure shims read caller C strings, emit diagnostics, and abort.
        unsafe { CStr::from_ptr(assertion).to_str().unwrap_or("") }
    };
    let fname = if fname.is_null() {
        ""
    } else {
        // SAFETY: assertion failure shims read caller C strings, emit diagnostics, and abort.
        unsafe { CStr::from_ptr(fname).to_str().unwrap_or("") }
    };
    let func = if func.is_null() {
        ""
    } else {
        // SAFETY: assertion failure shims read caller C strings, emit diagnostics, and abort.
        unsafe { CStr::from_ptr(func).to_str().unwrap_or("") }
    };
    // SAFETY: assertion failure shims read caller C strings, emit diagnostics, and abort.
    unsafe {
        // SAFETY: assertion failure shims read caller C strings, emit diagnostics, and abort.
        unsafe extern "C" {
            fn dprintf(
                fd: core::ffi::c_int,
                format: *const core::ffi::c_char,
                ...
            ) -> core::ffi::c_int;
        }
        dprintf(
            2,
            c"mimalloc PANIC or ASSERT FAILED: at %s:%d, %s\n  assertion: %s\n".as_ptr(),
            if fname.is_empty() {
                c"".as_ptr()
            } else {
                fname.as_ptr() as *const _
            },
            line,
            if func.is_empty() {
                c"".as_ptr()
            } else {
                func.as_ptr() as *const _
            },
            if assertion.is_empty() {
                c"".as_ptr()
            } else {
                assertion.as_ptr() as *const _
            },
        );
    }
    _mi_fprintf(
        None,
        ptr::null_mut(),
        format_args!(
            "mimalloc: assertion failed: at \"{}\":{}, {}\n  assertion: \"{}\"\n",
            fname, line, func, assertion
        ),
    );
    // SAFETY: assertion failure shims read caller C strings, emit diagnostics, and abort.
    unsafe { c_abort() };
}

pub(super) fn configure_error_limits(max_errors: c_long, max_warnings: c_long) {
    mi_max_error_count.store(max_errors as isize, Ordering::Release);
    mi_max_warning_count.store(max_warnings as isize, Ordering::Release);
    error_count.store(0, Ordering::Release);
    warning_count.store(0, Ordering::Release);
}

pub(super) fn add_stderr_output() {
    mi_add_stderr_output();
}
