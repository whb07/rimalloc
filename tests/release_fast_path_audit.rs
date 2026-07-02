use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

fn assert_not_no_mangle_item(source_name: &str, source: &str, internal_name: &str) {
    let Some(start) = source.find(internal_name) else {
        return;
    };
    let item_start = source[..start].rfind("\n\n").map_or(0, |idx| idx + 2);
    assert!(
        !source[item_start..start].contains("#[unsafe(no_mangle)]"),
        "{internal_name} in {source_name} should stay crate-internal instead of a dynamic export"
    );
}

fn source_item<'a>(source_name: &str, source: &'a str, marker: &str) -> &'a str {
    let start = source
        .find(marker)
        .unwrap_or_else(|| panic!("{source_name} should contain {marker}"));
    let body_start = source[start..]
        .find('{')
        .map(|idx| start + idx)
        .unwrap_or_else(|| panic!("{marker} should have a body in {source_name}"));
    let mut depth = 0usize;
    for (offset, byte) in source[body_start..].bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[start..=body_start + offset];
                }
            }
            _ => {}
        }
    }
    panic!("{marker} body should be balanced in {source_name}");
}

fn assert_no_callback_alloc_or_unwind(source_name: &str, marker: &str, item: &str) {
    for forbidden in [
        "Vec<",
        "Vec::",
        "Box<",
        "Box::",
        "String::",
        "format!",
        "to_string(",
        "to_owned(",
        "alloc::",
        "std::",
        "malloc(",
        "mi_malloc",
        "mi_zalloc",
        "mi_calloc",
        "mi_realloc",
        "_mi_meta_alloc",
        "_mi_meta_zalloc",
        "_mi_arenas_alloc",
    ] {
        assert!(
            !item.contains(forbidden),
            "{marker} in {source_name} should not allocate through {forbidden}"
        );
    }
    for forbidden in [
        "panic!",
        "unreachable!",
        "todo!",
        "unimplemented!",
        "assert!(",
        "debug_assert!(",
        "mi_assert_internal!",
        "mi_debug_assert!",
        "expect(",
        ".unwrap(",
        "catch_unwind",
        "resume_unwind",
    ] {
        assert!(
            !item.contains(forbidden),
            "{marker} in {source_name} should not panic or unwind through {forbidden}"
        );
    }
}

fn assert_no_rust_heap_constructs(source_name: &str, marker: &str, item: &str) {
    for forbidden in [
        "extern crate std",
        "use std",
        "std::",
        "extern crate alloc",
        "crate::alloc::",
        "::alloc::",
        "use alloc::",
        "Vec<",
        "Vec::",
        "Box<",
        "Box::",
        "String",
        "format!",
        "to_string(",
        "to_owned(",
        "_mi_meta_alloc",
        "_mi_meta_zalloc",
        "_mi_arenas_alloc",
    ] {
        assert!(
            !item.contains(forbidden),
            "{marker} in {source_name} should not allocate through {forbidden}"
        );
    }
}

fn collect_rs_sources(base: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    for entry in fs::read_dir(dir).expect("read source directory") {
        let entry = entry.expect("read source entry");
        let path = entry.path();
        if path.is_dir() {
            collect_rs_sources(base, &path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let rel = path
                .strip_prefix(base)
                .expect("source path under base")
                .to_string_lossy()
                .replace('\\', "/");
            let source = fs::read_to_string(&path).expect("read Rust source");
            out.push((rel, source));
        }
    }
}

fn dead_code_allowance_count(source: &str) -> usize {
    source
        .lines()
        .filter(|line| line.contains("dead_code"))
        .count()
}

fn module_dead_code_allowance_count(source: &str) -> usize {
    let mut count = 0;
    let mut in_module_allow = false;
    let mut has_dead_code = false;

    for line in source.lines() {
        if !in_module_allow && line.contains("#![allow") {
            in_module_allow = true;
            has_dead_code = line.contains("dead_code");
        } else if in_module_allow {
            has_dead_code |= line.contains("dead_code");
        }

        if in_module_allow && line.contains(']') {
            count += usize::from(has_dead_code);
            in_module_allow = false;
            has_dead_code = false;
        }
    }

    count
}

fn clippy_allowance_counts(sources: &[(String, String)]) -> BTreeMap<(&str, &str), usize> {
    let mut counts = BTreeMap::new();
    for (rel, source) in sources {
        for line in source.lines() {
            if !line.contains("allow(clippy::") && !line.contains("expect(clippy::") {
                continue;
            }
            let Some(start) = line.find("clippy::") else {
                continue;
            };
            let lint = &line[start + "clippy::".len()..];
            let lint = lint
                .split(|ch: char| ch == ')' || ch == ',' || ch.is_whitespace())
                .next()
                .expect("lint name after clippy::");
            *counts.entry((rel.as_str(), lint)).or_insert(0) += 1;
        }
    }
    counts
}

fn package_clippy_lints(manifest: &str) -> BTreeMap<&str, &str> {
    let section_start = manifest
        .find("[lints.clippy]")
        .expect("manifest has clippy lint section");
    let section = &manifest[section_start..];
    let section_end = section[1..]
        .find("\n[")
        .map_or(section.len(), |idx| idx + 1);
    let mut lints = BTreeMap::new();
    for line in section[..section_end].lines().skip(1) {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        lints.insert(name.trim(), value.trim());
    }
    lints
}

fn rust_allowance_counts<'a>(
    sources: &'a [(String, String)],
    lints: &'static [&'static str],
) -> BTreeMap<(&'a str, &'static str), usize> {
    let mut counts = BTreeMap::new();
    for (rel, source) in sources {
        for line in source.lines() {
            for &lint in lints {
                if line.contains(lint) {
                    *counts.entry((rel.as_str(), lint)).or_insert(0) += 1;
                }
            }
        }
    }
    counts
}

#[path = "release_fast_path_audit/datatype_acceptance.rs"]
mod datatype_acceptance;
#[path = "release_fast_path_audit/release_policy.rs"]
mod release_policy;
#[path = "release_fast_path_audit/runtime_gates.rs"]
mod runtime_gates;
#[path = "release_fast_path_audit/todo_backlog.rs"]
mod todo_backlog;
#[path = "release_fast_path_audit/unsafe_performance.rs"]
mod unsafe_performance;

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
            && atomic_rs
                .matches("SAFETY[AUDIT:internal-atomic-facade]")
                .count()
                >= 90
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
            && libc_rs.matches("SAFETY[AUDIT:ffi-libc-helpers]").count() >= 35,
        "libc helper unsafe boundaries should stay covered by local operation markers without a migration allowlist entry"
    );
    assert!(
        !stats_rs.contains("fn format_amount(mut n: i64, unit: i64) -> String")
            && !stats_rs.contains("format!(\"{n}   \"")
            && !stats_rs.contains("format!(\"{n}   {suffix:<3}\"")
            && !stats_rs.contains("format!(\"{magnitude}i{suffix}\"")
            && !stats_rs.contains("format!(\"{magnitude}{suffix}\"")
            && !stats_rs.contains("format!(\"{whole}.{frac1} {unitdesc:<3}\"")
            && !stats_rs.contains("let label = format!(\"bin{pagekind:>2}  {i:>3}\");")
            && !stats_rs.contains("String::new()")
            && !stats_rs.contains("to_string_lossy")
            && stats_rs.contains("fn format_amount(buf: &mut [c_char; 32], mut n: i64, unit: i64)")
            && stats_rs.contains("let mut buf = [0 as c_char; 32];")
            && stats_rs.contains("let mut label = [0 as c_char; 64];")
            && stats_rs.contains("format_amount(&mut buf, n, unit);")
            && stats_rs.contains("CStr::from_ptr(fmt) }.to_bytes()"),
        "stats amount formatting should stay on stack buffers like C"
    );
    assert!(
        !stats_rs.contains("&format!(\n            \"{prefix}{")
            && !stats_rs.contains("&format!(\"  \\\"{name}\\\": \")")
            && !stats_rs.contains("&format!(\"  \\\"{name}\\\": {val},\\n\")")
            && !stats_rs.contains("&format!(\n            \"    \\\"{name}\\\": {val}")
            && stats_rs.contains("fn c_char_buffer_str(buf: &[c_char]) -> &str")
            && stats_rs.contains("let mut buf = [0 as c_char; 128];")
            && stats_rs.contains("mi_json_buf_print(hbuf, c_char_buffer_str(&buf));"),
        "stats JSON formatting should use fixed stack buffers like C"
    );
    assert!(
        !unsafe_allowlist
            .lines()
            .any(|line| line.starts_with("src/allocator/stats.rs\t"))
            && stats_rs
                .matches("SAFETY[AUDIT:stats-reporting-boundary]")
                .count()
                >= 150,
        "stats reporting unsafe boundaries should stay covered by local operation markers without a migration allowlist entry"
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
            && options_rs
                .matches("SAFETY[AUDIT:options-callback-boundary]")
                .count()
                >= 76,
        "options/callback unsafe boundaries should stay covered by local operation markers without a migration allowlist entry"
    );
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

