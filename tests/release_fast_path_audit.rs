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

#[path = "release_fast_path_audit/release_policy.rs"]
mod release_policy;
#[path = "release_fast_path_audit/todo_backlog.rs"]
mod todo_backlog;
#[path = "release_fast_path_audit/unsafe_performance.rs"]
mod unsafe_performance;

#[test]
fn debug_assert_macros_are_release_gated_like_c_mimalloc() {
    let internal_rs = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/internal/mod.rs");
    let source = fs::read_to_string(internal_rs).expect("read src/internal/mod.rs");

    assert!(
        source.contains("if $crate::internal::MI_DEBUG > 0 && !$expr"),
        "mi_debug_assert! must compile out its expression when MI_DEBUG == 0"
    );
    assert!(
        source.contains("if $crate::internal::MI_DEBUG > 0 {\n            let left = &$left;"),
        "mi_debug_assert_eq! must compile out operand evaluation when MI_DEBUG == 0"
    );
}

#[test]
fn huge_allocation_stats_are_release_gated_like_c_mimalloc() {
    let page_rs = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/allocator/page.rs");
    let source = fs::read_to_string(page_rs).expect("read src/allocator/page.rs");
    let start = source
        .find("unsafe fn mi_huge_page_alloc")
        .expect("find mi_huge_page_alloc");
    let end = source[start..]
        .find("\n}\n\n#[inline(always)]")
        .map(|offset| start + offset)
        .expect("find end of mi_huge_page_alloc");
    let body = &source[start..end];

    assert!(
        body.contains(
            "if MI_STAT > 0 {\n                __mi_stat_increase(\n                    ptr::addr_of_mut!((*theap).stats.malloc_huge),"
        ) && body.contains(
            "__mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.malloc_huge_count), 1);"
        ),
        "release huge allocation stats must compile out when MI_STAT == 0, matching C mi_theap_stat_* macros"
    );
}

