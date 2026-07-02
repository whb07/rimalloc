use super::*;

#[path = "primitive_options/stats.rs"]
mod stats;

#[test]
fn stderr_output_routes_through_platform_primitive() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let options_rs = fs::read_to_string(root.join("src/allocator/options.rs"))
        .expect("read src/allocator/options.rs");
    let os_rs =
        fs::read_to_string(root.join("src/platform/os.rs")).expect("read src/platform/os.rs");
    let unix_rs =
        fs::read_to_string(root.join("src/platform/unix.rs")).expect("read src/platform/unix.rs");

    assert!(
        options_rs.contains("use crate::prim::_mi_prim_out_stderr;")
            && !options_rs.contains("use crate::os::_mi_prim_out_stderr;"),
        "default stderr output should route through the platform primitive layer"
    );
    assert!(
        unix_rs.contains("fn write(fd: c_int, buf: *const c_void, count: usize) -> ssize_t;")
            && unix_rs.contains("let n = unsafe {\n            write(")
            && unix_rs.contains("2,\n                bytes.as_ptr().add(written)")
            && !unix_rs.contains("use std::io")
            && !unix_rs.contains("use std::ffi::CString")
            && !unix_rs.contains("format!(\"/sys/devices/system/node/node{}\"")
            && unix_rs.contains("fn mi_prim_numa_node_path(node: usize, buf: &mut [c_char; 128])")
            && unix_rs.contains("let mut buf = [0 as c_char; 128];")
            && !unix_rs.contains("stderr().lock()")
            && !os_rs.contains("fn _mi_prim_out_stderr")
            && !os_rs.contains("fn _mi_prim_process_info")
            && !os_rs.contains("use std::io"),
        "Unix primitive startup/output paths should use stack buffers and direct syscalls, matching C"
    );
}