#[test]
fn crate_root_exposes_only_curated_rust_facade() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let raw_rs = fs::read_to_string(root.join("src/raw/mod.rs")).expect("read src/raw/mod.rs");
    let unsafe_allowlist = fs::read_to_string(root.join("docs/unsafe-migration-allowlist.tsv"))
        .expect("read unsafe migration allowlist");

    assert!(
        lib_rs.contains("pub mod raw;")
            && lib_rs.contains("pub use ffi::callbacks as unsafe_callbacks;")
            && lib_rs.contains("pub struct MiMalloc;")
            && !lib_rs.contains("CallbackError")
            && !lib_rs.contains("DeferredFreeCallbackGuard, ErrorCallbackGuard")
            && lib_rs.contains("HeapAllocation")
            && lib_rs.contains("InitializedAllocation")
            && lib_rs.contains("StatsSnapshot"),
        "crate root should expose the raw facade, unsafe-adjacent callbacks, and finalized safe Rust allocator facade"
    );

    assert!(
        !unsafe_allowlist.contains("src/lib.rs")
            && lib_rs.matches("SAFETY[AUDIT:global-alloc]").count() >= 9
            && lib_rs.matches("SAFETY[AUDIT:panic-boundary]").count() >= 4
            && lib_rs.contains("#[unsafe(no_mangle)]")
            && lib_rs.contains("unsafe extern \"C\""),
        "crate-root unsafe boundaries should stay covered by local operation markers without a migration allowlist entry"
    );

    for module in ["abi", "headers", "mimalloc"] {
        assert!(
            raw_rs.contains(&format!("pub mod {module}")),
            "raw facade should intentionally expose `{module}` compatibility items"
        );
    }

    for module in [
        "alloc",
        "alloc_aligned",
        "alloc_override",
        "alloc_posix",
        "arena",
        "arena_meta",
        "atomic",
        "bitmap",
        "bits",
        "free",
        "heap",
        "init",
        "internal",
        "libc",
        "memory",
        "options",
        "os",
        "page",
        "page_map",
        "prim",
        "random",
        "stats",
        "theap",
        "threadlocal",
        "track",
        "types",
    ] {
        assert!(
            !lib_rs.contains(&format!("pub mod {module};")),
            "implementation module `{module}` should not be public at the crate root"
        );
        assert!(
            !raw_rs.contains(&format!("pub mod {module}")),
            "raw facade should not expose implementation module `{module}`"
        );
    }
}

#[test]
fn safe_byte_allocations_keep_object_ownership_separate() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let safe_api_md =
        fs::read_to_string(root.join("docs/safe-api.md")).expect("read docs/safe-api.md");
    let object_ownership_md = fs::read_to_string(root.join("docs/object-ownership.md"))
        .expect("read docs/object-ownership.md");
    let api_inventory_md =
        fs::read_to_string(root.join("docs/api-inventory.md")).expect("read api inventory");
    let safety_model_md =
        fs::read_to_string(root.join("docs/rust-safety-model.md")).expect("read safety model");
    let release_md =
        fs::read_to_string(root.join("docs/release.md")).expect("read docs/release.md");
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let allocation_rs =
        fs::read_to_string(root.join("src/safe/allocation.rs")).expect("read allocation.rs");
    let heap_rs = fs::read_to_string(root.join("src/safe/heap.rs")).expect("read heap.rs");

    for required in [
        "## Future Object Ownership APIs",
        "The current allocation wrappers own bytes, not Rust objects.",
        "docs/object-ownership.md",
        "MiBox<T>",
        "HeapBox<'heap, T>",
        "constructor panic cleanup",
        "zero-sized value",
        "unsized-value scope",
        "deallocation through the owning allocator or\nheap",
    ] {
        assert!(
            safe_api_md.contains(required),
            "safe object ownership policy should document {required}"
        );
    }
    for required in [
        "Safe Object Ownership Design Note",
        "`MiBox<T>` and `HeapBox<'heap, T>` are out of scope before the next release.",
        "try_new(value: T) -> Result<MiBox<T>, (AllocError, T)>",
        "try_new_with(init: impl FnOnce() -> T) -> Result<MiBox<T>, AllocError>",
        "Constructor panic cleanup deallocates raw storage",
        "Destructor panic behavior is documented and tested under `std`.",
        "Zero-sized values run their destructor and do not allocate.",
        "Heap-tied construction cannot outlive the creating heap borrow.",
        "Safe code cannot construct `MiBox<T>` or `HeapBox<'heap, T>` from private",
    ] {
        assert!(
            object_ownership_md.contains(required),
            "object ownership design note should include marker {required}"
        );
    }
    for required in [
        "Object ownership APIs such as `MiBox<T>` and `HeapBox<'heap, T>` are not part",
        "docs/object-ownership.md",
    ] {
        assert!(
            api_inventory_md.contains(required),
            "API inventory should record object ownership deferral marker {required}"
        );
        assert!(
            safety_model_md.contains(required),
            "safety model should record object ownership deferral marker {required}"
        );
    }
    for required in [
        "Confirm no object ownership APIs such as `MiBox<T>`, `HeapBox<'heap, T>`",
        "completing `docs/object-ownership.md`",
        "construction\n   failure, initialization cleanup, destructor and panic behavior",
        "zero-sized\n   and unsized value scope",
        "compile-fail tests for private representation and heap\n   lifetime misuse",
    ] {
        assert!(
            release_md.contains(required),
            "release checklist should keep object ownership policy gated by {required}"
        );
    }

    for (rel, source) in [
        ("src/lib.rs", lib_rs.as_str()),
        ("src/safe/allocation.rs", allocation_rs.as_str()),
        ("src/safe/heap.rs", heap_rs.as_str()),
    ] {
        for forbidden in [
            "struct MiBox",
            "struct HeapBox",
            "pub type MiBox",
            "pub type HeapBox",
            "impl<T> Allocation",
            "fn new_object",
            "fn allocate_object",
        ] {
            assert!(
                !source.contains(forbidden),
                "{rel} should keep object ownership APIs separate from current byte allocation wrappers"
            );
        }
    }
}

