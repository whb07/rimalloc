use super::*;

#[test]
fn options_state_uses_internal_locks_like_c() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let unsafe_allowlist = fs::read_to_string(root.join("docs/unsafe-migration-allowlist.tsv"))
        .expect("read unsafe migration allowlist");
    let atomic_rs = fs::read_to_string(root.join("src/internal/atomic.rs"))
        .expect("read src/internal/atomic.rs");
    let init_rs =
        fs::read_to_string(root.join("src/allocator/init.rs")).expect("read src/allocator/init.rs");
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
        ("internal-atomic-fetch-boundary", 14),
        ("internal-atomic-integer-fetch-boundary", 8),
        ("internal-atomic-cas-boundary", 13),
        ("internal-atomic-i64-cas-boundary", 4),
        ("internal-atomic-exchange-boundary", 4),
        ("internal-atomic-max-boundary", 2),
        ("internal-atomic-volatile-delta-boundary", 3),
        ("internal-atomic-pointer-boundary", 6),
        ("internal-atomic-guard-once-boundary", 12),
        ("internal-atomic-pthread-lock-boundary", 11),
        ("internal-atomic-fallback-lock-boundary", 9),
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
}