#[test]
fn preloading_state_matches_c_mimalloc_lifecycle() {
    let init_rs = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/allocator/init.rs");
    let source = fs::read_to_string(init_rs).expect("read src/allocator/init.rs");
    let preload_fn = source
        .find("pub(crate) extern \"C\" fn _mi_preloading() -> bool")
        .expect("find _mi_preloading");
    let thread_init = source
        .find("pub unsafe extern \"C\" fn mi_thread_init()")
        .expect("find mi_thread_init");
    let auto_init = source
        .find("pub(crate) unsafe extern \"C\" fn _mi_auto_process_init()")
        .expect("find _mi_auto_process_init");
    let process_init_inner = source
        .find("unsafe fn mi_process_init_inner()")
        .expect("find mi_process_init_inner");
    let process_init = source
        .find("pub unsafe extern \"C\" fn mi_process_init()")
        .expect("find mi_process_init");
    let process_done = source
        .find("pub unsafe extern \"C\" fn mi_process_done()")
        .expect("find mi_process_done");

    assert!(
        source.contains("static OS_PRELOADING: AtomicBool = AtomicBool::new(true);"),
        "_mi_preloading should default to true until process initialization, matching C"
    );
    assert!(
        source[..preload_fn].ends_with("#[inline(never)]\n")
            && source[preload_fn..].contains("OS_PRELOADING.load(Ordering::Relaxed)")
            && !source[preload_fn..preload_fn + 120].contains("false"),
        "_mi_preloading must read process state instead of returning a constant"
    );
    assert!(
        source[auto_init..].contains("OS_PRELOADING.store(false, Ordering::Relaxed);"),
        "_mi_auto_process_init should clear preloading before allocator initialization"
    );
    assert!(
        source[thread_init..auto_init].contains("mi_process_init()")
            && !source[thread_init..auto_init].contains("_mi_auto_process_init()"),
        "allocation-triggered thread init should call mi_process_init, not the constructor-only auto init path"
    );
    assert!(
        source[process_init..process_done].contains("_mi_auto_process_init()")
            && !source[process_init..process_done].contains("mi_process_init_inner()"),
        "mi_process_init should enter the same post-preload activation path as process attach"
    );
    assert!(
        source[process_init_inner..process_init].contains("_mi_process_is_initialized = true;"),
        "mi_process_init_inner should set process initialization state like C's mi_process_init_once"
    );
    assert!(
        source[process_init_inner..process_init].contains("mi_detect_cpu_features();")
            && source[process_init_inner..process_init].find("mi_detect_cpu_features();")
                < source[process_init_inner..process_init].find("mi_subproc_main_init();"),
        "process initialization should detect CPU copy/popcnt features before allocator subsystem initialization"
    );
    assert!(
        source[process_done..].contains("OS_PRELOADING.store(true, Ordering::Relaxed);"),
        "mi_process_done should restore preloading after allocator teardown"
    );
}

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
            && libc_rs.contains("*out.ptr.add(out.used) = 0;"),
        "Rust-side bounded snprintf formatting should write directly into the caller buffer"
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
        atomic_rs.contains(
            "#[cfg(all(target_os = \"linux\", not(debug_assertions)))]\n#[link(name = \"pthread\")]"
        )
            && atomic_rs.contains("fn pthread_mutex_trylock(mutex: *mut pthread_mutex_t)")
            && atomic_rs.contains("fn pthread_mutex_lock(mutex: *mut pthread_mutex_t)")
            && atomic_rs.contains("fn pthread_mutex_unlock(mutex: *mut pthread_mutex_t)")
            && atomic_rs.contains("fn pthread_mutex_destroy(mutex: *mut pthread_mutex_t)")
            && atomic_rs.contains("pthread_mutex_trylock(ptr::addr_of_mut!((*lock).mutex)) == 0")
            && atomic_rs.contains("pthread_mutex_lock(ptr::addr_of_mut!((*lock).mutex))")
            && atomic_rs.contains("pthread_mutex_unlock(ptr::addr_of_mut!((*lock).mutex))")
            && atomic_rs.contains("pthread_mutex_destroy(ptr::addr_of_mut!((*lock).mutex))")
            && atomic_rs.contains("#[cfg(not(all(target_os = \"linux\", not(debug_assertions))))]\npub unsafe extern \"C\" fn mi_lock_acquire"),
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
            && alloc_rs.contains("fn product_size_or_report(count: usize, size: usize)")
            && alloc_rs.contains("pub(crate) fn calloc_request_or_report(")
            && alloc_rs.contains("fn alloc_product_or_report(")
            && alloc_rs
                .contains("crate::internal::mi_count_size_overflow(count, size, &mut total)")
            && alloc_rs.contains(
                "crate::core_types::CallocRequest::from_checked_total(count, size, total).ok()"
            )
            && alloc_rs.contains(
                "crate::core_types::AllocProduct::from_checked_total(count, size, total).ok()"
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
    let mut sources = Vec::new();
    collect_rs_sources(root, &src, &mut sources);

    let core_mod =
        fs::read_to_string(root.join("src/types/mod.rs")).expect("read core_types/mod.rs");
    let core_layout_tests = fs::read_to_string(root.join("src/types/layout_tests.rs"))
        .expect("read core_types layout tests");
    let heap_rs =
        fs::read_to_string(root.join("src/types/heap.rs")).expect("read core_types/heap.rs");
    let heap_api_rs =
        fs::read_to_string(root.join("src/allocator/heap.rs")).expect("read src/allocator/heap.rs");
    let ptr_rs = fs::read_to_string(root.join("src/types/ptr.rs")).expect("read core_types/ptr.rs");
    let scalar_rs =
        fs::read_to_string(root.join("src/types/scalar.rs")).expect("read core_types/scalar.rs");
    let page_states_rs = fs::read_to_string(root.join("src/types/page_states.rs"))
        .expect("read core_types/page_states.rs");
    let validation_extra_rs = fs::read_to_string(root.join("src/types/validation_extra.rs"))
        .expect("read core_types/validation_extra.rs");
    for module in [
        "arena",
        "atomic",
        "free_actions",
        "freelist",
        "freelist_wrappers",
        "heap",
        "layout",
        "memid",
        "page",
        "page_queues",
        "page_states",
        "ptr",
        "scalar",
        "validation",
        "validation_extra",
    ] {
        assert!(
            core_mod.contains(&format!("mod {module};")),
            "core_types should own the {module} semantic wrapper module"
        );
    }

    for witness in [
        "LocalPage",
        "ForeignPage",
        "AbandonedPageType",
        "AbandonedMappedPageType",
        "OwnedAbandonedPage",
        "ActivePage",
        "AvailablePage",
        "FullPage",
        "RetiredPage",
        "DestroyedPage",
    ] {
        assert!(
            page_states_rs.contains(&format!("pub(crate) struct {witness}(PagePtr);")),
            "{witness} should keep its page pointer field private"
        );
        assert!(
            !page_states_rs.contains(&format!("pub(crate) struct {witness}(pub(crate) PagePtr);")),
            "{witness} should not expose tuple construction outside page_states"
        );
    }
    assert!(
        page_states_rs.contains("pub(crate) unsafe fn new_unchecked(page: PagePtr) -> Self")
            && page_states_rs.contains("pub(crate) fn page(self) -> PagePtr"),
        "page-state witnesses should expose explicit unchecked constructors and page accessors"
    );
    assert!(
        page_states_rs.contains("pub(crate) struct ThreadHeapLocal(ThreadHeapPtr);")
            && page_states_rs
                .contains("pub(crate) unsafe fn new_unchecked(theap: ThreadHeapPtr) -> Self")
            && page_states_rs.contains("pub(crate) fn ptr(self) -> ThreadHeapPtr")
            && page_states_rs
                .contains("pub(crate) fn borrowed(self) -> crate::core_types::ThreadHeapBorrowed")
            && page_states_rs
                .contains("pub(crate) unsafe fn state(self) -> crate::core_types::ThreadHeapState")
            && !page_states_rs
                .contains("pub(crate) struct ThreadHeapLocal(pub(crate) ThreadHeapPtr);"),
        "ThreadHeapLocal should keep its thread-heap pointer private behind explicit construction"
    );
    assert!(
        heap_rs.contains("pub(crate) struct ThreadHeapBorrowed(ThreadHeapPtr);")
            && heap_rs.contains("pub(crate) enum ThreadHeapState")
            && heap_rs.contains("Uninitialized(ThreadHeapPtr)")
            && heap_rs.contains("Initialized(ThreadHeapBorrowed)")
            && heap_rs.contains("Main(ThreadHeapBorrowed)")
            && heap_rs.contains("Deleted(ThreadHeapPtr)")
            && heap_rs.contains("pub(crate) unsafe fn new(theap: ThreadHeapPtr) -> Option<Self>")
            && heap_rs.contains("if unsafe { theap.heap() }.is_some()")
            && heap_rs.contains("pub(crate) unsafe fn try_local(")
            && heap_rs.contains("match unsafe { self.state() }")
            && heap_rs.contains("ThreadHeapState::Uninitialized(_) | ThreadHeapState::Deleted(_) => return Err(self),")
            && heap_rs.contains("pub(crate) unsafe fn state(self) -> ThreadHeapState")
            && heap_rs.contains("if unsafe { self.tld() }.is_none()")
            && heap_rs.contains("crate::internal::_mi_is_heap_main(heap.as_ptr())")
            && heap_rs.contains(
                "const _: unsafe fn(ThreadHeapPtr) -> ThreadHeapState = ThreadHeapPtr::state;"
            )
            && core_mod.contains("ThreadHeapBorrowed")
            && core_mod.contains("ThreadHeapState")
            && core_layout_tests.contains("assert_layout!(ThreadHeapBorrowed, ThreadHeapPtr);"),
        "thread-heap witnesses should encode borrowed/local initialized, main, and deleted states"
    );
    assert!(
        heap_api_rs.contains("HeapPtr")
            && heap_api_rs.contains("ThreadHeapPtr")
            && heap_api_rs.contains("unsafe fn mi_heap_theap_typed(heap: Option<HeapPtr>) -> Option<ThreadHeapPtr>")
            && heap_api_rs.contains("unsafe { mi_heap_theap_typed(HeapPtr::new(heap)) }")
            && heap_api_rs.contains(".map(ThreadHeapPtr::as_ptr)")
            && heap_api_rs.contains("ThreadHeapPtr::new(unsafe { _mi_heap_theap(heap) })")
            && !heap_api_rs.contains("pub unsafe extern \"C\" fn mi_heap_theap(heap: *mut mi_heap_t) -> *mut mi_theap_t {\n    // SAFETY: upheld by the caller contract for this C ABI boundary.\n    unsafe { _mi_heap_theap(heap) }\n}"),
        "mi_heap_theap should parse the raw ABI pointer once and use HeapPtr/ThreadHeapPtr internally"
    );
    assert!(
        ptr_rs.contains("pub(crate) unsafe fn numa_node(self) -> NumaNode")
            && ptr_rs.contains("pub(crate) unsafe fn set_numa_node(self, node: NumaNode)")
            && heap_api_rs.contains("HeapPtr::new(unsafe { mi_heap_main() })")
            && heap_api_rs.contains("HeapPtr::new(heap)")
            && heap_api_rs.contains(
                "unsafe fn mi_heap_set_numa_affinity_typed(heap: HeapPtr, numa_node: crate::core_types::NumaNode)"
            )
            && heap_api_rs.contains("heap.set_numa_node(numa_node.normalize(numa_count))")
            && !heap_api_rs.contains("(*heap).numa_node = numa_node.normalize(numa_count).get();"),
        "mi_heap_set_numa_affinity should parse the raw ABI pointer once and use HeapPtr/NumaNode internally"
    );
    for witness in ["MainHeap", "OwnedHeap", "BorrowedHeap"] {
        assert!(
            heap_rs.contains(&format!("pub(crate) struct {witness}(HeapPtr);"))
                && heap_rs.contains("pub(crate) unsafe fn new_unchecked(heap: HeapPtr) -> Self")
                && heap_rs.contains("pub(crate) fn ptr(self) -> HeapPtr")
                && !heap_rs.contains(&format!("pub(crate) struct {witness}(pub(crate) HeapPtr);")),
            "{witness} should be a live heap wrapper with explicit construction and pointer access"
        );
        assert!(
            core_mod.contains("pub(crate) use heap::")
                && core_mod.contains(witness)
                && core_layout_tests.contains(&format!("assert_layout!({witness}, HeapPtr);")),
            "{witness} should be re-exported and layout-checked"
        );
    }
    assert!(
        heap_rs.contains("pub(crate) unsafe fn new(heap: HeapPtr) -> Option<Self>")
            && heap_rs.contains("crate::internal::_mi_is_heap_main(heap.as_ptr())")
            && heap_rs.contains("const _: unsafe fn(HeapPtr) -> Option<MainHeap> = MainHeap::new;"),
        "MainHeap should have a checked constructor backed by the runtime main-heap predicate"
    );
    assert!(
        heap_rs.contains("const _: unsafe fn(HeapPtr) -> Option<OwnedHeap> = OwnedHeap::new;")
            && heap_rs.contains("impl OwnedHeap {\n    /// # Safety\n    /// `heap` must point to initialized heap metadata owned by the current\n    /// delete or destroy path.\n    #[inline(always)]\n    pub(crate) unsafe fn new(heap: HeapPtr) -> Option<Self> {\n        if unsafe { crate::internal::_mi_is_heap_main(heap.as_ptr()) } {\n            None\n        } else {\n            Some(Self(heap))\n        }\n    }"),
        "OwnedHeap should have a checked constructor that rejects the main heap"
    );
    assert!(
        heap_rs.contains("pub(crate) unsafe fn from_allocation_raw(heap: *mut mi_heap_t) -> Option<Self>")
            && heap_rs.contains("pub(crate) unsafe fn from_lookup_raw(heap: *mut mi_heap_t) -> Option<Self>")
            && heap_rs.contains("const _: unsafe fn(*mut mi_heap_t) -> Option<BorrowedHeap> = BorrowedHeap::from_allocation_raw;")
            && heap_rs.contains("const _: unsafe fn(*mut mi_heap_t) -> Option<BorrowedHeap> = BorrowedHeap::from_lookup_raw;")
            && heap_rs.matches("HeapPtr::new(heap).map(Self)").count() >= 2,
        "BorrowedHeap should have checked raw constructors for allocation and lookup contexts"
    );
    for witness in ["MainSubproc", "BorrowedSubproc"] {
        assert!(
            heap_rs.contains(&format!("pub(crate) struct {witness}(SubprocPtr);"))
                && heap_rs.contains("pub(crate) fn ptr(self) -> SubprocPtr")
                && !heap_rs.contains(&format!(
                    "pub(crate) struct {witness}(pub(crate) SubprocPtr);"
                ))
                && core_mod.contains(witness)
                && core_layout_tests.contains(&format!("assert_layout!({witness}, SubprocPtr);")),
            "{witness} should be a live subproc wrapper with explicit construction and pointer access"
        );
    }
    assert!(
        heap_rs.contains(
            "const _: unsafe fn(SubprocPtr) -> MainSubproc = MainSubproc::new_unchecked;"
        ) && heap_rs
            .contains("const _: unsafe fn(SubprocPtr) -> BorrowedSubproc = BorrowedSubproc::new;"),
        "subproc witness constructors should be compile-time checked"
    );
    assert!(
        ptr_rs.contains("pub(crate) unsafe fn thread_id(self) -> Option<ThreadId>")
            && ptr_rs
                .contains("pub(crate) unsafe fn set_thread_id(self, thread_id: Option<ThreadId>)")
            && ptr_rs.contains("pub(crate) unsafe fn subproc(self) -> Option<SubprocPtr>")
            && ptr_rs
                .contains("pub(crate) unsafe fn set_subproc(self, subproc: Option<SubprocPtr>)")
            && ptr_rs.contains("pub(crate) unsafe fn stats(self) -> Option<*mut mi_stats_t>")
            && ptr_rs.contains("self.subproc().map(|subproc| subproc.stats())")
            && ptr_rs.contains(
                "pub(crate) unsafe fn arena_count_cell(self) -> Option<AtomicUsizeField>"
            )
            && ptr_rs.contains(
                "pub(crate) unsafe fn arena(self, index: ArenaIndex) -> Option<ArenaPtr>"
            )
            && ptr_rs.contains(
                "pub(crate) unsafe fn set_arena(self, index: ArenaIndex, arena: Option<ArenaPtr>)"
            )
            && ptr_rs.contains("pub(crate) unsafe fn memid_ptr(self) -> *mut mi_memid_t"),
        "TldPtr should expose typed thread id, subproc, stats, memid, and arena-data accessors"
    );
    assert!(
        scalar_rs.contains("pub(crate) struct SubprocSeq(pub(crate) usize);")
            && scalar_rs.contains("pub(crate) struct ArenaCount(pub(crate) usize);")
            && scalar_rs.contains("pub(crate) struct ThreadCount(pub(crate) usize);")
            && scalar_rs.contains("pub(crate) struct HeapCount(pub(crate) usize);")
            && ptr_rs.contains(
                "pub(crate) unsafe fn arena_count(self, order: MemoryOrder) -> ArenaCount"
            )
            && ptr_rs.contains(
                "pub(crate) unsafe fn set_arena_count(self, count: ArenaCount, order: MemoryOrder)"
            )
            && ptr_rs.contains(
                "pub(crate) unsafe fn thread_count(self, order: MemoryOrder) -> ThreadCount"
            )
            && ptr_rs.contains(
                "pub(crate) unsafe fn thread_total_count(self, order: MemoryOrder) -> ThreadCount"
            )
            && ptr_rs
                .contains("pub(crate) unsafe fn heap_count(self, order: MemoryOrder) -> HeapCount")
            && ptr_rs.contains(
                "pub(crate) unsafe fn heap_total_count(self, order: MemoryOrder) -> HeapCount"
            ),
        "subproc sequence and count fields should have scalar wrappers plus typed SubprocPtr accessors"
    );
    let heap_delete_rs =
        fs::read_to_string(root.join("src/allocator/heap.rs")).expect("read heap.rs");
    let heap_delete_body = heap_delete_rs
        .split("pub unsafe extern \"C\" fn mi_heap_delete(heap: *mut mi_heap_t)")
        .nth(1)
        .expect("find mi_heap_delete body");
    let owned_reject = heap_delete_body
        .find("let Some(heap) = (unsafe { OwnedHeap::new(heap) }) else")
        .expect("mi_heap_delete should reject the main heap through OwnedHeap");
    let delete_typed = heap_delete_body
        .find("unsafe { mi_heap_delete_typed(heap) };")
        .expect("mi_heap_delete should enter the typed delete path after validation");
    assert!(
        owned_reject < delete_typed,
        "mi_heap_delete must reject MainHeap before entering the OwnedHeap delete path"
    );
    let heap_delete_typed_body = heap_delete_rs
        .split("unsafe fn mi_heap_delete_typed(heap: OwnedHeap)")
        .nth(1)
        .expect("find mi_heap_delete_typed body");
    assert!(
        heap_delete_typed_body.contains("mi_heap_free_theaps(heap);")
            && heap_delete_typed_body.contains("_mi_heap_move_pages(heap_raw, mi_heap_main());")
            && heap_delete_typed_body.contains("mi_heap_free(heap);"),
        "mi_heap_delete_typed should free thread heaps through OwnedHeap, move pages, and free OwnedHeap"
    );
    assert!(
        heap_delete_rs.contains("unsafe fn mi_heap_delete_typed(heap: OwnedHeap)")
            && heap_delete_rs.contains("unsafe fn mi_heap_free_theaps(heap: OwnedHeap)")
            && heap_delete_rs.contains("unsafe fn mi_heap_free(heap: OwnedHeap)")
            && !heap_delete_rs.contains("unsafe fn mi_heap_delete_typed(heap: MainHeap)")
            && !heap_delete_rs.contains("unsafe fn mi_heap_free(heap: MainHeap)"),
        "main heaps should have no typed entry point into owned deletion helpers"
    );
    let heap_destroy_body = heap_delete_rs
        .split("pub unsafe extern \"C\" fn mi_heap_destroy(heap: *mut mi_heap_t)")
        .nth(1)
        .expect("find mi_heap_destroy body");
    let destroy_main_reject = heap_destroy_body
        .find("let Some(heap) = (unsafe { OwnedHeap::new(heap) }) else")
        .expect("mi_heap_destroy should reject the main heap through OwnedHeap");
    let force_destroy = heap_destroy_body
        .find("mi_heap_force_destroy_owned(heap)")
        .expect("mi_heap_destroy should force-destroy an OwnedHeap after validation");
    assert!(
        destroy_main_reject < force_destroy,
        "mi_heap_destroy must reject MainHeap before the owned destroy path"
    );
    assert!(
        heap_delete_rs.contains("unsafe fn mi_heap_free(heap: OwnedHeap)")
            && heap_delete_rs.contains("unsafe fn mi_heap_force_destroy_main(heap: MainHeap)")
            && heap_delete_rs.contains("unsafe fn mi_heap_force_destroy_owned(heap: OwnedHeap)")
            && heap_delete_rs.contains("if let Some(heap) = MainHeap::new(heap)")
            && heap_delete_rs.contains("} else if let Some(heap) = OwnedHeap::new(heap)"),
        "heap deletion helpers should use OwnedHeap/MainHeap typed teardown witnesses"
    );
    let heap_check_owned_body = heap_delete_rs
        .split("pub unsafe extern \"C\" fn mi_heap_check_owned(heap: *mut mi_heap_t, p: *const c_void) -> bool")
        .nth(1)
        .expect("find mi_heap_check_owned body");
    assert!(
        heap_check_owned_body.contains("BorrowedHeap::from_lookup_raw(heap)")
            && heap_check_owned_body.contains("unsafe { mi_heap_check_owned_typed(heap, p) }")
            && heap_delete_rs.contains(
                "unsafe fn mi_heap_check_owned_typed(heap: BorrowedHeap, p: *const c_void) -> bool"
            )
            && heap_delete_rs.contains("OwnedHeap::new(heap.ptr())")
            && heap_delete_rs.contains(
                "unsafe fn mi_heap_check_owned_owned(heap: OwnedHeap, p: *const c_void) -> bool"
            )
            && heap_delete_rs.contains(
                "unsafe fn mi_heap_check_owned_borrowed(heap: BorrowedHeap, p: *const c_void) -> bool"
            ),
        "mi_heap_check_owned should parse heap arguments through BorrowedHeap and route non-main checks through OwnedHeap"
    );
    let page_queues_rs =
        fs::read_to_string(root.join("src/types/page_queues.rs")).expect("read page_queues.rs");
    for witness in ["NormalPageQueue", "HugePageQueue", "FullPageQueue"] {
        assert!(
            page_queues_rs.contains(&format!("pub(crate) struct {witness}(PageQueuePtr);")),
            "{witness} should keep its queue pointer field private"
        );
        assert!(
            !page_queues_rs.contains(&format!(
                "pub(crate) struct {witness}(pub(crate) PageQueuePtr);"
            )),
            "{witness} should not expose tuple construction outside page_queues"
        );
    }
    assert!(
        page_queues_rs.contains("pub(crate) unsafe fn new_unchecked(ptr: PageQueuePtr) -> Self")
            && page_queues_rs
                .contains("pub(crate) unsafe fn new(ptr: PageQueuePtr) -> Option<Self>")
            && page_queues_rs.contains("pub(crate) fn ptr(self) -> PageQueuePtr"),
        "page queue witnesses should expose checked/unchecked constructors and queue accessors"
    );
    assert!(
        page_queues_rs.contains("block_size.get() <= crate::types::MI_LARGE_MAX_OBJ_SIZE")
            && page_queues_rs
                .contains("crate::types::MI_LARGE_MAX_OBJ_SIZE + size_of::<usize>()")
            && page_queues_rs
                .contains("crate::types::MI_LARGE_MAX_OBJ_SIZE + (2 * size_of::<usize>())")
            && page_queues_rs
                .contains("const _: unsafe fn(PageQueuePtr) -> Option<NormalPageQueue> = NormalPageQueue::new;")
            && page_queues_rs
                .contains("const _: unsafe fn(PageQueuePtr) -> Option<HugePageQueue> = HugePageQueue::new;")
            && page_queues_rs
                .contains("const _: unsafe fn(PageQueuePtr) -> Option<FullPageQueue> = FullPageQueue::new;"),
        "page queue checked constructors should inspect normal/huge/full queue block-size identity"
    );
    assert!(
        page_queues_rs.contains("impl QueuedPage<NormalQueue>")
            && page_queues_rs.contains(
                "pub(crate) unsafe fn new(\n        page: crate::core_types::ActivePage,\n        queue: NormalPageQueue,"
            )
            && page_queues_rs.contains("impl QueuedPage<FullQueue>")
            && page_queues_rs.contains(
                "pub(crate) unsafe fn new(\n        page: crate::core_types::FullPage,\n        queue: FullPageQueue,"
            )
            && page_queues_rs.contains("crate::page::mi_page_queue_contains(queue.ptr().as_ptr(), page.page().as_ptr())")
            && page_queues_rs.contains("queue: PageQueue::normal(queue)")
            && page_queues_rs.contains("queue: PageQueue::full(queue)"),
        "queued-page witnesses should be constructed only after matching page/queue witnesses and linked-list membership checks"
    );
    assert!(
        page_queues_rs.contains("pub(crate) struct PageQueue<K> {\n    ptr: PageQueuePtr,")
            && !page_queues_rs.contains("pub(crate) ptr: PageQueuePtr")
            && page_queues_rs.contains("impl PageQueue<NormalQueue>")
            && page_queues_rs.contains("impl PageQueue<FullQueue>")
            && page_queues_rs.contains("pub(crate) fn normal(queue: NormalPageQueue) -> Self")
            && page_queues_rs.contains("pub(crate) fn full(queue: FullPageQueue) -> Self"),
        "generic PageQueue should keep its raw field private and expose only kind-specific constructors"
    );
    let memid_rs = fs::read_to_string(root.join("src/types/memid.rs")).expect("read memid.rs");
    let memid_tests_rs =
        fs::read_to_string(root.join("src/types/memid/tests.rs")).expect("read memid tests");
    assert!(
        memid_rs.contains("raw: crate::internal::mi_memid_t,")
            && !memid_rs.contains("pub(crate) raw: crate::internal::mi_memid_t,")
            && memid_rs.contains("pub(crate) const fn into_raw(self)")
            && memid_rs.contains("pub(crate) const fn as_raw(&self)"),
        "MemId should keep raw mi_memid_t private behind explicit raw forwarding accessors"
    );
    assert!(
        memid_rs.contains("pub(crate) enum MemIdView {")
            && memid_rs.contains("Unknown(MemKindCode),")
            && memid_rs
                .contains("pub(crate) fn view(raw: crate::internal::mi_memid_t) -> MemIdView")
            && memid_rs.contains("let os = unsafe { raw.mem.os };")
            && memid_rs.contains("let m = unsafe { raw.mem.meta };")
            && memid_rs.contains("let a = unsafe { raw.mem.arena };")
            && memid_rs.contains("return MemIdView::Unknown(code);")
            && memid_tests_rs
                .contains("fn memid_view_returns_unknown_for_null_meta_or_arena_union_pointers()")
            && memid_tests_rs
                .contains("fn memid_view_returns_unknown_for_invalid_arena_slice_range()")
            && memid_tests_rs.contains("MemKind::from(MemKindCode(99))")
            && memid_tests_rs.contains("MemKind::Unknown(MemKindCode(99))")
            && memid_rs.contains("MemKind::Unknown(c) => MemIdView::Unknown(c),"),
        "MemId::view should own union-arm reads and map unknown or corrupt discriminants to Unknown"
    );
    let page_core_rs =
        fs::read_to_string(root.join("src/types/page.rs")).expect("read core_types/page.rs");
    let atomic_rs = fs::read_to_string(root.join("src/types/atomic.rs")).expect("read atomic.rs");
    let internal_atomic_rs = fs::read_to_string(root.join("src/internal/atomic.rs"))
        .expect("read src/internal/atomic.rs");
    let strict_check_rs =
        fs::read_to_string(root.join("scripts/check-strict-types.py")).expect("read checker");
    assert!(
        page_core_rs.contains("pub(crate) struct PageFlags(usize);")
            && page_core_rs.contains("pub(crate) const fn new(bits: usize) -> Option<Self>")
            && page_core_rs.contains("if (bits & !MI_PAGE_FLAG_MASK) == 0")
            && page_core_rs.contains("pub(crate) struct PageThreadId(usize);")
            && page_core_rs.contains("pub(crate) fn from_parts(owner: PageOwnerId, flags: PageFlags) -> Self")
            && page_core_rs.contains("Self(tid_val | flags.bits())")
            && page_core_rs.contains("pub(crate) fn flags(self) -> PageFlags")
            && page_core_rs.contains("pub(crate) fn owner_bits(self) -> usize")
            && page_core_rs.contains("self.0 & !MI_PAGE_FLAG_MASK")
            && page_core_rs.contains("pub(crate) fn classify_free_path(self, current: ThreadId) -> FreePathKind")
            && atomic_rs.contains("pub(crate) struct AtomicPageThreadId(pub(crate) AtomicUsizeField);")
            && atomic_rs.contains("pub(crate) unsafe fn load(self) -> PageThreadId")
            && atomic_rs.contains("pub(crate) unsafe fn set_flags(self, flags: PageFlags, set: bool) -> PageThreadId")
            && atomic_rs.contains("unsafe { self.0.fetch_or(flags.bits(), Ordering::Relaxed) }")
            && atomic_rs.contains("unsafe { self.0.fetch_and(!flags.bits(), Ordering::Relaxed) }")
            && atomic_rs.contains("pub(crate) unsafe fn store_owner(self, owner: PageOwnerId)")
            && atomic_rs.contains("pub(crate) unsafe fn compare_exchange_owner(\n        self,\n        expected: &mut PageThreadId,")
            && strict_check_rs.contains("ALLOWED_XTHREAD_ID = {")
            && strict_check_rs.contains("\"src/types/page.rs\"")
            && strict_check_rs.contains("\"src/types/atomic.rs\"")
            && strict_check_rs.contains("kind=\"xthread-id-bitop\"")
            && strict_check_rs.contains("message=\"direct xthread_id bit operation outside typed helpers\""),
        "xthread_id bit operations should be isolated to PageThreadId/PageFlags/AtomicPageThreadId and enforced by the strict checker"
    );
    let freelist_rs = fs::read_to_string(root.join("src/types/freelist.rs"))
        .expect("read core_types/freelist.rs");
    assert!(
        freelist_rs.contains("pub(crate) struct ThreadFreeWord(pub(crate) usize);")
            && freelist_rs.contains("pub(crate) const OWNED_BIT: usize = 1;")
            && freelist_rs.contains("let ptr_val = self.0 & !Self::OWNED_BIT;")
            && freelist_rs.contains("use super::constants::mi_block_t;")
            && freelist_rs.contains("pub(crate) fn head_raw(self) -> *mut mi_block_t")
            && freelist_rs.contains("pub(crate) const fn is_owned(self) -> bool")
            && freelist_rs.contains("(self.0 & Self::OWNED_BIT) != 0")
            && freelist_rs.contains(
                "pub(crate) fn from_parts(head: Option<BlockPtr>, owned: ThreadFreeOwner) -> Self"
            )
            && freelist_rs
                .contains("crate::internal::mi_assert_internal!((val & Self::OWNED_BIT) == 0);")
            && freelist_rs.contains("Self(ptr_val | bit)")
            && atomic_rs
                .contains("pub(crate) struct AtomicThreadFree(pub(crate) AtomicUsizeField);")
            && atomic_rs.contains("pub(crate) fn load_relaxed(self) -> ThreadFreeWord")
            && atomic_rs.contains("pub(crate) fn push_remote_acq_rel(")
            && atomic_rs.contains(
                "pub(crate) unsafe fn release_acq_rel(self, expected: &mut ThreadFreeWord) -> bool"
            )
            && atomic_rs.contains("pub(crate) unsafe fn collect_acq_rel(self) -> RemoteFreeList")
            && atomic_rs.contains("pub(crate) unsafe fn claim_acq_rel(self) -> bool")
            && strict_check_rs.contains("ALLOWED_XTHREAD_FREE = {")
            && strict_check_rs.contains("\"src/types/freelist.rs\"")
            && strict_check_rs.contains("\"src/types/atomic.rs\"")
            && strict_check_rs.contains("kind=\"xthread-free-bitop\"")
            && strict_check_rs
                .contains("message=\"direct xthread_free bit operation outside typed helpers\""),
        "xthread_free bit operations should be isolated to ThreadFreeWord/AtomicThreadFree and enforced by the strict checker"
    );
    let ptr_core_rs =
        fs::read_to_string(root.join("src/types/ptr.rs")).expect("read core_types/ptr.rs");
    let core_types_layout_tests_rs = fs::read_to_string(root.join("src/types/layout_tests.rs"))
        .expect("read core_types layout tests");
    let freelist_wrappers_rs = fs::read_to_string(root.join("src/types/freelist_wrappers.rs"))
        .expect("read core_types/freelist_wrappers.rs");
    let internal_rs =
        fs::read_to_string(root.join("src/internal/mod.rs")).expect("read src/internal/mod.rs");
    let allowlist_rs =
        fs::read_to_string(root.join("strict-type-allowlist.txt")).expect("read allowlist");
    assert!(
        ptr_core_rs.contains("#[repr(transparent)]\n        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]\n        pub(crate) struct $name(NonNull<$raw_type>);")
            && ptr_core_rs.contains("#[inline(always)]\n            pub(crate) fn new(ptr: *mut $raw_type) -> Option<Self>")
            && ptr_core_rs.contains("#[inline(always)]\n            pub(crate) unsafe fn new_unchecked(ptr: *mut $raw_type) -> Self")
            && ptr_core_rs.contains("#[inline(always)]\n            pub(crate) fn as_ptr(self) -> *mut $raw_type")
            && ptr_core_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]\npub(crate) struct PageConstPtr(NonNull<mi_page_t>);")
            && page_states_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy)]\npub(crate) struct LocalPage(PagePtr);")
            && page_states_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy)]\npub(crate) struct OwnedAbandonedPage(PagePtr);")
            && page_queues_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy)]\npub(crate) struct NormalPageQueue(PageQueuePtr);")
            && page_queues_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy)]\npub(crate) struct FullPageQueue(PageQueuePtr);")
            && validation_extra_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy)]\npub(crate) struct FreeBlock(BlockPtr);")
            && freelist_wrappers_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy, Debug)]\npub(crate) struct AvailableFreeList(pub(crate) Option<BlockPtr>);")
            && freelist_wrappers_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy, Debug)]\npub(crate) struct LocalFreeList(pub(crate) Option<BlockPtr>);")
            && core_types_layout_tests_rs.contains("fn pointer_wrappers_match_nonnull_layout()")
            && core_types_layout_tests_rs.contains("fn optional_pointer_wrappers_are_pointer_sized()")
            && core_types_layout_tests_rs.contains("fn witness_wrappers_match_pointer_layout()")
            && core_types_layout_tests_rs
                .contains("fn packed_and_atomic_wrappers_match_raw_layout()"),
        "hot strict-type wrappers should stay transparent, copyable where they carry copyable fast-path witnesses, inlineable, and covered by layout/niche tests"
    );
    assert!(
        freelist_wrappers_rs.contains("pub(crate) struct EncodedPtr<T> {")
            && freelist_wrappers_rs.contains("word: EncodedWord,")
            && freelist_wrappers_rs.contains(
                "pub(crate) unsafe fn read_word_from_raw(block: *const mi_block_t) -> Self"
            )
            && freelist_wrappers_rs
                .contains("word: EncodedWord::new(unsafe { (*block.cast_mut()).next })")
            && freelist_wrappers_rs
                .contains("pub(crate) unsafe fn write_word_to_raw(self, block: BlockPtr)")
            && freelist_wrappers_rs
                .contains("unsafe { (*block.as_ptr()).next = self.word.get() };")
            && freelist_wrappers_rs
                .contains("unsafe fn read_from_raw(block: *const mi_block_t) -> Self")
            && freelist_wrappers_rs.contains("Self(unsafe { (*block.cast_mut()).next })")
            && freelist_wrappers_rs.contains("unsafe fn write_to(self, block: BlockPtr)")
            && freelist_wrappers_rs.contains("unsafe { (*block.as_ptr()).next = self.0 };")
            && freelist_wrappers_rs.contains("pub(crate) enum BlockLink {")
            && freelist_wrappers_rs.contains("FreeList(EncodedPtr<crate::types::mi_block_t>),")
            && freelist_wrappers_rs.contains("Guarded(GuardedBlockTag),")
            && freelist_wrappers_rs
                .contains("pub(crate) unsafe fn read_from_raw(block: *const mi_block_t) -> Self")
            && freelist_wrappers_rs
                .contains("Self::FreeList(unsafe { EncodedPtr::read_word_from_raw(block) })")
            && freelist_wrappers_rs
                .contains("pub(crate) unsafe fn write_to(self, block: BlockPtr)")
            && freelist_wrappers_rs
                .contains("Self::FreeList(link) => unsafe { link.write_word_to_raw(block) }")
            && freelist_wrappers_rs
                .contains("Self::Guarded(tag) => unsafe { tag.write_to(block) }")
            && freelist_wrappers_rs
                .contains("pub(crate) struct AvailableFreeList(pub(crate) Option<BlockPtr>);")
            && freelist_wrappers_rs
                .contains("pub(crate) struct LocalFreeList(pub(crate) Option<BlockPtr>);")
            && freelist_wrappers_rs
                .contains("let next_raw = unsafe { mi_block_next(raw_page, head_raw) };")
            && freelist_wrappers_rs
                .contains("unsafe { mi_block_set_next(raw_page, block_raw, prev_raw) };")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn encoded_link(self) -> EncodedPtr<mi_block_t>")
            && ptr_core_rs.contains("unsafe { EncodedPtr::read_word_from_raw(self.as_ptr()) }")
            && ptr_core_rs.contains("pub(crate) unsafe fn write_link(self, link: BlockLink)")
            && ptr_core_rs.contains("unsafe { link.write_to(self) };")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn next(self, page: PagePtr) -> Option<BlockPtr>")
            && ptr_core_rs.contains(
                "pub(crate) unsafe fn set_next(self, page: PagePtr, next: Option<BlockPtr>)"
            )
            && ptr_core_rs.contains("unsafe { self.write_link(BlockLink::FreeList(link)) };")
            && strict_check_rs.contains("ALLOWED_BLOCK_NEXT = {")
            && strict_check_rs.contains("\"src/types/freelist_wrappers.rs\"")
            && strict_check_rs.contains("kind=\"block-next-access\"")
            && strict_check_rs
                .contains("message=\"direct mi_block_s.next access outside free-list wrappers\""),
        "mi_block_s.next reads/writes should stay behind EncodedPtr, BlockLink, BlockPtr, or free-list wrappers and strict checker coverage"
    );
    assert!(
        atomic_rs.contains("pub(crate) struct AtomicUsizeField(pub(crate) *mut usize);")
            && atomic_rs.contains("pub(crate) struct AtomicI64Field(pub(crate) *mut i64);")
            && atomic_rs.contains("pub(crate) struct AtomicPtrField<T>(pub(crate) *mut *mut T);")
            && atomic_rs.contains("let atomic = unsafe { AtomicUsize::from_ptr(self.0) };")
            && atomic_rs.contains("let atomic = unsafe { AtomicI64::from_ptr(self.0) };")
            && atomic_rs.contains("let atomic = unsafe { AtomicPtr::from_ptr(self.0) };")
            && atomic_rs.contains("pub(crate) unsafe fn load(self, order: MemoryOrder) -> usize")
            && atomic_rs
                .contains("pub(crate) unsafe fn store(self, value: usize, order: MemoryOrder)")
            && atomic_rs.contains("pub(crate) unsafe fn load(self, order: MemoryOrder) -> i64")
            && atomic_rs
                .contains("pub(crate) unsafe fn store(self, value: i64, order: MemoryOrder)")
            && atomic_rs.contains("pub(crate) unsafe fn load(self, order: MemoryOrder) -> *mut T")
            && atomic_rs
                .contains("pub(crate) unsafe fn store(self, value: *mut T, order: MemoryOrder)")
            && strict_check_rs.contains("ALLOWED_ATOMIC_CAST = {")
            && strict_check_rs.contains("\"src/types/atomic.rs\"")
            && strict_check_rs.contains("kind=\"atomic-cast\"")
            && strict_check_rs
                .contains("message=\"pointer-to-atomic cast outside atomic field wrappers\"")
            && strict_check_rs.contains("ATOMIC_CAST_RE = re.compile")
            && !strict_check_rs.contains("\"src/internal/atomic.rs\",\n}")
            && strict_check_rs.contains("ALLOWLIST = ROOT / \"strict-type-allowlist.txt\""),
        "pointer-to-atomic casts for allocator metadata should be centralized in typed atomic field wrappers and guarded by the strict checker"
    );
    assert!(
        internal_atomic_rs.contains("pub enum mi_memory_order_e {")
            && internal_atomic_rs.contains("pub type mi_memory_order = mi_memory_order_e;")
            && internal_atomic_rs.contains("fn load_order(order: mi_memory_order_e) -> Ordering")
            && internal_atomic_rs.contains("fn store_order(order: mi_memory_order_e) -> Ordering")
            && internal_atomic_rs.contains("fn rmw_order(order: mi_memory_order_e) -> Ordering")
            && internal_atomic_rs.contains("fn cas_failure_order(order: mi_memory_order_e) -> Ordering")
            && internal_atomic_rs.contains("pub unsafe extern \"C\" fn mi_atomic_load_explicit(p: *const usize, mo: mi_memory_order_e) -> usize")
            && internal_atomic_rs.contains("pub unsafe extern \"C\" fn mi_atomic_store_explicit(p: *mut usize, x: usize, mo: mi_memory_order_e)")
            && atomic_rs.contains("pub(crate) type MemoryOrder = Ordering;")
            && atomic_rs
                .contains("pub(crate) unsafe fn load(self, order: MemoryOrder) -> usize")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn arena_count(self, order: MemoryOrder) -> ArenaCount")
            && ptr_core_rs.contains(
                "pub(crate) unsafe fn set_arena_count(self, count: ArenaCount, order: MemoryOrder)"
            )
            && ptr_core_rs
                .contains("pub(crate) unsafe fn heap_count(self, order: MemoryOrder) -> HeapCount"),
        "C-compatible mi_memory_order_e should stay in the atomic facade while internal wrappers use MemoryOrder/Ordering"
    );
    assert!(
        ptr_core_rs.contains("pub(crate) fn ptr_exposed_addr<T>(ptr: *const T) -> usize")
            && ptr_core_rs.contains("ptr as usize")
            && ptr_core_rs.contains("ptr.addr()")
            && ptr_core_rs.contains("pub(crate) fn ptr_addr_diff<T, U>(")
            && ptr_core_rs.contains("ptr_exposed_addr(hi).wrapping_sub(ptr_exposed_addr(lo))")
            && ptr_core_rs.contains("pub(crate) fn ptr_align_up<T>(")
            && ptr_core_rs
                .contains(".map_addr(|addr| crate::internal::_mi_align_up(addr, alignment))")
            && ptr_core_rs.contains(
                "crate::internal::_mi_align_up(ptr_exposed_addr(ptr), alignment) as *mut c_void"
            )
            && ptr_core_rs.contains("pub(crate) fn ptr_align_down<T>(")
            && ptr_core_rs
                .contains(".map_addr(|addr| crate::internal::_mi_align_down(addr, alignment))")
            && ptr_core_rs.contains(
                "crate::internal::_mi_align_down(ptr_exposed_addr(ptr), alignment) as *mut c_void"
            )
            && freelist_rs.contains("pub(crate) struct ThreadFreeWord(pub(crate) usize);")
            && freelist_rs.contains("use super::constants::mi_block_t;")
            && freelist_rs.contains("use super::ptr::{BlockPtr, ptr_exposed_addr};")
            && freelist_rs.contains("pub(crate) fn head_raw(self) -> *mut mi_block_t")
            && freelist_rs.contains("crate::provenance::resolve_address(ptr_val)")
            && freelist_rs.contains("ptr_val as *mut mi_block_t")
            && freelist_rs.contains("Some(unsafe { BlockPtr::new_unchecked(ptr) })")
            && freelist_rs.contains("let val = ptr_exposed_addr(ptr.as_ptr());")
            && freelist_rs.contains("crate::provenance::remember_address(val, ptr.as_ptr());")
            && freelist_wrappers_rs.contains("use super::ptr::{BlockPtr, ptr_exposed_addr};")
            && freelist_wrappers_rs.contains("pub(crate) struct EncodedWord(usize);")
            && freelist_wrappers_rs.contains("let word = ptr_exposed_addr(block);")
            && freelist_wrappers_rs.contains("crate::provenance::remember_address(word, block);")
            && freelist_wrappers_rs.contains("word as *mut mi_block_t")
            && internal_rs.contains("pub(crate) fn mi_ptr_decode(")
            && internal_rs.contains(
                "let decoded_addr = mi_rotr(x.get().wrapping_sub(keys[0]), keys[0]) ^ keys[1];"
            )
            && internal_rs.contains("crate::provenance::resolve_encoded(x.get(), null, keys)")
            && internal_rs.contains("let p = decoded_addr as *mut c_void;")
            && internal_rs.contains("pub(crate) fn mi_ptr_encode(")
            && internal_rs.contains("crate::core_types::ptr_exposed_addr(null)")
            && internal_rs.contains("crate::core_types::ptr_exposed_addr(p)")
            && internal_rs
                .contains("crate::provenance::remember_encoded(word.get(), null, keys, p);")
            && strict_check_rs.contains("PTR_USIZE_RE = re.compile")
            && strict_check_rs.contains("ALLOWED_PTR_USIZE = {")
            && strict_check_rs.contains("\"src/types/ptr.rs\"")
            && strict_check_rs.contains("kind=\"pointer-usize-roundtrip\"")
            && strict_check_rs.contains("domain=\"provenance\"")
            && strict_check_rs.contains(
                "message=\"pointer/integer round trip outside typed provenance helpers\""
            )
            && allowlist_rs.contains(
                "pointer-usize-roundtrip|src/types/freelist_wrappers.rs|*|encoded-pointer|"
            )
            && allowlist_rs
                .contains("pointer-usize-roundtrip|src/internal/mod.rs|*|packed-pointer|")
            && allowlist_rs.contains(
                "pointer-usize-roundtrip|src/allocator/aligned.rs|*|alignment/provenance|"
            )
            && allowlist_rs
                .contains("pointer-usize-roundtrip|src/allocator/alloc.rs|*|padding/provenance|")
            && allowlist_rs
                .contains("pointer-usize-roundtrip|src/types/freelist.rs|*|packed-thread-free|")
            && allowlist_rs.contains("pointer-usize-roundtrip|src/allocator/free.rs|*|validation|")
            && allowlist_rs.contains("pointer-usize-roundtrip|src/platform/os.rs|*|os-memory|"),
        "pointer-to-integer round trips should stay behind typed packed-pointer/provenance helpers or explicit strict-type allowlist boundaries"
    );
    assert!(
        strict_check_rs.contains("METADATA_REF_RE = re.compile")
            && strict_check_rs.contains("kind=\"metadata-reference\"")
            && strict_check_rs.contains("domain=\"metadata\"")
            && strict_check_rs
                .contains("message=\"broad raw metadata reference outside typed field helpers\"")
            && allowlist_rs.contains("metadata-reference|src/allocator/free.rs|*|mi_page_t|")
            && allowlist_rs.contains("metadata-reference|src/allocator/stats.rs|*|mi_heap_t|")
            && allowlist_rs.contains("metadata-reference|src/types/atomic.rs|*|atomic-field|")
            && ptr_core_rs.contains("let raw = self.as_ptr();")
            && ptr_core_rs.contains("ptr::read(ptr::addr_of!((*raw).block_size))")
            && ptr_core_rs
                .contains("ptr::write(ptr::addr_of_mut!((*raw).block_size), block_size.get())")
            && ptr_core_rs
                .contains("AvailableFreeList::from_raw(ptr::read(ptr::addr_of!((*raw).free)))")
            && ptr_core_rs.contains("ptr::write(ptr::addr_of_mut!((*raw).free), head)")
            && ptr_core_rs
                .contains("LocalFreeList::from_raw(ptr::read(ptr::addr_of!((*raw).local_free)))")
            && ptr_core_rs.contains("ptr::write(ptr::addr_of_mut!((*raw).local_free), head)")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn thread_id_cell(self) -> AtomicPageThreadId")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn thread_free_cell(self) -> AtomicThreadFree")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn heap_cell(self) -> AtomicPtrField<mi_heap_t>")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn arena_count_cell(self) -> AtomicUsizeField")
            && ptr_core_rs
                .contains("pub(crate) unsafe fn heap_count_cell(self) -> AtomicUsizeField"),
        "aliasable allocator metadata should be accessed through pointer wrapper field helpers and guarded against broad Rust references"
    );
    let callbacks_rs = fs::read_to_string(root.join("src/types/callbacks.rs"))
        .expect("read core_types/callbacks.rs");
    let types_rs = fs::read_to_string(root.join("src/types/constants.rs"))
        .expect("read src/types/constants.rs");
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let options_rs = fs::read_to_string(root.join("src/allocator/options.rs"))
        .expect("read src/allocator/options.rs");
    let page_rs =
        fs::read_to_string(root.join("src/allocator/page.rs")).expect("read src/allocator/page.rs");
    let arena_rs = fs::read_to_string(root.join("src/allocator/arena.rs"))
        .expect("read src/allocator/arena.rs");
    let init_rs =
        fs::read_to_string(root.join("src/allocator/init.rs")).expect("read src/allocator/init.rs");
    let threadlocal_rs = fs::read_to_string(root.join("src/allocator/threadlocal.rs"))
        .expect("read src/allocator/threadlocal.rs");
    assert!(
        callbacks_rs.contains("pub(crate) struct CallbackArg(*mut c_void);")
            && callbacks_rs.contains("callback_arg_wrapper!(DeferredFreeCallbackArg);")
            && callbacks_rs.contains("callback_arg_wrapper!(OutputCallbackArg);")
            && callbacks_rs.contains("callback_arg_wrapper!(ErrorCallbackArg);")
            && callbacks_rs.contains("callback_arg_wrapper!(HeapVisitCallbackArg);")
            && callbacks_rs.contains("callback_arg_wrapper!(BlockVisitCallbackArg);")
            && callbacks_rs.contains("callback_arg_wrapper!(ThreadLocalClaimCallbackArg);")
            && callbacks_rs.contains(
                "optional_callback_wrapper!(DeferredFreeCallback, mi_deferred_free_fun);"
            )
            && callbacks_rs.contains("optional_callback_wrapper!(OutputCallback, mi_output_fun);")
            && callbacks_rs.contains("optional_callback_wrapper!(ErrorCallback, mi_error_fun);")
            && callbacks_rs
                .contains("optional_callback_wrapper!(HeapVisitCallback, mi_heap_visit_fun);")
            && callbacks_rs
                .contains("optional_callback_wrapper!(BlockVisitCallback, mi_block_visit_fun);")
            && callbacks_rs.contains("pub(crate) struct ThreadLocalClaimCallback(mi_claim_fun_t);")
            && core_mod.contains("DeferredFreeCallback")
            && core_mod.contains("OutputCallback")
            && core_mod.contains("ErrorCallback")
            && core_mod.contains("HeapVisitCallback")
            && core_mod.contains("BlockVisitCallback")
            && core_mod.contains("ThreadLocalClaimCallback")
            && core_mod.contains("DeferredFreeCallbackArg")
            && core_mod.contains("OutputCallbackArg")
            && core_mod.contains("ErrorCallbackArg")
            && core_mod.contains("HeapVisitCallbackArg")
            && core_mod.contains("BlockVisitCallbackArg")
            && core_mod.contains("ThreadLocalClaimCallbackArg")
            && types_rs.contains("pub type mi_output_fun = unsafe extern \"C\" fn(")
            && types_rs.contains("pub type mi_error_fun = unsafe extern \"C\" fn(")
            && types_rs.contains("pub type mi_deferred_free_fun = unsafe extern \"C\" fn(")
            && types_rs.contains("pub type mi_heap_visit_fun = unsafe extern \"C\" fn(")
            && types_rs.contains("pub type mi_block_visit_fun = unsafe extern \"C\" fn(")
            && types_rs.contains("pub type mi_commit_fun_t = unsafe extern \"C\" fn(")
            && types_rs.contains("pub type mi_claim_fun_t =\n    unsafe extern \"C\" fn(")
            && abi_rs.contains("out: Option<mi_output_fun>, arg: *mut c_void")
            && abi_rs.contains("fun: Option<mi_error_fun>, arg: *mut c_void")
            && abi_rs.contains("deferred_free: Option<mi_deferred_free_fun>, arg: *mut c_void")
            && abi_rs.contains("visitor: Option<mi_block_visit_fun>")
            && abi_rs.contains("visitor: Option<mi_heap_visit_fun>")
            && !types_rs.contains("CallbackArg")
            && !abi_rs.contains("CallbackArg")
            && page_rs.contains(
                "let callback = crate::core_types::DeferredFreeCallback::from_storage_ptr(fun_ptr);"
            )
            && page_rs
                .contains("let arg = crate::core_types::DeferredFreeCallbackArg::from_raw(arg);")
            && options_rs.contains(
                "fn mi_out_get_default(parg: Option<&mut OutputCallbackArg>) -> OutputCallback"
            )
            && options_rs.contains("let arg = OutputCallbackArg::from_raw(arg);")
            && options_rs.contains(
                "let arg = ErrorCallbackArg::from_raw(mi_error_arg.load(Ordering::Acquire));"
            )
            && arena_rs.contains("visitor: BlockVisitCallback,")
            && arena_rs.contains("arg: BlockVisitCallbackArg,")
            && arena_rs.contains("BlockVisitCallback::new(Some(visitor))")
            && arena_rs.contains("BlockVisitCallbackArg::from_raw(arg)")
            && init_rs.contains("let visitor = HeapVisitCallback::new(visitor);")
            && init_rs.contains("let arg = HeapVisitCallbackArg::from_raw(arg);")
            && threadlocal_rs.contains("ThreadLocalClaimCallback::new(mi_thread_local_claim_fun)"),
        "callback function and context pointers should be typed inside allocator internals while raw callback typedefs stay at C ABI boundaries"
    );
    let padding_rs =
        fs::read_to_string(root.join("src/types/padding.rs")).expect("read padding.rs");
    assert!(
        padding_rs.contains("pub(crate) struct PaddingCanary(u32);")
            && padding_rs.contains("pub(crate) struct PaddingDelta(u32);")
            && !padding_rs.contains("pub(crate) struct PaddingCanary(pub(crate) u32);")
            && !padding_rs.contains("pub(crate) struct PaddingDelta(pub(crate) u32);")
            && padding_rs.contains("pub(crate) const fn get(self) -> u32"),
        "padding scalar witnesses should keep backing fields private behind accessors"
    );
    assert!(
        padding_rs.contains(
            "pub(crate) struct Padding {\n    canary: PaddingCanary,\n    delta: PaddingDelta,\n}"
        ) && padding_rs.contains(
            "pub(crate) const fn new(canary: PaddingCanary, delta: PaddingDelta) -> Self"
        ) && padding_rs.contains("pub(crate) const fn canary(self) -> PaddingCanary")
            && padding_rs.contains("pub(crate) const fn delta(self) -> PaddingDelta"),
        "Padding should keep aggregate fields private behind constructor/accessors"
    );
    for witness in ["AllocatedBlock", "FreeBlock"] {
        assert!(
            validation_extra_rs.contains(&format!("pub(crate) struct {witness}(BlockPtr);")),
            "{witness} should keep its block pointer field private"
        );
    }
    assert!(
        validation_extra_rs.contains("pub(crate) unsafe fn new_unchecked(block: BlockPtr) -> Self")
            && validation_extra_rs
                .contains("pub(in crate::types) unsafe fn new_unchecked(block: BlockPtr) -> Self")
            && validation_extra_rs.contains("pub(crate) fn block(self) -> BlockPtr"),
        "allocated/free block witnesses should expose constructors only at the needed boundary"
    );
    for witness in ["AllocatedBlock", "FreeBlock"] {
        assert!(
            !validation_extra_rs.contains(&format!(
                "pub(crate) struct {witness}(pub(crate) BlockPtr);"
            )),
            "{witness} should not expose tuple construction outside validation_extra"
        );
    }

    for (rel, source) in &sources {
        if rel.ends_with("/tests.rs")
            || rel == "src/types/atomic/concurrency_models.rs"
            || rel == "src/types/layout_tests.rs"
        {
            continue;
        }

        if rel != "src/types/memid.rs" && rel != "src/types/constants.rs" {
            for raw_union_arm in ["mem.os", "mem.arena", "mem.meta"] {
                assert!(
                    !source.contains(raw_union_arm),
                    "{rel} should inspect memid union arms through MemId::view, not {raw_union_arm}"
                );
            }
        }

        if rel != "src/types/atomic.rs" {
            for atomic_cast in [
                "AtomicUsize::from_ptr",
                "cast::<AtomicUsize>",
                "cast::<core::sync::atomic::AtomicUsize>",
            ] {
                assert!(
                    !source.contains(atomic_cast),
                    "{rel} should route pointer-to-atomic casts through core_types::Atomic*Field"
                );
            }
        }

        if rel.starts_with("src/types/") {
            for deref_impl in ["impl core::ops::Deref", "impl Deref", "Deref for"] {
                assert!(
                    !source.contains(deref_impl),
                    "{rel} should keep allocator metadata wrappers raw-pointer based, without Deref"
                );
            }
        }

        if rel != "src/types/freelist_wrappers.rs" {
            for block_next_access in [
                "(*block).next",
                "(*block.cast_mut()).next",
                "(*block.as_ptr()).next",
            ] {
                assert!(
                    !source.contains(block_next_access),
                    "{rel} should read/write mi_block_s.next through BlockLink, EncodedPtr, or free-list wrappers"
                );
            }
        }

        if rel != "src/types/padding.rs" && rel != "src/types/constants.rs" {
            for raw_padding_access in [
                "mi_padding_t",
                "(*padding).canary",
                "(*padding).delta",
                "cast::<mi_padding_t>",
            ] {
                assert!(
                    !source.contains(raw_padding_access),
                    "{rel} should route padding metadata access through core_types::PaddingPtr"
                );
            }
        }
    }

    let types_spec = fs::read_to_string(root.join("TYPES_SPEC.md")).expect("read TYPES_SPEC.md");
    let atomic =
        fs::read_to_string(root.join("src/types/atomic.rs")).expect("read atomic wrappers");
    let padding_rs =
        fs::read_to_string(root.join("src/types/padding.rs")).expect("read padding wrappers");
    let validation_types = fs::read_to_string(root.join("src/types/validation.rs"))
        .expect("read core_types/validation.rs");
    let alloc_rs = fs::read_to_string(root.join("src/allocator/alloc.rs")).expect("read alloc.rs");
    let free_rs = fs::read_to_string(root.join("src/allocator/free.rs")).expect("read free.rs");
    assert!(
        padding_rs.contains(
            "pub(crate) fn from_page_block_keys(page: PagePtr, block: BlockPtr, keys: PageKeys) -> Self"
        )
            && padding_rs.contains("page.as_ptr().cast()")
            && padding_rs.contains("block.as_ptr().cast()")
            && !padding_rs.contains("page: *const c_void")
            && !padding_rs.contains("block: *const c_void")
            && padding_rs.contains(
                "pub(crate) fn for_usable(delta: usize, usable_size: UsableSize) -> Option<Self>"
            )
            && padding_rs.contains("u32::try_from(delta).ok().map(Self)")
            && alloc_rs.contains("PaddingCanary::from_page_block_keys")
            && alloc_rs.contains("PagePtr::new_unchecked(page)")
            && alloc_rs.contains("PaddingDelta::for_usable(delta, usable_size)")
            && alloc_rs.contains("crate::core_types::PaddingDelta::new_unchecked(delta as u32)")
            && free_rs.contains("PaddingCanary::from_page_block_keys")
            && free_rs.contains("let page_ptr = crate::core_types::PagePtr::new_unchecked(page.cast_mut());")
            && free_rs.contains("let block_ptr = crate::core_types::BlockPtr::new_unchecked(block.cast_mut());")
            && free_rs.contains("PaddingDelta::for_usable("),
        "padding metadata should construct canaries from typed PagePtr/BlockPtr plus PageKeys and deltas with the usable-size bound"
    );
    assert!(
        atomic.contains("pub(crate) struct AtomicThreadFree(pub(crate) AtomicUsizeField);")
            && !atomic.contains("pub(crate) struct AtomicThreadFree(pub(crate) *mut usize);"),
        "AtomicThreadFree should be backed by AtomicUsizeField, matching TYPES_SPEC.md"
    );
    assert!(
        types_spec.contains("The thread-free atomic cell is `AtomicThreadFree(AtomicUsizeField)`")
            && types_spec
                .contains("Earlier\nplans used the placeholder name `ThreadFreeCell(*mut usize)`")
            && !types_spec.contains("pub(crate) struct ThreadFreeCell(*mut usize);")
            && types_spec.contains("pub(crate) fn push_remote_acq_rel(")
            && types_spec.contains(
                "pub(crate) unsafe fn release_acq_rel(self, expected: &mut ThreadFreeWord) -> bool;"
            ),
        "TYPES_SPEC.md should document AtomicThreadFree as the reconciled thread-free cell"
    );
    assert!(
        types_spec.contains(
            "`AbandonedCountCell` is the staged arena-domain name for the same storage now operated on by `AtomicAbandonedCount(*const AtomicUsize)`"
        ) && types_spec.contains(
            "`ArenaPageCell` is the staged arena-domain name for the same storage now operated on by `AtomicArenaPage(*const AtomicPtr<c_void>)`"
        ) && types_spec.contains(
            "`ArenaPageTablePtr` remains the non-atomic page-table pointer witness"
        ),
        "TYPES_SPEC.md should reconcile staged arena page-table cell names with live atomic wrappers"
    );

    let page_rs = fs::read_to_string(root.join("src/allocator/page.rs")).expect("read page.rs");
    assert!(
        free_rs.contains(
            "pub(crate) unsafe fn mi_free_block_local(\n    page_wrapper: crate::core_types::LocalPage,\n    block_wrapper: crate::core_types::BlockForFree,"
        ) && free_rs.contains(
            "pub(crate) unsafe fn mi_free_block_mt(\n    page_wrapper: crate::core_types::ForeignPage,"
        ) && page_rs.contains(
            "pub(crate) unsafe fn _mi_theap_page_reclaim(\n    page: crate::core_types::OwnedAbandonedPage,"
        ),
        "typed free/reclaim entry points should reject raw page ownership states"
    );
    assert!(
        page_rs.contains(
            "pub(crate) unsafe fn try_reclaim_abandoned(\n    page_wrapper: crate::core_types::OwnedAbandonedPage,\n    target: crate::core_types::ThreadHeapLocal,"
        )
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_theap_page_reclaim(\n    page: crate::core_types::OwnedAbandonedPage,\n    target: crate::core_types::ThreadHeapLocal,"
            )
            && page_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_theap_page_reclaim_raw")
            && page_rs.contains("_mi_theap_page_reclaim(wrapper, theap_wrapper);")
            && page_states_rs.contains("pub(crate) fn reclaim(self, _target: ThreadHeapLocal) -> ActivePage")
            && free_rs.contains("unsafe fn mi_abandoned_page_try_reclaim(\n    page: crate::core_types::OwnedAbandonedPage,")
            && free_rs.contains("_mi_theap_page_reclaim(\n            page,\n            crate::core_types::ThreadHeapLocal::new_unchecked")
            && !page_rs.contains("pub(crate) unsafe fn _mi_theap_page_reclaim(\n    page: *mut mi_page_t")
            && !page_rs.contains("pub(crate) unsafe fn _mi_theap_page_reclaim(\n    theap: *mut mi_theap_t")
            && !free_rs.contains("unsafe fn mi_abandoned_page_try_reclaim(\n    page: *mut mi_page_t"),
        "reclaim should accept only OwnedAbandonedPage and ThreadHeapLocal below the raw shim"
    );
    assert!(
        validation_types.contains("impl BlockForFree {\n    fn new(block: BlockPtr, was_guarded: WasGuarded) -> Self")
            && validation_types.contains("pub(crate) fn prepare_free(self) -> Option<ValidatedFree>")
            && validation_types.contains("let block = BlockForFree::new(located.block(), located.was_guarded());")
            && validation_types.contains("Some(ValidatedFree { user, page, block })")
            && free_rs.contains(
                "pub(crate) unsafe fn mi_free_block_local(\n    page_wrapper: crate::core_types::LocalPage,\n    block_wrapper: crate::core_types::BlockForFree,"
            )
            && free_rs.contains("mi_free_block_local(\n                    local_page,\n                    validated.block(),")
            && !free_rs.contains("BlockForFree::new")
            && !free_rs.contains("ValidatedFree::new"),
        "mi_free_block_local should accept only LocalPage plus validation-created BlockForFree"
    );
    assert!(
        free_rs.contains(
            "pub(crate) unsafe fn mi_free_block_mt(\n    page_wrapper: crate::core_types::ForeignPage,\n    block_wrapper: crate::core_types::BlockForFree,"
        )
            && free_rs.contains("mi_free_block_mt(foreign_page, validated.block());")
            && free_rs.contains("mi_free_block_mt(abandoned_page.remote_free_page(), validated.block());")
            && page_states_rs.contains("pub(crate) fn remote_free_page(self) -> ForeignPage")
            && free_rs.contains(
                "unsafe fn mi_free_try_collect_mt_typed(\n    page: crate::core_types::OwnedAbandonedPage,\n    remote_free: crate::core_types::RemoteFreeList,"
            )
            && free_rs.contains(
                "pub(crate) unsafe extern \"C\" fn mi_free_try_collect_mt(\n    page: *mut mi_page_t,\n    mt_free: *mut mi_block_t,"
            )
            && free_rs.contains("crate::core_types::OwnedAbandonedPage::new_unchecked")
            && !free_rs.contains("pub(crate) unsafe fn mi_free_block_mt(\n    page: *mut mi_page_t")
            && !free_rs.contains("unsafe fn mi_free_try_collect_mt_typed(\n    page: *mut mi_page_t"),
        "remote free should enter through ForeignPage or abandoned-page witnesses before touching remote-free internals"
    );
    assert!(
        page_rs.contains(
            "pub(crate) unsafe fn mi_page_queue_enqueue_from(\n    to: crate::core_types::FullPageQueue,\n    page: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,"
        ) && page_rs.contains(
            "pub(crate) unsafe fn mi_page_queue_enqueue_from_full(\n    to: crate::core_types::NormalPageQueue,\n    page: crate::core_types::QueuedPage<crate::core_types::FullQueue>,"
        ) && page_queues_rs.contains("impl QueuedPage<NormalQueue>")
            && page_queues_rs.contains("pub(crate) unsafe fn into_full(self, to: FullPageQueue)"),
        "page queue transitions should require typed queue/page witnesses"
    );
    assert!(
        page_rs.contains(
            "unsafe fn mi_page_queue_append_full_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::FullPageQueue,\n    append: crate::core_types::FullPageQueue,"
        )
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_free_full_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Full>,\n    queue: crate::core_types::FullPageQueue,"
            )
            && page_rs.contains("crate::core_types::FullPage::new_unchecked(page_wrapper.page())")
            && page_rs.contains(
                "crate::core_types::QueuedPage::<crate::core_types::FullQueue>::new(full, queue)"
            )
            && page_rs.contains(
                "unsafe fn mi_page_unfull_typed(\n    page: crate::core_types::QueuedPage<crate::core_types::FullQueue>,\n    to: crate::core_types::NormalPageQueue,"
            )
            && page_queues_rs.contains(
                "pub(crate) unsafe fn new(\n        page: crate::core_types::FullPage,\n        queue: FullPageQueue,"
            )
            && page_queues_rs.contains("crate::page::mi_page_is_in_full(page.page().as_ptr())")
            && page_queues_rs.contains("queue: PageQueue::full(queue)")
            && !page_rs.contains("unsafe fn mi_page_queue_append_full_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::PageQueuePtr")
            && !page_rs.contains("pub(crate) unsafe fn _mi_page_free_full_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Full>,\n    queue: crate::core_types::PageQueuePtr")
            && !page_rs.contains(
                "unsafe fn mi_page_unfull_typed(\n    page: crate::core_types::FullPage"
        ),
        "full-queue operations should accept only FullPage witnesses and FullPageQueue handles"
    );
    assert!(
        page_queues_rs.contains(
            "pub(crate) unsafe fn new(\n        page: crate::core_types::ActivePage,\n        queue: NormalPageQueue,"
        )
            && page_queues_rs.contains("let queue_ptr = queue.ptr().as_ptr();")
            && page_queues_rs.contains("let page_ptr = page.page().as_ptr();")
            && page_queues_rs.contains("&& !crate::page::mi_page_is_in_full(page_ptr)")
            && page_queues_rs.contains(
                ".is_some_and(|block_size| page.page().block_size().get() == block_size.get())"
            )
            && page_queues_rs.contains(
                "crate::page::mi_page_is_huge(page_ptr)\n                        && crate::page::mi_page_queue_is_huge(queue_ptr)"
            )
            && page_queues_rs.contains("queue: PageQueue::normal(queue)")
            && page_rs.contains(
                "unsafe fn mi_page_queue_push_at_end_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    page: crate::core_types::ActivePage,"
            )
            && page_rs.contains(
                "unsafe fn mi_page_queue_push_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    page: crate::core_types::ActivePage,"
            )
            && page_rs.contains(
                "unsafe fn mi_page_queue_move_to_front_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    queued: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,"
            )
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_free_active_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,\n    queued: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,"
            )
            && page_rs.contains(
                "unsafe fn mi_page_queue_append_normal_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    append: crate::core_types::NormalPageQueue,"
            )
            && !page_queues_rs.contains(
                "pub(crate) unsafe fn new(\n        page: crate::core_types::FullPage,\n        queue: NormalPageQueue,"
            )
            && !page_rs.contains("unsafe fn mi_page_queue_push_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    page: crate::core_types::PagePtr")
            && !page_rs.contains("pub(crate) unsafe fn _mi_page_free_active_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,\n    queued: crate::core_types::PagePtr"),
        "normal queue operations should accept only active non-full pages matching NormalPageQueue or the huge-page sentinel queue"
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
        ("src/allocator/arena.rs", 14),
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
        (("src/allocator/arena.rs", "too_many_arguments"), 5),
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
        (("src/allocator/arena.rs", "non_camel_case_types"), 6),
        (("src/allocator/arena.rs", "non_upper_case_globals"), 1),
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