#[test]
fn interpose_linux_malloc_override_uses_c_tls_fast_path() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = fs::read_to_string(root.join("Cargo.toml")).expect("read Cargo.toml");
    let unsafe_allowlist = fs::read_to_string(root.join("docs/unsafe-migration-allowlist.tsv"))
        .expect("read unsafe migration allowlist");
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let tls_shim = fs::read_to_string(root.join("src/tls_shim.c")).expect("read src/tls_shim.c");
    let tls_recurse_shim = fs::read_to_string(root.join("src/tls_recurse_shim.c"))
        .expect("read src/tls_recurse_shim.c");
    let build_rs = fs::read_to_string(root.join("build.rs")).expect("read build.rs");
    let aligned_rs = fs::read_to_string(root.join("src/allocator/aligned.rs"))
        .expect("read src/allocator/aligned.rs");
    let alloc_rs = fs::read_to_string(root.join("src/allocator/alloc.rs"))
        .expect("read src/allocator/alloc.rs");
    let free_rs =
        fs::read_to_string(root.join("src/allocator/free.rs")).expect("read src/allocator/free.rs");
    let override_rs =
        fs::read_to_string(root.join("src/raw/macros.rs")).expect("read src/raw/macros.rs");
    let prim_rs =
        fs::read_to_string(root.join("src/platform/mod.rs")).expect("read src/platform/mod.rs");
    let init_rs =
        fs::read_to_string(root.join("src/allocator/init.rs")).expect("read src/allocator/init.rs");
    let threadlocal_rs = fs::read_to_string(root.join("src/allocator/threadlocal.rs"))
        .expect("read src/allocator/threadlocal.rs");

    assert!(
        cargo_toml.contains("crate-type = [\"rlib\"]")
            && cargo_toml.contains("c-abi = []")
            && cargo_toml.contains("cxx-overrides = [\"c-abi\"]")
            && cargo_toml.contains("tls-shim = [\"c-abi\"]")
            && cargo_toml.contains("interpose = [\"c-abi\", \"cxx-overrides\", \"tls-shim\"]")
            && cargo_toml.contains("lto = \"fat\"")
            && cargo_toml.contains("codegen-units = 1"),
        "ordinary builds should stay rlib-first while the explicit interpose feature restores the optimized cdylib artifact mode"
    );
    assert!(
        lib_rs.contains("not(feature = \"std\")")
            && lib_rs.contains("no_std")
            && lib_rs.contains("#[panic_handler]")
            && lib_rs.contains("#[link_name = \"abort\"]")
            && lib_rs.contains("extern \"C\" fn rust_eh_personality()"),
        "Linux release cdylibs should avoid linking Rust std panic/backtrace machinery unless the explicit std feature is enabled for tooling"
    );
    assert!(
        tls_shim.contains("__mi_theap_default")
            && tls_shim.contains("__mi_theap_cached")
            && tls_shim.contains("tls_model(\"initial-exec\")")
            && tls_shim.contains("visibility(\"hidden\")")
            && tls_shim.contains("= &_mi_theap_empty")
            && tls_shim.contains("mi_thread_locals_empty")
            && tls_shim.contains("mi_thread_locals")
            && tls_shim.contains("= &mi_thread_locals_empty")
            && tls_shim.contains("_mi_tls_thread_locals_get")
            && !tls_shim.contains("mi_tls_recurse")
            && !tls_shim.contains("static __thread unsigned char recurse"),
        "default Linux allocator fast paths should read hidden initial-exec TLS cells like C"
    );
    assert!(
        tls_recurse_shim.contains("static __thread unsigned char recurse")
            && tls_recurse_shim.contains("tls_model(\"initial-exec\")")
            && tls_recurse_shim.contains("_mi_tls_recurse_get")
            && tls_recurse_shim.contains("return recurse != 0;")
            && tls_recurse_shim.contains("_mi_tls_recurse_set")
            && tls_recurse_shim.contains("recurse = (value != 0);")
            && !tls_recurse_shim.contains("int mi_tls_recurse"),
        "Linux recursion guard TLS should stay byte-sized and isolated after heap TLS like C"
    );
    assert!(
        prim_rs.contains("fn mi_linux_x64_theap_default_get()")
            && prim_rs.contains("fn mi_linux_x64_theap_default_set(theap: Option<ThreadHeapPtr>)")
            && prim_rs.contains("fn mi_linux_x64_theap_cached_get()")
            && prim_rs.contains("fn mi_linux_x64_theap_cached_set(theap: Option<ThreadHeapPtr>)")
            && prim_rs.contains("__mi_theap_default@GOTTPOFF")
            && prim_rs.contains("__mi_theap_cached@GOTTPOFF")
            && init_rs.contains("fn mi_linux_x64_theap_main_get()")
            && init_rs.contains("fn mi_linux_x64_theap_main_set(theap: Option<ThreadHeapPtr>)")
            && init_rs.contains("__mi_theap_main@GOTTPOFF"),
        "Linux x86_64 heap TLS accessors should inline C-style initial-exec loads and stores"
    );
    assert!(
        prim_rs.matches("tls_offset = out(reg) _").count() >= 2
            && init_rs.contains("tls_offset = out(reg) _")
            && threadlocal_rs.contains("tls_offset = out(reg) _"),
        "Linux x86_64 TLS setters should not let the offset scratch register overlap the stored pointer"
    );
    assert!(
        prim_rs.contains("pub unsafe fn __mi_theap_empty() -> *mut mi_theap_t")
            && prim_rs.contains("\"lea {empty}, [rip + _mi_theap_empty]\"")
            && init_rs.contains("core::arch::global_asm!(\".hidden _mi_theap_empty\")"),
        "Linux x86_64 empty-theap address loads should use the hidden local symbol directly like C"
    );
    assert!(
        threadlocal_rs.contains("fn mi_linux_x64_thread_locals_get()")
            && threadlocal_rs
                .contains("fn mi_linux_x64_thread_locals_set(tls: *mut mi_thread_locals_t)")
            && threadlocal_rs.contains("mi_thread_locals@GOTTPOFF")
            && threadlocal_rs.contains("static mi_thread_locals_empty: mi_thread_locals_t;")
            && !tls_shim.contains("static __thread void* mi_tls_thread_locals"),
        "Linux x86_64 dynamic thread-local storage should use C's mi_thread_locals TLS cell directly"
    );
    assert!(
        !unsafe_allowlist.contains("src/allocator/threadlocal.rs")
            && threadlocal_rs
                .matches("SAFETY[AUDIT:threadlocal-key-manager]")
                .count()
                >= 40,
        "dynamic thread-local unsafe boundaries should stay covered by local operation markers without a migration allowlist entry"
    );
    assert!(
        threadlocal_rs.contains(
            "if mi_thread_locals_free.load(Ordering::Acquire).is_null() {\n        return;\n    }"
        ),
        "dynamic thread-local cleanup should skip the lock path when no dynamic TLS slots were allocated"
    );
    assert!(
        build_rs.contains("let interpose = feature_enabled(\"INTERPOSE\")")
            && build_rs.contains("let cxx_overrides = feature_enabled(\"CXX_OVERRIDES\")")
            && build_rs.contains("let tls_shim = feature_enabled(\"TLS_SHIM\")")
            && build_rs.contains("if cxx_overrides")
            && build_rs.contains("let needs_c_tls_shim = tls_shim || !std")
            && build_rs.contains("if needs_c_tls_shim")
            && build_rs.contains("target_arch == \"x86_64\"")
            && build_rs.contains("cargo:rustc-cfg=mi_c_malloc_fast")
            && build_rs.contains("src/tls_shim.c")
            && build_rs.contains("src/tls_recurse_shim.c")
            && build_rs.contains("ar.arg(\"crs\").arg(&lib).arg(&tls_obj).arg(&recurse_obj);"),
        "assembly malloc fast override must be limited to the explicit interpose build mode and supported target"
    );
    assert!(
        override_rs.contains("#[unsafe(naked)]")
            && override_rs.contains("pub unsafe extern \"C\" fn mi_cxx_new_fast_64")
            && !override_rs.contains("pub unsafe extern \"C\" fn mi_cxx_new_array_fast_64")
            && override_rs.contains("core::arch::naked_asm!")
            && override_rs.contains(".set _Znam, _Znwm")
            && override_rs.contains("try_new = sym crate::alloc::mi_theap_try_new")
            && override_rs.contains("\"xor edx, edx\"")
            && !override_rs.contains("\"jmp mi_new\"")
            && override_rs.matches("\"endbr64\"").count() >= 4
            && override_rs.contains("__mi_theap_default@GOTTPOFF")
            && override_rs.contains("[rdi + rax*8 + 0x110]")
            && override_rs.contains("malloc_generic = sym crate::page::_mi_malloc_generic")
            && override_rs.contains("stosb_max = sym crate::init::_mi_cpu_stosb_max")
            && override_rs.contains("\"mul rsi\"")
            && override_rs.contains("\"jo .Lmi_calloc_overflow\"")
            && override_rs.contains("\"mov edx, 1\"")
            && override_rs.contains("\"rep stosb\"")
            && override_rs.contains("\"call memset\"")
            && override_rs.contains("core::arch::global_asm!")
            && override_rs.contains(".globl free")
            && override_rs.contains("\"malloc:\"")
            && override_rs.contains("\"calloc:\"")
            && override_rs.contains(".set mi_malloc, malloc")
            && override_rs.contains(".set __libc_malloc, malloc")
            && override_rs.contains(".globl mi_calloc")
            && override_rs.contains(".set mi_calloc, calloc")
            && override_rs.contains(".globl __libc_calloc")
            && override_rs.contains(".set __libc_calloc, calloc")
            && override_rs.contains(".p2align 4")
            && override_rs.contains(".size free, .-free")
            && override_rs.contains(".set mi_free, free")
            && override_rs.contains(".set __libc_free, free")
            && override_rs.contains(".set _ZdlPv, free")
            && override_rs.contains(".set _ZdaPv, free")
            && override_rs.contains(".fill 0x30,1,0x90")
            && override_rs.contains(".fill 0x520,1,0x90")
            && override_rs.contains("page_map = sym crate::page_map::_mi_page_map")
            && override_rs.contains("retire = sym crate::page::_mi_page_retire")
            && override_rs.contains("collect_mt = sym crate::free::mi_free_try_collect_mt")
            && override_rs.contains("generic_local = sym crate::free::mi_free_generic_local")
            && override_rs.contains("generic_mt = sym crate::free::mi_free_generic_mt")
            && override_rs.contains("\"cmp rdx, 3\"")
            && override_rs.contains("\"lock cmpxchg qword ptr [rdx], rcx\"")
            && override_rs.contains("\"jmp {try_new}\""),
        "default Linux malloc/new/free overrides should use direct naked-assembly fast paths"
    );
    assert!(
        alloc_rs.contains("cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))")
            && alloc_rs.contains("pub unsafe extern \"C\" fn mi_malloc"),
        "Rust mi_malloc export gate must stay paired with the assembly fast override"
    );
    assert!(
        alloc_rs.contains("cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))")
            && alloc_rs.contains("pub unsafe extern \"C\" fn mi_calloc")
            && override_rs.contains("cfg(all(not(windows), not(mi_c_malloc_fast)))")
            && override_rs.contains("export_name = \"calloc\""),
        "Rust calloc/mi_calloc export gates must stay paired with the assembly fast override"
    );
    assert!(
        override_rs.contains("cfg(all(not(windows), not(mi_c_malloc_fast)))")
            && override_rs.contains("export_name = \"malloc\""),
        "Rust malloc override gate must stay paired with the assembly fast override"
    );
    for export_name in [
        "reallocf",
        "strdup",
        "strndup",
        "malloc_good_size",
        "vfree",
        "_aligned_malloc",
        "__libc_realloc",
        "__libc_cfree",
        "__libc_valloc",
        "__libc_pvalloc",
        "__libc_memalign",
        "__posix_memalign",
    ] {
        assert!(
            override_rs.contains(&format!("export_name = \"{export_name}\"")),
            "Rust Unix/Linux override surface should export {export_name} like C mimalloc"
        );
    }
    assert!(
        override_rs.contains(".weak reallocarr")
            && override_rs.contains(".set reallocarr, mi_reallocarr")
            && override_rs.contains("export_name = \"__libc_malloc\"")
            && override_rs.contains("export_name = \"__libc_calloc\"")
            && override_rs.contains("export_name = \"__libc_free\""),
        "Rust override surface should keep C's libc aliases and weak reallocarr compatibility"
    );
    assert!(
        aligned_rs.contains("#[cfg(not(mi_c_malloc_fast))]\n#[unsafe(no_mangle)]")
            && aligned_rs.contains("#[cfg(mi_c_malloc_fast)]\n#[unsafe(naked)]")
            && aligned_rs.contains("pub unsafe extern \"C\" fn mi_malloc_aligned")
            && aligned_rs.contains("\"endbr64\"")
            && aligned_rs.contains("__mi_theap_default@GOTTPOFF")
            && aligned_rs.contains("target = sym mi_theap_malloc_aligned"),
        "release mi_malloc_aligned should load the default heap from direct TLS like C"
    );
    assert!(
        override_rs.contains("\"free:\"")
            && override_rs.contains("\"sub word ptr [rdi + 0x10], 1\"")
            && override_rs.contains("\"mov qword ptr [rdi + 0x18], rsi\"")
            && override_rs.contains("\"je 7f\"")
            && override_rs.contains("\"3:\"")
            && override_rs.contains("\"ret\"")
            && override_rs.contains("\"cmp byte ptr [rdi + 0x16], 0\"")
            && override_rs.contains("\"jne 3b\"")
            && override_rs.contains(".set mi_free, free")
            && free_rs.contains("cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))"),
        "Unix free/mi_free exports must keep the C-like aligned assembly fast override and hot local-free branch layout"
    );
    assert!(
        free_rs.contains("#[cfg(mi_c_malloc_fast)]\ncore::arch::global_asm!(")
            && free_rs.contains("\".globl mi_free_size\"")
            && free_rs.contains("\"mi_free_size:\"")
            && free_rs.contains("\"jmp mi_free\"")
            && free_rs.contains("\".globl mi_free_size_aligned\"")
            && free_rs.contains("\"mi_free_size_aligned:\"")
            && free_rs.contains("\"jmp mi_free_size\"")
            && free_rs.contains("\".globl mi_free_aligned\"")
            && free_rs.contains("\"mi_free_aligned:\"")
            && free_rs
                .matches("cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))")
                .count()
                >= 4,
        "release free-size/aligned public variants should remain C-shaped forwarding stubs in the fast build"
    );
}