#[test]
fn options_state_uses_internal_locks_like_c() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let unsafe_allowlist = fs::read_to_string(root.join("docs/unsafe-migration-allowlist.tsv"))
        .expect("read unsafe migration allowlist");
    let alloc_rs = fs::read_to_string(root.join("src/allocator/alloc.rs"))
        .expect("read src/allocator/alloc.rs");
    let atomic_rs = fs::read_to_string(root.join("src/internal/atomic.rs"))
        .expect("read src/internal/atomic.rs");
    let free_rs =
        fs::read_to_string(root.join("src/allocator/free.rs")).expect("read src/allocator/free.rs");
    let init_rs =
        fs::read_to_string(root.join("src/allocator/init.rs")).expect("read src/allocator/init.rs");
    let libc_rs = fs::read_to_string(root.join("src/ffi/libc.rs")).expect("read src/ffi/libc.rs");
    let options_rs = fs::read_to_string(root.join("src/allocator/options.rs"))
        .expect("read src/allocator/options.rs");
    let prim_rs =
        fs::read_to_string(root.join("src/platform/mod.rs")).expect("read src/platform/mod.rs");
    let random_rs = fs::read_to_string(root.join("src/internal/random.rs"))
        .expect("read src/internal/random.rs");
    let stats_rs = fs::read_to_string(root.join("src/allocator/stats.rs"))
        .expect("read src/allocator/stats.rs");

    assert!(
        !options_rs.contains("std::sync::{Mutex")
            && !options_rs.contains("Mutex<")
            && !options_rs.contains("MutexGuard")
            && !options_rs.contains("std::process::abort")
            && !options_rs.contains("eprintln!")
            && !random_rs.contains("eprintln!"),
        "options/random state should not pull in std mutex/abort/stdio machinery"
    );
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/internal/atomic.rs\t"))
            && !atomic_rs.contains("SAFETY[AUDIT:internal-atomic-facade]")
            && atomic_rs.contains("fn load_order(order: mi_memory_order_e) -> Ordering")
            && atomic_rs.contains("fn store_order(order: mi_memory_order_e) -> Ordering")
            && atomic_rs.contains("fn rmw_order(order: mi_memory_order_e) -> Ordering")
            && atomic_rs.contains("fn cas_failure_order(order: mi_memory_order_e) -> Ordering")
            && atomic_rs
                .contains("fn pthread_mutex_trylock(mutex: *mut pthread_mutex_t) -> c_int;")
            && atomic_rs.contains("pub unsafe extern \"C\" fn mi_lock_try_acquire")
            && atomic_rs.contains("pub unsafe extern \"C\" fn _mi_atomic_once_enter")
            && atomic_rs.contains("pub unsafe extern \"C\" fn _mi_atomic_once_release"),
        "internal atomic facade should stay covered by local operation markers without a migration allowlist entry"
    );
    assert!(
        !atomic_rs.contains("SAFETY[AUDIT:internal-atomic-field-boundary]"),
        "internal atomic field boundary should be split into focused operation IDs"
    );
    for (marker, expected_count) in [
        ("internal-atomic-raw-view-boundary", 5),
        ("internal-atomic-load-store-boundary", 12),
        ("internal-atomic-fetch-boundary", 22),
        ("internal-atomic-cas-boundary", 17),
        ("internal-atomic-exchange-boundary", 4),
        ("internal-atomic-max-boundary", 2),
        ("internal-atomic-volatile-delta-boundary", 3),
        ("internal-atomic-pointer-boundary", 6),
        ("internal-atomic-guard-once-boundary", 12),
        ("internal-atomic-platform-lock-boundary", 20),
    ] {
        let needle = format!("SAFETY[AUDIT:{marker}]");
        let count = atomic_rs.matches(needle.as_str()).count();
        assert_eq!(
            count, expected_count,
            "internal atomic audit marker `{marker}` should have exactly {expected_count} source markers"
        );
    }
    assert!(
        !init_rs.contains("std::sync::Once")
            && !init_rs.contains(".call_once(")
            && !random_rs.contains("OnceLock")
            && !random_rs.contains("Instant")
            && !stats_rs.contains("OnceLock")
            && !stats_rs.contains("Instant")
            && init_rs.contains("static PROCESS_INIT: mi_atomic_once_t")
            && init_rs.contains("static TLS_SLOTS_INIT: mi_atomic_once_t")
            && !init_rs.contains("static SUBPROC_MAIN_INIT")
            && !init_rs.contains("MaybeUninit")
            && init_rs.contains("mi_atomic_do_once_enter(ptr::addr_of!(PROCESS_INIT)")
            && init_rs.contains("mi_atomic_do_once_enter(ptr::addr_of!(TLS_SLOTS_INIT)")
            && prim_rs.contains("_mi_prim_alloc_huge_os_pages, _mi_prim_clock_now")
            && random_rs.contains("unsafe { crate::prim::_mi_prim_clock_now() }")
            && stats_rs.contains("_mi_heap_theap_peek, _mi_prim_clock_now")
            && stats_rs.contains("unsafe { _mi_prim_clock_now() }"),
        "process/TLS initialization and clocks should use mimalloc's internal primitives"
    );
    assert!(
        !libc_rs.contains("let mut rendered = String::new()")
            && !libc_rs.contains("rendered.as_bytes()")
            && libc_rs.contains("struct FixedBuffer")
            && libc_rs.contains("impl fmt::Write for FixedBuffer")
            && libc_rs.contains("ptr::copy_nonoverlapping(s.as_ptr(), self.ptr.add(self.used), n)")
            && libc_rs.contains("*out.ptr.add(out.used) = 0;")
            && libc_rs.contains("#[cfg(unix)]\nunsafe extern \"C\"")
            && libc_rs.contains("fn vsnprintf(")
            && libc_rs.contains("#[cfg(not(unix))]"),
        "Rust-side bounded snprintf formatting should write directly into the caller buffer"
    );
    assert!(
        !unsafe_allowlist.contains("src/ffi/libc.rs")
            && !libc_rs.contains("SAFETY[AUDIT:ffi-libc-helpers]"),
        "libc helper unsafe boundaries should stay covered by local operation markers without a migration allowlist entry"
    );
    for (audit_id, min_count) in [
        ("ffi-libc-string-boundary", 27),
        ("ffi-libc-env-boundary", 2),
        ("ffi-libc-output-boundary", 5),
        ("ffi-libc-format-boundary", 8),
        ("ffi-libc-retention-boundary", 8),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert!(
            libc_rs.matches(&marker).count() >= min_count,
            "libc helper boundary {audit_id} should retain at least {min_count} local operation markers"
        );
    }
    stats::assert_stats_format_and_markers(&unsafe_allowlist, &stats_rs);
    assert!(
        options_rs.contains("static mut options_lock: mi_lock_t")
            && options_rs.contains("static mut mi_error_handler_lock: mi_lock_t")
            && options_rs.contains(
                "let lock = MiLockPtr(NonNull::new_unchecked(addr_of_mut!(options_lock)));"
            )
            && options_rs.contains(
                "let lock = MiLockPtr(NonNull::new_unchecked(addr_of_mut!(mi_error_handler_lock)));"
            )
            && options_rs.contains("let _lock = MiLockGuard::acquire(lock);")
            && !options_rs.contains("mi_lock_acquire(addr_of_mut!(options_lock))")
            && !options_rs.contains("mi_lock_release(addr_of_mut!(options_lock))")
            && !options_rs.contains("mi_lock_acquire(addr_of_mut!(mi_error_handler_lock))")
            && !options_rs.contains("mi_lock_release(addr_of_mut!(mi_error_handler_lock))")
            && options_rs.contains("#[link_name = \"abort\"]"),
        "options state should use typed mimalloc lock guards and C abort primitives"
    );
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/options.rs\t"))
            && !options_rs.contains("SAFETY[AUDIT:options-callback-boundary]")
            && !options_rs.contains("SAFETY[AUDIT:options-message-output-boundary]"),
        "options/callback unsafe boundaries should stay split without a migration allowlist entry"
    );
    for (audit_id, min_count) in [
        ("options-storage-lock-boundary", 15),
        ("options-libc-file-symbol-boundary", 2),
        ("options-recurse-guard-boundary", 3),
        ("options-delayed-output-buffer-boundary", 12),
        ("options-stderr-output-boundary", 4),
        ("options-output-callback-boundary", 7),
        ("options-format-output-boundary", 8),
        ("options-print-output-boundary", 3),
        ("options-warning-error-callback-boundary", 1),
        ("options-allocation-failure-callback-boundary", 10),
        ("options-assertion-boundary", 8),
    ] {
        let marker = format!("SAFETY[AUDIT:{audit_id}]");
        assert!(
            options_rs.matches(&marker).count() >= min_count,
            "options unsafe boundary {audit_id} should retain at least {min_count} local operation markers"
        );
    }
    assert!(
        atomic_rs.contains(
            "#[cfg(all(target_os = \"linux\", not(debug_assertions)))]\n#[link(name = \"pthread\")]"
        ) && atomic_rs.contains("fn pthread_mutex_trylock(mutex: *mut pthread_mutex_t)")
            && atomic_rs.contains("fn pthread_mutex_lock(mutex: *mut pthread_mutex_t)")
            && atomic_rs.contains("fn pthread_mutex_unlock(mutex: *mut pthread_mutex_t)")
            && atomic_rs.contains("fn pthread_mutex_destroy(mutex: *mut pthread_mutex_t)")
            && atomic_rs.contains(
                "unsafe { pthread_mutex_trylock(ptr::addr_of_mut!((*lock).mutex)) == 0 }"
            )
            && atomic_rs.contains(
                "let err = unsafe { pthread_mutex_lock(ptr::addr_of_mut!((*lock).mutex)) };"
            )
            && atomic_rs
                .contains("unsafe { pthread_mutex_unlock(ptr::addr_of_mut!((*lock).mutex)) };")
            && atomic_rs
                .contains("unsafe { pthread_mutex_destroy(ptr::addr_of_mut!((*lock).mutex)) };")
            && atomic_rs.contains("#[cfg(not(all(target_os = \"linux\", not(debug_assertions))))]")
            && atomic_rs.contains("pub unsafe extern \"C\" fn mi_lock_acquire")
            && atomic_rs.contains("while unsafe { !mi_lock_try_acquire(lock) }"),
        "optimized Linux locks should use the C MI_USE_PTHREADS pthread_mutex_t branch, with the atomic fallback kept for debug/test binaries"
    );
    assert!(
        !options_rs
            .contains("&format!(\"v{vermajor}.{verminor}.{verpatch} (Rust translation)\\n\")")
            && !options_rs.contains("CStr::from_ptr(desc.name).to_string_lossy().into_owned()")
            && !options_rs
                .contains("&format!(\n                \"option '{name}': {value} {}\\n\"")
            && !options_rs.contains("&format!(\n            \"debug level : {}\\n\"")
            && options_rs.contains(
                "format_args!(\"v{vermajor}.{verminor}.{verpatch} (Rust translation)\\n\")"
            )
            && options_rs.contains("CStr::from_ptr(desc.name).to_bytes()")
            && options_rs
                .contains("format_args!(\n                \"option '{name}': {value} {}\\n\"")
            && options_rs.contains("format_args!(\n            \"debug level : {}\\n\""),
        "options printing should use direct fixed-buffer fprintf formatting like C"
    );
    assert!(
        !options_rs
            .contains("let prefix_text = unsafe { CStr::from_ptr(prefix).to_string_lossy() };")
            && !options_rs.contains("\"{prefix_text}thread 0x{:x}: \"")
            && options_rs.contains("fn write_bytes(&mut self, bytes: &[u8])")
            && options_rs.contains("_mi_strnlen(prefix, 33)")
            && options_rs.contains("core::slice::from_raw_parts(prefix.cast::<u8>(), prefix_len)")
            && options_rs.contains("tprefix.write_bytes(prefix_bytes);")
            && options_rs
                .contains("write!(tprefix, \"thread 0x{:x}: \", _mi_thread_id() as usize)"),
        "message thread prefixes should use C-style bounded stack byte copying"
    );
    assert!(
        !options_rs.contains("CStr::from_ptr(fmt).to_string_lossy()")
            && !options_rs.contains("format_args!(\"{}\", CStr::from_ptr(fmt)")
            && options_rs.contains("fn mi_vfputs_thread(")
            && options_rs.contains("fn mi_show_error_message_raw(message: *const c_char)")
            && options_rs.contains(
                "mi_vfputs_thread(None, ptr::null_mut(), c\"mimalloc: warning: \".as_ptr(), fmt);"
            )
            && options_rs.contains("mi_show_error_message_raw(fmt);"),
        "C warning/error message shims should route raw C strings without lossy Rust decoding"
    );
    assert!(
        !alloc_rs.contains("CStr::from_ptr(msg).to_string_lossy()")
            && !free_rs.contains("CStr::from_ptr(msg).to_string_lossy()")
            && alloc_rs.contains("crate::free::mi_validate_pointer(p.cast_mut(), msg)")
            && free_rs.contains("core::str::from_utf8_unchecked(CStr::from_ptr(msg).to_bytes())"),
        "pointer validation messages should route through raw C message names without lossy Rust decoding"
    );
}