#[test]
fn raw_type_opacity_decisions_stay_documented_and_checked() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let init_rs =
        fs::read_to_string(root.join("src/allocator/init.rs")).expect("read src/allocator/init.rs");
    let stats_rs = fs::read_to_string(root.join("src/allocator/stats.rs"))
        .expect("read src/allocator/stats.rs");
    let types_rs = fs::read_to_string(root.join("src/types/constants.rs"))
        .expect("read src/types/constants.rs");
    let api_doc =
        fs::read_to_string(root.join("docs/api-inventory.md")).expect("read api inventory");

    let reexports_start = abi_rs
        .find("pub use crate::types::{")
        .expect("find abi type reexports");
    let reexports_end = abi_rs[reexports_start..]
        .find("};")
        .map(|offset| reexports_start + offset)
        .expect("find end of abi type reexports");
    let type_reexports = &abi_rs[reexports_start..reexports_end];

    for exposed in [
        "mi_arena_id_t",
        "mi_block_visit_fun",
        "mi_commit_fun_t",
        "mi_deferred_free_fun",
        "mi_error_fun",
        "mi_heap_area_t",
        "mi_heap_t",
        "mi_heap_visit_fun",
        "mi_output_fun",
        "mi_subproc_id_t",
        "mi_theap_t",
    ] {
        assert!(
            type_reexports.contains(exposed),
            "default raw ABI should intentionally expose {exposed}"
        );
    }
    for hidden in ["mi_arena_t", "mi_subproc_t", "mi_page_t", "mi_block_t"] {
        assert!(
            !type_reexports.contains(hidden),
            "default raw ABI should not re-export implementation layout {hidden}"
        );
    }

    assert!(
        types_rs.contains("pub struct mi_subproc_id_t {\n    _mi_subproc_id: *mut c_void,\n}")
            && !types_rs.contains("pub _mi_subproc_id")
            && types_rs.contains("pub(crate) fn from_ptr<T>(ptr: *mut T) -> Self")
            && types_rs.contains("pub(crate) fn as_ptr<T>(self) -> *mut T")
            && init_rs.contains("mi_subproc_id_t::from_ptr(")
            && init_rs.contains("subproc_id.as_ptr::<mi_subproc_t>()")
            && stats_rs.contains("subproc_id.as_ptr::<MiSubprocStatsPrefix>()")
            && !init_rs.contains("._mi_subproc_id")
            && !stats_rs.contains("._mi_subproc_id"),
        "mi_subproc_id_t should remain an opaque by-value C token with crate-local pointer conversion"
    );

    assert!(
        api_doc.contains("### Raw ABI Type Inventory")
            && api_doc.contains("| `mi_subproc_id_t` | `repr(C)` opaque identifier |")
            && api_doc.contains("Callers can pass it back to subproc APIs but cannot inspect or construct it from fields.")
            && api_doc.contains("| `mi_heap_s` / `mi_heap_t` | `mi_heap_t` is re-exported by `src/ffi/abi.rs` |")
            && api_doc.contains("| `mi_arena_s` / `mi_arena_t` | Not re-exported by `src/ffi/abi.rs` today |"),
        "raw type opacity decisions should stay documented in the public API inventory"
    );
}

#[test]
fn allocation_product_paths_use_typed_requests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let aligned_rs =
        fs::read_to_string(root.join("src/allocator/aligned.rs")).expect("read alloc_aligned.rs");
    let alloc_rs = fs::read_to_string(root.join("src/allocator/alloc.rs"))
        .expect("read src/allocator/alloc.rs");
    let posix_rs = fs::read_to_string(root.join("src/raw/posix.rs")).expect("read alloc_posix.rs");
    let layout_rs = fs::read_to_string(root.join("src/types/layout.rs")).expect("read layout.rs");

    let product_overflow_checks = alloc_rs.matches("mi_count_size_overflow").count()
        + aligned_rs.matches("mi_count_size_overflow").count()
        + posix_rs.matches("mi_count_size_overflow").count();

    assert!(
        product_overflow_checks == 1
            && !alloc_rs.contains("Temp compat bridge")
            && !aligned_rs.contains("mi_count_size_overflow")
            && !aligned_rs.contains("_mi_theap_realloc_zero(")
            && !posix_rs.contains("mi_count_size_overflow")
            && alloc_rs.contains(
                "fn product_size_or_report(\n    count: crate::core_types::ItemCount,\n    size: crate::core_types::RequestedSize,"
            )
            && alloc_rs.contains("pub(crate) fn calloc_request_or_report(")
            && alloc_rs.contains("fn alloc_product_or_report(")
            && alloc_rs
                .contains("crate::internal::mi_count_size_overflow(count.get(), size.get(), &mut total)")
            && alloc_rs.contains(
                "crate::core_types::CallocRequest::from_checked_total(count.get(), size.get(), total.get())"
            )
            && alloc_rs.contains(
                "crate::core_types::AllocProduct::from_checked_total(count.get(), size.get(), total.get())"
            )
            && !alloc_rs.contains("CallocRequest::new(count, size).ok()")
            && !alloc_rs.contains("AllocProduct::new(count, size).ok()")
            && alloc_rs.contains("unsafe fn mi_theap_calloc_req(")
            && alloc_rs.contains("unsafe fn mi_theap_mallocn_req(")
            && alloc_rs.contains("unsafe fn mi_theap_realloc_req(")
            && alloc_rs.contains("unsafe fn mi_theap_realloc_request(")
            && alloc_rs.contains("unsafe fn mi_theap_reallocf_req(")
            && alloc_rs.contains("unsafe fn mi_theap_reallocn_req(")
            && alloc_rs.contains("unsafe fn mi_theap_recalloc_req(")
            && alloc_rs.contains("alloc_product_or_report(newcount, size)")
            && alloc_rs.contains(
                "unsafe fn mi_theap_alloc_new_n(\n    theap: *mut mi_theap_t,\n    req: crate::core_types::AllocProduct,"
            )
            && alloc_rs.contains("realloc_req: crate::core_types::ReallocRequest,")
            && alloc_rs.contains("mi_theap_recalloc_req(theap, req, realloc_req)")
            && alloc_rs.contains("mi_theap_realloc_typed(theap, realloc_req)")
            && alloc_rs.contains("crate::core_types::ReallocRequest::from_null(")
            && alloc_rs.contains("crate::core_types::ReallocRequest::from_existing(")
            && alloc_rs.contains("zero_new_tail == crate::core_types::Zeroed::Yes")
            && alloc_rs.contains("req.zero_new_tail(),")
            && layout_rs.contains("pub(crate) struct AllocProduct")
            && layout_rs.contains("pub(crate) struct ReallocRequest")
            && layout_rs.contains("enum ReallocSource")
            && layout_rs.contains("pub(crate) fn zero_new_tail(&self) -> Zeroed")
            && layout_rs.contains("pub(crate) unsafe fn from_checked_total(")
            && aligned_rs.contains("unsafe fn mi_theap_realloc_zero_aligned_at_req(")
            && aligned_rs.contains("let offset = crate::core_types::AlignmentOffset::new(offset);")
            && aligned_rs.contains(
                "crate::core_types::LayoutRequest::aligned_at(newsize, alignment, offset, zeroed)"
            )
            && aligned_rs.contains("mi_theap_realloc_layout(theap, p, layout)")
            && aligned_rs.contains("calloc_request_or_report(count, size)")
            && aligned_rs.contains("calloc_request_or_report(newcount, size)")
            && posix_rs.contains("crate::core_types::CallocRequest::new(count, size)")
            && posix_rs.contains("SAFETY[AUDIT:raw-posix-compat]")
            && posix_rs.matches("SAFETY[AUDIT:raw-posix-compat]").count() >= 60
            && posix_rs.contains("pub unsafe extern \"C\" fn mi_posix_memalign")
            && posix_rs.contains("pub unsafe extern \"C\" fn mi_reallocarray")
            && posix_rs.contains("pub unsafe extern \"C\" fn mi_reallocarr")
            && posix_rs.contains("pub unsafe extern \"C\" fn mi_dupenv_s")
            && layout_rs.contains("pub(crate) fn layout(self) -> LayoutRequest")
            && layout_rs.contains("let total_size = count.checked_mul(size)"),
        "calloc product-size paths should use CallocRequest, non-calloc product paths should use AllocProduct, and both should share one C-compatible overflow reporting adapter"
    );
}

#[test]
fn realloc_validation_uses_pointer_lookup_boundary() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let aligned_rs =
        fs::read_to_string(root.join("src/allocator/aligned.rs")).expect("read alloc_aligned.rs");
    let alloc_rs = fs::read_to_string(root.join("src/allocator/alloc.rs"))
        .expect("read src/allocator/alloc.rs");
    let layout_rs = fs::read_to_string(root.join("src/types/layout.rs")).expect("read layout.rs");

    assert!(
        alloc_rs.contains(
            "unsafe fn mi_validate_ptr_page(\n    p: *const c_void,\n    msg: *const c_char,\n) -> crate::core_types::PointerLookup"
        ) && alloc_rs.contains("crate::free::mi_validate_pointer(p.cast_mut(), msg)")
            && alloc_rs.contains("unsafe fn mi_realloc_old_allocation(")
            && alloc_rs.contains("crate::core_types::PointerLookup::Valid(valid)")
            && alloc_rs.contains("let user = valid.user();")
            && alloc_rs.contains("let page = valid.page();")
            && alloc_rs.contains("let location = valid.location();")
            && alloc_rs.contains("let block = location.block();")
            && alloc_rs.contains("Some(crate::core_types::OldAllocation::from_validated_parts(")
            && alloc_rs.contains("unsafe fn mi_realloc_request_from_ptr(")
            && alloc_rs
                .contains("let Some(old) = mi_realloc_old_allocation(p, c\"mi_realloc\".as_ptr())")
            && alloc_rs.contains("unsafe fn mi_theap_realloc_validated_core(")
            && alloc_rs.contains("zero_new_tail: crate::core_types::Zeroed,")
            && alloc_rs.contains("crate::core_types::ReallocRequest::from_null(")
            && alloc_rs.contains("crate::core_types::ReallocRequest::from_existing(")
            && alloc_rs.contains("req.old(),")
            && alloc_rs.contains("let page = old.page().as_ptr();")
            && alloc_rs.contains("let size = old.usable();")
            && alloc_rs
                .contains("crate::core_types::PreserveBytes::new(old.usable(), new_requested)")
            && alloc_rs.contains("let preserved = preserve.map_or(0, |preserve| preserve.get());")
            && alloc_rs.contains("let Some(preserve) = preserve else")
            && alloc_rs.contains("let copysize = preserve.get();")
            && !alloc_rs.contains("let copysize = if newsize > size { size } else { newsize };")
            && aligned_rs.contains("let preserve = crate::core_types::PreserveBytes::new(")
            && aligned_rs.contains("crate::core_types::UsableSize::new(size)")
            && aligned_rs.contains("_mi_memcpy_aligned(newp, p, preserve.get())")
            && !aligned_rs.contains("usize::min(newsize, size)")
            && alloc_rs.contains("crate::core_types::PointerLookup::Invalid { .. }")
            && layout_rs.contains("pub(crate) struct OldAllocation")
            && layout_rs.contains("pub(crate) struct PreserveBytes(usize);")
            && layout_rs.contains("Self(usize::min(old.get(), new.get()))")
            && layout_rs.contains("unsafe fn new(")
            && !layout_rs.contains("pub(crate) unsafe fn new(")
            && layout_rs.contains("pub(crate) unsafe fn from_validated_parts(")
            && layout_rs.contains("unsafe { Self::new(ptr, usable, page, block) }")
            && layout_rs.contains("pub(crate) fn usable(self) -> UsableSize")
            && layout_rs.contains("pub(crate) fn page(self) -> PagePtr")
            && layout_rs.contains("enum ReallocSource")
            && layout_rs.contains("ReallocSource::Null")
            && layout_rs.contains("ReallocSource::Existing(old)")
            && layout_rs.contains("pub(crate) fn from_null(")
            && layout_rs.contains("pub(crate) fn from_existing(")
            && layout_rs.contains("pub(crate) fn old(&self) -> Option<OldAllocation>")
            && !layout_rs.contains("old: Option<OldAllocation>")
            && !layout_rs.contains("with_old")
            && !alloc_rs.contains("unsafe fn mi_validate_ptr_page(p: *const c_void, msg: *const c_char) -> *mut mi_page_t")
            && !alloc_rs.contains("_mi_safe_ptr_page"),
        "realloc must use PointerLookup and construct OldAllocation only after validation"
    );
}

#[test]
fn generic_allocation_flags_use_typed_boundary() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let alloc_rs = fs::read_to_string(root.join("src/allocator/alloc.rs"))
        .expect("read src/allocator/alloc.rs");
    let page_rs =
        fs::read_to_string(root.join("src/allocator/page.rs")).expect("read src/allocator/page.rs");
    let layout_rs = fs::read_to_string(root.join("src/types/layout.rs")).expect("read layout.rs");

    assert!(
        !alloc_rs.contains("usize::from(zero) | huge_alignment")
            && !page_rs.contains("zero_huge_alignment & !1")
            && !page_rs.contains("zero_huge_alignment & 1")
            && alloc_rs.contains("crate::core_types::MallocGenericFlags::new(")
            && alloc_rs.contains("fn maybe_alignment_or_report(alignment: usize)")
            && alloc_rs.contains("crate::core_types::MaybeAlignment::new(alignment)")
            && alloc_rs.contains("crate::core_types::MaybeHugeAlignment::new(huge_alignment.get())")
            && alloc_rs.contains("fn no_alignment() -> crate::core_types::MaybeAlignment")
            && alloc_rs.contains("unsafe fn mi_new_aligned_typed(")
            && alloc_rs.contains("unsafe fn mi_new_aligned_nothrow_typed(")
            && alloc_rs.contains("alignment: crate::core_types::Alignment,")
            && page_rs.contains("pub(crate) unsafe fn _mi_malloc_generic_typed(")
            && page_rs.contains("theap: crate::core_types::ThreadHeapPtr")
            && page_rs.contains("pub(crate) unsafe fn _mi_theap_or_default_typed(")
            && page_rs.contains("Some(crate::core_types::ThreadHeapPtr::new_unchecked(theap))")
            && page_rs.contains("layout: crate::core_types::LayoutRequest")
            && page_rs.contains("let size = layout.padded().get();")
            && page_rs.contains("let flags = crate::core_types::MallocGenericFlags::from_raw(zero_huge_alignment)")
            && layout_rs.contains("pub(crate) unsafe fn from_raw(raw: usize) -> Self")
            && page_rs.contains("crate::core_types::LayoutRequest::from_padded_unchecked(")
            && alloc_rs.contains("fn malloc_layout_or_report(size: usize, zero: bool)")
            && alloc_rs.contains("crate::core_types::LayoutRequest::zalloc(size)")
            && alloc_rs.contains("crate::core_types::LayoutRequest::malloc(size)")
            && alloc_rs.contains("let p = _mi_malloc_generic_typed(theap.ptr(), layout, flags, usable)")
            && alloc_rs.contains("crate::core_types::LayoutRequest::from_padded_unchecked(")
            && alloc_rs.contains("unsafe fn mi_theap_malloc_generic(\n    theap: ThreadHeapLocal,\n    layout: crate::core_types::LayoutRequest,\n    flags: crate::core_types::MallocGenericFlags,")
            && alloc_rs.contains("unsafe fn mi_theap_malloc_zero_req(\n    theap: *mut mi_theap_t,\n    layout: crate::core_types::LayoutRequest,\n    huge_alignment: crate::core_types::MaybeAlignment,")
            && alloc_rs.contains("let Some(layout) = malloc_layout_or_report(size, zero)")
            && !alloc_rs.contains("mi_theap_malloc_generic(theap, size, zero, huge_alignment, usable)")
            && layout_rs.contains("pub(crate) unsafe fn from_padded_unchecked(")
            && layout_rs.contains("requested_size = padded.get().wrapping_sub(crate::types::MI_PADDING_SIZE)")
            && page_rs.contains("flags.zeroed() == crate::core_types::Zeroed::Yes")
            && page_rs.contains("flags.huge_alignment().get()")
            && layout_rs.contains("pub(crate) struct MaybeHugeAlignment(usize);")
            && layout_rs.contains("pub(crate) struct MallocGenericFlags(usize);")
            && !page_rs.contains("pub(crate) unsafe fn _mi_malloc_generic_typed(\n    mut theap: *mut mi_theap_t,")
            && !page_rs.contains("pub(crate) unsafe fn _mi_malloc_generic_typed(\n    mut theap: *mut mi_theap_t,\n    size: usize,"),
        "generic allocation should use MallocGenericFlags instead of manually packing zero and huge alignment bits"
    );
}

#[test]
fn c_memory_order_enum_stays_in_atomic_facade() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src");
    let mut sources = Vec::new();
    collect_rs_sources(root, &src, &mut sources);

    for (rel, source) in sources {
        if rel == "src/internal/atomic.rs" {
            continue;
        }
        assert!(
            !source.contains("mi_memory_order_e")
                && !source.contains("mi_memory_order::")
                && !source.contains("mi_memory_order_"),
            "{rel} should use core_types::MemoryOrder or Ordering, not C-compatible mi_memory_order_e"
        );
    }
}

#[test]
fn strict_type_modules_remain_no_std_no_heap_alloc_no_std_sync() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let core_types = root.join("src/types");
    let mut sources = Vec::new();
    collect_rs_sources(root, &core_types, &mut sources);

    let allowed_core_atomic_modules = BTreeSet::from([
        "src/types/arena.rs",
        "src/types/atomic.rs",
        "src/types/constants.rs",
        "src/types/ptr.rs",
    ]);

    for (rel, source) in sources {
        for forbidden in [
            "extern crate std",
            "use std",
            "std::",
            "extern crate alloc",
            "crate::alloc::",
            "::alloc::",
            "use alloc::",
            "Vec<",
            "Vec::",
            "Box<",
            "Box::",
            "String",
            "format!",
            "to_string(",
            "to_owned(",
            "mi_malloc",
            "mi_zalloc",
            "mi_calloc",
            "mi_realloc",
            "mi_heap_zalloc",
            "_mi_meta_alloc",
            "_mi_meta_zalloc",
            "_mi_arenas_alloc",
            "Mutex",
            "MutexGuard",
            "RwLock",
            "OnceLock",
            "LazyLock",
            "Condvar",
            "std::thread",
        ] {
            assert!(
                !source.contains(forbidden),
                "{rel} should not introduce std, heap allocation, formatting allocation, or std sync through {forbidden}"
            );
        }

        if source.contains("core::sync::atomic") {
            assert!(
                allowed_core_atomic_modules.contains(rel.as_str()),
                "{rel} should route strict-type atomic access through the dedicated atomic/cell wrappers"
            );
        }
    }
}

#[test]
fn strict_type_wrappers_do_not_allocate_on_alloc_free_hot_paths() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let alloc_rs = fs::read_to_string(root.join("src/allocator/alloc.rs"))
        .expect("read src/allocator/alloc.rs");
    let page_rs =
        fs::read_to_string(root.join("src/allocator/page.rs")).expect("read src/allocator/page.rs");
    let free_rs =
        fs::read_to_string(root.join("src/allocator/free.rs")).expect("read src/allocator/free.rs");
    let core_types = root.join("src/types");
    let mut strict_type_sources = Vec::new();
    collect_rs_sources(root, &core_types, &mut strict_type_sources);

    for (rel, source) in strict_type_sources {
        assert_no_rust_heap_constructs(&rel, "strict type module", &source);
    }

    for (source_name, source, marker) in [
        (
            "src/allocator/alloc.rs",
            alloc_rs.as_str(),
            "pub(crate) unsafe fn _mi_page_malloc_zero_typed(",
        ),
        (
            "src/allocator/alloc.rs",
            alloc_rs.as_str(),
            "unsafe fn mi_theap_malloc_generic(",
        ),
        (
            "src/allocator/alloc.rs",
            alloc_rs.as_str(),
            "unsafe fn mi_theap_malloc_zero_req(",
        ),
        (
            "src/allocator/page.rs",
            page_rs.as_str(),
            "pub(crate) unsafe fn _mi_malloc_generic_typed(",
        ),
        (
            "src/allocator/free.rs",
            free_rs.as_str(),
            "pub(crate) unsafe extern \"C\" fn mi_free_generic_local(",
        ),
        (
            "src/allocator/free.rs",
            free_rs.as_str(),
            "pub(crate) unsafe extern \"C\" fn mi_free_generic_mt(",
        ),
        (
            "src/allocator/free.rs",
            free_rs.as_str(),
            "unsafe fn mi_prepare_validated_free(",
        ),
        (
            "src/allocator/free.rs",
            free_rs.as_str(),
            "unsafe fn mi_free_validated(",
        ),
        (
            "src/allocator/free.rs",
            free_rs.as_str(),
            "unsafe fn mi_free_fast_try(",
        ),
        (
            "src/allocator/free.rs",
            free_rs.as_str(),
            "pub(crate) unsafe fn mi_route_free(",
        ),
    ] {
        let item = source_item(source_name, source, marker);
        assert_no_rust_heap_constructs(source_name, marker, item);
    }

    for marker in [
        "pub(crate) unsafe extern \"C\" fn mi_free_generic_local(",
        "pub(crate) unsafe extern \"C\" fn mi_free_generic_mt(",
        "unsafe fn mi_prepare_validated_free(",
        "unsafe fn mi_free_validated(",
        "unsafe fn mi_free_fast_try(",
        "pub(crate) unsafe fn mi_route_free(",
    ] {
        let item = source_item("src/allocator/free.rs", &free_rs, marker);
        for forbidden in [
            "mi_malloc(",
            "mi_zalloc(",
            "mi_calloc(",
            "mi_realloc(",
            "mi_heap_zalloc(",
            "_mi_meta_alloc",
            "_mi_meta_zalloc",
            "_mi_arenas_alloc",
        ] {
            assert!(
                !item.contains(forbidden),
                "{marker} in src/allocator/free.rs should not allocate on the free hot path through {forbidden}"
            );
        }
    }

    assert!(
        alloc_rs.contains("crate::core_types::LayoutRequest::from_padded_unchecked(")
            && alloc_rs.contains("crate::core_types::MallocGenericFlags::new(")
            && alloc_rs.contains("crate::core_types::RequestedSize::new(size).padded_unchecked()")
            && alloc_rs.contains(
                "unsafe fn mi_theap_malloc_small_zero_nonnull(\n    theap: ThreadHeapLocal,\n    size: crate::core_types::RequestedSize,"
            )
            && alloc_rs.contains(
                "unsafe fn mi_heap_try_new_typed(\n    heap: BorrowedHeap,\n    size: crate::core_types::RequestedSize,"
            )
            && alloc_rs.contains(
                "unsafe fn mi_heap_alloc_new_typed(\n    heap: BorrowedHeap,\n    size: crate::core_types::RequestedSize,"
            )
            && page_rs.contains("layout: crate::core_types::LayoutRequest")
            && page_rs.contains("flags: crate::core_types::MallocGenericFlags")
            && free_rs.contains(".prepare_free()")
            && free_rs.contains("validated: crate::core_types::ValidatedFree")
            && free_rs.contains(
                "unsafe fn mi_free_validated(validated: crate::core_types::ValidatedFree,"
            )
            && free_rs.contains("let owner = validated.page().classify_for_free(current_tid);")
            && free_rs.contains("owner: crate::core_types::PageFreeOwner,"),
        "allocation/free hot paths should continue to consume strict-type wrappers under the no-allocation source guard"
    );

    assert!(
        free_rs.contains("fn mi_free_fast_path_enabled(usable: *mut usize) -> bool")
            && free_rs.contains("&& !cfg!(debug_assertions)")
            && free_rs.contains("&& MI_DEBUG == 0")
            && free_rs.contains("&& MI_SECURE == 0")
            && free_rs.contains("&& MI_GUARDED == 0")
            && free_rs.contains("&& MI_PADDING == 0")
            && free_rs.contains("if mi_free_fast_try(p, usable) {\n            return;\n        }")
            && free_rs.contains("crate::core_types::FreePathKind::LocalFast =>")
            && free_rs.contains("crate::core_types::FreePathKind::RemotePlain")
            && free_rs.contains("if !page_tid.is_abandoned() && !page_tid.is_abandoned_mapped()")
            && source_item(
                "src/allocator/free.rs",
                &free_rs,
                "pub unsafe extern \"C\" fn mi_ufree("
            )
            .contains("mi_route_free(p, c\"mi_ufree\".as_ptr(), usable);"),
        "mi_free should keep the release-only no-usable fast route while mi_ufree keeps the usable-size route"
    );
}

#[test]
fn datatype_acceptance_criteria_have_cross_cutting_guards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src");
    let mut rust_sources = Vec::new();
    collect_rs_sources(root, &src, &mut rust_sources);

    let types_spec = fs::read_to_string(root.join("TYPES_SPEC.md")).expect("read TYPES_SPEC.md");
    let unsafe_migration_allowlist =
        fs::read_to_string(root.join("docs/unsafe-migration-allowlist.tsv"))
            .expect("read unsafe migration allowlist");
    let strict_type_allowlist = fs::read_to_string(root.join("strict-type-allowlist.txt"))
        .expect("read strict-type allowlist");
    let strict_type_checker = fs::read_to_string(root.join("scripts/check-strict-types.py"))
        .expect("read strict-type checker");

    datatype_acceptance::assert_datatype_acceptance_criteria_have_cross_cutting_guards(
        datatype_acceptance::DatatypeAcceptanceSources {
            rust_sources: &rust_sources,
            types_spec: &types_spec,
            unsafe_migration_allowlist: &unsafe_migration_allowlist,
            strict_type_allowlist: &strict_type_allowlist,
            strict_type_checker: &strict_type_checker,
        },
    );
}

#[test]
fn phase6_dead_code_allowances_stay_intentional() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src");
    let mut sources = Vec::new();
    collect_rs_sources(root, &src, &mut sources);

    let mut actual = BTreeMap::new();
    let mut module_actual = BTreeMap::new();
    for (rel, source) in &sources {
        let count = dead_code_allowance_count(source);
        if count > 0 {
            actual.insert(rel.as_str(), count);
        }
        let module_count = module_dead_code_allowance_count(source);
        if module_count > 0 {
            module_actual.insert(rel.as_str(), module_count);
        }
    }

    let expected = BTreeMap::from([
        ("src/allocator/alloc.rs", 1),
        ("src/allocator/aligned.rs", 1),
        ("src/raw/macros.rs", 1),
        ("src/allocator/arena.rs", 1),
        ("src/allocator/arena/debug.rs", 13),
        ("src/allocator/arena_meta.rs", 1),
        ("src/internal/atomic.rs", 1),
        ("src/internal/bitmap.rs", 1),
        ("src/types/freelist_wrappers.rs", 1),
        ("src/allocator/free.rs", 3),
        ("src/allocator/init.rs", 1),
        ("src/internal/mod.rs", 1),
        ("src/ffi/libc.rs", 1),
        ("src/allocator/options.rs", 1),
        ("src/allocator/page.rs", 1),
        ("src/platform/mod.rs", 1),
        ("src/platform/emscripten.rs", 1),
        ("src/platform/macos.rs", 1),
        ("src/platform/unix.rs", 1),
        ("src/platform/wasi.rs", 1),
        ("src/platform/windows.rs", 3),
        ("src/allocator/static_alloc.rs", 1),
        ("src/allocator/stats.rs", 1),
        ("src/allocator/thread_heap.rs", 1),
        ("src/allocator/tracking.rs", 6),
        ("src/types/constants.rs", 1),
        ("src/types/track.rs", 2),
    ]);
    let expected_module_level = BTreeMap::from([
        ("src/allocator/aligned.rs", 1),
        ("src/raw/macros.rs", 1),
        ("src/allocator/arena_meta.rs", 1),
        ("src/internal/atomic.rs", 1),
        ("src/internal/bitmap.rs", 1),
        ("src/allocator/init.rs", 1),
        ("src/internal/mod.rs", 1),
        ("src/ffi/libc.rs", 1),
        ("src/allocator/options.rs", 1),
        ("src/allocator/page.rs", 1),
        ("src/platform/mod.rs", 1),
        ("src/platform/emscripten.rs", 1),
        ("src/platform/macos.rs", 1),
        ("src/platform/unix.rs", 1),
        ("src/platform/wasi.rs", 1),
        ("src/platform/windows.rs", 1),
        ("src/allocator/static_alloc.rs", 1),
        ("src/allocator/stats.rs", 1),
        ("src/allocator/thread_heap.rs", 1),
        ("src/types/constants.rs", 1),
    ]);

    assert_eq!(
        actual, expected,
        "Phase 6 dead_code allowances should stay on the audited compatibility/scaffold allowlist"
    );
    assert_eq!(
        module_actual, expected_module_level,
        "broad module-level dead_code allowances should stay limited to permanent compatibility shims"
    );
    assert!(
        !actual.contains_key("src/lib.rs"),
        "crate-wide dead_code suppression should not return"
    );

    let safe_allowances: Vec<_> = actual
        .keys()
        .filter(|path| path.starts_with("src/safe"))
        .collect();
    assert!(
        safe_allowances.is_empty(),
        "safe facade modules should stay free of dead_code allowances: {safe_allowances:?}"
    );

    let core_type_allowances: Vec<_> = actual
        .keys()
        .filter(|path| path.starts_with("src/types/"))
        .copied()
        .collect();
    assert_eq!(
        core_type_allowances,
        vec![
            "src/types/constants.rs",
            "src/types/freelist_wrappers.rs",
            "src/types/track.rs"
        ],
        "core type dead_code allowances should remain item-level and explicitly justified"
    );
}

#[test]
fn phase6_clippy_allowances_stay_intentional() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src");
    let mut sources = Vec::new();
    collect_rs_sources(root, &src, &mut sources);

    let expected = BTreeMap::from([
        (("src/allocator/arena.rs", "too_many_arguments"), 4),
        (("src/allocator/arena/debug.rs", "too_many_arguments"), 1),
        (("src/internal/bitmap.rs", "many_single_char_names"), 1),
        (("src/allocator/free.rs", "absurd_extreme_comparisons"), 1),
        (("src/ffi/libc.rs", "many_single_char_names"), 2),
        (("src/raw/macros.rs", "must_use_candidate"), 1),
        (("src/platform/os.rs", "absurd_extreme_comparisons"), 3),
        (("src/platform/os.rs", "too_many_arguments"), 1),
        (("src/platform/unix.rs", "struct_field_names"), 1),
        (("src/allocator/page.rs", "absurd_extreme_comparisons"), 1),
        (("src/internal/random.rs", "many_single_char_names"), 1),
    ]);

    assert_eq!(
        clippy_allowance_counts(&sources),
        expected,
        "localized Clippy allowances should remain limited to audited C-shaped arity and secure/debug constant comparisons"
    );

    let safe_rs = fs::read_to_string(root.join("src/safe/mod.rs")).expect("read safe facade");
    let callbacks_rs =
        fs::read_to_string(root.join("src/ffi/callbacks.rs")).expect("read callback guards");
    let raw_rs = fs::read_to_string(root.join("src/raw/mod.rs")).expect("read raw facade");
    let safe_boundary_script =
        fs::read_to_string(root.join("scripts/check-safe-boundary.py")).expect("read safe check");
    assert!(
        safe_rs.starts_with("#![deny(clippy::must_use_candidate)]"),
        "safe Rust facade should locally deny missing must-use markers even while translated internals keep the package allowance"
    );
    assert!(
        callbacks_rs.contains("#![deny(clippy::must_use_candidate)]")
            && raw_rs.contains("#![deny(clippy::must_use_candidate)]")
            && safe_boundary_script.contains("STRICT_LINT_POLICY_FILES = {")
            && safe_boundary_script.contains("\"src/safe/mod.rs\"")
            && safe_boundary_script.contains("\"src/ffi/callbacks.rs\"")
            && safe_boundary_script.contains("\"src/raw/mod.rs\"")
            && safe_boundary_script.contains("SAFE_FACING_LINT_FILES = [")
            && safe_boundary_script.contains("allow(clippy::"),
        "safe-facing callback and raw facade glue should locally deny must-use drift and CI should reject unaudited Clippy allowances"
    );
}

#[test]
fn package_clippy_allowances_stay_intentional() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("read manifest");
    let lints = package_clippy_lints(&manifest);

    let allowed = BTreeSet::from([
        "borrow_as_ptr",
        "bool_to_int_with_if",
        "branches_sharing_code",
        "cast_lossless",
        "cast_possible_truncation",
        "cast_possible_wrap",
        "cast_precision_loss",
        "cast_ptr_alignment",
        "cast_sign_loss",
        "checked_conversions",
        "doc_markdown",
        "elidable_lifetime_names",
        "explicit_iter_loop",
        "if_not_else",
        "inline_always",
        "items_after_statements",
        "iter_with_drain",
        "large_stack_arrays",
        "literal_string_with_formatting_args",
        "manual_let_else",
        "many_single_char_names",
        "map_unwrap_or",
        "match_same_arms",
        "match_wild_err_arm",
        "missing_const_for_fn",
        "missing_errors_doc",
        "must_use_candidate",
        "needless_continue",
        "needless_pass_by_value",
        "needless_raw_string_hashes",
        "no_effect_underscore_binding",
        "option_if_let_else",
        "option_option",
        "ptr_as_ptr",
        "ptr_cast_constness",
        "pub_underscore_fields",
        "redundant_closure_for_method_calls",
        "redundant_pub_crate",
        "ref_as_ptr",
        "semicolon_if_nothing_returned",
        "significant_drop_tightening",
        "similar_names",
        "single_match_else",
        "suboptimal_flops",
        "too_many_lines",
        "trivially_copy_pass_by_ref",
        "uninlined_format_args",
        "unnecessary_semicolon",
        "unreadable_literal",
        "use_self",
        "used_underscore_binding",
        "used_underscore_items",
        "useless_let_if_seq",
        "while_float",
        "wildcard_imports",
    ]);

    let actual_allowed: BTreeSet<_> = lints
        .iter()
        .filter_map(|(lint, level)| (*level == "\"allow\"").then_some(*lint))
        .collect();
    assert_eq!(
        actual_allowed, allowed,
        "package-level Clippy allowances should stay on the audited mechanical-translation allowlist"
    );

    for lint in ["all", "cargo", "nursery", "pedantic", "missing_safety_doc"] {
        assert!(
            !matches!(lints.get(lint), Some(&"\"allow\"")),
            "{lint} must not become a package-level allowance"
        );
    }
}

#[test]
fn phase6_unsafe_op_allowances_stay_intentional() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src");
    let mut sources = Vec::new();
    collect_rs_sources(root, &src, &mut sources);

    let actual = sources
        .iter()
        .filter_map(|(rel, source)| {
            source
                .contains("unsafe_op_in_unsafe_fn")
                .then_some(rel.as_str())
        })
        .collect::<BTreeSet<_>>();
    let expected = BTreeSet::<&str>::new();

    assert_eq!(
        actual, expected,
        "broad unsafe_op_in_unsafe_fn allowances should not be reintroduced"
    );
    assert!(
        !actual.contains("src/allocator/aligned.rs")
            && !actual.contains("src/allocator/alloc.rs")
            && !actual.contains("src/raw/posix.rs")
            && !actual.contains("src/raw/macros.rs")
            && !actual.contains("src/allocator/arena_meta.rs")
            && !actual.contains("src/internal/atomic.rs")
            && !actual.contains("src/internal/bitmap.rs")
            && !actual.contains("src/types/mod.rs")
            && !actual.contains("src/allocator/free.rs")
            && !actual.contains("src/allocator/heap.rs")
            && !actual.contains("src/internal/mod.rs")
            && !actual.contains("src/ffi/libc.rs")
            && !actual.contains("src/allocator/options.rs")
            && !actual.contains("src/platform/os.rs")
            && !actual.contains("src/allocator/page.rs")
            && !actual.contains("src/platform/mod.rs")
            && !actual.contains("src/platform/unix.rs")
            && !actual.contains("src/internal/random.rs")
            && !actual.contains("src/allocator/thread_heap.rs")
            && !actual.contains("src/allocator/threadlocal.rs")
            && !actual.contains("src/allocator/tracking.rs"),
        "allocation helpers, aligned allocation helpers, allocation override shims, POSIX allocation shims, arena metadata, atomic/lock helpers, bitmap helpers, core types, free-path helpers, heap helpers, internal helpers, libc helpers, options helpers, OS boundary helpers, page helpers, Unix/Emscripten/WASI/macOS primitive helpers, primitive TLS helpers, random helpers, thread-heap helpers, thread-local helpers, and tracking shims should not regain unsafe_op_in_unsafe_fn after explicit unsafe cleanup"
    );
}

#[test]
fn phase6_c_compat_allowances_stay_intentional() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src");
    let mut sources = Vec::new();
    collect_rs_sources(root, &src, &mut sources);

    const LINTS: &[&str] = &[
        "non_camel_case_types",
        "non_upper_case_globals",
        "non_snake_case",
        "unused_macros",
        "unused_imports",
        "improper_ctypes_definitions",
    ];
    let expected = BTreeMap::from([
        (("src/ffi/abi.rs", "non_camel_case_types"), 1),
        (("src/ffi/abi.rs", "non_upper_case_globals"), 1),
        (("src/allocator/alloc.rs", "non_snake_case"), 1),
        (("src/allocator/aligned.rs", "non_camel_case_types"), 1),
        (("src/allocator/aligned.rs", "non_upper_case_globals"), 1),
        (("src/raw/macros.rs", "improper_ctypes_definitions"), 1),
        (("src/raw/macros.rs", "non_camel_case_types"), 1),
        (("src/raw/macros.rs", "non_snake_case"), 1),
        (("src/raw/macros.rs", "non_upper_case_globals"), 1),
        (("src/allocator/arena.rs", "non_camel_case_types"), 1),
        (("src/allocator/arena.rs", "non_upper_case_globals"), 1),
        (("src/allocator/arena/debug.rs", "non_camel_case_types"), 2),
        (
            ("src/allocator/arena/heap_pages.rs", "non_camel_case_types"),
            2,
        ),
        (("src/allocator/arena/prefix.rs", "non_camel_case_types"), 1),
        (("src/allocator/arena_meta.rs", "non_camel_case_types"), 1),
        (("src/allocator/arena_meta.rs", "non_upper_case_globals"), 1),
        (("src/internal/atomic.rs", "non_camel_case_types"), 1),
        (("src/internal/atomic.rs", "non_upper_case_globals"), 1),
        (("src/internal/bitmap.rs", "non_camel_case_types"), 1),
        (("src/internal/bitmap.rs", "non_snake_case"), 1),
        (("src/internal/bitmap.rs", "non_upper_case_globals"), 1),
        (("src/internal/bits.rs", "non_camel_case_types"), 1),
        (("src/internal/bits.rs", "non_upper_case_globals"), 1),
        (("src/ffi/headers.rs", "non_upper_case_globals"), 14),
        (("src/allocator/init.rs", "non_camel_case_types"), 1),
        (("src/allocator/init.rs", "non_snake_case"), 7),
        (("src/allocator/init.rs", "non_upper_case_globals"), 1),
        (("src/internal/mod.rs", "non_camel_case_types"), 1),
        (("src/internal/memid.rs", "non_camel_case_types"), 1),
        (("src/internal/mod.rs", "non_snake_case"), 1),
        (("src/internal/mod.rs", "non_upper_case_globals"), 1),
        (("src/internal/mod.rs", "unused_imports"), 1),
        (("src/lib.rs", "unused_imports"), 2),
        (("src/ffi/libc.rs", "non_camel_case_types"), 1),
        (("src/ffi/libc.rs", "non_upper_case_globals"), 1),
        (("src/allocator/options.rs", "non_camel_case_types"), 1),
        (("src/allocator/options.rs", "non_upper_case_globals"), 1),
        (("src/platform/os.rs", "non_camel_case_types"), 1),
        (("src/platform/os.rs", "non_upper_case_globals"), 1),
        (("src/allocator/page.rs", "non_camel_case_types"), 1),
        (("src/allocator/page.rs", "non_upper_case_globals"), 1),
        (("src/allocator/page_map.rs", "non_camel_case_types"), 1),
        (("src/allocator/page_map.rs", "non_upper_case_globals"), 1),
        (("src/platform/mod.rs", "non_camel_case_types"), 1),
        (("src/platform/mod.rs", "non_upper_case_globals"), 1),
        (("src/platform/emscripten.rs", "non_camel_case_types"), 1),
        (("src/platform/emscripten.rs", "non_upper_case_globals"), 1),
        (("src/platform/macos.rs", "non_camel_case_types"), 1),
        (("src/platform/macos.rs", "non_snake_case"), 1),
        (("src/platform/macos.rs", "non_upper_case_globals"), 1),
        (("src/platform/macos.rs", "unused_imports"), 1),
        (("src/platform/macos.rs", "unused_macros"), 1),
        (("src/platform/unix.rs", "non_camel_case_types"), 1),
        (("src/platform/unix.rs", "non_upper_case_globals"), 1),
        (("src/platform/wasi.rs", "non_camel_case_types"), 1),
        (("src/platform/wasi.rs", "non_upper_case_globals"), 1),
        (("src/platform/windows.rs", "non_camel_case_types"), 1),
        (("src/platform/windows.rs", "non_snake_case"), 1),
        (("src/platform/windows.rs", "non_upper_case_globals"), 1),
        (("src/platform/windows.rs", "unused_macros"), 1),
        (("src/internal/random.rs", "non_camel_case_types"), 1),
        (("src/internal/random.rs", "non_upper_case_globals"), 1),
        (("src/raw/mod.rs", "unused_imports"), 1),
        (("src/allocator/stats.rs", "non_camel_case_types"), 1),
        (("src/allocator/stats.rs", "non_upper_case_globals"), 1),
        (("src/allocator/thread_heap.rs", "non_camel_case_types"), 1),
        (("src/allocator/threadlocal.rs", "non_camel_case_types"), 1),
        (
            ("src/allocator/threadlocal.rs", "non_upper_case_globals"),
            1,
        ),
        (("src/types/constants.rs", "non_camel_case_types"), 1),
        (("src/types/constants.rs", "non_upper_case_globals"), 1),
    ]);

    assert_eq!(
        rust_allowance_counts(&sources, LINTS),
        expected,
        "C compatibility allowances should stay limited to audited ABI, re-export, and mechanical-translation surfaces"
    );
    for cleaned in ["src/allocator/static_alloc.rs", "src/allocator/tracking.rs"] {
        assert!(
            !rust_allowance_counts(&sources, LINTS)
                .keys()
                .any(|(rel, _)| *rel == cleaned),
            "{cleaned} should not regain C naming allowances"
        );
    }
}

#[test]
fn aligned_os_hints_are_randomized_like_c_release_builds() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let os_rs =
        fs::read_to_string(root.join("src/platform/os.rs")).expect("read src/platform/os.rs");
    let init_rs =
        fs::read_to_string(root.join("src/allocator/init.rs")).expect("read src/allocator/init.rs");

    assert!(
        os_rs.contains("let mut init = MI_HINT_BASE;")
            && os_rs.contains("if cfg!(feature = \"secure\") || cfg!(not(debug_assertions))")
            && os_rs.contains("let theap = unsafe { _mi_theap_default() };")
            && os_rs.contains("if unsafe { !mi_theap_is_initialized(theap) }")
            && os_rs.contains("return core::ptr::null_mut();")
            && os_rs.contains("let r = unsafe { crate::theap::_mi_theap_random_next(theap) };")
            && os_rs.contains("init += (MI_HINT_ALIGN * ((r >> 17) & 0xFFFFF)) % MI_HINT_AREA;")
            && os_rs.contains("hint.saturating_add(size),\n            init,"),
        "aligned OS allocation hints should randomize the base from the thread heap like C mimalloc"
    );
    assert!(
        init_rs.contains(
            "crate::random::_mi_random_reinit_if_weak(ptr::addr_of_mut!(theap_main.random))"
        ),
        "process initialization should reinitialize a weak main random context like C mimalloc"
    );
}

#[test]
fn cxx_new_wrappers_export_itanium_abi_symbols() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let override_rs =
        fs::read_to_string(root.join("src/raw/macros.rs")).expect("read src/raw/macros.rs");

    assert!(
        override_rs.contains("export_name = \"_Znwm\"")
            && override_rs.contains("export_name = \"_Znam\"")
            && override_rs.contains("not(mi_c_malloc_fast)")
            && override_rs.contains("not(mi_cxx_shim_linked)")
            && override_rs.contains("export_name = \"_ZnwmRKSt9nothrow_t\"")
            && override_rs.contains("export_name = \"_ZnamRKSt9nothrow_t\""),
        "64-bit Rust C++ operator new fallbacks must export Itanium ABI symbols only when the C++ shim is not linked"
    );
    assert!(
        override_rs.contains("export_name = \"_ZnwmSt11align_val_t\"")
            && override_rs.contains("export_name = \"_ZnamSt11align_val_t\"")
            && override_rs.contains("export_name = \"_ZnwmSt11align_val_tRKSt9nothrow_t\"")
            && override_rs.contains("export_name = \"_ZnamSt11align_val_tRKSt9nothrow_t\""),
        "aligned C++ operator new wrappers must export Itanium ABI symbols"
    );
}

#[test]
fn linux_cdylib_export_script_declares_allocator_symbols() {
    let build_rs = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("build.rs"))
        .expect("read build.rs");

    assert!(
        build_rs.contains("if c_abi")
            && build_rs.contains("cargo:rustc-link-arg=-Wl,--version-script="),
        "Linux C ABI artifact builds should pass the allocator export script through the cdylib link step"
    );
    assert!(
        build_rs.contains("PUBLIC_HEADERS")
            && build_rs.contains("CXX_EXPORT_PATTERNS")
            && build_rs.contains("MALLOC_EXPORT_PATTERNS")
            && build_rs.contains("public_mi_symbols_from_headers")
            && build_rs.contains("if cxx_shim_linked")
            && build_rs.contains("if interpose")
            && build_rs.contains("exports.push_str(&symbol)")
            && !build_rs.contains("mi_*;")
            && build_rs.contains("local:\n    *;"),
        "version script should export exact public-header mi_* APIs while hiding unmatched internals"
    );
    assert!(
        !build_rs.contains("_Z*;")
            && build_rs.contains("\"_Zda*\"")
            && build_rs.contains("\"_Zdl*\"")
            && build_rs.contains("\"_Zna*\"")
            && build_rs.contains("\"_Znw*\"")
            && build_rs.contains("\"malloc*\"")
            && build_rs.contains("\"free*\"")
            && build_rs.contains("\"realloc*\""),
        "version script must keep allocator and C++ ABI override names available only through mode groups"
    );
}
