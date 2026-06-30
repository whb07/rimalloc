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
    let release_md =
        fs::read_to_string(root.join("docs/release.md")).expect("read docs/release.md");
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let allocation_rs =
        fs::read_to_string(root.join("src/safe/allocation.rs")).expect("read allocation.rs");
    let heap_rs = fs::read_to_string(root.join("src/safe/heap.rs")).expect("read heap.rs");

    for required in [
        "## Future Object Ownership APIs",
        "The current allocation wrappers own bytes, not Rust objects.",
        "MiBox<T>",
        "HeapBox<'heap, T>",
        "constructor panic cleanup",
        "deallocation through the owning allocator or heap",
    ] {
        assert!(
            safe_api_md.contains(required),
            "safe object ownership policy should document {required}"
        );
    }
    for required in [
        "Confirm no object ownership APIs such as `MiBox<T>`, `HeapBox<'heap, T>`",
        "construction failure, destructor and panic",
        "allocator/heap ownership, and threading traits",
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
fn release_candidates_require_full_benchmark_evidence() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let release_md =
        fs::read_to_string(root.join("docs/release.md")).expect("read docs/release.md");
    let performance_md =
        fs::read_to_string(root.join("docs/performance.md")).expect("read docs/performance.md");
    let benchmark_results_md =
        fs::read_to_string(root.join("BENCHMARK_RESULTS.md")).expect("read BENCHMARK_RESULTS.md");
    let check_bench_full = fs::read_to_string(root.join("scripts/check-bench-full.sh"))
        .expect("read full bench script");

    for required in [
        "Before a release candidate, run `scripts/check-bench-full.sh`",
        "capture\n   `scripts/env-info.sh`",
        "target/bench-full/default-rust/criterion/benchmark-thresholds-full.csv",
        "target/bench-full/interpose/criterion/benchmark-thresholds-full.csv",
        "artifact modes, Rust/C ratios, and noisy-benchmark notes",
    ] {
        assert!(
            release_md.contains(required),
            "release checklist should require full benchmark evidence: {required}"
        );
    }

    for required in [
        "Before a release candidate, run `scripts/check-bench-full.sh`",
        "capture\n  `scripts/env-info.sh`",
        "target/bench-full/default-rust/criterion/benchmark-thresholds-full.csv",
        "target/bench-full/interpose/criterion/benchmark-thresholds-full.csv",
        "Rust-vs-C ratios come from the native scenario\nbenchmarks in the default Rust library mode",
    ] {
        assert!(
            performance_md.contains(required),
            "performance policy should define release candidate benchmark evidence: {required}"
        );
    }

    for required in [
        "CARGO_TARGET_DIR=\"$default_target_dir\"",
        "CARGO_TARGET_DIR=\"$interpose_target_dir\"",
        "--csv-out \"$default_target_dir/criterion/benchmark-thresholds-full.csv\"",
        "--csv-out \"$interpose_target_dir/criterion/benchmark-thresholds-full.csv\"",
        "cargo bench --features \"bench interpose\" --bench hotpath",
        "--allow-missing",
    ] {
        assert!(
            check_bench_full.contains(required),
            "full benchmark script should keep release artifact modes separate: {required}"
        );
    }

    for required in [
        "## Release Candidate Artifact-Mode Evidence",
        "default Rust library",
        "interpose hot path",
        "Do not reuse Criterion baselines across artifact modes.",
        "Rust-vs-C ratios emitted by `scripts/summarize-criterion.py`",
    ] {
        assert!(
            benchmark_results_md.contains(required),
            "benchmark report template should identify artifact mode evidence: {required}"
        );
    }
}

#[test]
fn strict_wrapper_hot_path_changes_require_assembly_evidence() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let strict_types_md =
        fs::read_to_string(root.join("docs/strict-types.md")).expect("read docs/strict-types.md");
    let performance_md =
        fs::read_to_string(root.join("docs/performance.md")).expect("read docs/performance.md");
    let release_md =
        fs::read_to_string(root.join("docs/release.md")).expect("read docs/release.md");
    let benchmark_results_md =
        fs::read_to_string(root.join("BENCHMARK_RESULTS.md")).expect("read BENCHMARK_RESULTS.md");
    let capture_release_assembly =
        fs::read_to_string(root.join("scripts/capture-release-assembly.sh"))
            .expect("read capture-release-assembly.sh");

    for required in [
        "New strict internal wrappers on allocator hot paths require release assembly",
        "Run `scripts/capture-release-assembly.sh`",
        "free-list pop, local free, remote free, small\n  page lookup, and any changed hot path",
        "no added hot-path branch, call, load, store,\n  atomic memory operation, lock prefix, stack spill, formatting, allocation,\n  panic path, bounds-check slow path, or wrapper-constructor traffic",
    ] {
        assert!(
            strict_types_md.contains(required),
            "strict-type policy should gate new hot-path wrappers with assembly evidence: {required}"
        );
    }

    for required in [
        "A new strict internal wrapper changes release assembly for free-list pop",
        "local free, remote free, small page lookup",
        "adding a branch, call, memory operation, lock prefix, stack spill, formatting",
        "panic path, bounds-check slow path, or wrapper-constructor traffic",
    ] {
        assert!(
            performance_md.contains(required),
            "performance policy should treat unproven strict-wrapper assembly changes as regressions: {required}"
        );
    }

    for required in [
        "For new strict internal wrappers on allocator hot paths",
        "`scripts/capture-release-assembly.sh` output covering free-list pop, local\n   free, remote free, small page lookup, and any changed hot path",
        "fail signoff\n   unless the evidence shows no added branch, call, memory traffic, stack spill",
        "panic path, bounds-check slow path, or wrapper\n   constructor traffic without measured justification",
    ] {
        assert!(
            release_md.contains(required),
            "release checklist should fail unproven strict-wrapper hot-path changes: {required}"
        );
    }

    for required in [
        "## Strict-Type Assembly Comparison Checklist",
        "Use `scripts/capture-release-assembly.sh` before and after each strict-type\nhot-path migration.",
        "`mi_malloc`",
        "`mi_malloc_small`",
        "`mi_free`",
        "`_mi_theap_malloc_zero_ex`",
    ] {
        assert!(
            benchmark_results_md.contains(required),
            "benchmark report should preserve strict-wrapper assembly evidence guidance: {required}"
        );
    }

    for symbol in [
        "capture_symbol mi_malloc",
        "capture_symbol mi_malloc_small",
        "capture_symbol mi_free",
        "capture_symbol _mi_theap_malloc_zero_ex",
    ] {
        assert!(
            capture_release_assembly.contains(symbol),
            "assembly capture script should include strict-wrapper hot-path symbol: {symbol}"
        );
    }
}

#[test]
fn release_notes_publish_per_release_performance_summaries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let benchmark_results_md =
        fs::read_to_string(root.join("BENCHMARK_RESULTS.md")).expect("read BENCHMARK_RESULTS.md");
    let performance_md =
        fs::read_to_string(root.join("docs/performance.md")).expect("read docs/performance.md");
    let release_md =
        fs::read_to_string(root.join("docs/release.md")).expect("read docs/release.md");
    let changelog_md = fs::read_to_string(root.join("CHANGELOG.md")).expect("read CHANGELOG.md");

    for required in [
        "## Per-Release Performance Summary Template",
        "native scenario geomean Rust/C median ratio",
        "hot-path deltas against previous release evidence",
        "release-blocking regressions, if any",
        "noisy watch items separated from release-blocking regressions",
        "links to raw Criterion summaries and threshold CSV artifacts",
        "Release notes must link to the matching `BENCHMARK_RESULTS.md` section",
    ] {
        assert!(
            benchmark_results_md.contains(required),
            "benchmark report should define per-release performance summary fields: {required}"
        );
    }

    for required in [
        "Publish a per-release performance summary in `BENCHMARK_RESULTS.md`",
        "Include geomean Rust/C ratios, hot-path deltas against the\n  previous release evidence",
        "release-blocking regressions, noisy watch items",
        "links to raw Criterion summaries or threshold CSV artifacts",
    ] {
        assert!(
            performance_md.contains(required),
            "performance policy should require per-release summaries: {required}"
        );
    }

    for required in [
        "Publish the per-release performance summary in `BENCHMARK_RESULTS.md`",
        "summarizing geomean Rust/C ratios, hot-path deltas against\n   previous release evidence",
        "noisy watch items, release-blocking regressions",
        "links to raw Criterion summaries or threshold CSV artifacts",
        "link the\n   release notes to `BENCHMARK_RESULTS.md` or the attached performance report",
    ] {
        assert!(
            release_md.contains(required),
            "release checklist should require linked performance summaries: {required}"
        );
    }

    for required in [
        "Release notes must link to `BENCHMARK_RESULTS.md`",
        "geomean Rust/C ratios, hot-path deltas, noisy watch\n  items",
        "release-blocking regressions, and raw Criterion or threshold CSV\n  artifacts",
    ] {
        assert!(
            changelog_md.contains(required),
            "release notes should carry performance-summary link policy: {required}"
        );
    }
}

#[test]
fn rust_safety_model_page_covers_safe_and_raw_boundaries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = fs::read_to_string(root.join("README.md")).expect("read README.md");
    let safety_model =
        fs::read_to_string(root.join("docs/rust-safety-model.md")).expect("read safety model");

    assert!(
        readme.contains("docs/rust-safety-model.md") && readme.contains("Rust safety model"),
        "README should link to the dedicated Rust safety model page"
    );

    for required in [
        "# Rust Safety Model",
        "`Allocation` and `HeapAllocation<'heap>` own byte allocations",
        "heap allocations borrow the heap witness",
        "Uninitialized memory is never exposed as `&[u8]` or `&mut [u8]`.",
        "`unsafe_callbacks::ErrorCallbackGuard`",
        "`unsafe_callbacks::DeferredFreeCallbackGuard` are Tier U unsafe Rust APIs",
        "Callers must\nserialize registration, restoration, and callback entry",
        "The raw Rust facade mirrors the C API.",
        "C ABI exports and allocator hot paths parse raw inputs at narrow boundaries",
        "`SAFETY[AUDIT:*]` tags",
        "Allocator hot paths are guarded by no-allocation evidence.",
        "Strict internal types live under `src/types/` and are crate-private.",
        "Hot wrappers\nuse transparent representation, inline constructors/accessors, and layout/niche\ntests",
    ] {
        assert!(
            safety_model.contains(required),
            "Rust safety model should document required boundary: {required}"
        );
    }
}

#[test]
fn architecture_map_covers_public_layers_and_policy_gates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let architecture =
        fs::read_to_string(root.join("ARCHITECTURE.md")).expect("read ARCHITECTURE.md");

    for required in [
        "# Architecture",
        "Tier S, safe Rust facade: `src/safe/`",
        "Tier U, unsafe Rust conveniences: `src/ffi/callbacks.rs`",
        "Tier R, raw Rust facade: `src/raw/`",
        "Tier C, exported C ABI: exported symbols implemented through `src/ffi/`",
        "Tier I, internal implementation: `src/allocator/`, `src/types/`,",
        "`src/internal/`, and `src/platform/`",
        "Build glue: `build.rs` owns C ABI and interposition build setup",
        "Policy gates: `scripts/` and source-shape tests enforce API inventory",
        "`scripts/check-layer-boundaries.py`",
    ] {
        assert!(
            architecture.contains(required),
            "ARCHITECTURE.md should cover {required}"
        );
    }
}

#[test]
fn core_types_remains_crate_private_not_public_raw_api() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let raw_rs = fs::read_to_string(root.join("src/raw/mod.rs")).expect("read src/raw/mod.rs");

    assert!(
        lib_rs.contains("pub(crate) use types as core_types;")
            && !lib_rs.contains("pub mod core_types;")
            && !lib_rs.contains("pub use core_types")
            && !lib_rs.contains("pub use crate::core_types"),
        "core_types should remain crate-private at the crate root"
    );

    assert!(
        !raw_rs.contains("mod core_types")
            && !raw_rs.contains("pub mod core_types")
            && !raw_rs.contains("pub use core_types")
            && !raw_rs.contains("pub use crate::core_types")
            && !raw_rs.contains("crate::core_types"),
        "default raw modules should not expose core_types or strict wrapper routes"
    );
}

#[test]
fn abi_module_owns_c_header_declarations() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let mimalloc_rs =
        fs::read_to_string(root.join("src/ffi/mimalloc.rs")).expect("read src/ffi/mimalloc.rs");
    let raw_rs = fs::read_to_string(root.join("src/raw/mod.rs")).expect("read src/raw/mod.rs");

    assert!(
        abi_rs.contains("Mechanical translation of `include/mimalloc.h`")
            && abi_rs.contains("unsafe extern \"C\"")
            && abi_rs.contains("pub fn mi_malloc(size: usize) -> *mut c_void;")
            && abi_rs.contains("pub fn mi_heap_new() -> *mut mi_heap_t;"),
        "src/ffi/abi.rs should own the raw mimalloc C header declarations"
    );
    assert!(
        mimalloc_rs.contains("pub(crate) use crate::abi::*;")
            && !mimalloc_rs.contains("unsafe extern \"C\""),
        "src/ffi/mimalloc.rs should stay an internal compatibility alias, not the public header mirror"
    );
    assert!(
        raw_rs.contains("pub mod abi") && raw_rs.contains("pub use crate::abi::*;"),
        "raw facade should expose ABI declarations through the ABI owner"
    );
}

#[test]
fn abi_c_declarations_remain_raw_not_strict_typed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let declarations = source_item("src/ffi/abi.rs", &abi_rs, "unsafe extern \"C\"");

    for forbidden in [
        "crate::core_types",
        "core_types::",
        "NonNull",
        "HeapPtr",
        "ThreadHeapLocal",
        "BorrowedHeap",
        "LayoutRequest",
        "RequestedSize",
        "PaddedSize",
        "UsableSize",
        "BlockSize",
        "ArenaPtr",
        "SliceCount",
        "SliceIndex",
        "ThreadSeq",
        "OsMemId",
        "OsMemPtr",
        "CommittedBytes",
        "ReservedBytes",
        "ValidatedFree",
        "LocalPage",
        "ForeignPage",
        "FullPage",
        "NormalPageQueue",
        "FullPageQueue",
        "MemIdView",
        "MemoryOrder",
    ] {
        assert!(
            !declarations.contains(forbidden),
            "src/ffi/abi.rs C declarations should not expose strict type `{forbidden}`"
        );
    }

    assert!(
        declarations.contains("pub fn mi_malloc(size: usize) -> *mut c_void;")
            && declarations.contains("pub fn mi_free(p: *mut c_void);")
            && declarations.contains("pub fn mi_heap_malloc(heap: *mut mi_heap_t, size: usize) -> *mut c_void;")
            && declarations.contains("pub fn mi_heap_new() -> *mut mi_heap_t;")
            && declarations.contains("pub fn mi_heap_visit_blocks(\n        heap: *mut mi_heap_t,\n        visit_blocks: bool,\n        visitor: Option<mi_block_visit_fun>,\n        arg: *mut c_void,\n    ) -> bool;")
            && declarations.contains("pub fn mi_process_info(\n        elapsed_msecs: *mut usize,"),
        "src/ffi/abi.rs declarations should keep the C header raw pointer shape"
    );
}

#[test]
fn translated_raw_layouts_stay_in_types_substrate() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let types_rs = fs::read_to_string(root.join("src/types/constants.rs"))
        .expect("read src/types/constants.rs");
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let raw_rs = fs::read_to_string(root.join("src/raw/mod.rs")).expect("read src/raw/mod.rs");

    for layout in [
        "pub struct mi_page_queue_t",
        "pub struct mi_block_s",
        "pub struct mi_heap_area_t",
        "pub struct mi_page_s",
        "pub struct mi_heap_s",
        "pub struct mi_theap_s",
        "pub struct mi_arena_s",
        "pub struct mi_subproc_s",
        "pub struct mi_arena_pages_s",
        "pub struct mi_tld_s",
    ] {
        assert!(
            types_rs.contains("#[repr(C)]") && types_rs.contains(layout),
            "src/types/constants.rs should remain the private raw repr(C) layout substrate for {layout}"
        );
        assert!(
            !abi_rs.contains(layout) && !raw_rs.contains(layout),
            "{layout} should not be redefined in public ABI/facade modules"
        );
    }

    assert!(
        abi_rs.contains("pub use crate::types::{")
            && abi_rs.contains("mi_heap_t")
            && abi_rs.contains("mi_theap_t")
            && raw_rs.contains("pub mod abi {\n    pub use crate::abi::*;\n}")
            && !types_rs.contains("use crate::core_types")
            && !abi_rs.contains("core_types")
            && !raw_rs.contains("core_types"),
        "raw layout exposure should flow from src/types/constants.rs through raw::abi without leaking strict wrappers"
    );
}

#[test]
fn public_c_layout_exposure_uses_raw_abi_facade() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let raw_rs = fs::read_to_string(root.join("src/raw/mod.rs")).expect("read src/raw/mod.rs");
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let api_doc =
        fs::read_to_string(root.join("docs/api-inventory.md")).expect("read api inventory");

    assert!(
        abi_rs.contains("pub use crate::types::{")
            && abi_rs.contains("mi_heap_area_t")
            && abi_rs.contains("mi_heap_t")
            && abi_rs.contains("mi_subproc_id_t")
            && raw_rs.contains("pub mod abi {\n    pub use crate::abi::*;\n}")
            && raw_rs.contains("pub mod mimalloc {\n    pub use crate::abi::*;\n}")
            && raw_rs.contains("pub use mimalloc::*;"),
        "public C layouts and aliases should be exposed through raw::abi, raw::mimalloc, and root raw C-header re-exports"
    );
    assert!(
        !raw_rs.contains("pub mod types")
            && api_doc.contains("By default, `raw` exposes the root C-header mirror plus `raw::abi`,\n`raw::mimalloc`, and `raw::headers`.")
            && !lib_rs.contains("pub mod types;")
            && !lib_rs.contains("pub mod core_types;")
            && !raw_rs.contains("pub use crate::core_types")
            && !abi_rs.contains("core_types"),
        "raw::types should remain unavailable and core_types must not be a public C layout route"
    );
}

#[test]
fn raw_facade_remains_c_shaped_without_strict_type_exports() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let raw_rs = fs::read_to_string(root.join("src/raw/mod.rs")).expect("read src/raw/mod.rs");

    assert!(
        raw_rs.contains("Explicit C-shaped compatibility facade")
            && raw_rs.contains("pub mod abi {\n    pub use crate::abi::*;\n}")
            && raw_rs.contains("pub mod mimalloc {\n    pub use crate::abi::*;\n}")
            && raw_rs.contains("pub mod headers {\n    pub use crate::headers::*;\n}")
            && raw_rs.contains("pub use mimalloc::*;"),
        "src/raw/mod.rs should keep the default facade as the C-header mirror"
    );

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
        "static_c",
        "stats",
        "theap",
        "threadlocal",
        "track",
        "types",
    ] {
        assert!(
            !raw_rs.contains(&format!("pub mod {module}"))
                && !raw_rs.contains(&format!("pub use crate::{module}::*")),
            "raw::{module} should not expose implementation modules"
        );
    }

    for forbidden in [
        "pub mod core_types",
        "pub use crate::core_types",
        "crate::core_types",
        "core_types::",
        "HeapPtr",
        "ThreadHeapLocal",
        "BorrowedHeap",
        "LayoutRequest",
        "RequestedSize",
        "PaddedSize",
        "UsableSize",
        "BlockSize",
        "ArenaPtr",
        "SliceCount",
        "SliceIndex",
        "ThreadSeq",
        "OsMemId",
        "OsMemPtr",
        "CommittedBytes",
        "ReservedBytes",
        "ValidatedFree",
        "LocalPage",
        "ForeignPage",
        "FullPage",
        "NormalPageQueue",
        "FullPageQueue",
        "MemIdView",
        "MemoryOrder",
    ] {
        assert!(
            !raw_rs.contains(forbidden),
            "src/raw/mod.rs should not expose strict type `{forbidden}`"
        );
    }
}

#[test]
fn raw_macro_unsafe_boundary_policy_stays_documented() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let abi_doc = fs::read_to_string(root.join("docs/abi.md")).expect("read docs/abi.md");
    let headers_rs = fs::read_to_string(root.join("src/ffi/headers.rs")).expect("read headers.rs");
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let unsafe_doc =
        fs::read_to_string(root.join("docs/unsafe-audit.md")).expect("read unsafe audit");

    assert!(
        abi_rs.contains("SAFETY[AUDIT:raw-macro-boundary]")
            && headers_rs.contains("SAFETY[AUDIT:raw-macro-boundary]")
            && unsafe_doc.contains("SAFETY[AUDIT:raw-macro-boundary]")
            && abi_doc.contains("## Raw Macro Unsafe Boundary"),
        "raw macro unsafe-boundary audit tag should be mirrored in source and docs"
    );
    assert!(
        abi_doc.contains("Typed allocation macros in `src/ffi/abi.rs`")
            && abi_doc.contains("Header override macros in `src/ffi/headers.rs`")
            && abi_doc.contains("There are no intentionally safe public allocation macros today")
            && lib_rs.contains("Header override macros follow the same raw unsafe-boundary policy")
            && lib_rs.contains("mechanical_mimalloc::malloc!(64)"),
        "raw macro docs should distinguish typed/header macros and state that all current public allocation macros require caller unsafe"
    );
    assert!(
        !abi_rs.contains("unsafe { $crate::raw::")
            && !headers_rs.contains("unsafe { $crate::raw::"),
        "public raw macros should not hide raw calls inside internal unsafe blocks"
    );
}

#[test]
fn repository_tests_do_not_import_private_raw_modules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tests_dir = root.join("tests");
    let internal_modules = [
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
        "static_c",
        "stats",
        "theap",
        "threadlocal",
        "track",
        "types",
    ];

    for entry in fs::read_dir(tests_dir).expect("read tests directory") {
        let path = entry.expect("read tests entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }

        let source = fs::read_to_string(&path).expect("read test source");
        let uses_raw_internal = internal_modules.iter().any(|module| {
            let direct = format!("use mechanical_mimalloc::raw::{module}");
            let group_inline = format!("use mechanical_mimalloc::raw::{{{module}");
            let group_multiline = format!("use mechanical_mimalloc::raw::{{\n    {module}");
            source.contains(&direct)
                || source.contains(&group_inline)
                || source.contains(&group_multiline)
        });

        assert!(
            !uses_raw_internal,
            "{} imports removed private raw implementation modules",
            path.display()
        );
    }
}

#[test]
fn raw_pointer_nullability_policy_stays_documented() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let abi_doc = fs::read_to_string(root.join("docs/abi.md")).expect("read docs/abi.md");
    let raw_rs = fs::read_to_string(root.join("src/raw/mod.rs")).expect("read src/raw/mod.rs");
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let ptr_rs = fs::read_to_string(root.join("src/types/ptr.rs")).expect("read src/types/ptr.rs");
    let safe_rs = fs::read_to_string(root.join("src/safe/mod.rs")).expect("read src/safe/mod.rs");
    let safe_allocations_rs = fs::read_to_string(root.join("src/safe/allocation.rs"))
        .expect("read src/safe/allocation.rs");
    let safe_errors_rs =
        fs::read_to_string(root.join("src/safe/error.rs")).expect("read src/safe/error.rs");
    let aligned_exports =
        fs::read_to_string(root.join("tests/aligned_exports.rs")).expect("read aligned_exports");
    let compat_exports =
        fs::read_to_string(root.join("tests/compat_exports.rs")).expect("read compat_exports");
    let fuzz_alloc_sequences = fs::read_to_string(root.join("tests/fuzz_alloc_sequences.rs"))
        .expect("read fuzz_alloc_sequences");
    let parity = fs::read_to_string(root.join("scripts/check-behavioral-parity.py"))
        .expect("read check-behavioral-parity.py");

    assert!(
        abi_doc.contains("## Raw Pointer Nullability")
            && abi_doc.contains("The raw C boundary uses raw pointers in signatures.")
            && abi_doc.contains("It does not use `NonNull`\nin raw C signatures")
            && abi_doc.contains("`mi_free` and matching deallocation APIs accept null")
            && abi_doc.contains("`mi_realloc` and matching realloc-family APIs treat a null input pointer as\n  an allocation request")
            && abi_doc.contains("out-pointers require valid writable storage")
            && abi_doc.contains("metadata pointers must be valid and\n  non-null")
            && abi_doc.contains("Safe Rust APIs must not expose C null/error conventions directly"),
        "docs/abi.md should define the raw pointer nullability policy across raw, internal, and safe APIs"
    );

    assert!(
        raw_rs.contains("Public raw signatures intentionally use `*mut` and `*const` rather than\n//! `NonNull`")
            && raw_rs.contains("Allocation functions\n//! return null on failure")
            && raw_rs.contains("`mi_free` accepts null as a no-op")
            && raw_rs.contains("`mi_realloc` with a null input pointer behaves like allocation")
            && raw_rs.contains("Metadata\n//! pointers, out-pointers, and callback context pointers are valid, non-null"),
        "src/raw/mod.rs rustdoc should state raw C nullability examples and non-null caller obligations"
    );
    assert!(
        abi_rs.contains("Public declarations in this module keep the C header pointer shape.")
            && abi_rs.contains("This module does not use\n//! `NonNull` in raw C signatures.")
            && !abi_rs.contains("NonNull<"),
        "src/ffi/abi.rs should keep public C-header declarations raw-pointer based"
    );

    assert!(
        ptr_rs.contains("pub(crate) struct $name(NonNull<$raw_type>);")
            && ptr_rs.contains("NonNull::new(ptr).map(Self)")
            && ptr_rs.contains("`ptr` must be non-null and point to valid metadata or data")
            && ptr_rs.contains("pub(crate) struct PageConstPtr(NonNull<mi_page_t>)")
            && ptr_rs.contains("NonNull::new(ptr as *mut mi_page_t).map(Self)"),
        "crate-private NonNull pointer wrappers should require checked or explicitly unsafe construction"
    );

    assert!(
        safe_errors_rs.contains("Cause: mimalloc returned null for a valid Rust [`Layout`].")
            && safe_allocations_rs
                .contains("let ptr = NonNull::new(ptr).ok_or(AllocError { layout })?;")
            && safe_allocations_rs.contains("pub const fn as_non_null(&self) -> NonNull<u8>")
            && safe_allocations_rs.contains("pub const fn as_ptr(&self) -> *const u8")
            && safe_allocations_rs.contains("pub const fn as_mut_ptr(&mut self) -> *mut u8")
            && !safe_rs.contains("Option<*mut")
            && !safe_rs.contains("Option<*const")
            && !safe_rs.contains("Result<*mut")
            && !safe_rs.contains("Result<*const")
            && !safe_allocations_rs.contains("Option<*mut")
            && !safe_allocations_rs.contains("Option<*const")
            && !safe_allocations_rs.contains("Result<*mut")
            && !safe_allocations_rs.contains("Result<*const"),
        "safe facade should translate allocator null returns before exposing non-null allocation handles"
    );

    assert!(
        aligned_exports.contains("mi_realloc_aligned(ptr::null_mut(), 96, 64)")
            && aligned_exports.contains("mi_heap_realloc_aligned(heap, ptr::null_mut(), 96, 64)")
            && compat_exports.contains("mi_aligned_recalloc(ptr::null_mut(), 4, 8, 64)")
            && fuzz_alloc_sequences.contains("mi_realloc(ptr::null_mut(), program.size(2048))")
            && parity.contains("mi_free(NULL);")
            && parity.contains("result_bool(\"free_plain_null_ok\", 1);")
            && parity.contains("result_bool(\"realloc_null_allocates\""),
        "tests and parity script should cover null-accepted free and realloc-family behavior"
    );
}

#[test]
fn public_callback_abi_inventory_stays_documented() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let abi_doc = fs::read_to_string(root.join("docs/abi.md")).expect("read docs/abi.md");
    let types_rs = fs::read_to_string(root.join("src/types/constants.rs"))
        .expect("read src/types/constants.rs");
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let options_rs = fs::read_to_string(root.join("src/allocator/options.rs"))
        .expect("read src/allocator/options.rs");
    let page_rs =
        fs::read_to_string(root.join("src/allocator/page.rs")).expect("read src/allocator/page.rs");
    let arena_rs = fs::read_to_string(root.join("src/allocator/arena.rs"))
        .expect("read src/allocator/arena.rs");

    for callback in [
        "mi_output_fun",
        "mi_error_fun",
        "mi_deferred_free_fun",
        "mi_heap_visit_fun",
        "mi_block_visit_fun",
        "mi_commit_fun_t",
    ] {
        assert!(
            abi_doc.contains(callback) && types_rs.contains(&format!("pub type {callback}")),
            "{callback} should be listed in docs/abi.md and declared in src/types/constants.rs"
        );
    }

    for contract_word in [
        "Nullability",
        "Lifetime",
        "Threading",
        "Reentrancy",
        "Unwind",
    ] {
        assert!(
            types_rs.matches(contract_word).count() >= 6,
            "each public callback type should document {contract_word}"
        );
    }
    assert!(
        types_rs.matches("panic").count() >= 6 && types_rs.matches("unwind").count() >= 6,
        "each public callback type should document panic/unwind behavior"
    );

    for family in [
        "Output callback",
        "Error callback",
        "Deferred free callback",
        "Heap visitor callback",
        "Block visitor callback",
        "Managed-memory commit callback",
    ] {
        assert!(
            abi_doc.contains(family),
            "docs/abi.md should list the {family} callback family"
        );
    }

    assert!(
        abi_rs.contains(
            "pub fn mi_register_output(out: Option<mi_output_fun>, arg: *mut c_void);"
        ) && abi_rs
            .contains("pub fn mi_register_error(fun: Option<mi_error_fun>, arg: *mut c_void);")
            && abi_rs.contains(
                "pub fn mi_register_deferred_free(deferred_free: Option<mi_deferred_free_fun>, arg: *mut c_void);"
            )
            && abi_rs.contains("visitor: Option<mi_block_visit_fun>")
            && abi_rs.contains("visitor: Option<mi_heap_visit_fun>")
            && abi_rs.contains("commit_fun: Option<mi_commit_fun_t>"),
        "src/ffi/abi.rs should expose callback-accepting C declarations with nullable Option callbacks"
    );

    for forbidden in [
        "OutputCallback",
        "ErrorCallback",
        "DeferredFreeCallback",
        "HeapVisitCallback",
        "BlockVisitCallback",
        "ThreadLocalClaimCallback",
        "CallbackArg",
        "dyn Fn",
        "impl Fn",
    ] {
        assert!(
            !types_rs.contains(forbidden) && !abi_rs.contains(forbidden),
            "raw callback typedefs and ABI declarations must not use {forbidden}"
        );
    }

    assert!(
        options_rs.contains("pub extern \"C\" fn mi_register_output")
            && options_rs.contains("pub extern \"C\" fn mi_register_error")
            && page_rs.contains("pub unsafe extern \"C\" fn mi_register_deferred_free")
            && arena_rs.contains("pub unsafe extern \"C\" fn mi_manage_memory")
            && abi_doc.contains("mi_claim_fun_t")
            && abi_doc.contains("mi_forall_set_fun_t")
            && abi_doc.contains("crate-private implementation modules"),
        "callback registrations and internal callback families should be inventoried at their public boundary"
    );
}

#[test]
fn callback_trampolines_remain_no_alloc_no_unwind_raw_abi() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let types_rs = fs::read_to_string(root.join("src/types/constants.rs"))
        .expect("read src/types/constants.rs");
    let abi_rs = fs::read_to_string(root.join("src/ffi/abi.rs")).expect("read src/ffi/abi.rs");
    let callbacks_rs = fs::read_to_string(root.join("src/types/callbacks.rs"))
        .expect("read core_types/callbacks.rs");
    let options_rs = fs::read_to_string(root.join("src/allocator/options.rs"))
        .expect("read src/allocator/options.rs");
    let page_rs =
        fs::read_to_string(root.join("src/allocator/page.rs")).expect("read src/allocator/page.rs");
    let arena_rs = fs::read_to_string(root.join("src/allocator/arena.rs"))
        .expect("read src/allocator/arena.rs");
    let theap_rs = fs::read_to_string(root.join("src/allocator/thread_heap.rs"))
        .expect("read src/allocator/thread_heap.rs");
    let init_rs =
        fs::read_to_string(root.join("src/allocator/init.rs")).expect("read src/allocator/init.rs");

    for (source_name, source, marker) in [
        (
            "src/types/callbacks.rs",
            callbacks_rs.as_str(),
            "pub(crate) unsafe fn invoke(\n        self,\n        force: bool,",
        ),
        (
            "src/types/callbacks.rs",
            callbacks_rs.as_str(),
            "pub(crate) unsafe fn invoke(self, message: *const c_char,",
        ),
        (
            "src/types/callbacks.rs",
            callbacks_rs.as_str(),
            "pub(crate) unsafe fn invoke(self, err: i32,",
        ),
        (
            "src/types/callbacks.rs",
            callbacks_rs.as_str(),
            "pub(crate) unsafe fn invoke(self, heap: HeapPtr,",
        ),
        (
            "src/types/callbacks.rs",
            callbacks_rs.as_str(),
            "pub(crate) unsafe fn invoke(\n        self,\n        heap: HeapPtr,",
        ),
        (
            "src/types/callbacks.rs",
            callbacks_rs.as_str(),
            "pub(crate) unsafe fn invoke(\n        self,\n        slice_index: usize,",
        ),
        (
            "src/allocator/options.rs",
            options_rs.as_str(),
            "pub extern \"C\" fn mi_register_output",
        ),
        (
            "src/allocator/options.rs",
            options_rs.as_str(),
            "pub extern \"C\" fn mi_register_error",
        ),
        (
            "src/allocator/page.rs",
            page_rs.as_str(),
            "pub(crate) unsafe extern \"C\" fn _mi_deferred_free",
        ),
        (
            "src/allocator/page.rs",
            page_rs.as_str(),
            "pub unsafe extern \"C\" fn mi_register_deferred_free",
        ),
        (
            "src/allocator/arena.rs",
            arena_rs.as_str(),
            "pub unsafe extern \"C\" fn mi_heap_visit_blocks",
        ),
        (
            "src/allocator/arena.rs",
            arena_rs.as_str(),
            "pub unsafe extern \"C\" fn mi_heap_visit_abandoned_blocks",
        ),
        (
            "src/allocator/thread_heap.rs",
            theap_rs.as_str(),
            "pub unsafe extern \"C\" fn mi_theap_visit_blocks",
        ),
        (
            "src/allocator/init.rs",
            init_rs.as_str(),
            "pub unsafe extern \"C\" fn mi_subproc_visit_heaps",
        ),
    ] {
        let item = source_item(source_name, source, marker);
        assert_no_callback_alloc_or_unwind(source_name, marker, item);
    }

    assert!(
        types_rs.contains("pub type mi_output_fun = unsafe extern \"C\" fn")
            && types_rs.contains("pub type mi_error_fun = unsafe extern \"C\" fn")
            && types_rs.contains("pub type mi_deferred_free_fun = unsafe extern \"C\" fn")
            && types_rs.contains("pub type mi_heap_visit_fun = unsafe extern \"C\" fn")
            && types_rs.contains("pub type mi_block_visit_fun = unsafe extern \"C\" fn")
            && types_rs.contains("pub type mi_commit_fun_t = unsafe extern \"C\" fn")
            && types_rs.contains("pub type mi_claim_fun_t =\n    unsafe extern \"C\" fn"),
        "src/types/constants.rs should keep public and unstable callback typedefs raw C ABI function pointers"
    );
    assert!(
        abi_rs.contains("out: Option<mi_output_fun>")
            && abi_rs.contains("fun: Option<mi_error_fun>")
            && abi_rs.contains("deferred_free: Option<mi_deferred_free_fun>")
            && abi_rs.contains("visitor: Option<mi_block_visit_fun>")
            && abi_rs.contains("visitor: Option<mi_heap_visit_fun>")
            && abi_rs.contains("commit_fun: Option<mi_commit_fun_t>")
            && !types_rs.contains("CallbackArg")
            && !abi_rs.contains("CallbackArg"),
        "src/ffi/abi.rs should keep nullable raw callback typedefs at the C ABI boundary"
    );
}

#[test]
fn safe_callback_guards_document_lifetime_restore_and_unwind_policy() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let abi_doc = fs::read_to_string(root.join("docs/abi.md")).expect("read docs/abi.md");
    let safe_doc =
        fs::read_to_string(root.join("docs/safe-api.md")).expect("read docs/safe-api.md");
    let unsafe_doc =
        fs::read_to_string(root.join("docs/unsafe-audit.md")).expect("read docs/unsafe-audit.md");
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let unsafe_callbacks_rs =
        fs::read_to_string(root.join("src/ffi/callbacks.rs")).expect("read unsafe callbacks");

    assert!(
        abi_doc.contains("Raw callback registration remains unsafe.")
            && abi_doc.contains("`unsafe_callbacks::ErrorCallbackGuard` and")
            && abi_doc.contains("restore the previous registration on drop")
            && abi_doc.contains(
                "catch panic or unwind before returning across the\ncallback C ABI boundary"
            )
            && abi_doc.contains("Nested borrowed guards must be dropped in LIFO order")
            && abi_doc.contains("Output callbacks remain raw"),
        "docs/abi.md should document the safe callback guard proof and remaining raw output callback gap"
    );
    assert!(
        safe_doc.contains("`unsafe_callbacks`")
            && safe_doc.contains("`FnMut` callback guards")
            && safe_doc.contains("Tier U unsafe Rust convenience APIs")
            && safe_doc.contains("catch Rust panics")
            && safe_doc.contains("Nested guards must be dropped in LIFO order")
            && unsafe_doc.contains("SAFETY[AUDIT:unsafe-callback]")
            && unsafe_doc.contains("restores that previous registration on drop")
            && unsafe_doc.contains("restores the process-global snapshot")
            && unsafe_doc.contains("catches Rust\n  panics before callback ABI return")
            && unsafe_doc.contains("`error_callback_trampoline::<F>`")
            && unsafe_doc.contains("`deferred_free_callback_trampoline::<F>`")
            && unsafe_doc.contains(
                "no concurrent or reentrant entry may access the same\n  borrowed callback"
            ),
        "safe API and unsafe audit docs should prove safe callback guard lifetime, restore, and unwind behavior"
    );

    assert!(
        lib_rs.contains("pub use ffi::callbacks as unsafe_callbacks;")
            && unsafe_callbacks_rs.contains("std::panic::catch_unwind")
            && unsafe_callbacks_rs.contains("mi_register_error")
            && unsafe_callbacks_rs.contains("mi_register_deferred_free")
            && !unsafe_callbacks_rs.contains("mi_register_output"),
        "unsafe callback namespace should expose only callback guards that catch panics and restore previous registrations"
    );
}

#[test]
fn public_docs_match_current_build_mode_and_layout() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = fs::read_to_string(root.join("README.md")).expect("read README.md");
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let features_doc =
        fs::read_to_string(root.join("docs/features.md")).expect("read docs/features.md");
    let abi_doc = fs::read_to_string(root.join("docs/abi.md")).expect("read docs/abi.md");
    let api_inventory_doc =
        fs::read_to_string(root.join("docs/api-inventory.md")).expect("read docs/api-inventory.md");
    let compatibility_doc =
        fs::read_to_string(root.join("docs/compatibility.md")).expect("read docs/compatibility.md");
    let limitations_doc =
        fs::read_to_string(root.join("docs/limitations.md")).expect("read docs/limitations.md");
    let strict_types_doc =
        fs::read_to_string(root.join("docs/strict-types.md")).expect("read docs/strict-types.md");
    let memory_ordering_doc = fs::read_to_string(root.join("docs/memory-ordering.md"))
        .expect("read docs/memory-ordering.md");
    let unsafe_audit_doc =
        fs::read_to_string(root.join("docs/unsafe-audit.md")).expect("read docs/unsafe-audit.md");
    let assembly_doc = fs::read_to_string(root.join("docs/assembly-intrinsics.md"))
        .expect("read docs/assembly-intrinsics.md");
    let final_api_doc = fs::read_to_string(root.join("docs/final-api-surface-review.md"))
        .expect("read docs/final-api-surface-review.md");
    let raw_migration_doc = fs::read_to_string(root.join("docs/raw-facade-migration.md"))
        .expect("read docs/raw-facade-migration.md");
    let mimalloc_bench_rs =
        fs::read_to_string(root.join("benches/mimalloc_bench.rs")).expect("read mimalloc_bench.rs");

    for source in [&readme, &lib_rs, &abi_doc, &api_inventory_doc] {
        assert!(
            source.contains(
                "cargo rustc --features interpose --lib --release -- --crate-type cdylib"
            ),
            "public build docs should show the explicit interpose cdylib command"
        );
    }
    assert!(
        readme.contains("The default library build produces")
            && readme.contains("an `rlib` for Rust integration")
            && lib_rs.contains("The default library build produces")
            && lib_rs.contains("an `rlib` with a deliberately small Rust-facing surface")
            && abi_doc.contains("Default Cargo library builds produce an `rlib`"),
        "public docs should describe the rlib-first default build mode"
    );
    for required in [
        "## Choose a Path",
        "### 1. Rust Global Allocator",
        "### 2. Safe Rust Owned Bytes, Heaps, Stats, and Options",
        "### 3. C ABI Artifact and Interposition",
        "### Unsafe Raw Facade",
        "Use `MiMalloc` when you want Rust standard-library allocations",
        "Use the safe facade when you want Rust-native ownership and typed errors",
        "Use the opt-in C ABI artifact when you need mimalloc-compatible dynamic symbols",
        "Use `mechanical_mimalloc::raw` only when you can uphold mimalloc's pointer",
        "The raw Rust path surface is not stabilized as a Rust SemVer API.",
    ] {
        assert!(
            readme.contains(required),
            "README should be organized around the intended user stories: {required}"
        );
    }
    let global_allocator = readme.find("### 1. Rust Global Allocator").unwrap();
    let safe_facade = readme
        .find("### 2. Safe Rust Owned Bytes, Heaps, Stats, and Options")
        .unwrap();
    let c_abi = readme
        .find("### 3. C ABI Artifact and Interposition")
        .unwrap();
    let raw_facade = readme.find("### Unsafe Raw Facade").unwrap();
    assert!(
        global_allocator < safe_facade && safe_facade < c_abi && c_abi < raw_facade,
        "README should introduce safe Rust and C ABI user stories before the unsafe raw facade"
    );
    assert!(
        mimalloc_bench_rs
            .contains("cargo rustc --features interpose --lib --release -- --crate-type cdylib")
            && !mimalloc_bench_rs.contains("cargo build --release for Rust mimalloc cdylib")
            && !mimalloc_bench_rs.contains("run `cargo build --release`"),
        "native benchmark helper should build the opt-in interpose cdylib, not the default rlib artifact"
    );

    for feature in [
        "`bench-diagnostics`",
        "`c-abi`",
        "`cxx-overrides`",
        "`tls-shim`",
        "`interpose`",
    ] {
        assert!(
            readme.contains(feature)
                && lib_rs.contains(feature)
                && features_doc.contains(feature)
                && compatibility_doc.contains(feature),
            "{feature} should be documented wherever public feature policy is summarized"
        );
    }

    for stale in [
        "This repository currently builds both",
        "This crate is currently a dual artifact",
        "The crate builds a `cdylib`",
        "The Tier C `cdylib` exports",
        "keeps the ABI declaration owner flat",
        "instead of creating a\nnew `src/raw/` or `src/ffi/` tree",
        "`raw.rs` must remain",
        "`core_types`:",
        "`src/core_types`",
        "`src/types.rs` and `src/types/`",
        "`src/types.rs`",
        "`src/abi.rs`",
        "`src/alloc_override.rs`",
        "`src/init.rs`, `src/prim.rs`",
        "`src/atomic.rs`",
        "`src/atomic.rs` remains",
    ] {
        assert!(
            !readme.contains(stale)
                && !lib_rs.contains(stale)
                && !features_doc.contains(stale)
                && !abi_doc.contains(stale)
                && !api_inventory_doc.contains(stale)
                && !compatibility_doc.contains(stale)
                && !limitations_doc.contains(stale)
                && !strict_types_doc.contains(stale)
                && !memory_ordering_doc.contains(stale)
                && !unsafe_audit_doc.contains(stale)
                && !assembly_doc.contains(stale)
                && !final_api_doc.contains(stale)
                && !raw_migration_doc.contains(stale),
            "public docs should not retain stale build-mode or pre-layout wording: {stale}"
        );
    }
}

#[test]
fn ffi_linking_guide_matches_release_artifact_modes_and_consumers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let guide =
        fs::read_to_string(root.join("docs/ffi-linking.md")).expect("read docs/ffi-linking.md");
    let build_release = fs::read_to_string(root.join("scripts/build-release-artifacts.py"))
        .expect("read scripts/build-release-artifacts.py");
    let artifact_consumer =
        fs::read_to_string(root.join("scripts/check-release-artifact-consumer.py"))
            .expect("read scripts/check-release-artifact-consumer.py");
    let link_metadata = fs::read_to_string(root.join("scripts/check-release-link-metadata.py"))
        .expect("read scripts/check-release-link-metadata.py");
    let dlopen = fs::read_to_string(root.join("scripts/check-release-dlopen.py"))
        .expect("read scripts/check-release-dlopen.py");
    let ld_preload = fs::read_to_string(root.join("scripts/check-release-ld-preload.py"))
        .expect("read scripts/check-release-ld-preload.py");
    let cxx_artifacts = fs::read_to_string(root.join("scripts/check-release-cxx-artifacts.py"))
        .expect("read scripts/check-release-cxx-artifacts.py");

    for heading in [
        "## Release Artifact Layout",
        "## Dynamic C ABI Linking",
        "## Static C ABI Linking",
        "## LD_PRELOAD Interposition",
        "## C++ Operator Override Modes",
        "## Headers and Link Metadata",
        "## Supported Targets and Caveats",
    ] {
        assert!(
            guide.contains(heading),
            "FFI linking guide should include {heading}"
        );
    }

    for artifact_name in [
        "rust-rlib",
        "c-abi-cdylib",
        "interpose-cdylib",
        "c-abi-static-preview",
        "interpose-static-preview",
    ] {
        assert!(
            guide.contains(artifact_name) && build_release.contains(artifact_name),
            "FFI linking guide should document staged artifact mode {artifact_name}"
        );
    }

    for command_or_flag in [
        "cargo rustc --release --features c-abi --lib -- --crate-type cdylib",
        "cargo rustc --features interpose --lib --release -- --crate-type cdylib",
        "RUSTFLAGS=\"-C lto=off -C linker-plugin-lto=no -C embed-bitcode=no\"",
        "--crate-type staticlib",
        "LD_PRELOAD=/path/to/libmechanical_mimalloc-interpose-cdylib.so ./program",
    ] {
        assert!(
            guide.contains(command_or_flag),
            "FFI linking guide should show the current artifact command or flag: {command_or_flag}"
        );
    }

    for header in [
        "mimalloc.h",
        "mimalloc-stats.h",
        "mimalloc-override.h",
        "mimalloc-new-delete.h",
    ] {
        assert!(
            guide.contains(header)
                && build_release.contains(header)
                && artifact_consumer.contains(header),
            "FFI linking guide should document staged header {header}"
        );
    }

    for metadata_target in [
        "mechanical_mimalloc.pc",
        "mechanical_mimalloc-config.cmake",
        "mechanical_mimalloc::c_abi",
        "mechanical_mimalloc::interpose",
        "mechanical_mimalloc::c_abi_static_preview",
        "mechanical_mimalloc::interpose_static_preview",
    ] {
        assert!(
            guide.contains(metadata_target)
                && (build_release.contains(metadata_target)
                    || link_metadata.contains(metadata_target)),
            "FFI linking guide should document generated link metadata target {metadata_target}"
        );
    }

    for validation_script in [
        "scripts/check-release-artifact-consumer.py",
        "scripts/check-release-link-metadata.py",
        "scripts/check-release-dlopen.py",
        "scripts/check-release-ld-preload.py",
        "scripts/check-release-cxx-artifacts.py",
    ] {
        assert!(
            guide.contains(validation_script),
            "FFI linking guide should name consumer validation script {validation_script}"
        );
    }
    assert!(
        artifact_consumer.contains("\"c-abi-cdylib\"")
            && artifact_consumer.contains("\"c-abi-static-preview\"")
            && artifact_consumer.contains("\"interpose-cdylib\"")
            && artifact_consumer.contains("\"interpose-static-preview\"")
            && dlopen.contains("\"c-abi-cdylib\"")
            && ld_preload.contains("\"interpose-cdylib\"")
            && cxx_artifacts.contains("\"interpose-cdylib\"")
            && cxx_artifacts.contains("\"interpose-static-preview\""),
        "consumer scripts should exercise every artifact mode claimed by the FFI linking guide"
    );
    assert!(
        guide.contains("Linux x86_64 GNU is the maintained production target")
            && guide.contains("Unsupported platforms must not infer a stable C\nABI")
            && guide.contains("Static archives are preview-only")
            && guide.contains("not a stable static ABI"),
        "FFI linking guide should state supported target and preview/static caveats"
    );
}

#[test]
fn symbol_versioning_policy_matches_symbol_scripts_and_manifests() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let policy = fs::read_to_string(root.join("docs/symbol-versioning.md"))
        .expect("read docs/symbol-versioning.md");
    let abi_doc = fs::read_to_string(root.join("docs/abi.md")).expect("read docs/abi.md");
    let release_doc =
        fs::read_to_string(root.join("docs/release.md")).expect("read docs/release.md");
    let readme = fs::read_to_string(root.join("README.md")).expect("read README.md");
    let symbol_script = fs::read_to_string(root.join("scripts/check-public-symbols.py"))
        .expect("read scripts/check-public-symbols.py");
    let abi_diff_script =
        fs::read_to_string(root.join("scripts/check-abi-diff.py")).expect("read abi diff script");
    let classification_tsv =
        fs::read_to_string(root.join("symbols/public-symbol-classification.tsv"))
            .expect("read public symbol classification");

    for heading in [
        "## Public C Header Symbols",
        "## Malloc-Family Interposition Symbols",
        "## C++ Operator Symbols",
        "## Extension ABI Naming Rules",
        "## Static Archive Internal-Symbol Policy",
        "## Release-to-Release ABI Diff Expectations",
        "## Required Checks",
    ] {
        assert!(
            policy.contains(heading),
            "symbol versioning policy should include {heading}"
        );
    }

    for header in [
        "mimalloc-3.3.2/include/mimalloc.h",
        "mimalloc-3.3.2/include/mimalloc-stats.h",
    ] {
        assert!(
            policy.contains(header) && symbol_script.contains(header),
            "symbol policy should document public header source {header}"
        );
    }

    for script_constant in [
        "MALLOC_OVERRIDE_SYMBOLS",
        "LIBC_OVERRIDE_SYMBOLS",
        "MALLOC_FAMILY_SYMBOLS",
        "CXX_OPERATOR_OVERRIDE_SYMBOLS",
    ] {
        assert!(
            policy.contains(script_constant) && symbol_script.contains(script_constant),
            "symbol policy should document script-controlled symbol group {script_constant}"
        );
    }

    for representative_symbol in [
        "malloc",
        "calloc",
        "realloc",
        "free",
        "posix_memalign",
        "__libc_malloc",
        "__posix_memalign",
        "_Znwm",
        "_Znam",
        "_ZdlPv",
        "_ZdaPv",
    ] {
        assert!(
            policy.contains(representative_symbol) && symbol_script.contains(representative_symbol),
            "symbol policy should name representative checked symbol {representative_symbol}"
        );
    }

    for classification in [
        "internal-export-required",
        "rust-extension",
        "platform-helper-extension",
    ] {
        assert!(
            policy.contains(classification) && classification_tsv.contains(classification),
            "symbol policy should define TSV classification {classification}"
        );
    }

    for manifest in [
        "c-abi-dynamic.tsv",
        "c-abi-secure-dynamic.tsv",
        "c-abi-guarded-dynamic.tsv",
        "c-abi-track-valgrind-dynamic.tsv",
        "c-abi-track-asan-dynamic.tsv",
        "interpose-dynamic.tsv",
    ] {
        assert!(
            policy.contains(manifest) && root.join("docs/abi-manifests").join(manifest).exists(),
            "symbol policy should list checked ABI manifest {manifest}"
        );
    }

    for command_or_marker in [
        "scripts/check-public-symbols.py --dynamic-mi",
        "scripts/check-public-symbols.py --dynamic-groups",
        "scripts/check-public-symbols.py --dynamic-manifests",
        "scripts/check-public-symbols.py --write-dynamic-manifests",
        "scripts/check-abi-symbols.sh",
        "scripts/check-abi-diff.py --previous-ref <previous-tag>",
        "scripts/check-abi-diff.py --previous-dir <previous-release>/abi-manifests",
        "ABI-CHANGE: <manifest> <symbol>",
        "ABI-CHANGE: <manifest> *",
    ] {
        assert!(
            policy.contains(command_or_marker),
            "symbol policy should document release command or marker {command_or_marker}"
        );
    }
    assert!(
        abi_diff_script.contains("visibility-changed")
            && abi_diff_script.contains("binding-changed")
            && abi_diff_script.contains("version-changed")
            && abi_diff_script.contains("type-changed")
            && policy.contains("visibility-changed")
            && policy.contains("binding-changed")
            && policy.contains("version-changed")
            && policy.contains("type-changed"),
        "symbol policy should match ABI diff classifications"
    );
    assert!(
        policy.contains("mechanical_mimalloc_*")
            && policy.contains("There is no supported extension ABI in the current release policy")
            && policy.contains("Static archives are preview-only")
            && abi_doc.contains("docs/symbol-versioning.md")
            && release_doc.contains("docs/symbol-versioning.md")
            && readme.contains("docs/symbol-versioning.md"),
        "symbol policy should be linked and should define extension/static policy"
    );
}

#[test]
fn c_mimalloc_migration_guide_maps_headers_apis_callbacks_and_platforms() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let guide = fs::read_to_string(root.join("docs/c-mimalloc-migration.md"))
        .expect("read docs/c-mimalloc-migration.md");
    let readme = fs::read_to_string(root.join("README.md")).expect("read README.md");
    let release_doc =
        fs::read_to_string(root.join("docs/release.md")).expect("read docs/release.md");
    let compatibility_doc =
        fs::read_to_string(root.join("docs/compatibility.md")).expect("read compatibility docs");
    let abi_doc = fs::read_to_string(root.join("docs/abi.md")).expect("read docs/abi.md");
    let ffi_linking =
        fs::read_to_string(root.join("docs/ffi-linking.md")).expect("read ffi linking docs");
    let safe_doc = fs::read_to_string(root.join("docs/safe-api.md")).expect("read safe API docs");
    let features_doc =
        fs::read_to_string(root.join("docs/features.md")).expect("read feature docs");

    for heading in [
        "## Header to Artifact Map",
        "## Allocation and Free APIs",
        "## Callback Migration",
        "## Options and Statistics",
        "## Unsupported Platform and Feature Differences",
    ] {
        assert!(
            guide.contains(heading),
            "C mimalloc migration guide should include {heading}"
        );
    }

    for linked_doc in [
        "docs/compatibility.md",
        "docs/abi.md",
        "docs/ffi-linking.md",
        "docs/symbol-versioning.md",
        "docs/platforms.md",
        "docs/limitations.md",
    ] {
        assert!(
            guide.contains(linked_doc),
            "C mimalloc migration guide should link {linked_doc}"
        );
    }
    assert!(
        readme.contains("docs/c-mimalloc-migration.md")
            && release_doc.contains("docs/c-mimalloc-migration.md"),
        "migration guide should be discoverable from README and release policy docs"
    );

    for header in [
        "mimalloc.h",
        "mimalloc-stats.h",
        "mimalloc-override.h",
        "mimalloc-new-delete.h",
    ] {
        assert!(
            guide.contains(header) && ffi_linking.contains(header),
            "migration guide should map C header {header} to release artifacts"
        );
    }
    for artifact in [
        "c-abi-cdylib",
        "interpose-cdylib",
        "interpose-static-preview",
    ] {
        assert!(
            guide.contains(artifact) && ffi_linking.contains(artifact),
            "migration guide should map to artifact {artifact}"
        );
    }

    for c_api in [
        "mi_malloc",
        "mi_zalloc",
        "mi_calloc",
        "mi_realloc",
        "mi_free",
        "mi_heap_*",
    ] {
        assert!(
            guide.contains(c_api),
            "migration guide should discuss C allocation API {c_api}"
        );
    }
    for rust_choice in [
        "MiMalloc",
        "Allocation",
        "InitializedAllocation",
        "Heap",
        "HeapAllocation",
        "mechanical_mimalloc::raw",
        "raw::headers",
        "c-header-macros",
    ] {
        assert!(
            guide.contains(rust_choice),
            "migration guide should map to Rust choice {rust_choice}"
        );
    }

    for callback_or_obligation in [
        "unsafe_callbacks::ErrorCallbackGuard",
        "unsafe_callbacks::DeferredFreeCallbackGuard",
        "mi_register_error",
        "mi_register_deferred_free",
        "Tier U",
        "Nested guards must be dropped in LIFO order",
        "Output callbacks",
    ] {
        assert!(
            guide.contains(callback_or_obligation)
                && (abi_doc.contains(callback_or_obligation)
                    || safe_doc.contains(callback_or_obligation)
                    || compatibility_doc.contains(callback_or_obligation)),
            "migration guide should align callback migration term {callback_or_obligation}"
        );
    }
    assert!(
        guide.contains("serialize registration")
            && guide.contains("restoration, and callback entry")
            && safe_doc.contains("concurrent registration")
            && safe_doc.contains("restoration, or callback entry remains the caller's obligation")
            && abi_doc.contains("restoration, or callback entry for the borrowed closure"),
        "migration guide should align callback serialization obligations with safe and ABI docs"
    );

    for option_or_stats in [
        "GlobalOptions",
        "OptionKey",
        "OptionError",
        "StatsSnapshot",
        "StatCount",
        "StatTotal",
        "ProcessInfo",
    ] {
        assert!(
            guide.contains(option_or_stats) && safe_doc.contains(option_or_stats),
            "migration guide should map safe option/stat wrapper {option_or_stats}"
        );
    }

    for feature_or_platform in [
        "Linux x86_64 GNU",
        "`std`",
        "`secure`",
        "`guarded`",
        "`track-valgrind`",
        "`track-asan`",
        "`c-abi`",
        "`cxx-overrides`",
        "`tls-shim`",
        "`interpose`",
    ] {
        assert!(
            guide.contains(feature_or_platform)
                && (features_doc.contains(feature_or_platform)
                    || ffi_linking.contains(feature_or_platform)
                    || compatibility_doc.contains(feature_or_platform)),
            "migration guide should align feature/platform caveat {feature_or_platform}"
        );
    }
    assert!(
        guide.contains("default features are empty")
            && features_doc.contains("Default features are intentionally empty")
            && guide.contains("Static archives are\npreview-only")
            && ffi_linking.contains("Static archives are preview-only"),
        "migration guide should align default-feature and static-preview caveats"
    );
}

#[test]
fn production_status_text_agrees_across_public_docs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = fs::read_to_string(root.join("README.md")).expect("read README.md");
    let limitations =
        fs::read_to_string(root.join("docs/limitations.md")).expect("read limitations");
    let platforms = fs::read_to_string(root.join("docs/platforms.md")).expect("read platforms");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");

    for doc in [&readme, &limitations, &platforms, &release] {
        for required in [
            "Production status:",
            "Supported: Tier S safe Rust facade, Tier U unsafe callback guards, and Tier C",
            "dynamic C ABI release artifacts `c-abi-cdylib` and `interpose-cdylib`.",
            "Preview-only: staticlib release artifacts `c-abi-static-preview` and",
            "`interpose-static-preview`.",
            "Unstable: Tier R raw Rust facade path stability, translated implementation",
            "modules, internal strict types, extension ABI, and static archive internal",
            "symbols.",
            "Unsupported: source-present non-maintained platforms are not release artifact",
            "targets.",
            "Maintained target: Linux x86_64 GNU only.",
        ] {
            assert!(
                doc.contains(required),
                "production status docs should agree on: {required}"
            );
        }
    }
    assert!(
        readme.contains("The raw Rust facade is unstable as a Rust path surface")
            && limitations.contains("The `raw` facade exposes C-shaped unsafe APIs")
            && platforms.contains("CI may run smoke validation")
            && release.contains("Tier R is the unsafe raw Rust facade"),
        "production status text should keep raw instability and smoke-target caveats visible"
    );
}

#[test]
fn platform_promotion_policy_requires_release_evidence_checklist() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let platforms = fs::read_to_string(root.join("docs/platforms.md")).expect("read platforms");

    for required in [
        "## Platform Promotion Checklist",
        "Compile and test CI for the platform",
        "ABI/layout checks where the platform has a C ABI story",
        "C and C++ link tests where FFI artifacts are claimed",
        "Sanitizer or equivalent hardening evidence",
        "Platform primitive audit",
        "Benchmark evidence on the promoted platform",
        "Documented limitations",
        "Until every checklist item is complete, the platform remains source-present or\nsmoke-tested only, not maintained.",
    ] {
        assert!(
            platforms.contains(required),
            "docs/platforms.md should require promotion evidence: {required}"
        );
    }
    for check in [
        "scripts/check-layout.sh",
        "scripts/check-release-artifact-consumer.py",
        "scripts/check-release-link-metadata.py",
        "scripts/check-release-dlopen.py",
        "scripts/check-release-ld-preload.py",
        "scripts/check-release-cxx-artifacts.py",
        "ASan/TSan",
        "Miri",
        "Valgrind",
    ] {
        assert!(
            platforms.contains(check),
            "platform promotion checklist should name relevant validation evidence: {check}"
        );
    }
}

#[test]
fn source_present_platforms_stay_out_of_release_marketing() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme = fs::read_to_string(root.join("README.md")).expect("read README.md");
    let platforms = fs::read_to_string(root.join("docs/platforms.md")).expect("read platforms");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");
    let validation_workflow = fs::read_to_string(root.join(".github/workflows/validation.yml"))
        .expect("read validation workflow");

    for target in ["macOS", "Windows", "WASI", "Emscripten"] {
        assert!(
            readme.contains(target) && platforms.contains(target) && release.contains(target),
            "source-present or smoke target should be named without marketing overclaim: {target}"
        );
    }
    assert!(
        (readme.contains("Linux aarch64") || readme.contains("Linux\naarch64"))
            && platforms.contains("Linux aarch64")
            && release.contains("Linux aarch64"),
        "source-present or smoke target should be named without marketing overclaim: Linux aarch64"
    );
    for smoke_job in [
        "Linux aarch64 smoke validation",
        "macOS smoke validation",
        "Windows smoke validation",
    ] {
        assert!(
            validation_workflow.contains(smoke_job)
                && platforms.contains(smoke_job)
                && platforms.contains("do\nnot by themselves promote those platforms"),
            "CI smoke job should be documented as non-promoting: {smoke_job}"
        );
    }
    for required in [
        "Linux x86_64 GNU is the only maintained target.",
        "smoke-tested or\nsource-present only unless promoted in docs/platforms.md",
        "triaged as portability issues",
        "not release blockers unless\nthey reproduce on Linux x86_64 GNU",
        "cross-platform safety/security\nbug",
    ] {
        assert!(
            platforms.contains(required),
            "platform docs should include unsupported target release-note language: {required}"
        );
    }
    assert!(
        readme.contains("The maintained target for the current port is Linux x86_64 GNU only.")
            && readme.contains("they do not imply maintained target\nsupport")
            && readme.contains("source-present only unless `docs/platforms.md` promotes them")
            && release.contains("unsupported-target triage language from\n   `docs/platforms.md`")
            && release.contains("do not market Linux aarch64, macOS, Windows, WASI,\n   Emscripten, or other source-present targets as maintained"),
        "README and release policy should avoid source-present platform marketing claims"
    );
}

#[test]
fn private_vulnerability_reporting_process_is_concrete() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let security = fs::read_to_string(root.join("SECURITY.md")).expect("read SECURITY.md");

    for required in [
        "https://github.com/whb07/mechanical_mimalloc/security/advisories/new",
        "Do not file a public issue with vulnerability details",
        "If GitHub private vulnerability reporting is unavailable",
        "Acknowledge receipt within 3 business days",
        "initial triage within 7 calendar days",
        "severity/fix-plan update within 14 calendar days",
        "Coordinate public disclosure through a GitHub Security Advisory",
    ] {
        assert!(
            security.contains(required),
            "SECURITY.md should define concrete private reporting process: {required}"
        );
    }
    for report_detail in [
        "affected crate version, release tag, or commit SHA",
        "target triple",
        "`c-abi` cdylib",
        "`interpose` cdylib",
        "preview static archive",
        "enabled features",
        "minimized reproducer",
        "whether C mimalloc 3.3.2 reproduces",
        "sanitizer, Miri, Valgrind",
    ] {
        assert!(
            security.contains(report_detail),
            "SECURITY.md should tell reporters to include {report_detail}"
        );
    }
    assert!(
        security.contains("Linux x86_64 GNU is the currently maintained target")
            && security.contains("triaged as portability issues")
            && security.contains("reproduces on Linux x86_64 GNU")
            && security.contains("cross-platform safety or security bug")
            && security.contains("not release blockers for the maintained target line"),
        "SECURITY.md should keep unsupported-target security triage clear"
    );
}

#[test]
fn release_signing_policy_and_dry_run_are_concrete() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let signing =
        fs::read_to_string(root.join("docs/release-signing.md")).expect("read release-signing");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");
    let build_release = fs::read_to_string(root.join("scripts/build-release-artifacts.py"))
        .expect("read build-release-artifacts");
    let integrity = fs::read_to_string(root.join("scripts/check-release-integrity.py"))
        .expect("read check-release-integrity");

    for required in [
        "Release signing uses SSH Ed25519 signatures",
        "signed annotated Git tags",
        "git tag -s vX.Y.Z",
        "SHA256SUMS.sig",
        "ssh-ed25519-detached-sha256sums",
        "MECHANICAL_MIMALLOC_RELEASE_SIGNING_KEY",
        "mechanical_mimalloc_release",
        "ephemeral test-only",
        "Do not treat an allowed-signers file from an\nuntrusted asset bundle as the trust root.",
        "## Rotation",
    ] {
        assert!(
            signing.contains(required),
            "release signing policy should document {required}"
        );
    }

    for script_marker in [
        "ssh-keygen",
        "-Y",
        "sign",
        "verify_checksum_signature",
        "SHA256SUMS.sig",
        "release-signing.pub",
        "release-allowed-signers",
        "MECHANICAL_MIMALLOC_RELEASE_SIGNING_KEY",
        "ephemeral-dry-run-key",
        "ssh-ed25519-detached-sha256sums",
    ] {
        assert!(
            build_release.contains(script_marker),
            "release artifact builder should wire signing marker {script_marker}"
        );
    }

    assert!(
        integrity.contains("builder.validate_integrity(OUT_DIR, manifest)"),
        "release integrity dry run should verify signatures through the builder validation path"
    );
    assert!(
        release.contains("docs/release-signing.md")
            && release.contains("SHA256SUMS.sig")
            && release.contains("configured release SSH Ed25519 key")
            && release.contains("public key fingerprint from `release-artifacts.json`")
            && release.contains("signed annotated source tag with the release SSH\n   signing key"),
        "release checklist should include checksum signature and source tag verification"
    );
}

#[test]
fn release_dependency_report_is_attached_and_tool_aware() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dependencies =
        fs::read_to_string(root.join("docs/dependencies.md")).expect("read dependencies");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");
    let build_release = fs::read_to_string(root.join("scripts/build-release-artifacts.py"))
        .expect("read build-release-artifacts");
    let integrity = fs::read_to_string(root.join("scripts/check-release-integrity.py"))
        .expect("read check-release-integrity");

    for required in [
        "target/release-artifacts/integrity/dependencies.md",
        "cargo metadata --locked --format-version 1 --all-features",
        "direct runtime, build, and development dependencies",
        "resolved third-party package graph",
        "C compiler (`CC` or `cc`)",
        "C++ compiler (`CXX` or `c++`)",
        "archiver (`AR` or `ar`)",
    ] {
        assert!(
            dependencies.contains(required),
            "dependency policy should document release dependency report item: {required}"
        );
    }

    for marker in [
        "write_dependency_report",
        "dependency-report",
        "cargo_metadata_all_features",
        "cargo metadata",
        "--all-features",
        "## Direct Runtime Dependencies",
        "## Direct Development Dependencies",
        "## Build Tool Requirements",
        "C compiler",
        "C++ compiler",
        "Archiver",
        "## Resolved Third-Party Packages",
    ] {
        assert!(
            build_release.contains(marker),
            "release artifact builder should generate dependency report marker {marker}"
        );
    }

    assert!(
        integrity.contains("builder.validate_integrity(OUT_DIR, manifest)"),
        "release integrity dry run should validate dependency report through builder validation"
    );
    assert!(
        release.contains("target/release-artifacts/integrity/dependencies.md")
            && release.contains("dependency/build-tool report")
            && release.contains("attach or link that\n   dependency/build-tool report"),
        "release checklist should require the generated dependency report in release notes"
    );
}

#[test]
fn reproducible_release_instructions_are_concrete_and_checked() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let reproducibility =
        fs::read_to_string(root.join("docs/reproducibility.md")).expect("read reproducibility");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");
    let script = fs::read_to_string(root.join("scripts/check-release-reproducibility.py"))
        .expect("read check-release-reproducibility");

    for required in [
        "Release Artifact Reproduction",
        "git status --short",
        "git submodule status --recursive",
        "rust-toolchain.toml",
        "Rust channel: `1.92.0`",
        "Required components: `rustfmt`, `clippy`",
        "cargo fetch --locked",
        "scripts/build-release-artifacts.py",
        "scripts/check-release-integrity.py",
        "target/release-artifacts/SHA256SUMS",
        "target/release-artifacts/release-artifacts.json",
        "target/release-artifacts/integrity/abi-symbols/",
        "scripts/env-info.sh > target/release-artifacts/integrity/environment.txt",
        "scripts/check-release-reproducibility.py",
    ] {
        assert!(
            reproducibility.contains(required),
            "reproducibility docs should include release reproduction marker {required}"
        );
    }

    for artifact_mode in [
        "c-abi-cdylib",
        "c-abi-static-preview",
        "interpose-cdylib",
        "interpose-static-preview",
    ] {
        assert!(
            reproducibility.contains(artifact_mode),
            "reproducibility docs should name artifact mode {artifact_mode}"
        );
    }

    for marker in [
        "tracked-file checkouts",
        "build_release",
        "artifact_inventory",
        "normalized_symbol_manifest",
        "SHA256SUMS",
        "integrity/abi-symbols",
        "release artifact names and ABI symbol manifests reproduce",
    ] {
        assert!(
            script.contains(marker),
            "reproducibility script should include marker {marker}"
        );
    }

    assert!(
        release.contains("scripts/check-release-reproducibility.py")
            && release.contains("reproduces artifact names and ABI symbol manifests"),
        "release checklist should require the reproduction gate"
    );
}

#[test]
fn supported_version_policy_is_concrete_and_release_aligned() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let security = fs::read_to_string(root.join("SECURITY.md")).expect("read SECURITY.md");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");

    for required in [
        "Until the first published release",
        "current `master` branch only",
        "Latest published release line",
        "Receives security patch releases",
        "Older release lines",
        "Unsupported unless release notes explicitly announce extended support",
        "Release candidate or other pre-release branches",
        "Receive fixes only until the final release for that version is published",
        "There is no implicit long-term support line",
        "affected\nversions, the first fixed version",
        "required upgrade path",
        "Pre-release branches stop receiving security fixes",
    ] {
        assert!(
            security.contains(required),
            "security policy should define supported-version marker {required}"
        );
    }

    for required in [
        "Supported Version Policy",
        "`SECURITY.md` is the authoritative supported-version policy",
        "latest published release line receives security patch releases",
        "older release\nlines are unsupported unless release notes explicitly announce extended support",
        "release candidate or other pre-release branches stop receiving fixes",
        "affected versions, the\nfirst fixed version",
        "whether older lines receive fixes",
        "when pre-release branches stop\n   receiving fixes",
    ] {
        assert!(
            release.contains(required),
            "release policy should align supported-version marker {required}"
        );
    }
}

#[test]
fn cve_and_advisory_policy_is_linked_and_concrete() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let advisory = fs::read_to_string(root.join("docs/security-advisories.md"))
        .expect("read security advisory policy");
    let security = fs::read_to_string(root.join("SECURITY.md")).expect("read SECURITY.md");
    let governance = fs::read_to_string(root.join("docs/governance.md")).expect("read governance");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");

    for required in [
        "Severity Triage",
        "Critical",
        "High",
        "Medium",
        "Low",
        "CVE Handling",
        "Request or reserve a CVE",
        "Embargo Handling",
        "default embargo target is up to 90 calendar days",
        "Embargo extensions require reporter agreement",
        "Coordinated Disclosure",
        "private security branch or private advisory fork",
        "Publish fixed releases, tags, checksums, release notes, changelog, and the\n   advisory in the same disclosure window",
        "Changelog and Advisory Publication",
        "The changelog `Security` entry must include",
        "The GitHub Security Advisory must include",
    ] {
        assert!(
            advisory.contains(required),
            "advisory policy should include marker {required}"
        );
    }

    for required in [
        "affected versions, release lines, target platforms, artifact modes, and\n  features",
        "whether C mimalloc 3.3.2 reproduces the same behavior",
        "affected versions and first fixed version",
        "workaround or mitigation status",
        "links to release notes, changelog, signed checksums",
    ] {
        assert!(
            advisory.contains(required),
            "advisory policy should require publication field {required}"
        );
    }

    assert!(
        security.contains("docs/security-advisories.md")
            && governance.contains("docs/security-advisories.md")
            && release.contains("docs/security-advisories.md"),
        "security, governance, and release docs should link to advisory policy"
    );
    assert!(
        governance.contains("severity triage, CVE handling, embargoes")
            && release.contains("changelog `Security` entry requirements"),
        "governance and release docs should summarize advisory policy obligations"
    );
}

#[test]
fn fuzz_corpus_and_reproducer_policy_maps_to_security_and_unsafe_audit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let testing = fs::read_to_string(root.join("docs/testing.md")).expect("read testing");
    let security = fs::read_to_string(root.join("SECURITY.md")).expect("read SECURITY.md");
    let unsafe_audit =
        fs::read_to_string(root.join("docs/unsafe-audit.md")).expect("read unsafe audit");

    for required in [
        "fuzz/corpus/alloc_sequences/<descriptive-name>",
        "fuzz/fuzz_targets/alloc_sequences.rs",
        "A focused Rust regression test under `tests/` or the owning source module",
        "Corpus additions are required",
        "valid\nsafe API use, documented raw/C ABI use, release artifact behavior",
        "allocator-owned memory corruption, realloc preservation, zeroing,\nalignment, heap ownership, cross-thread free",
        "Do not commit corpus inputs that depend on invalid caller UB",
        "A\nfinding is an allocator bug",
        "A\nfinding is invalid-caller UB",
        "forged pointers, foreign\nallocations, use-after-free",
        "Confirmed fuzz findings that touch unsafe invariants must update the unsafe\naudit mapping",
        "docs/unsafe-audit-evidence.tsv",
        "SAFETY[AUDIT:*]",
        "Private security reproducers stay out of the public corpus",
    ] {
        assert!(
            testing.contains(required),
            "testing docs should define fuzz reproducer policy marker {required}"
        );
    }

    for required in [
        "docs/testing.md",
        "fuzz corpus and minimized reproducer policy",
        "invalid-caller undefined behavior versus allocator bugs",
        "unsafe-audit evidence",
    ] {
        assert!(
            security.contains(required),
            "security policy should link fuzz policy marker {required}"
        );
    }

    assert!(
        unsafe_audit.contains("Fuzz Finding Mapping")
            && unsafe_audit.contains("docs/testing.md")
            && unsafe_audit.contains("docs/unsafe-audit-evidence.tsv")
            && unsafe_audit.contains("SAFETY[AUDIT:*]"),
        "unsafe audit docs should map confirmed fuzz findings to audit evidence"
    );
}

#[test]
fn hardening_threat_model_is_concrete_and_linked_from_security_policy() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let threat_model = fs::read_to_string(root.join("docs/hardening-threat-model.md"))
        .expect("read hardening threat model");
    let security = fs::read_to_string(root.join("SECURITY.md")).expect("read SECURITY.md");
    let release = fs::read_to_string(root.join("docs/release.md")).expect("read release");

    for required in [
        "Hardening Threat Model",
        "Baseline Assumptions",
        "The `secure` feature is expected to mitigate or detect selected allocator\nmetadata corruptions",
        "secure-mode padding, canaries, encoded free lists",
        "Non-goals",
        "The `guarded` feature is expected to detect selected out-of-bounds and\nuse-after-free patterns",
        "guarded size bounds, sample rate, sample\ncount",
        "Out Of Scope For Malicious Unsafe Callers",
        "forged pointers, foreign allocations, or interior pointers",
        "use-after-free, double-free, or realloc-after-free",
        "arbitrary process-memory write, code execution, or data-race capability",
        "Entropy, Canaries, And Metadata Corruption",
        "predictable entropy, repeated\ncanaries, or failed random initialization",
        "not a cryptographic boundary",
        "Allocator metadata corruption is in scope",
        "Hardening reports should include",
    ] {
        assert!(
            threat_model.contains(required),
            "hardening threat model should include marker {required}"
        );
    }

    assert!(
        security.contains("docs/hardening-threat-model.md")
            && security.contains("`secure`, `guarded`, malicious unsafe callers")
            && release.contains("docs/hardening-threat-model.md"),
        "security and release docs should link to hardening threat model"
    );
}

#[test]
fn memory_corruption_triage_checklist_is_concrete_in_security_policy() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let security = fs::read_to_string(root.join("SECURITY.md")).expect("read SECURITY.md");

    for required in [
        "Memory Corruption Triage Checklist",
        "allocation/free/realloc sequence",
        "API names, sizes, alignments",
        "zeroing expectations, preserved-byte expectations",
        "allocation origin, heap or\n  main-heap ownership",
        "thread ownership, cross-thread transfer",
        "feature set and target platform",
        "Cargo features, target triple",
        "`secure`/`guarded`/tracking mode",
        "sanitizer/Valgrind/Miri evidence where applicable",
        "exact commands,\n  logs, stack traces, core dump notes",
        "upstream C mimalloc comparison against vendored C mimalloc 3.3.2",
        "FFI symbol/artifact mode details",
        "`c-abi-cdylib`, `interpose-cdylib`",
        "`LD_PRELOAD`, `dlopen`, C++\n  operator override use",
        "unexpected exported or resolved symbol",
    ] {
        assert!(
            security.contains(required),
            "security policy should include memory-corruption checklist marker {required}"
        );
    }
}

#[test]
fn upstream_sync_policy_includes_cadence_and_required_evidence_updates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let upstream = fs::read_to_string(root.join("docs/upstream.md")).expect("read upstream docs");

    for required in [
        "Review Cadence",
        "at least quarterly and before every\nrelease candidate",
        "upstream publishes a security\nrelease, CVE",
        "Do not update vendored sources without updating parity,\nABI, release, and benchmark evidence",
        "When upstream public headers or C ABI declarations change, re-run",
        "scripts/check-public-abi.py",
        "scripts/check-public-symbols.py --write-dynamic-manifests",
        "scripts/check-abi-symbols.sh",
        "scripts/check-release-artifact-consumer.py",
        "When upstream implementation behavior changes, re-run",
        "scripts/check-behavioral-parity.py",
        "scripts/check-unsafe-audit-evidence.py",
        "scripts/check-fuzz-smoke.sh",
        "For allocator hot paths, page/arena behavior, platform primitives",
        "Evidence Updates",
        "`../UPSTREAM_DIFF.md`: baseline version",
        "`PARITY.md`: supported parity matrix",
        "ABI manifests under `docs/abi-manifests/`",
        "`BENCHMARK_RESULTS.md`: benchmark command lines",
        "geomean Rust/C ratios",
    ] {
        assert!(
            upstream.contains(required),
            "upstream sync policy should include marker {required}"
        );
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
            && freelist_rs.contains("crate::mi_assert_internal!((val & Self::OWNED_BIT) == 0);")
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
    for (rel, source) in &sources {
        let count = dead_code_allowance_count(source);
        if count > 0 {
            actual.insert(rel.as_str(), count);
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
        ("src/internal/bits.rs", 1),
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
        ("src/internal/provenance.rs", 1),
        ("src/allocator/static_alloc.rs", 1),
        ("src/allocator/stats.rs", 1),
        ("src/allocator/thread_heap.rs", 1),
        ("src/allocator/tracking.rs", 1),
        ("src/types/constants.rs", 1),
    ]);

    assert_eq!(
        actual, expected,
        "Phase 6 dead_code allowances should stay on the audited compatibility/scaffold allowlist"
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
        vec!["src/types/constants.rs", "src/types/freelist_wrappers.rs"],
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
        "needless_pass_by_ref_mut",
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
        "unnecessary_wraps",
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

#[test]
fn free_path_internals_are_not_dynamic_exports() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let page_map = fs::read_to_string(root.join("src/allocator/page_map.rs"))
        .expect("read src/allocator/page_map.rs");
    let internal =
        fs::read_to_string(root.join("src/internal/mod.rs")).expect("read src/internal/mod.rs");
    let alloc_rs = fs::read_to_string(root.join("src/allocator/alloc.rs"))
        .expect("read src/allocator/alloc.rs");
    let arena_rs = fs::read_to_string(root.join("src/allocator/arena.rs"))
        .expect("read src/allocator/arena.rs");
    let arena_meta_rs = fs::read_to_string(root.join("src/allocator/arena_meta.rs"))
        .expect("read src/allocator/arena_meta.rs");
    let atomic_rs = fs::read_to_string(root.join("src/internal/atomic.rs"))
        .expect("read src/internal/atomic.rs");
    let bitmap_rs = fs::read_to_string(root.join("src/internal/bitmap.rs"))
        .expect("read src/internal/bitmap.rs");
    let free_rs =
        fs::read_to_string(root.join("src/allocator/free.rs")).expect("read src/allocator/free.rs");
    let libc_rs = fs::read_to_string(root.join("src/ffi/libc.rs")).expect("read src/ffi/libc.rs");
    let memid_rs =
        fs::read_to_string(root.join("src/types/memid.rs")).expect("read core_types/memid.rs");
    let callbacks_rs = fs::read_to_string(root.join("src/types/callbacks.rs"))
        .expect("read core_types/callbacks.rs");
    let freelist_types = fs::read_to_string(root.join("src/types/freelist_wrappers.rs"))
        .expect("read core_types/freelist_wrappers.rs");
    let arena_types =
        fs::read_to_string(root.join("src/types/arena.rs")).expect("read core_types/arena.rs");
    let page_types =
        fs::read_to_string(root.join("src/types/page.rs")).expect("read core_types/page.rs");
    let scalar_types =
        fs::read_to_string(root.join("src/types/scalar.rs")).expect("read core_types/scalar.rs");
    let atomic_types =
        fs::read_to_string(root.join("src/types/atomic.rs")).expect("read core_types/atomic.rs");
    let ptr_rs = fs::read_to_string(root.join("src/types/ptr.rs")).expect("read core_types/ptr.rs");
    let validation_types = fs::read_to_string(root.join("src/types/validation.rs"))
        .expect("read core_types/validation.rs");
    let page_states_rs = fs::read_to_string(root.join("src/types/page_states.rs"))
        .expect("read core_types/page_states.rs");
    let page_queues_rs = fs::read_to_string(root.join("src/types/page_queues.rs"))
        .expect("read core_types/page_queues.rs");
    let os_rs =
        fs::read_to_string(root.join("src/platform/os.rs")).expect("read src/platform/os.rs");
    let options_rs = fs::read_to_string(root.join("src/allocator/options.rs"))
        .expect("read src/allocator/options.rs");
    let page_rs =
        fs::read_to_string(root.join("src/allocator/page.rs")).expect("read src/allocator/page.rs");
    let random_rs = fs::read_to_string(root.join("src/internal/random.rs"))
        .expect("read src/internal/random.rs");
    let stats_rs = fs::read_to_string(root.join("src/allocator/stats.rs"))
        .expect("read src/allocator/stats.rs");
    let theap_rs = fs::read_to_string(root.join("src/allocator/thread_heap.rs"))
        .expect("read src/allocator/thread_heap.rs");
    let init_rs =
        fs::read_to_string(root.join("src/allocator/init.rs")).expect("read src/allocator/init.rs");
    let threadlocal_rs = fs::read_to_string(root.join("src/allocator/threadlocal.rs"))
        .expect("read src/allocator/threadlocal.rs");
    let alloc_aligned_rs = fs::read_to_string(root.join("src/allocator/aligned.rs"))
        .expect("read src/allocator/aligned.rs");
    let heap_rs =
        fs::read_to_string(root.join("src/allocator/heap.rs")).expect("read src/allocator/heap.rs");
    let prim_rs =
        fs::read_to_string(root.join("src/platform/mod.rs")).expect("read src/platform/mod.rs");
    let prim_unix_rs =
        fs::read_to_string(root.join("src/platform/unix.rs")).expect("read src/platform/unix.rs");

    assert!(
        !free_rs.contains("clippy::if_same_then_else")
            && !bitmap_rs.contains("clippy::if_same_then_else"),
        "free.rs and bitmap.rs should not retain obsolete if_same_then_else allowances"
    );

    for internal_name in [
        "pub static mut _mi_page_map",
        "pub unsafe extern \"C\" fn _mi_page_map_init",
        "pub unsafe extern \"C\" fn _mi_page_map_register",
        "pub unsafe extern \"C\" fn _mi_page_map_unregister",
        "pub unsafe extern \"C\" fn _mi_page_map_unregister_range",
        "pub unsafe extern \"C\" fn _mi_page_map_unsafe_destroy",
        "pub unsafe extern \"C\" fn _mi_safe_ptr_page",
    ] {
        let no_mangle_export = format!("#[unsafe(no_mangle)]\n{internal_name}");
        assert!(
            !page_map.contains(&no_mangle_export),
            "{internal_name} should stay crate-internal instead of a dynamic export"
        );
    }
    assert!(
        page_map
            .matches("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_map_init(")
            .count()
            == 2
            && page_map
                .matches(
                    "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_map_register(",
                )
                .count()
                == 2
            && page_map
                .matches(
                    "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_map_unregister(",
                )
                .count()
                == 2
            && page_map
                .matches(
                    "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_map_unregister_range(",
                )
                .count()
                == 2
            && page_map
                .matches(
                    "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_map_unsafe_destroy(",
                )
                .count()
                == 2
            && page_map.contains("static MI_PAGE_MAP_UNREGISTER_RANGE_KEEP")
            && arena_rs.contains("_mi_page_map_register(page)")
            && arena_rs.contains("_mi_page_map_unregister(page)")
            && init_rs.contains("_mi_page_map_unsafe_destroy(subproc)"),
        "page-map init, register, unregister, range-unregister, and unsafe-destroy helpers should keep C's local call boundaries"
    );
    assert!(
        page_map
            .matches("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_safe_ptr_page(")
            .count()
            == 2
            && page_map
                .matches("pub(crate) unsafe fn _mi_safe_ptr_page_registered(")
                .count()
                == 2
            && page_map.contains("unsafe { _mi_safe_ptr_page_registered(p) }\n        .map(|registered| registered.page().as_ptr())")
            && alloc_rs.contains("crate::free::mi_validate_pointer(p.cast_mut(), msg)")
            && free_rs.contains("let registered = _mi_safe_ptr_page_registered(p);")
            && free_rs.contains("Some(registered.page())")
            && heap_rs.contains("let page: *mut mi_page_t = unsafe { _mi_safe_ptr_page(p) };")
            && heap_rs.contains("let page = _mi_safe_ptr_page(p);")
            && arena_rs.contains("let page = unsafe { _mi_safe_ptr_page(start) };"),
        "safe pointer page lookup should keep C's _mi_safe_ptr_page local call boundary while typed validation requires a registered-page witness"
    );
    assert!(
        theap_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_heap_area_init(")
            && theap_rs.contains("let bsize = full_bsize.get();")
            && theap_rs.contains("let reserved = crate::page::mi_page_reserved_count(page);")
            && theap_rs.contains("(*area).reserved = reserved.as_usize() * bsize;")
            && theap_rs.contains("_mi_heap_area_init(ptr::addr_of_mut!(xarea.area), page);")
            && arena_rs.contains("use crate::theap::{")
            && arena_rs.contains("_mi_heap_area_init")
            && arena_rs.contains("_mi_heap_area_init(ptr::addr_of_mut!(area), page)")
            && !arena_rs.contains("unsafe fn mi_heap_area_init(")
            && !internal.contains("pub fn _mi_heap_area_init("),
        "heap area visitation should keep C's shared _mi_heap_area_init local helper boundary"
    );
    assert!(
        init_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_is_heap_main(")
            && init_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_is_theap_main(")
            && init_rs.contains("static MI_IS_THEAP_MAIN_KEEP")
            && heap_rs.contains("if _mi_is_heap_main(heap) {")
            && heap_rs.contains("if let Some(heap) = MainHeap::new(heap)")
            && heap_rs.contains("} else if let Some(heap) = OwnedHeap::new(heap)")
            && page_rs.contains("(*page).heap = if _mi_is_heap_main(heap) {")
            && arena_rs.contains("if unsafe { _mi_is_heap_main(heap_from) }")
            && theap_rs.contains("_mi_is_heap_main(_mi_theap_heap(theap).cast_const())"),
        "main heap/theap predicates should keep C's local helper boundaries and major callers"
    );
    assert!(
        arena_meta_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_meta_is_meta_page(")
            && arena_meta_rs.contains("static MI_META_IS_META_PAGE_KEEP")
            && arena_meta_rs.contains("mi_meta_pages.load(Ordering::Acquire)")
            && arena_meta_rs.contains("mpage = mi_meta_page_next(mpage);")
            && arena_rs.contains("} else if unsafe { _mi_meta_is_meta_page(start) }"),
        "meta page debug classification should keep C's _mi_meta_is_meta_page helper boundary"
    );
    assert!(
        page_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_deferred_free(")
            && page_rs.contains("let heartbeat = mi_theap_increment_heartbeat(theap);")
            && page_rs.contains("crate::core_types::AtomicPtrField(ptr::addr_of_mut!(deferred_free))\n            .load(crate::core_types::MemoryOrder::Acquire);")
            && page_rs.contains(
                "let callback = crate::core_types::DeferredFreeCallback::from_storage_ptr(fun_ptr);"
            )
            && page_rs.contains("crate::core_types::AtomicPtrField(ptr::addr_of_mut!(deferred_arg))\n                .load(crate::core_types::MemoryOrder::Acquire);")
            && page_rs
                .contains("let arg = crate::core_types::DeferredFreeCallbackArg::from_raw(arg);")
            && page_rs.contains(".store(arg.as_ptr(), crate::core_types::MemoryOrder::Release);")
            && page_rs.contains("let f = callback.storage_ptr();")
            && page_rs.contains("crate::core_types::AtomicPtrField(ptr::addr_of_mut!(deferred_free))\n            .store(f, crate::core_types::MemoryOrder::Release);")
            && !page_rs.contains("mi_atomic_load_ptr_acquire(ptr::addr_of!(deferred_free))")
            && !page_rs.contains("mi_atomic_load_ptr_acquire(ptr::addr_of!(deferred_arg))")
            && !page_rs.contains("mi_atomic_store_ptr_release(ptr::addr_of_mut!(deferred_arg)")
            && !page_rs.contains("mi_atomic_store_ptr_release(ptr::addr_of_mut!(deferred_free)")
            && !page_rs.contains("fun(force, heartbeat.get(), arg);")
            && page_rs.contains("callback.invoke(force, heartbeat, arg);")
            && page_rs.contains("_mi_deferred_free(theap, false);")
            && theap_rs.contains("_mi_deferred_free(theap, force);"),
        "deferred free callback dispatch should keep C's _mi_deferred_free helper boundary"
    );

    for (source_name, source, internal_name) in [
        (
            "src/allocator/alloc.rs",
            &alloc_rs,
            "pub unsafe extern \"C\" fn _mi_page_malloc_zero",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_alloc",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_alloc_aligned",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_collect",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_contain",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_free",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_abandon",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_alloc",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_free",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_try_reabandon_to_mapped",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_unabandon",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_unsafe_destroy_all",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_heap_visit_blocks",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_heap_move_pages",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_heap_destroy_pages",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arena_id_none",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arena_from_id",
        ),
        (
            "src/allocator/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arena_memid_is_suitable",
        ),
        (
            "src/allocator/arena_meta.rs",
            &arena_meta_rs,
            "pub unsafe extern \"C\" fn _mi_meta_free",
        ),
        (
            "src/allocator/arena_meta.rs",
            &arena_meta_rs,
            "pub unsafe extern \"C\" fn _mi_meta_is_meta_page",
        ),
        (
            "src/allocator/arena_meta.rs",
            &arena_meta_rs,
            "pub unsafe extern \"C\" fn _mi_meta_zalloc",
        ),
        (
            "src/allocator/free.rs",
            &free_rs,
            "pub unsafe extern \"C\" fn _mi_page_ptr_unalign",
        ),
        (
            "src/allocator/free.rs",
            &free_rs,
            "pub unsafe extern \"C\" fn _mi_padding_shrink",
        ),
        (
            "src/allocator/page.rs",
            &page_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_page_abandon",
        ),
        (
            "src/allocator/page.rs",
            &page_rs,
            "pub unsafe extern \"C\" fn _mi_page_free",
        ),
        (
            "src/allocator/page.rs",
            &page_rs,
            "pub unsafe extern \"C\" fn _mi_page_init",
        ),
        (
            "src/allocator/page.rs",
            &page_rs,
            "pub unsafe extern \"C\" fn _mi_deferred_free",
        ),
        ("src/ffi/libc.rs", &libc_rs, "pub(crate) fn _mi_snprintf"),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_vsnprintf",
        ),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub extern \"C\" fn _mi_toupper",
        ),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strnicmp",
        ),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_streq",
        ),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strlcpy",
        ),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strlcat",
        ),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strlen",
        ),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strnlen",
        ),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strnstr",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_free",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_mem_init",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_alloc",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_commit",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_reuse",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_decommit",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_reset",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_protect",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_alloc_huge_os_pages",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_numa_node",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_numa_node_count",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_clock_now",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_process_info",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_out_stderr",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_random_buf",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_getenv",
        ),
        (
            "src/ffi/libc.rs",
            &libc_rs,
            "pub(crate) extern \"C\" fn _mi_popcount_generic",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_init_auto_done",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_done_auto_done",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_associate_default_theap",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_is_in_threadpool",
        ),
        (
            "src/platform/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_yield",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_alloc",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_zalloc",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_free",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_free_ex",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_page_size",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_good_alloc_size",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_has_overcommit",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_has_virtual_reserve",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_virtual_address_bits",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_reuse",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_commit",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_commit_ex",
        ),
        (
            "src/platform/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_purge_ex",
        ),
        (
            "src/internal/random.rs",
            &random_rs,
            "pub(crate) extern \"C\" fn _mi_os_random_weak",
        ),
        (
            "src/internal/random.rs",
            &random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_split",
        ),
        (
            "src/internal/random.rs",
            &random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_next",
        ),
        (
            "src/internal/random.rs",
            &random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_init",
        ),
        (
            "src/internal/random.rs",
            &random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_init_weak",
        ),
        (
            "src/internal/random.rs",
            &random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_reinit_if_weak",
        ),
        (
            "src/allocator/alloc.rs",
            &alloc_rs,
            "pub unsafe extern \"C\" fn _mi_theap_malloc_zero_ex",
        ),
        (
            "src/allocator/alloc.rs",
            &alloc_rs,
            "pub unsafe extern \"C\" fn _mi_theap_malloc_zero",
        ),
        (
            "src/allocator/alloc.rs",
            &alloc_rs,
            "pub unsafe extern \"C\" fn _mi_theap_realloc_zero",
        ),
        (
            "src/allocator/alloc.rs",
            &alloc_rs,
            "pub unsafe extern \"C\" fn _mi_theap_malloc_guarded",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub extern \"C\" fn _mi_thread_id",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_thread_done",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_guarded_init",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_options_init",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_default_set",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_cached_set",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_default_safe",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_main_safe",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_empty_get",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_is_heap_main",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_is_theap_main",
        ),
        ("src/allocator/init.rs", &init_rs, "fn _mi_subproc_to_id"),
        ("src/allocator/init.rs", &init_rs, "fn _mi_preloading"),
        ("src/allocator/init.rs", &init_rs, "fn _mi_subproc_main"),
        ("src/allocator/init.rs", &init_rs, "fn _mi_subproc"),
        ("src/allocator/init.rs", &init_rs, "fn _mi_subproc_from_id"),
        (
            "src/allocator/init.rs",
            &init_rs,
            "fn _mi_subproc_heap_main",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "fn _mi_auto_process_init",
        ),
        (
            "src/allocator/init.rs",
            &init_rs,
            "fn _mi_auto_process_done",
        ),
        (
            "src/allocator/threadlocal.rs",
            &threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_locals_init",
        ),
        (
            "src/allocator/threadlocal.rs",
            &threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_locals_done",
        ),
        (
            "src/allocator/threadlocal.rs",
            &threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_local_set",
        ),
        (
            "src/allocator/threadlocal.rs",
            &threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_local_get",
        ),
        (
            "src/allocator/threadlocal.rs",
            &threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_locals_thread_done",
        ),
        (
            "src/allocator/threadlocal.rs",
            &threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_local_free",
        ),
        (
            "src/allocator/heap.rs",
            &heap_rs,
            "pub unsafe extern \"C\" fn _mi_heap_force_destroy",
        ),
        (
            "src/allocator/heap.rs",
            &heap_rs,
            "pub unsafe extern \"C\" fn _mi_heap_theap_get_peek",
        ),
        (
            "src/allocator/heap.rs",
            &heap_rs,
            "pub unsafe extern \"C\" fn _mi_heap_theap_get_or_init",
        ),
        (
            "src/allocator/page.rs",
            &page_rs,
            "pub unsafe extern \"C\" fn _mi_theap_collect_retired",
        ),
        (
            "src/allocator/alloc.rs",
            &alloc_rs,
            "pub(crate) unsafe extern \"C\" fn mi_theap_alloc_new",
        ),
        (
            "src/allocator/thread_heap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_collect_abandon",
        ),
        (
            "src/allocator/thread_heap.rs",
            &theap_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_theap_init",
        ),
        (
            "src/allocator/thread_heap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_create",
        ),
        (
            "src/allocator/thread_heap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_incref",
        ),
        (
            "src/allocator/thread_heap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_decref",
        ),
        (
            "src/allocator/thread_heap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_free",
        ),
        (
            "src/allocator/thread_heap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_delete",
        ),
        (
            "src/allocator/thread_heap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_random_next",
        ),
        ("src/platform/mod.rs", &prim_rs, "fn _mi_allocator_init"),
        ("src/platform/mod.rs", &prim_rs, "fn _mi_allocator_done"),
        (
            "src/allocator/options.rs",
            &options_rs,
            "fn _mi_warning_message",
        ),
        (
            "src/allocator/options.rs",
            &options_rs,
            "fn _mi_error_message",
        ),
        (
            "src/allocator/options.rs",
            &options_rs,
            "fn _mi_option_get_fast",
        ),
        (
            "src/allocator/options.rs",
            &options_rs,
            "fn _mi_assert_fail",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub unsafe extern \"C\" fn _mi_stats_merge_into",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_increase_mt",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_decrease_mt",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub extern \"C\" fn _mi_stats_init",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_counter_increase_mt",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_increase_mt",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_decrease_mt",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_increase",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_decrease",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_counter_increase",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_increase",
        ),
        (
            "src/allocator/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_decrease",
        ),
        (
            "src/allocator/options.rs",
            &options_rs,
            "pub(crate) fn _mi_raw_message",
        ),
        (
            "src/allocator/options.rs",
            &options_rs,
            "pub(crate) fn _mi_message",
        ),
    ] {
        let no_mangle_export = format!("#[unsafe(no_mangle)]\n{internal_name}");
        assert!(
            !source.contains(&no_mangle_export),
            "{internal_name} in {source_name} should stay crate-internal instead of a dynamic export"
        );
        assert_not_no_mangle_item(source_name, source, internal_name);
    }

    assert!(
        !internal.contains("pub static mut _mi_page_map:")
            && !internal.contains("pub fn _mi_page_map_init() -> bool;")
            && !internal.contains("pub fn _mi_arenas_alloc(")
            && !internal.contains("pub fn _mi_meta_zalloc(")
            && !internal.contains("pub fn _mi_page_free(")
            && !internal.contains("pub fn _mi_page_malloc_zero(")
            && !internal.contains("pub fn _mi_vsnprintf(")
            && !internal.contains("pub fn _mi_strlcpy(")
            && !internal.contains("pub fn _mi_os_random_weak(")
            && !internal.contains("pub fn _mi_random_init(")
            && !internal.contains("pub fn _mi_random_next(")
            && !internal.contains("pub fn _mi_theap_random_next(")
            && !internal.contains("pub fn _mi_os_alloc(")
            && !internal.contains("pub fn _mi_os_page_size(")
            && !internal.contains("pub fn _mi_os_commit(")
            && !internal.contains("pub fn _mi_os_purge_ex(")
            && !internal.contains("pub fn _mi_thread_done(")
            && !internal.contains("pub fn _mi_thread_id(")
            && !internal.contains("pub fn _mi_theap_guarded_init(")
            && !internal.contains("pub fn _mi_theap_options_init(")
            && !internal.contains("pub fn _mi_theap_default_set(")
            && !internal.contains("pub fn _mi_theap_cached_set(")
            && !internal.contains("pub fn _mi_theap_default_safe(")
            && !internal.contains("pub fn _mi_theap_main_safe(")
            && !internal.contains("pub fn _mi_theap_collect_retired(")
            && !internal.contains("pub fn _mi_theap_collect_abandon(")
            && !internal.contains("pub fn _mi_theap_init(")
            && !internal.contains("pub fn _mi_theap_free(")
            && !internal.contains("pub fn _mi_theap_incref(")
            && !internal.contains("pub fn _mi_theap_decref(")
            && !internal.contains("pub fn _mi_theap_area_visit_blocks(")
            && !internal.contains("pub fn _mi_theap_malloc_zero(")
            && !internal.contains("pub fn _mi_theap_malloc_zero_ex(")
            && !internal.contains("pub fn _mi_theap_realloc_zero(")
            && !internal.contains("pub fn _mi_theap_create(")
            && !internal.contains("pub fn _mi_heap_move_pages(")
            && !internal.contains("pub fn _mi_heap_destroy_pages(")
            && !internal.contains("pub fn _mi_heap_force_destroy(")
            && !internal.contains("pub fn _mi_padding_shrink(")
            && !internal.contains("pub fn _mi_is_heap_main(")
            && !internal.contains("pub fn _mi_is_theap_main(")
            && !internal.contains("pub fn _mi_warning_message(")
            && !internal.contains("pub fn _mi_verbose_message(")
            && !internal.contains("pub fn _mi_trace_message(")
            && !internal.contains("pub fn _mi_option_get_fast(")
            && !internal.contains("pub fn _mi_error_message(")
            && !internal.contains("pub fn _mi_auto_process_init(")
            && !internal.contains("pub fn _mi_auto_process_done(")
            && !internal.contains("pub fn _mi_allocator_init(")
            && !internal.contains("pub fn _mi_allocator_done(")
            && !internal.contains("pub fn _mi_preloading(")
            && !internal.contains("pub fn _mi_subproc(")
            && !internal.contains("pub fn _mi_subproc_main(")
            && !internal.contains("pub fn _mi_subproc_heap_main(")
            && !internal.contains("pub fn _mi_subproc_from_id(")
            && !internal.contains("pub fn _mi_deferred_free(")
            && !internal.contains("pub fn _mi_arena_id_none(")
            && !internal.contains("pub fn _mi_arena_from_id(")
            && !internal.contains("pub fn _mi_arena_memid_is_suitable(")
            && !internal.contains("pub static _mi_page_empty:")
            && !internal.contains("pub static _mi_theap_empty:")
            && !internal.contains("pub static _mi_theap_empty_wrong:")
            && internal.contains("pub(crate) use crate::page_map::{")
            && internal.contains("pub(crate) use crate::os::{"),
        "page-map internals should be imported through crate paths, not extern declarations"
    );
    assert!(
        !alloc_aligned_rs.contains("fn _mi_theap_malloc_guarded(")
            && !alloc_aligned_rs.contains("fn _mi_theap_malloc_zero(")
            && !alloc_aligned_rs.contains("fn _mi_theap_malloc_zero_ex(")
            && !alloc_aligned_rs.contains("fn _mi_theap_realloc_zero(")
            && !alloc_aligned_rs.contains("fn _mi_padding_shrink("),
        "aligned allocation internals should call crate-local theap allocation helpers directly"
    );
    assert!(
        free_rs.contains("#[cfg(mi_c_malloc_fast)]")
            && free_rs.contains("\".hidden _mi_padding_shrink\"")
            && free_rs.contains("\"_mi_padding_shrink:\"")
            && free_rs.contains("\"ret\"")
            && free_rs.contains("#[cfg(not(mi_c_malloc_fast))]\n#[inline(never)]")
            && free_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_padding_shrink(")
            && free_rs.contains("if MI_PADDING == 0")
            && free_rs.contains("mi_page_decode_padding(page, block, &mut delta, &mut bsize)")
            && free_rs.contains(
                ".write_delta(\n            crate::core_types::PaddingDelta::for_usable("
            )
            && free_rs.contains("crate::core_types::PaddingDelta::new_unchecked(new_delta as u32)")
            && alloc_aligned_rs.contains("#[cfg(mi_c_malloc_fast)]")
            && alloc_aligned_rs.contains("\"call _mi_padding_shrink\"")
            && alloc_aligned_rs.contains("fn mi_padding_shrink_call(")
            && alloc_aligned_rs.contains("unsafe { _mi_padding_shrink(page, block, min_size) };")
            && alloc_aligned_rs.contains(
                "unsafe { mi_padding_shrink_call(page, p.cast::<mi_block_t>(), adjust + size) };"
            ),
        "aligned allocation should keep C's _mi_padding_shrink helper boundary"
    );
    assert!(
        page_rs.contains("#[inline(never)]\npub extern \"C\" fn mi_bin(size: usize) -> usize")
            && page_rs
                .contains("#[inline(never)]\npub extern \"C\" fn _mi_bin(size: usize) -> usize")
            && page_rs.contains(
                "#[inline(never)]\npub extern \"C\" fn _mi_bin_size(bin: usize) -> usize"
            )
            && page_rs.contains("pub(crate) fn mi_bin_impl(size: usize) -> usize")
            && page_rs.contains("fn mi_bin_from_raw_size(size: usize) -> crate::core_types::Bin")
            && page_rs.contains(
                "pub(crate) fn mi_bin_from_padded(padded: crate::core_types::PaddedSize)"
            )
            && page_rs.contains(
                "pub(crate) fn mi_bin_from_block_size(\n    block_size: crate::core_types::BlockSize,"
            )
            && page_rs.contains(
                "pub(crate) fn mi_bin_from_usable(usable: crate::core_types::UsableSize)"
            )
            && page_rs.contains("mi_bin_impl(size)")
            && page_rs.contains("_mi_theap_empty")
            && page_rs.contains(".pages[bin].block_size")
            && page_rs.contains("_mi_bin_size(mi_bin_from_padded(padded).as_usize())")
            && stats_rs.contains("_mi_bin_size(bin)"),
        "bin lookup helpers should keep C's mi_bin/_mi_bin/_mi_bin_size boundaries and empty-theap table dependency"
    );
    assert!(
        alloc_rs.contains("let usable_size = mi_page_usable_block_size(page);")
            && alloc_rs.contains("let bin = mi_bin_from_usable(usable_size).as_usize();")
            && free_rs.contains("mi_bin_from_usable(bsize).as_usize()")
            && arena_rs.contains("let bin = mi_bin_from_block_size(block_size);")
            && internal.contains("pub(crate) unsafe fn mi_page_block_size_typed(")
            && internal.contains(") -> crate::core_types::BlockSize")
            && internal.contains("mi_page_block_size_typed(page).get()")
            && internal.contains("mi_page_block_size_typed(page).usable()")
            && arena_rs.contains("mi_bin_from_block_size(unsafe { mi_page_block_size_typed(page) })")
            && internal.contains(
                "block_size: crate::core_types::BlockSize,\n) -> *mut mi_page_queue_t {\n    unsafe {\n        let size = block_size.get();"
            )
            && internal.contains("crate::page::mi_bin_from_block_size(block_size).as_usize()")
            && page_rs.contains("pub(crate) unsafe fn mi_page_bin_typed(")
            && page_rs.contains(") -> crate::core_types::Bin")
            && page_rs.contains("pub(crate) unsafe fn _mi_page_stats_bin_typed(")
            && page_rs.contains(") -> crate::core_types::StatsBin")
            && page_rs.contains("let bin = mi_page_bin_typed(page).as_usize();")
            && page_rs.contains("let bin = _mi_page_stats_bin_typed(page).as_usize();")
            && page_rs.contains("let bin = mi_bin_from_block_size(block_size).as_usize();")
            && page_rs.contains("== mi_bin_from_block_size(crate::core_types::BlockSize::new_unchecked(\n                    mi_page_queue_block_size_raw(prev),")
            && !page_rs.contains("pub(crate) fn _mi_bin_typed(size: usize)")
            && !alloc_rs.contains("_mi_bin_typed(")
            && !free_rs.contains("_mi_bin_typed(")
            && !arena_rs.contains("_mi_bin_typed(")
            && !internal.contains("_mi_bin_typed("),
        "internal bin users should consume typed Bin wrappers before raw array indexing"
    );
    assert!(
        free_rs.contains("pub(crate) unsafe fn mi_page_usable_size_of_typed(")
            && free_rs.contains(") -> crate::core_types::UsableSize")
            && free_rs.contains("pub(crate) unsafe fn mi_route_usable_size_typed(")
            && free_rs.contains("pub(crate) unsafe fn _mi_usable_size_typed(")
            && free_rs.contains("_mi_usable_size_typed(p, page).get()")
            && alloc_rs.contains("use crate::free::_mi_usable_size_typed;")
            && alloc_rs.contains("let size = _mi_usable_size_typed(p, page);")
            && !alloc_rs.contains("use crate::free::_mi_usable_size;"),
        "internal usable-size routing should carry UsableSize until raw ABI/comparison boundaries"
    );
    assert!(
        page_rs.contains("pub(crate) fn mi_good_size_typed(")
            && page_rs.contains(") -> crate::core_types::GoodSize")
            && page_rs.contains(
                "mi_good_size_typed(crate::core_types::RequestedSize::new(size)).get()"
            )
            && alloc_rs.contains("mi_good_size_typed(crate::core_types::RequestedSize::new(size)).get()")
            && alloc_aligned_rs
                .contains("mi_good_size_typed(crate::core_types::RequestedSize::new(size)).get()")
            && stats_rs.contains("mi_good_size_typed(RequestedSize::new(12 * 1024)).get()")
            && !stats_rs.contains("use crate::mimalloc::{mi_good_size")
            && !alloc_rs.contains("use crate::page::{_mi_bin_typed, _mi_malloc_generic_typed, MI_MAX_ALLOC_SIZE, mi_good_size};")
            && !alloc_aligned_rs.contains("mi_good_size(size)"),
        "internal good-size routing should carry GoodSize until raw ABI/comparison boundaries"
    );
    assert!(
        internal.contains(
            "pub(crate) fn mi_direct_bin_from_size(size: usize) -> crate::core_types::DirectBin"
        ) && internal.contains(
            "pub(crate) fn mi_direct_bin_from_padded(\n    padded: crate::core_types::PaddedSize,"
        ) && internal.contains("mi_direct_bin_from_size(padded.get())")
            && internal.contains("let idx = crate::core_types::WordCount::from_size(size).get();")
            && internal.contains("let idx = mi_direct_bin_from_padded(padded).as_usize();")
            && page_rs.contains("let direct_idx = mi_direct_bin_from_size(size);")
            && page_rs.contains("let idx = direct_idx.as_usize();")
            && page_rs.contains(".direct_page(direct_idx)")
            && page_rs.contains("theap_ptr.fill_direct_pages(")
            && ptr_rs.contains(
                "pub(crate) unsafe fn fill_direct_pages(\n        self,\n        start: DirectBin,\n        end: DirectBin,"
            )
            && !internal.contains(
                "let idx = _mi_wsize_from_size(size);\n    (*theap).pages_free_direct[idx]"
            )
            && !page_rs.contains("pages_free_direct"),
        "direct page-cache indexes should pass through DirectBin before pages_free_direct indexing"
    );
    assert!(
        options_rs.contains(
            "#[inline(never)]\npub(crate) extern \"C\" fn _mi_option_get_fast(option: mi_option_t) -> c_long"
        ) && options_rs.contains("mi_option_get_fast_impl(option)")
            && options_rs.contains("option_value_load(idx)")
            && free_rs.contains("_mi_option_get_fast(mi_option_t::mi_option_page_reclaim_on_free)")
            && free_rs.contains("_mi_option_get_fast(mi_option_t::mi_option_page_cross_thread_max_reclaim)")
            && page_rs.contains("_mi_option_get_fast(mi_option_t::mi_option_page_max_candidates)"),
        "fast option reads should keep C's local _mi_option_get_fast helper boundary on free/page policy paths"
    );
    assert!(
        heap_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_heap_theap_get_peek(heap: *const mi_heap_t) -> *mut mi_theap_t"
        ) && heap_rs.contains("if heap.is_null() || _mi_is_heap_main(heap)")
            && heap_rs.contains("crate::init::_mi_theap_main_safe()")
            && heap_rs.contains("_mi_thread_local_get((*heap).theap).cast::<mi_theap_t>()")
            && heap_rs.contains("let mut theap = unsafe { _mi_heap_theap_get_peek(heap) };")
            && prim_rs.contains("_mi_heap_theap_get_peek(heap)"),
        "heap/theap cache misses should keep C's local _mi_heap_theap_get_peek no-create helper boundary"
    );
    assert!(
        init_rs.contains("#[inline(never)]\npub extern \"C\" fn _mi_is_main_thread() -> bool")
            && init_rs.contains("#[inline(always)]\nfn mi_is_main_thread_fast() -> bool")
            && init_rs.contains("let main_tid = unsafe { tld_main.thread_id };")
            && init_rs.contains("main_tid == 0 || main_tid == current_thread_id()")
            && init_rs.contains("pub extern \"C\" fn _mi_is_main_thread() -> bool {\n    mi_is_main_thread_fast()\n}")
            && !init_rs.contains("static MAIN_THREAD_ID")
            && !init_rs.contains("mi_mark_main_thread")
            && init_rs.contains("#[inline(never)]\npub(crate) extern \"C\" fn _mi_preloading() -> bool")
            && init_rs.contains("OS_PRELOADING.load(Ordering::Relaxed)")
            && prim_rs.contains("#[cfg(mi_c_malloc_fast)]\ncore::arch::global_asm!(")
            && prim_rs.contains("\".hidden _mi_is_redirected\"")
            && prim_rs.contains("\"_mi_is_redirected:\"")
            && prim_rs.contains("\"xor eax,eax\"")
            && prim_rs.contains("pub(crate) fn _mi_is_redirected() -> bool;")
            && prim_rs.contains("#[cfg(not(mi_c_malloc_fast))]\n#[inline(never)]\npub unsafe extern \"C\" fn _mi_is_redirected() -> bool")
            && prim_rs.contains("false")
            && init_rs.contains("pub extern \"C\" fn mi_is_redirected() -> bool {\n    unsafe { crate::prim::_mi_is_redirected() }\n}"),
        "lifecycle predicates should keep C's _mi_is_main_thread/_mi_preloading/_mi_is_redirected helper boundaries"
    );
    assert!(
        prim_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_allocator_init(message: *mut *const c_char) -> bool"
        ) && prim_rs.contains("unsafe { *message = ptr::null() };")
            && prim_rs.contains("\".hidden _mi_allocator_done\"")
            && prim_rs.contains("\"_mi_allocator_done:\"")
            && prim_rs.contains("pub(crate) fn _mi_allocator_done();")
            && prim_rs.contains("#[cfg(not(mi_c_malloc_fast))]\n#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_allocator_done()")
            && options_rs.contains("#[inline(never)]\npub(crate) extern \"C\" fn _mi_options_init()")
            && options_rs.contains("for i in 0..MI_OPTION_COUNT")
            && options_rs.contains("mi_max_error_count.store(max_errors as isize, Ordering::Release);")
            && options_rs.contains("#[inline(never)]\npub(crate) extern \"C\" fn _mi_options_post_init()")
            && options_rs.contains("mi_add_stderr_output();")
            && options_rs.contains("mi_options_print();")
            && init_rs.contains("_mi_options_post_init();")
            && init_rs.contains("if unsafe { crate::prim::_mi_is_redirected() }")
            && init_rs.contains("_mi_verbose_message(format_args!(\"malloc is redirected.\\n\"));")
            && init_rs.contains("let mut msg: *const c_char = ptr::null();")
            && init_rs.contains("unsafe { _mi_allocator_init(ptr::addr_of_mut!(msg)) };")
            && init_rs.contains("mi_option_is_enabled(mi_option_t::mi_option_show_errors)")
            && init_rs.contains("unsafe { _mi_fputs(None, ptr::null_mut(), ptr::null(), msg) };")
            && init_rs.contains("_mi_allocator_done();"),
        "process init/done should keep C's allocator/options helper boundaries and redirector hook sequence"
    );
    assert!(
        options_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_fputs(")
            && options_rs.contains("#[link_name = \"stdout\"]")
            && options_rs.contains("#[link_name = \"stderr\"]")
            && options_rs.contains("fn mi_output_is_c_file(out: mi_output_fun) -> bool")
            && options_rs.contains("p == unsafe { C_STDOUT } || p == unsafe { C_STDERR }")
            && options_rs.contains("let use_default = match out {")
            && options_rs.contains("None => true,")
            && options_rs.contains("Some(out) => mi_output_is_c_file(out),")
            && options_rs.contains(
                "fn mi_out_get_default(parg: Option<&mut OutputCallbackArg>) -> OutputCallback"
            )
            && options_rs.contains("let default_out = mi_out_get_default(Some(&mut default_arg));")
            && options_rs.contains("unsafe { default_out.invoke(message, default_arg) };")
            && options_rs.contains("let out = OutputCallback::new(Some(out));")
            && options_rs.contains("let arg = OutputCallbackArg::from_raw(arg);")
            && options_rs.contains("unsafe { out.invoke(message, arg) };")
            && stats_rs.contains("_mi_fputs(buf.out, buf.arg, ptr::null(), buf.buf);")
            && init_rs.contains("unsafe { _mi_fputs(None, ptr::null_mut(), ptr::null(), msg) };"),
        "_mi_fputs should keep C's local helper boundary and stdout/stderr default-output compatibility"
    );
    assert!(
        libc_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_getenv(")
            && libc_rs.contains("if name.is_null() || result.is_null() || result_size < 64")
            && libc_rs.contains("crate::prim::_mi_prim_getenv(name, result, result_size)")
            && prim_unix_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_getenv(")
            && prim_unix_rs.contains("for i in 0..10000usize")
            && prim_unix_rs.contains("unsafe { _mi_strnicmp(name, s, len) == 0")
            && prim_unix_rs.contains("if _mi_preloading() {\n        return false;")
            && prim_unix_rs.contains("s = unsafe { getenv(buf.as_ptr()) };")
            && options_rs.contains("crate::libc::_mi_getenv("),
        "_mi_getenv and _mi_prim_getenv should keep C's environment helper boundaries and option parser dependency"
    );
    assert!(
        libc_rs.contains("#[inline(never)]\npub(crate) extern \"C\" fn _mi_popcount_generic(")
            && libc_rs.contains("if x <= 1 {\n        return x;")
            && libc_rs.contains("if !x == 0 {\n        return MI_SIZE_BITS;")
            && libc_rs.contains("mi_popcount_generic64(x as u64)")
            && libc_rs.contains(
                "static MI_POPCOUNT_GENERIC_KEEP: extern \"C\" fn(usize) -> usize = _mi_popcount_generic;"
            ),
        "_mi_popcount_generic should keep C's local noinline generic popcount boundary"
    );
    for marker in [
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_mem_init(",
        "#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_prim_alloc(",
        "#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_prim_commit(",
        "#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_prim_reset(",
        "#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_prim_protect(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_reuse(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_alloc_huge_os_pages(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_numa_node_count(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_clock_now(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_process_info(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_out_stderr(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_random_buf(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_thread_init_auto_done(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_thread_done_auto_done(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_thread_associate_default_theap(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_thread_is_in_threadpool(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_prim_thread_yield(",
        "static MI_PRIM_MEM_INIT_KEEP",
        "static MI_PRIM_ALLOC_KEEP",
        "static MI_PRIM_COMMIT_KEEP",
        "static MI_PRIM_RESET_KEEP",
        "static MI_PRIM_PROTECT_KEEP",
        "static MI_PRIM_REUSE_KEEP",
        "static MI_PRIM_ALLOC_HUGE_OS_PAGES_KEEP",
        "static MI_PRIM_NUMA_NODE_COUNT_KEEP",
        "static MI_PRIM_CLOCK_NOW_KEEP",
        "static MI_PRIM_PROCESS_INFO_KEEP",
        "static MI_PRIM_OUT_STDERR_KEEP",
        "static MI_PRIM_RANDOM_BUF_KEEP",
        "static MI_PRIM_THREAD_INIT_AUTO_DONE_KEEP",
        "static MI_PRIM_THREAD_DONE_AUTO_DONE_KEEP",
        "static MI_PRIM_THREAD_ASSOCIATE_DEFAULT_THEAP_KEEP",
        "static MI_PRIM_THREAD_IS_IN_THREADPOOL_KEEP",
        "static MI_PRIM_THREAD_YIELD_KEEP",
    ] {
        assert!(
            prim_unix_rs.contains(marker),
            "Unix primitive helper boundary marker missing: {marker}"
        );
    }
    for marker in [
        "#[inline(never)]\npub(crate) extern \"C\" fn _mi_os_random_weak(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_random_split(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_random_next(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_random_init(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_random_init_weak(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_random_reinit_if_weak(",
    ] {
        assert!(
            random_rs.contains(marker),
            "random helper boundary marker missing: {marker}"
        );
    }
    assert!(
        !random_rs.contains("static MI_OS_RANDOM_WEAK_KEEP")
            && !random_rs.contains("static MI_RANDOM_SPLIT_KEEP")
            && !random_rs.contains("static MI_RANDOM_NEXT_KEEP")
            && !random_rs.contains("static MI_RANDOM_INIT_KEEP")
            && !random_rs.contains("static MI_RANDOM_INIT_WEAK_KEEP")
            && !random_rs.contains("static MI_RANDOM_REINIT_IF_WEAK_KEEP"),
        "random helper retention function pointers should stay absent; they force indirect calls in the release cdylib"
    );
    for marker in [
        "#[inline(never)]\npub(crate) extern \"C\" fn _mi_toupper(",
        "#[inline(never)]\n#[allow(clippy::many_single_char_names)]\npub(crate) unsafe extern \"C\" fn _mi_strnicmp(",
        "#[inline(never)]\n#[allow(clippy::many_single_char_names)]\npub(crate) unsafe extern \"C\" fn _mi_streq(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_strlcpy(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_strlcat(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_strnlen(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_strlen(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_strnstr(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_vsnprintf(",
        "#[inline(never)]\npub(crate) fn _mi_snprintf(",
        "static MI_TOUPPER_KEEP",
        "static MI_STRNICMP_KEEP",
        "static MI_STREQ_KEEP",
        "static MI_STRLCPY_KEEP",
        "static MI_STRLCAT_KEEP",
        "static MI_STRNLEN_KEEP",
        "static MI_STRLEN_KEEP",
        "static MI_STRNSTR_KEEP",
        "static MI_VSNPRINTF_KEEP",
        "static MI_SNPRINTF_KEEP",
    ] {
        assert!(
            libc_rs.contains(marker),
            "libc helper boundary marker missing: {marker}"
        );
    }
    assert!(
        options_rs.contains("#[inline(never)]\npub(crate) fn _mi_raw_message(")
            && options_rs.contains("static MI_RAW_MESSAGE_KEEP"),
        "_mi_raw_message should keep C's local raw-message helper boundary"
    );
    assert!(
        stats_rs.contains("#[inline(never)]\npub extern \"C\" fn _mi_stats_init(")
            && stats_rs.contains("static MI_STATS_INIT_KEEP: extern \"C\" fn() = _mi_stats_init;"),
        "_mi_stats_init should keep C's local process statistics initialization boundary"
    );
    assert!(
        init_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_auto_process_init()")
            && init_rs.contains(
                "static MI_AUTO_PROCESS_INIT_KEEP: unsafe extern \"C\" fn() = _mi_auto_process_init;"
            )
            && init_rs.contains("#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_auto_process_done()")
            && init_rs.contains(
                "static MI_AUTO_PROCESS_DONE_KEEP: unsafe extern \"C\" fn() = _mi_auto_process_done;"
            ),
        "_mi_auto_process_init and _mi_auto_process_done should keep C's local lifecycle helper boundaries"
    );
    assert!(
        stats_rs.contains(
            "#[inline(never)]\nunsafe extern \"C\" fn mi_stats_add(stats: *mut mi_stats_t, src: *const mi_stats_t)"
        ) && stats_rs.contains(
            "if stats.is_null() || src.is_null() || ptr::eq(stats.cast_const(), src)"
        ) && stats_rs.contains(
            "unsafe { mi_stats_add_nonnull(stats, src) };"
        ) && stats_rs.contains(
            "#[inline(never)]\nunsafe fn mi_stats_add_nonnull(stats: *mut mi_stats_t, src: *const mi_stats_t)"
        ) && stats_rs.contains(
            "mi_stat_count_add_mt(\n                    ptr::addr_of_mut!((*stats).$field),\n                    ptr::addr_of!((*src).$field),"
        ) && stats_rs.contains(
            "mi_stat_counter_add_mt(\n                    ptr::addr_of_mut!((*stats).$field),\n                    ptr::addr_of!((*src).$field),"
        ) && stats_rs.contains(
            "if MI_STAT > 1 {\n        let stats_malloc_bins ="
        )
            && stats_rs.contains("for i in 0..=MI_BIN_HUGE")
            && stats_rs.contains("let stats_page_bins =")
            && !stats_rs.contains("&mut stats.$field")
            && !stats_rs.contains("&src.$field")
            && !stats_rs.contains("iter_mut().zip(src.malloc_bins.iter())")
            && !stats_rs.contains("iter_mut().zip(src.page_bins.iter())")
            && !stats_rs.contains("._stat_reserved\n        .iter_mut()\n        .zip(src._stat_reserved.iter())")
            && !stats_rs.contains("._stat_counter_reserved\n        .iter_mut()\n        .zip(src._stat_counter_reserved.iter())")
            && !stats_rs.contains("stats.chunk_bins.iter_mut().zip(src.chunk_bins.iter())")
            && !stats_rs.contains("static MI_STATS_ADD_KEEP"),
        "mi_stats_add should keep C's raw-pointer aggregate helper boundary without a data-relocation keepalive, route checked callers to the non-null body, release bin-stat gates, and leave reserved extension arrays untouched"
    );
    let stats_add_nonnull_start = stats_rs
        .find("#[inline(never)]\nunsafe fn mi_stats_add_nonnull(")
        .expect("find mi_stats_add_nonnull");
    let stats_add_nonnull_end = stats_rs[stats_add_nonnull_start..]
        .find("\n}\n\nfn mi_stats_add_into")
        .map(|offset| stats_add_nonnull_start + offset)
        .expect("find end of mi_stats_add_nonnull");
    let stats_add_nonnull_body = &stats_rs[stats_add_nonnull_start..stats_add_nonnull_end];
    assert!(
        !stats_add_nonnull_body.contains("stats.is_null()")
            && !stats_add_nonnull_body.contains("src.is_null()")
            && !stats_add_nonnull_body.contains("ptr::eq(stats.cast_const(), src)"),
        "mi_stats_add_nonnull should keep the checked merge body free of duplicate entry guards"
    );
    let stats_add_into_start = stats_rs
        .find("fn mi_stats_add_into(to: &mut mi_stats_t, from: &mi_stats_t)")
        .expect("find mi_stats_add_into");
    let stats_add_into_end = stats_rs[stats_add_into_start..]
        .find("\n}\n\nfn mi_stats_copy")
        .map(|offset| stats_add_into_start + offset)
        .expect("find end of mi_stats_add_into");
    let stats_add_into_body = &stats_rs[stats_add_into_start..stats_add_into_end];
    assert!(
        stats_add_into_body
            .contains("unsafe { mi_stats_add(to as *mut mi_stats_t, from as *const mi_stats_t) };")
            && !stats_add_into_body.contains("mi_stats_add_nonnull("),
        "mi_stats_add_into should exercise C's checked raw mi_stats_add compatibility boundary"
    );
    let stats_merge_start = stats_rs
        .find("#[inline(never)]\npub unsafe extern \"C\" fn _mi_stats_merge_into(")
        .expect("find _mi_stats_merge_into");
    let stats_merge_end = stats_rs[stats_merge_start..]
        .find("\n}\n\n#[repr(C)]")
        .map(|offset| stats_merge_start + offset)
        .expect("find end of _mi_stats_merge_into");
    let stats_merge_body = &stats_rs[stats_merge_start..stats_merge_end];
    assert!(
        stats_merge_body.contains("mi_stats_add_nonnull(to, from);")
            && stats_merge_body.contains("_mi_memzero(from.cast(), size_of::<mi_stats_t>());")
            && !stats_merge_body.contains("mi_stats_add_into("),
        "_mi_stats_merge_into should match C's direct checked stats-add body plus memzero sequence"
    );
    assert!(
        options_rs
            .contains("#[inline(never)]\npub(crate) fn _mi_message(args: fmt::Arguments<'_>)")
            && options_rs.contains(
                "mi_vfprintf_thread(None, ptr::null_mut(), c\"mimalloc: \".as_ptr(), args);"
            )
            && options_rs
                .contains("static MI_MESSAGE_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_message;"),
        "_mi_message should keep C's local message helper boundary and mimalloc prefix path"
    );
    assert!(
        options_rs.contains("#[inline(never)]\nfn mi_recurse_enter_prim() -> bool")
            && options_rs.contains(
                "static MI_RECURSE_ENTER_PRIM_KEEP: fn() -> bool = mi_recurse_enter_prim;"
            )
            && options_rs.contains("#[inline(never)]\nfn mi_recurse_exit_prim()")
            && options_rs
                .contains("static MI_RECURSE_EXIT_PRIM_KEEP: fn() = mi_recurse_exit_prim;")
            && options_rs.contains("mi_recurse_enter_prim()")
            && options_rs.contains("mi_recurse_exit_prim();"),
        "option recursion guards should keep C's noinline primitive helper boundaries"
    );
    assert!(
        os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_numa_node_count() -> i32")
            && os_rs.contains(
                "static MI_OS_NUMA_NODE_COUNT_KEEP: unsafe extern \"C\" fn() -> i32 = _mi_os_numa_node_count;"
            )
            && prim_unix_rs.contains("#[inline(never)]\nunsafe fn unix_mmap_prim_aligned(")
            && prim_unix_rs.contains("static UNIX_MMAP_PRIM_ALIGNED_KEEP: unsafe fn("),
        "NUMA count and aligned mmap helpers should keep C's local OS helper boundaries"
    );
    assert!(
        libc_rs.contains("#[inline(never)]\nunsafe fn mi_out_num(")
            && libc_rs.contains("static MI_OUT_NUM_KEEP: unsafe fn(")
            && stats_rs.contains("#[inline(never)]\nfn mi_json_buf_print_value(")
            && stats_rs.contains(
                "static MI_JSON_BUF_PRINT_VALUE_KEEP: fn(&mut mi_json_buf_t, &str, i64) = mi_json_buf_print_value;"
            ),
        "number formatting and JSON scalar output should keep C's local formatting helper boundaries"
    );
    assert!(
        options_rs
            .contains("#[inline(never)]\npub(crate) fn _mi_trace_message(args: fmt::Arguments<'_>)")
            && options_rs.contains("mi_option_get(mi_option_t::mi_option_verbose) <= 1")
            && options_rs.contains(
                "static MI_TRACE_MESSAGE_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_trace_message;"
            )
            && options_rs.contains(
                "#[inline(never)]\npub(crate) fn _mi_verbose_message(args: fmt::Arguments<'_>)"
            )
            && options_rs.contains("!mi_option_is_enabled(mi_option_t::mi_option_verbose)")
            && options_rs.contains(
                "static MI_VERBOSE_MESSAGE_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_verbose_message;"
            )
            && internal.contains("crate::internal::_mi_trace_message(format_args!($($arg)*))"),
        "_mi_trace_message and _mi_verbose_message should keep C's local message helper boundaries without stale extern declarations"
    );
    assert!(
        options_rs.contains(
            "#[inline(never)]\npub(crate) fn _mi_warning_message_fmt(args: fmt::Arguments<'_>)"
        ) && options_rs.contains(
            "#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_warning_message(fmt: *const c_char)"
        ) && options_rs.contains(
            "static MI_WARNING_MESSAGE_FMT_KEEP: for<'a> fn(fmt::Arguments<'a>) = _mi_warning_message_fmt;"
        ) && options_rs.contains(
            "static MI_WARNING_MESSAGE_KEEP: unsafe extern \"C\" fn(*const c_char) = _mi_warning_message;"
        ) && options_rs.contains(
            "#[inline(never)]\npub(crate) fn _mi_error_message_fmt(err: Errno, args: fmt::Arguments<'_>)"
        ) && options_rs.contains(
            "#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_error_message(err: c_int, fmt: *const c_char)"
        ) && options_rs.contains(
            "static MI_ERROR_MESSAGE_FMT_KEEP: for<'a> fn(Errno, fmt::Arguments<'a>) = _mi_error_message_fmt;"
        ) && options_rs.contains(
            "static MI_ERROR_MESSAGE_KEEP: unsafe extern \"C\" fn(c_int, *const c_char) = _mi_error_message;"
        ) && !internal.contains("pub fn _mi_warning_message(")
            && !internal.contains("pub fn _mi_error_message("),
        "_mi_warning_message and _mi_error_message should keep C's local message helper boundaries without stale extern declarations"
    );
    assert!(
        os_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_has_overcommit()"
        ) && os_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_has_virtual_reserve()"
        ) && os_rs
            .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_large_page_size()")
            && os_rs.contains(
                "#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_minimal_purge_size()"
            )
            && os_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_guard_page_size()")
            && os_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_virtual_address_bits()"
            )
            && os_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_canuse_large_page(")
            && os_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_good_alloc_size("
            )
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_init()")
            && os_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_get_aligned_hint(")
            && os_rs.contains(
                "#[inline(never)]\n#[allow(clippy::absurd_extreme_comparisons)]\npub unsafe extern \"C\" fn _mi_os_secure_guard_page_size()"
            )
            && os_rs.contains(
                "#[inline(never)]\n#[allow(clippy::absurd_extreme_comparisons)]\npub unsafe extern \"C\" fn _mi_os_secure_guard_page_set_at("
            )
            && os_rs.contains(
                "#[inline(never)]\n#[allow(clippy::absurd_extreme_comparisons)]\npub unsafe extern \"C\" fn _mi_os_secure_guard_page_reset_at("
            )
            && os_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_commit(")
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_decommit(")
            && os_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_reuse(")
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_purge(")
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_protect(")
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_unprotect(")
            && os_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_free_ex(")
            && os_rs.contains("unsafe fn mi_os_current_subproc() -> Option<BorrowedSubproc>")
            && os_rs.contains(
                "unsafe fn mi_os_subproc_stats_mut(subproc: BorrowedSubproc) -> *mut mi_stats_t"
            )
            && os_rs.contains(
                "unsafe fn mi_os_prim_free(\n    memid: OsMemId,\n    commit_size: CommittedBytes,\n    subproc: Option<BorrowedSubproc>,"
            )
            && os_rs.contains("let subproc = subproc.or_else(|| mi_os_current_subproc());")
            && os_rs.contains("let MemIdView::Os(memid) = MemId::view(memid) else")
            && os_rs.contains(
                "let subproc = SubprocPtr::new(subproc).map(|subproc| unsafe { BorrowedSubproc::new(subproc) });"
            )
            && os_rs.contains("let addr = OsMemPtr::new(addr);")
            && os_rs.contains("let size = ReservedBytes::new(size);")
            && os_rs.contains(
                "unsafe { _mi_os_free_ex_typed(addr, size, still_committed, memid, subproc) };"
            )
            && os_rs.contains(
                "unsafe fn _mi_os_free_ex_typed(\n    addr: Option<OsMemPtr>,\n    size: ReservedBytes,\n    still_committed: bool,\n    os: OsMemId,\n    subproc: Option<BorrowedSubproc>,"
            )
            && os_rs.contains("if matches!(&os.kind, OsMemKind::Huge) {")
            && os_rs.contains(
                "unsafe {\n            mi_os_free_huge_os_pages(OsMemPtr::new(base), ReservedBytes::new(csize), subproc)\n        };"
            )
            && os_rs.contains("} else {\n        let committed = if still_committed {")
            && os_rs.contains("unsafe { mi_os_prim_free(memid, committed, subproc) };")
            && os_rs.contains(
                "unsafe fn mi_os_free_huge_os_pages(\n    p: Option<OsMemPtr>,\n    size: ReservedBytes,\n    subproc: Option<BorrowedSubproc>,"
            )
            && os_rs.contains(
                "mi_os_prim_free(memid, CommittedBytes::new(MI_HUGE_OS_PAGE_SIZE), subproc)"
            )
            && os_rs.contains(
                "mi_os_prim_free(memid, CommittedBytes::new(MI_HUGE_OS_PAGE_SIZE), None);"
            )
            && os_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_os_zalloc(")
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_alloc_aligned(")
            && os_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_os_numa_node()")
            && os_rs.contains("static MI_OS_PAGE_SIZE_KEEP")
            && os_rs.contains("static MI_OS_GUARD_PAGE_SIZE_KEEP")
            && os_rs.contains("static MI_OS_SECURE_GUARD_PAGE_SIZE_KEEP")
            && os_rs.contains("static MI_OS_ALLOC_KEEP")
            && os_rs.contains("static MI_OS_FREE_KEEP")
            && os_rs.contains("static MI_OS_REUSE_KEEP")
            && os_rs.contains("static MI_OS_PURGE_KEEP")
            && os_rs.contains("static MI_OS_PROTECT_KEEP")
            && os_rs.contains("static MI_OS_UNPROTECT_KEEP")
            && os_rs.contains("static MI_OS_SECURE_GUARD_PAGE_SET_AT_KEEP")
            && os_rs.contains("static MI_OS_SECURE_GUARD_PAGE_RESET_BEFORE_KEEP"),
        "OS helper wrappers should keep C's local helper boundaries and retained wrapper set"
    );
    assert!(
        page_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_free(")
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_free_active_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,\n    queued: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,"
            )
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_free_full_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Full>,\n    queue: crate::core_types::FullPageQueue,"
            )
            && page_rs.contains(
                "crate::core_types::QueuedPage::<crate::core_types::NormalQueue>::new(active, queue)"
            )
            && page_rs.contains("let _ = _mi_page_free_active_internal(all_free, queued);")
            && page_rs.contains("let _ = queued.remove();")
            && page_queues_rs.contains(
                "pub(crate) unsafe fn remove(self) -> crate::core_types::ActivePage"
            )
            && !page_rs.contains("pub(crate) unsafe fn _mi_page_free_internal<S>")
            && page_rs.contains("mi_page_set_theap(page, ptr::null_mut());")
            && page_rs.contains("let destroyed = _mi_arenas_page_free_typed(page_wrapper, owner);")
            && page_rs.contains("_mi_arenas_collect(false, false, (*theap).tld);")
            && arena_rs.contains(
                "pub(crate) unsafe fn _mi_arenas_page_free_typed<S>(\n    page: crate::core_types::AllFreePage<S>,\n    owner: crate::core_types::ThreadHeapLocal,"
            )
            && arena_rs
                .contains("unsafe { _mi_arenas_page_free(page.page().as_ptr(), owner.ptr().as_ptr()) };")
            && arena_rs.contains("page.free_to_arena(owner)")
            && theap_rs.contains("_mi_page_free(page, pq);"),
        "page release should keep C's local _mi_page_free helper boundary while using typed all-free page witnesses internally"
    );
    assert!(
        page_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_abandon(")
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_abandon_internal(\n    page_wrapper: crate::core_types::PageForAbandon,\n    queue: crate::core_types::PageQueuePtr,"
            )
            && page_rs.contains(") -> crate::core_types::PageAbandonOutcome {")
            && page_rs.contains("_mi_page_free_collect(page, false);")
            && page_rs.contains("if mi_page_all_free(page) {")
            && page_rs.contains("let destroyed = match page_wrapper {")
            && page_rs.contains("let Some(all_free) = page.mark_all_free() else {")
            && page_rs.contains("_mi_page_free_full_internal(all_free, queue)")
            && page_rs.contains("_mi_page_free_active_internal(all_free, queued)")
            && page_rs.contains("QueuedPage::<crate::core_types::FullQueue>::new")
            && page_rs.contains("QueuedPage::<crate::core_types::NormalQueue>::new")
            && page_rs.contains("let _ = queued.remove();")
            && page_rs.contains("mi_page_set_theap(page, ptr::null_mut());")
            && page_rs.contains("_mi_arenas_page_abandon(page, theap);")
            && page_rs.contains("_mi_arenas_collect(false, false, (*theap).tld);")
            && page_rs.contains("crate::core_types::PageAbandonOutcome::Abandoned")
            && page_rs.contains("crate::core_types::AbandonedPage::new_unchecked(page_ptr)")
            && page_rs.contains("let page_ptr = crate::core_types::PagePtr::new_unchecked(page);")
            && page_rs.contains("crate::core_types::FullPage::new_unchecked(page_ptr).for_abandon()")
            && page_rs.contains("crate::core_types::ActivePage::new_unchecked(page_ptr).for_abandon()")
            && theap_rs.contains("_mi_page_abandon(page, pq);"),
        "page abandon should keep C's local helper boundary while using typed active/full abandon witnesses internally"
    );
    assert!(
        page_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_free_collect(")
            && page_rs.contains(
                "#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_free_collect_partly("
            )
            && page_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_unfull(")
            && page_rs.contains(
                "#[inline(never)]\npub(crate) unsafe fn _mi_page_unfull_internal(\n    page_wrapper: crate::core_types::FullPage,"
            )
            && page_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_page_retire(page: *mut mi_page_t)")
            && page_rs.contains(
                "pub(crate) unsafe fn _mi_page_retire_internal(\n    page_wrapper: crate::core_types::AllFreePage<crate::core_types::Active>,"
            )
            && page_rs.contains("crate::core_types::AllFreePage::active_unchecked")
            && page_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_init(")
            && page_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_queue_is_valid(")
            && page_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_stats_bin(")
            && page_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_queue_append(")
            && page_rs.contains("static MI_PAGE_QUEUE_IS_VALID_KEEP")
            && page_rs.contains("static MI_PAGE_QUEUE_APPEND_KEEP")
            && page_rs.contains("mi_page_thread_free_collect(page);")
            && page_rs.contains("let atomic_tf = crate::core_types::AtomicThreadFree(")
            && page_rs.contains("crate::core_types::AtomicUsizeField(ptr::addr_of_mut!((*page).xthread_free))")
            && page_rs.contains("let remote = atomic_tf.collect_acq_rel();")
            && page_rs.contains("if remote.is_empty() {")
            && page_rs.contains("atomic_tf.load_relaxed().head_raw() == head")
            && alloc_rs.contains("let block = allocated.block().as_ptr();")
            && !alloc_rs.contains("allocated.0.as_ptr()")
            && free_rs.contains("block_wrapper.into_free_block()")
            && !free_rs.contains("crate::core_types::FreeBlock::new_unchecked")
            && !free_rs.contains("crate::core_types::FreeBlock(")
            && freelist_types.contains("AllocatedBlock::new_unchecked(head)")
            && freelist_types.contains("pub(crate) unsafe fn pop(page: AvailablePage)")
            && freelist_types.contains("let block_raw = block.block().as_ptr();")
            && !freelist_types.contains("AllocatedBlock(head)")
            && !freelist_types.contains("block.0.as_ptr()")
            && page_rs.contains("let thread_free = crate::core_types::AtomicThreadFree(crate::core_types::AtomicUsizeField(\n            ptr::addr_of_mut!((*page).xthread_free),\n        ))")
            && page_rs.contains("crate::core_types::ThreadFreeWord::from_parts(\n                    None,\n                    crate::core_types::ThreadFreeOwner::Owned")
            && !page_rs.contains("crate::internal::mi_tf_create(ptr::null_mut()")
            && !page_rs.contains("crate::internal::mi_tf_block(mi_atomic_load_explicit")
            && !page_rs.contains("(*page).xthread_free == 1")
            && freelist_types.contains("pub(crate) struct AlignedBlockTag(BlockTag);")
            && freelist_types.contains("pub(crate) struct GuardedBlockTag(BlockTag);")
            && freelist_types.contains("pub(crate) unsafe fn write_aligned_to(block: BlockPtr)")
            && freelist_types.contains("pub(crate) unsafe fn write_guarded_to(block: BlockPtr)")
            && freelist_types.contains("pub(crate) unsafe fn read_guarded_from_raw(")
            && freelist_types.contains(") -> Option<GuardedBlockTag>")
            && freelist_types.contains("pub(crate) unsafe fn read_from_raw(block: *const mi_block_t) -> Self")
            && freelist_types.contains("Self::FreeList(unsafe { EncodedPtr::read_word_from_raw(block) })")
            && freelist_types.contains("pub(crate) unsafe fn write_to(self, block: BlockPtr)")
            && atomic_types.contains("pub(crate) struct RemoteFreeList(Option<BlockPtr>);")
            && atomic_types.contains("fn from_detached_thread_free_head(head: Option<BlockPtr>) -> Self")
            && atomic_types.contains("pub(crate) fn head(self) -> Option<BlockPtr>")
            && atomic_types.contains("pub(crate) fn is_empty(self) -> bool")
            && atomic_types.contains("pub(crate) unsafe fn collect_into_local(")
            && !freelist_types.contains("pub(crate) struct RemoteFreeList(pub(crate) Option<BlockPtr>)")
            && alloc_rs.contains("crate::core_types::BlockTag::write_aligned_to(")
            && alloc_rs.contains("crate::core_types::BlockTag::write_guarded_to(")
            && alloc_aligned_rs.contains("crate::core_types::BlockTag::write_aligned_to(block)")
            && internal.contains("crate::core_types::BlockLink::is_guarded_raw(block)")
            && free_rs.contains("crate::core_types::BlockLink::is_guarded_raw(block)")
            && !alloc_rs.contains("crate::core_types::BlockLink::tagged(")
            && !alloc_aligned_rs.contains("crate::core_types::BlockLink::tagged(")
            && !internal.contains("BlockLink::read_from_raw(block).is_guarded_tag()")
            && !free_rs.contains("BlockLink::read_from_raw(block).is_guarded_tag()")
            && !freelist_types.contains("pub(crate) const fn tagged(tag: BlockTag) -> Self")
            && !freelist_types.contains("pub(crate) const ALIGNED: Self")
            && !freelist_types.contains("pub(crate) const GUARDED: Self")
            && !internal.contains("(*block).next")
            && !alloc_rs.contains("(*block).next = 0")
            && !alloc_rs.contains("(*block).next = MI_BLOCK_TAG_GUARDED")
            && !alloc_aligned_rs.contains("(*block).next = MI_BLOCK_TAG_ALIGNED")
            && !free_rs.contains("(*block).next == MI_BLOCK_TAG_GUARDED")
            && !internal.contains("(*block.cast_mut()).next == MI_BLOCK_TAG_GUARDED")
            && page_rs
                .contains("crate::mi_assert_internal!(!force || (*page).local_free.is_null());")
            && page_rs.contains("unsafe fn mi_page_move_to_full_typed(")
            && page_rs.contains("unsafe fn mi_page_unfull_typed(")
            && page_rs.contains(
                "pub(crate) unsafe fn mi_page_queue_enqueue_from(\n    to: crate::core_types::FullPageQueue,\n    page: crate::core_types::QueuedPage<crate::core_types::NormalQueue>,"
            )
            && page_rs.contains(
                "pub(crate) unsafe fn mi_page_queue_enqueue_from_full(\n    to: crate::core_types::NormalPageQueue,\n    page: crate::core_types::QueuedPage<crate::core_types::FullQueue>,"
            )
            && page_rs.contains(
                "pub(crate) unsafe fn mi_page_queue_enqueue_from_raw(\n    to: crate::core_types::PageQueuePtr,\n    from: crate::core_types::PageQueuePtr,"
            )
            && page_rs.contains("unsafe { page.into_full(to) }")
            && page_queues_rs.contains("impl QueuedPage<NormalQueue>")
            && page_queues_rs.contains("pub(crate) unsafe fn into_full(self, to: FullPageQueue)")
            && page_rs.contains("unsafe { page.unfull(to) }")
            && page_states_rs.contains("pub(crate) unsafe fn to_full(")
            && page_queues_rs.contains("impl QueuedPage<FullQueue>")
            && page_queues_rs
                .contains("pub(crate) unsafe fn unfull(self, to: NormalPageQueue)")
            && !page_states_rs.contains("pub(crate) unsafe fn unfull(")
            && !page_rs.contains("pub unsafe fn mi_page_queue_enqueue_from_ex(")
            && !page_rs.contains(
                "pub unsafe fn mi_page_queue_enqueue_from_ex(\n    to: *mut mi_page_queue_t,"
            )
            && !page_rs.contains(
                "pub unsafe fn mi_page_queue_enqueue_from(\n    to: *mut mi_page_queue_t,"
            )
            && !page_rs.contains(
                "pub unsafe fn mi_page_queue_enqueue_from_full(\n    to: *mut mi_page_queue_t,"
            )
            && page_rs.contains("match crate::core_types::MemId::view((*page).memid)")
            && page_rs.contains("crate::core_types::MemIdView::Os(os) => os")
            && !page_rs.contains("(*page).memid.mem.os.base")
            && page_rs.contains(
                "let _ = _mi_page_unfull_internal(crate::core_types::FullPage::new_unchecked("
            )
            && page_rs.contains("QueuedPage::<crate::core_types::FullQueue>::new")
            && page_rs.contains("mi_page_unfull_typed(queued, to)")
            && page_rs
                .contains("mi_page_move_to_full_typed(\n                crate::core_types::ActivePage")
            && free_rs.contains("let Some(all_free) = active.mark_all_free() else {")
            && free_rs.contains("_mi_page_retire_internal(all_free);")
            && free_rs
                .contains("_mi_page_unfull_internal(crate::core_types::FullPage::new_unchecked")
            && free_rs.contains("_mi_page_free_collect(page_raw, false);")
            && free_rs.contains("_mi_page_free_collect_partly(page_raw, mt_free.as_ptr());")
            && arena_rs.contains("crate::page::_mi_page_init(theap, page)")
            && arena_rs.contains("_mi_page_free_collect(page_raw, false)")
            && arena_rs.contains("_mi_page_stats_bin(page)")
            && theap_rs.contains("_mi_page_free_collect(page, true)")
            && page_rs.contains("unsafe fn mi_page_queue_is_valid_typed(")
            && page_rs.contains("pub(crate) unsafe fn mi_normal_page_queue_is_valid(")
            && page_rs.contains("pub(crate) unsafe fn mi_huge_page_queue_is_valid(")
            && page_rs.contains("pub(crate) unsafe fn mi_full_page_queue_is_valid(")
            && theap_rs.contains("mi_normal_page_queue_is_valid(")
            && theap_rs.contains("mi_huge_page_queue_is_valid(")
            && theap_rs.contains("mi_full_page_queue_is_valid(")
            && !theap_rs.contains("_mi_page_queue_is_valid("),
        "page lifecycle and page-queue helpers should keep C's local helper boundaries"
    );
    assert!(
        alloc_rs
            .contains("pub(crate) unsafe fn _mi_page_malloc_zero_typed(\n    theap: *mut mi_theap_t,\n    available: crate::core_types::AvailablePage,\n    padded: crate::core_types::PaddedSize,")
            && alloc_rs.contains("zeroed: crate::core_types::Zeroed,")
            && alloc_rs.contains("let page_ptr = available.page();")
            && alloc_rs.contains("let size = padded.get();")
            && alloc_rs.contains("crate::core_types::LayoutRequest::from_padded_unchecked(")
            && alloc_rs.contains("crate::core_types::Zeroed::from_bool(zero)")
            && alloc_rs.contains("_mi_page_malloc_zero_typed(")
            && alloc_rs.contains("let padded = crate::core_types::PaddedSize::new_unchecked(size);")
            && alloc_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_malloc_zero")
            && alloc_rs.contains("let Some(available) = active.available() else {")
            && alloc_rs.contains("_mi_page_malloc_zero_typed(theap, available, padded, zeroed, ptr::null_mut())")
            && page_rs.contains(
                "crate::core_types::Zeroed::from_bool(zero),\n            ptr::null_mut(),"
            )
            && internal.contains("pub(crate) unsafe fn _mi_theap_get_free_small_page_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    padded: crate::core_types::PaddedSize,")
            && internal.contains("let theap = theap.ptr().as_ptr();")
            && internal.contains("let idx = mi_direct_bin_from_padded(padded).as_usize();")
            && alloc_rs.contains("let page = _mi_theap_get_free_small_page_typed(local_theap, padded);")
            && alloc_rs.contains("_mi_page_malloc_zero_typed(theap, available, padded, zeroed, usable)")
            && alloc_aligned_rs.contains(
                "let page =\n            unsafe { crate::internal::_mi_theap_get_free_small_page_typed(local_theap, padded) };"
            )
            && alloc_aligned_rs.contains(
                "let available = unsafe { active.available().unwrap_unchecked() };"
            )
            && alloc_aligned_rs.contains(
                "_mi_page_malloc_zero_typed(\n                        theap,\n                        available,\n                        padded,"
            )
            && alloc_aligned_rs.contains("crate::core_types::Zeroed::from_bool(zero),")
            && alloc_aligned_rs.contains("let padded = layout.padded();")
            && !page_rs.contains("let p = _mi_page_malloc_zero(theap, page, size, zero);")
            && !alloc_aligned_rs
                .contains("let p = _mi_page_malloc_zero(theap, page, padsize, zero);")
            && !alloc_rs.contains("_mi_theap_get_free_small_page(theap, padded.get())")
            && !alloc_aligned_rs.contains("_mi_theap_get_free_small_page(theap, padsize)"),
        "page and aligned allocation should use typed padded-size helpers internally"
    );
    assert!(
        atomic_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_atomic_once_enter")
            && atomic_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_atomic_once_release")
            && atomic_types.contains("pub(crate) struct OnceGuard<'a>")
            && atomic_types
                .contains("pub(crate) unsafe fn try_enter(once: OnceStatePtr) -> Option<Self>")
            && atomic_types.contains("impl Drop for OnceGuard<'_>")
            && atomic_types.contains("crate::atomic::_mi_atomic_once_release(")
            && atomic_rs.contains("_once: crate::core_types::OnceGuard<'static>")
            && atomic_rs.contains("crate::core_types::OnceGuard::try_enter(state)")
            && !atomic_rs.contains("once: *mut mi_atomic_once_t,\n}")
            && !atomic_rs.contains("_mi_atomic_once_release(self.once);"),
        "atomic once should keep C's enter/release helper boundaries and release through typed OnceGuard Drop"
    );
    assert!(
        bitmap_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_bitmap_forall_set(")
            && bitmap_rs.contains(
                "#[inline(never)]\npub unsafe extern \"C\" fn _mi_bitmap_forall_setc_ranges("
            )
            && bitmap_rs.contains(
                "#[inline(never)]\npub unsafe extern \"C\" fn _mi_bitmap_forall_setc_rangesn("
            )
            && bitmap_rs
                .contains("return mi_bitmap_forall_setc_ranges_typed(bitmap, visit, ArenaPtr::new(arena), arg);")
            && arena_rs.contains("_mi_bitmap_forall_setc_rangesn("),
        "bitmap traversal should keep C's set/range traversal helper boundaries"
    );
    assert!(
        bitmap_rs.contains(
            "pub(crate) unsafe fn mi_bitmap_forall_set_typed(\n    bitmap: *mut mi_bitmap_t,\n    visit: mi_forall_set_fun_t,\n    arena: Option<ArenaPtr>,"
        ) && bitmap_rs.contains("unsafe { mi_bitmap_forall_set_typed(bitmap, visit, ArenaPtr::new(arena), arg) }")
            && bitmap_rs.contains("let index = SliceIndex::new(base_idx + bidx);")
            && bitmap_rs.contains("visit(index.get(), one.get() as usize, arena, arg)")
            && arena_rs.contains("ok = mi_bitmap_forall_set_typed(")
            && arena_rs.contains("Some(arena),"),
        "_mi_bitmap_forall_set should keep its raw extern shim but use typed arena/index wrappers internally"
    );
    assert!(
        bitmap_rs.contains(
            "pub(crate) unsafe fn mi_bitmap_forall_setc_ranges_typed(\n    bitmap: *mut mi_bitmap_t,\n    visit: mi_forall_set_fun_t,\n    arena: Option<ArenaPtr>,"
        ) && bitmap_rs.contains("unsafe { mi_bitmap_forall_setc_ranges_typed(bitmap, visit, ArenaPtr::new(arena), arg) }")
            && bitmap_rs.contains("let range = SliceRange::from_raw(")
            && bitmap_rs.contains("visit(range.start_usize(), range.count_usize(), arena, arg)"),
        "_mi_bitmap_forall_setc_ranges should keep its raw extern shim but use typed arena/range wrappers internally"
    );
    assert!(
        bitmap_rs.contains(
            "pub(crate) unsafe fn mi_bitmap_forall_setc_rangesn_typed(\n    bitmap: *mut mi_bitmap_t,\n    rngslices: SliceCount,"
        ) && bitmap_rs.contains("let rngslices_raw = if rngslices > MI_BFIELD_BITS {")
            && bitmap_rs.contains("crate::mi_assert_internal!(rngslices_raw > 0);")
            && bitmap_rs.contains("let rngslices = SliceCount::new_unchecked(rngslices_raw as u32);")
            && bitmap_rs.contains("mi_bitmap_forall_setc_rangesn_typed(bitmap, rngslices, visit, ArenaPtr::new(arena), arg)")
            && bitmap_rs.contains("let rngslices_count = rngslices;")
            && bitmap_rs.contains("let range = SliceRange::from_raw(idx as u32, rngslices_count.get())")
            && bitmap_rs.contains("visit(range.start_usize(), range.count_usize(), arena, arg)"),
        "_mi_bitmap_forall_setc_rangesn should keep its raw extern shim but use typed range/count wrappers internally"
    );
    for marker in [
        "#[inline(never)]\npub(crate) unsafe fn mi_bitmap_try_find_and_claim(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_size(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_init(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_unsafe_setN(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_setN(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_clearN(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_set(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_clear(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_popcountN(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_is_xsetN(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_is_setN(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_is_clearN(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_is_set(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_is_clear(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_is_all_clear(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_clear_once_set(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_bsr(",
        "#[inline(never)]\npub unsafe fn mi_bitmap_popcount(",
        "static MI_BITMAP_TRY_FIND_AND_CLAIM_KEEP",
        "static MI_BITMAP_SIZE_KEEP",
        "static MI_BITMAP_INIT_KEEP",
        "static MI_BITMAP_UNSAFE_SETN_KEEP",
        "static MI_BITMAP_SETN_KEEP",
        "static MI_BITMAP_CLEARN_KEEP",
        "static MI_BITMAP_SET_KEEP",
        "static MI_BITMAP_CLEAR_KEEP",
        "static MI_BITMAP_POPCOUNTN_KEEP",
        "static MI_BITMAP_IS_XSETN_KEEP",
        "static MI_BITMAP_IS_SETN_KEEP",
        "static MI_BITMAP_IS_CLEARN_KEEP",
        "static MI_BITMAP_IS_SET_KEEP",
        "static MI_BITMAP_IS_CLEAR_KEEP",
        "static MI_BITMAP_IS_ALL_CLEAR_KEEP",
        "static MI_BITMAP_CLEAR_ONCE_SET_KEEP",
        "static MI_BITMAP_BSR_KEEP",
        "static MI_BITMAP_POPCOUNT_KEEP",
    ] {
        assert!(
            bitmap_rs.contains(marker),
            "regular bitmap helper boundary marker missing: {marker}"
        );
    }
    assert!(
        bitmap_rs.contains(
            "pub(crate) unsafe fn mi_bitmap_try_find_and_claim(\n    bitmap: *mut mi_bitmap_t,\n    tseq: ThreadSeq,\n    pidx: &mut SliceIndex,"
        )
            && bitmap_rs.contains("claim: ThreadLocalClaimCallback,\n    arena: Option<ArenaPtr>,")
            && bitmap_rs.contains("let mut claim = claim;")
            && bitmap_rs.contains("let mut raw_idx = pidx.get();")
            && bitmap_rs.contains(".then(|| *pidx = SliceIndex::new(raw_idx))")
            && bitmap_rs.contains("(*claim_fun).invoke(slice_index, claim_data.arena,")
            && bitmap_rs.contains("arena: Option<ArenaPtr>,")
            && threadlocal_rs.contains(
                "let claim_fun = ThreadLocalClaimCallback::new(mi_thread_local_claim_fun);"
            )
            && threadlocal_rs.contains(
                "mi_bitmap_try_find_and_claim(slots, ThreadSeq::new(0), &mut idx, claim_fun, None)"
            )
            && threadlocal_rs.contains("mi_key_create(idx.get(), version)")
            && arena_rs.contains(
                "let claim_fun = ThreadLocalClaimCallback::new(mi_arena_try_claim_abandoned);"
            )
            && arena_rs.contains("let mut slice_index = SliceIndex::new(0);")
            && arena_rs.contains("claim_fun,\n                    Some(arena),")
            && arena_rs.contains("let page = mi_arena_page_at_slice(arena, slice_index);")
            && callbacks_rs.contains("impl ThreadLocalClaimCallback")
            && callbacks_rs.contains("arena: Option<ArenaPtr>,")
            && threadlocal_rs.contains("struct ThreadLocalKeepSet(ptr::NonNull<bool>);")
            && threadlocal_rs.contains("unsafe fn from_raw(keep_set: *mut bool) -> Self")
            && threadlocal_rs.contains("fn mi_thread_local_claim_fun_typed(\n    _slice_index: SliceIndex,\n    _arena: Option<ArenaPtr>,\n    keep_set: ThreadLocalKeepSet,")
            && threadlocal_rs.contains("ArenaPtr::new(arena),")
            && threadlocal_rs.contains("ThreadLocalKeepSet::from_raw(keep_set),")
            && !bitmap_rs.contains("*mut mi_claim_fun_t"),
        "thread-local claim callbacks should route through typed callback wrappers below raw typedefs"
    );
    for marker in [
        "#[inline(never)]\npub unsafe fn mi_bbitmap_debug_get_bin(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_size(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_init(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_unsafe_setN(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_bsr_inv(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_setN(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_try_clearNC(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_is_xsetN(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_try_find_and_clear(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_try_find_and_clear8(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_try_find_and_clearNX(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_try_find_and_clearNC(",
        "#[inline(never)]\npub unsafe fn mi_bbitmap_try_find_and_clearN_(",
        "static MI_BBITMAP_DEBUG_GET_BIN_KEEP",
        "static MI_BBITMAP_SIZE_KEEP",
        "static MI_BBITMAP_INIT_KEEP",
        "static MI_BBITMAP_UNSAFE_SETN_KEEP",
        "static MI_BBITMAP_BSR_INV_KEEP",
        "static MI_BBITMAP_SETN_KEEP",
        "static MI_BBITMAP_TRY_CLEARNC_KEEP",
        "static MI_BBITMAP_IS_XSETN_KEEP",
        "static MI_BBITMAP_TRY_FIND_AND_CLEAR_KEEP",
        "static MI_BBITMAP_TRY_FIND_AND_CLEAR8_KEEP",
        "static MI_BBITMAP_TRY_FIND_AND_CLEARNX_KEEP",
        "static MI_BBITMAP_TRY_FIND_AND_CLEARNC_KEEP",
        "static MI_BBITMAP_TRY_FIND_AND_CLEARN_KEEP",
    ] {
        assert!(
            bitmap_rs.contains(marker),
            "bbitmap helper boundary marker missing: {marker}"
        );
    }
    let bbitmap_find_start = bitmap_rs
        .find("#[inline(never)]\npub unsafe fn mi_bbitmap_try_find_and_clear(")
        .expect("find mi_bbitmap_try_find_and_clear");
    let bbitmap_find_end = bitmap_rs[bbitmap_find_start..]
        .find("#[inline(never)]\npub unsafe fn mi_bbitmap_try_find_and_clear8(")
        .map(|offset| bbitmap_find_start + offset)
        .expect("find end of mi_bbitmap_try_find_and_clear");
    let bbitmap_find_body = &bitmap_rs[bbitmap_find_start..bbitmap_find_end];
    assert!(
        !bbitmap_find_body.contains("mi_bbitmap_try_find_and_clear_generic(")
            && bbitmap_find_body.contains("mi_bchunk_try_find_and_clear(chunk, &mut cidx)")
            && bbitmap_find_body
                .contains("[(MI_CBIN_SMALL, cmap_small), (MI_CBIN_NONE, cmap_none)]"),
        "single-slice bbitmap find/clear should keep C's specialized static-inline wrapper shape"
    );
    for marker in [
        "#[inline(never)]\nfn mi_bchunk_xsetNC(",
        "#[inline(never)]\nfn mi_bchunk_popcountNC(",
        "#[inline(never)]\nfn mi_bchunk_is_xsetNC(",
        "#[inline(never)]\nfn mi_bchunk_try_clearNC(",
        "#[inline(never)]\nfn mi_bchunk_try_find_and_clear8(",
        "#[inline(never)]\nfn mi_bchunk_try_find_and_clearNX(",
        "#[inline(never)]\n#[allow(clippy::many_single_char_names)]\nfn mi_bchunk_try_find_and_clearNC(",
        "#[inline(never)]\nfn mi_bitmap_chunkmap_try_clear(",
        "#[inline(never)]\nfn mi_bbitmap_set_chunk_bin(",
        "#[inline(never)]\nfn mi_bbitmap_chunkmap_try_clear(",
        "static MI_BCHUNK_XSETNC_KEEP",
        "static MI_BCHUNK_POPCOUNTNC_KEEP",
        "static MI_BCHUNK_IS_XSETNC_KEEP",
        "static MI_BCHUNK_TRY_CLEARNC_KEEP",
        "static MI_BCHUNK_TRY_FIND_AND_CLEAR8_KEEP",
        "static MI_BCHUNK_TRY_FIND_AND_CLEARNX_KEEP",
        "static MI_BCHUNK_TRY_FIND_AND_CLEARNC_KEEP",
        "static MI_BITMAP_CHUNKMAP_TRY_CLEAR_KEEP",
        "static MI_BBITMAP_SET_CHUNK_BIN_KEEP",
        "static MI_BBITMAP_CHUNKMAP_TRY_CLEAR_KEEP",
    ] {
        assert!(
            bitmap_rs.contains(marker),
            "bchunk bitmap helper boundary marker missing: {marker}"
        );
    }
    for marker in [
        "#[inline(never)]\nunsafe fn mi_arena_slice_start(",
        "#[inline(never)]\nunsafe fn mi_arena_page_at_slice(",
        "#[inline(never)]\nunsafe fn mi_arenas_page_singleton_alloc(",
        "#[inline(never)]\nunsafe fn mi_arena_purge(",
        "#[inline(never)]\nunsafe fn mi_debug_show_chunks(",
        "static MI_ARENA_SLICE_START_KEEP",
        "static MI_ARENA_PAGE_AT_SLICE_KEEP",
        "static MI_ARENAS_PAGE_SINGLETON_ALLOC_KEEP",
        "static MI_ARENA_PURGE_KEEP",
        "static MI_DEBUG_SHOW_CHUNKS_KEEP",
    ] {
        assert!(
            arena_rs.contains(marker),
            "arena helper boundary marker missing: {marker}"
        );
    }
    let claim_start = arena_rs
        .find("unsafe fn mi_arena_try_claim_abandoned_at(")
        .expect("find mi_arena_try_claim_abandoned_at");
    let claim_end = arena_rs[claim_start..]
        .find("\n}\n\nunsafe extern \"C\" fn mi_arena_try_claim_abandoned(")
        .map(|offset| claim_start + offset)
        .expect("find end of mi_arena_try_claim_abandoned_at");
    let claim_body = &arena_rs[claim_start..claim_end];
    assert!(
        !claim_body.contains("mi_arena_page_at_slice(")
            && claim_body.contains("pages_meta.add(slice_index)")
            && claim_body.contains("slice_index * MI_ARENA_SLICE_SIZE"),
        "abandoned arena page claiming should keep C's inlined page lookup shape"
    );
    assert!(
        arena_rs.contains(
            "unsafe { mi_arena_try_claim_abandoned_at(SliceIndex::new(slice_index), arena, keep_abandoned) }"
        ),
        "abandoned arena page callback should parse raw slice indexes at the C ABI boundary"
    );
    assert!(
        arena_rs.contains(
            "unsafe fn mi_arena_slice_start(\n    arena: ArenaPtr,\n    slice_index: crate::core_types::SliceIndex,"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_page_at_slice(\n    arena: ArenaPtr,\n    slice_index: crate::core_types::SliceIndex,"
        ) && arena_rs.contains("unsafe fn mi_heap_visit_page_at_index(\n    slice_index: SliceIndex,")
            && arena_rs.contains("unsafe fn mi_arena_try_claim_abandoned_at(\n    slice_index: SliceIndex,")
            && arena_rs.contains("    slice_index: SliceIndex,\n    pbit_of_page:")
            && arena_rs.contains("mi_heap_visit_page_at_index(SliceIndex::new(slice_index), arena, arg)")
            && arena_rs.contains(
                "mi_arena_try_claim_abandoned_at(SliceIndex::new(slice_index), arena, keep_abandoned)"
            )
            && arena_rs
                .contains("mi_arena_page_at_slice(ArenaPtr::new_unchecked(arena), slice_index)")
            && arena_rs.contains("let page = mi_arena_page_at_slice(arena, slice_index);")
            && arena_rs.contains(
                "mi_arena_slice_start(arena_ptr, crate::core_types::SliceIndex::new(slice_index))"
            )
            && arena_rs.contains(
                "mi_arena_slice_start(ArenaPtr::new_unchecked(arena), range.start())\n            .as_ptr()\n            .cast::<c_void>()"
            )
            && !arena_rs.contains("unsafe fn mi_arena_page_at_slice(\n    arena: *mut mi_arena_t,")
            && !arena_rs.contains("unsafe fn mi_arena_slice_start(\n    arena: *mut mi_arena_t,"),
        "arena slice address/page helpers should receive typed ArenaPtr and SliceIndex values"
    );
    assert!(
        arena_types.contains(
            "pub(crate) fn single_at_offset(self, offset: usize) -> Result<Self, SliceError>"
        ) && arena_rs.contains("let Ok(single) = range.single_at_offset(i) else")
            && !arena_rs.contains("SliceRange::new(slice_index + i, 1"),
        "arena purge fallback should split SliceRange values without reopening raw slice arithmetic"
    );
    assert!(
        arena_rs.contains(
            "unsafe fn mi_arena_schedule_purge(arena: ArenaPtr, range: crate::core_types::SliceRange)"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_purge(arena: ArenaPtr, range: crate::core_types::SliceRange) -> bool"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_try_purge_range(arena: ArenaPtr, range: crate::core_types::SliceRange) -> bool"
        ) && !arena_rs.contains(
            "unsafe fn mi_arena_purge(arena: *mut mi_arena_t, range: crate::core_types::SliceRange) -> bool"
        ) && !arena_rs.contains(
            "unsafe fn mi_arena_try_purge_range(\n    arena: *mut mi_arena_t,\n    range: crate::core_types::SliceRange,\n) -> bool"
        ) && arena_rs.contains("match crate::core_types::MemId::view(memid)")
            && arena_rs.contains("crate::core_types::MemIdView::Arena(arena_memid)")
            && arena_rs.contains("let range = arena_memid.range;")
            && arena_rs.contains("fn mi_memid_is_os(memid: mi_memid_t) -> bool")
            && arena_rs.contains("crate::core_types::MemIdView::Os(_)")
            && memid_rs.contains("pub(crate) struct ExternalMemId")
            && memid_rs.contains("External(ExternalMemId)")
            && memid_rs.contains("impl MemId<ExternalMem>")
            && memid_rs.contains("pub(crate) fn external(")
            && arena_rs.contains(
                "memid = crate::core_types::MemId::<crate::core_types::NoneMem>::none().into_raw();"
            )
            && arena_rs
                .contains("crate::core_types::MemId::<crate::core_types::ExternalMem>::external")
            && arena_rs.contains("crate::core_types::MemIdView::External(external)")
            && os_rs.contains("crate::core_types::MemIdView::External(external)")
            && arena_rs.contains("let os_size = external.size.get();")
            && arena_rs.contains("bit_of_page = match crate::core_types::MemId::view(unsafe { (*page).memid })")
            && arena_rs.contains("crate::core_types::MemIdView::Arena(a) => a.range.count_usize() as isize")
            && os_rs.contains("external.size.get()")
            && os_rs.contains("let MemIdView::Os(memid) = MemId::view(memid) else")
            && memid_rs.contains("pub(crate) fn os_huge(")
            && memid_rs.contains("fn os_with_kind(")
            && internal.contains("crate::core_types::MemId::<crate::core_types::OsMem>::os(")
            && os_rs.contains("crate::core_types::MemId::<crate::core_types::OsMem>::os_huge(")
            && !os_rs.contains("mi_memkind_is_os(memid.memkind)")
            && !os_rs.contains("(*memid).memkind = crate::internal::mi_memkind_t::MI_MEM_OS_HUGE")
            && !arena_rs.contains("memid.mem.os.")
            && !arena_rs.contains("memid.mem.arena.")
            && !arena_rs.contains("memid.memkind = mi_memkind_t::MI_MEM_EXTERNAL")
            && !arena_rs.contains("memid.memkind = mi_memkind_t::MI_MEM_NONE")
            && !os_rs.contains("memid.mem.os.")
            && !internal.contains("memid.mem.os.base = base")
            && !internal.contains("memid.mem.os.size = size")
            && page_map.contains("fn mi_memid_os_base_size(memid: mi_memid_t)")
            && page_map.contains("crate::core_types::MemIdView::Os(os)")
            && page_map.contains("crate::core_types::MemIdView::External(external)")
            && !page_map.contains("memid.mem.os.")
            && !page_map.contains("mi_page_map_memid.mem.os.")
            && theap_rs.contains("let allocated_size = match crate::core_types::MemId::view(memid)")
            && theap_rs.contains("crate::core_types::MemIdView::Arena(a)")
            && theap_rs.contains("a.range.count_usize() * crate::arena::MI_ARENA_SLICE_SIZE")
            && theap_rs.contains("fn mi_memid_is_refcounted(memid: mi_memid_t) -> bool")
            && theap_rs.contains("crate::core_types::MemIdView::Unknown(code)")
            && theap_rs.contains("code.0 > crate::core_types::MemKindCode::STATIC.0")
            && theap_rs.contains("match crate::core_types::MemId::view(memid)")
            && theap_rs.contains("crate::core_types::MemIdView::HeapMain(_)")
            && theap_rs.contains("crate::core_types::MemIdView::Meta(_)")
            && theap_rs.contains("mi_memid_is_refcounted((*theap.as_ptr()).memid)")
            && atomic_types.contains(
                "pub(crate) struct AtomicThreadHeapRefcount(pub(crate) AtomicUsizeField);"
            )
            && atomic_types.contains("pub(crate) unsafe fn incref_acq_rel(self) -> usize")
            && atomic_types.contains("pub(crate) unsafe fn decref_acq_rel(self) -> usize")
            && ptr_rs.contains("pub(crate) unsafe fn refcount_cell(self) -> AtomicThreadHeapRefcount")
            && theap_rs.matches(".refcount_cell()").count() == 3
            && theap_rs.contains(".incref_acq_rel();")
            && theap_rs.contains(".decref_acq_rel()")
            && !theap_rs.contains("AtomicUsizeField(ptr::addr_of_mut!((*theap).refcount))")
            && !theap_rs.contains("mi_atomic_increment_acq_rel(ptr::addr_of_mut!((*theap).refcount))")
            && !theap_rs.contains("mi_atomic_fetch_sub_explicit(\n            ptr::addr_of_mut!((*theap).refcount),")
            && !theap_rs.contains("memid.mem.os.")
            && !theap_rs.contains("memid.memkind")
            && init_rs.contains("fn mi_memid_is_static(memid: mi_memid_t) -> bool")
            && init_rs.contains("crate::core_types::MemIdView::Static(_)")
            && memid_rs.contains("impl MemId<StaticMem>")
            && memid_rs.contains("pub(crate) const fn static_mem() -> Self")
            && init_rs.contains(
                "crate::core_types::MemId::<crate::core_types::StaticMem>::static_mem().into_raw()"
            )
            && init_rs.contains("!mi_memid_is_static((*main_prefix).memid)")
            && init_rs.contains("!mi_memid_is_static(theap_main.memid)")
            && !init_rs.contains(
                "crate::internal::_mi_memid_create(mi_memkind_t::MI_MEM_STATIC)"
            )
            && !init_rs.contains("(*main_prefix).memid.memkind != mi_memkind_t::MI_MEM_STATIC")
            && !init_rs.contains("theap_main.memid.memkind != mi_memkind_t::MI_MEM_STATIC")
            && internal.contains("pub fn mi_memid_is_os(memid: mi_memid_t) -> bool")
            && internal.contains("pub fn mi_memid_needs_no_free(memid: mi_memid_t) -> bool")
            && internal.contains("pub unsafe fn mi_page_is_in_arena(page: *const mi_page_t) -> bool")
            && internal.contains("crate::core_types::MemIdView::None(_)")
            && internal.contains("crate::core_types::MemIdView::External(_)")
            && internal.contains("crate::core_types::MemIdView::Static(_)")
            && internal.contains("crate::core_types::MemIdView::Arena(_)")
            && !internal.contains("pub unsafe fn _mi_memid_create(")
            && !internal.contains("raw.memkind = memkind")
            && internal.contains("mi_page_is_in_arena(page)")
            && arena_meta_rs.contains("MemId::<MetaMem>::meta(")
            && arena_meta_rs.contains("NonZeroU32::new_unchecked(block_count as u32)")
            && memid_rs.contains("impl MemId<MetaMem>")
            && memid_rs.contains("is_pinned: true")
            && memid_rs.contains("initially_committed: true")
            && memid_rs.contains("initially_zero: true")
            && !internal.contains("crate::types::mi_memkind_is_os(memid.memkind)")
            && !internal.contains("crate::types::mi_memkind_needs_no_free(memid.memkind)")
            && !internal.contains("(*page).memid.memkind == mi_memkind_t::MI_MEM_ARENA")
            && !internal.contains("memid.mem.meta.meta_page = mpage")
            && !internal.contains("memid.mem.meta.block_index = block_idx as u32")
            && !internal.contains("memid.mem.meta.block_count = block_count as u32")
            && arena_meta_rs.contains(
                "if let crate::core_types::MemIdView::Meta(meta) = crate::core_types::MemId::view(memid)"
            )
            && arena_meta_rs.contains("let block_count = meta.block_count.get() as usize;")
            && arena_meta_rs.contains("let block_idx = meta.block_index.0 as usize;")
            && !arena_meta_rs.contains("memid.mem.meta.")
            && !arena_meta_rs.contains("memid.memkind == crate::internal::mi_memkind_t::MI_MEM_META")
            && !arena_rs.contains("const fn mi_memkind_is_os(memkind: mi_memkind_t)")
            && !arena_rs.contains("mi_memkind_is_os(unsafe { (*arena.cast::<MiArenaPrefix>()).memid.memkind })")
            && !arena_rs.contains(
                "unsafe fn mi_arena_schedule_purge(arena: *mut mi_arena_t, slice_index: usize, slice_count: usize)"
            )
            && !arena_rs.contains(
                "unsafe fn mi_arena_schedule_purge(arena: *mut mi_arena_t, range: crate::core_types::SliceRange)"
            )
            && !arena_rs.contains(
                "unsafe fn mi_arena_purge(arena: *mut mi_arena_t, slice_index: usize, slice_count: usize)"
            ),
        "arena free/purge path should dispatch through MemIdView and carry SliceRange"
    );
    assert!(
        page_map.contains(
            "unsafe fn mi_page_map_ensure_committed(\n    idx: crate::core_types::PageMapIndex,\n    slice_count: crate::core_types::SliceCount,"
        ) && page_map.contains(
            "unsafe fn mi_page_map_ensure_committed(\n    idx: crate::core_types::PageMapIndex,\n    submap: *mut mi_submap_t,"
        ) && page_map.contains(
            "unsafe fn mi_page_map_set_range_prim(\n    page: PagePtr,\n    mut idx: crate::core_types::PageMapIndex,\n    mut sub_idx: crate::core_types::PageMapSubIndex,\n    slice_count: crate::core_types::SliceCount,"
        ) && page_map.contains(
            "unsafe fn mi_page_map_clear_range_prim("
        ) && page_map.contains(
            "unsafe fn mi_page_map_set_range(\n    page: PagePtr,\n    idx: crate::core_types::PageMapIndex,\n    sub_idx: crate::core_types::PageMapSubIndex,\n    slice_count: crate::core_types::SliceCount,"
        ) && page_map.contains(
            "unsafe fn mi_page_map_clear_range("
        ) && page_map.contains("unsafe fn mi_page_map_register_typed(page: PagePtr) -> bool")
            && page_map.contains("unsafe fn mi_page_map_unregister_typed(page: PagePtr)")
            && page_map.contains("let slice_count = SliceCount::from_usize(slice_count)?;")
            && page_map.contains("unsafe { mi_page_map_register_typed(page) }")
            && page_map.contains("unsafe { mi_page_map_unregister_typed(page) };")
            && !page_map.contains("unsafe fn mi_page_map_ensure_committed(idx: usize, slice_count: usize)")
            && !page_map.contains("unsafe fn mi_page_map_ensure_committed(idx: usize, submap:")
            && !page_map.contains("unsafe fn mi_page_map_set_range_prim(\n    page: *mut mi_page_t,\n    mut idx: usize,\n    mut sub_idx: usize,\n    mut slice_count: usize,")
            && !page_map.contains("unsafe fn mi_page_map_set_range_prim(\n    page: *mut mi_page_t,\n    mut idx: usize,\n    mut sub_idx: usize,\n    slice_count: crate::core_types::SliceCount,")
            && !page_map.contains("unsafe fn mi_page_map_set_range_prim(\n    page: *mut mi_page_t,\n    mut idx: crate::core_types::PageMapIndex,")
            && !page_map.contains("unsafe fn mi_page_map_set_range(\n    page: *mut mi_page_t,\n    idx: usize,\n    sub_idx: usize,\n    slice_count: usize,")
            && !page_map.contains("fn mi_page_map_get_idx(\n    page: *mut mi_page_t,"),
        "page-map range helpers should validate raw indexes/counts once and carry typed indexes plus SliceCount internally"
    );
    assert!(
        arena_rs.contains("SliceCount::from_usize_const(mi_slice_count_of_size(")
            && memid_rs.contains("slice_index: range.start_u32()")
            && memid_rs.contains("slice_count: range.count_u32()")
            && page_map.contains("let slice_count = SliceCount::from_usize(slice_count)?;")
            && !arena_rs.contains(" as u32")
            && !page_map.contains(" as u32")
            && !memid_rs.contains(" as u32")
            && !arena_rs.contains("SliceRange::from_raw(slice_index as")
            && !arena_rs.contains("SliceRange::from_raw(slice_index, slice_count as")
            && !page_map.contains("SliceCount::from_usize(slice_count as"),
        "arena and page-map slice fields should use checked typed conversions instead of lossy casts"
    );
    assert!(
        page_types.contains("pub(crate) struct RegisteredPageRange {\n    page: PagePtr,\n    index: PageMapIndex,\n    sub_index: PageMapSubIndex,\n    slice_count: SliceCount,\n}")
            && page_types.contains("pub(crate) unsafe fn new_unchecked(\n        page: PagePtr,\n        index: PageMapIndex,\n        sub_index: PageMapSubIndex,\n        slice_count: SliceCount,\n    ) -> Self")
            && page_types.contains("pub(crate) fn page(self) -> PagePtr")
            && page_types.contains("pub(crate) fn index(self) -> PageMapIndex")
            && page_types.contains("pub(crate) fn sub_index(self) -> PageMapSubIndex")
            && page_types.contains("pub(crate) fn slice_count(self) -> SliceCount")
            && page_map.contains("crate::core_types::RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count)\n    };")
            && page_map.contains("crate::mi_debug_assert!(registered.page().as_ptr() == page.as_ptr());")
            && page_map.contains("crate::mi_debug_assert!(registered.index() == idx);")
            && page_map.contains("crate::mi_debug_assert!(registered.sub_index() == sub_idx);")
            && page_map.contains("crate::mi_debug_assert!(registered.slice_count() == slice_count);")
            && page_map.contains("Some(unsafe { RegisteredPageRange::new_unchecked(page, idx, sub_idx, slice_count) })"),
        "RegisteredPageRange should be an opaque page-map registration witness over PagePtr, PageMapIndex, PageMapSubIndex, and SliceCount"
    );
    assert!(
        atomic_types.contains("pub(crate) struct AtomicPtrField<T>(pub(crate) *mut *mut T);")
            && atomic_types
                .contains("pub(crate) unsafe fn load(self, order: MemoryOrder) -> *mut T")
            && atomic_types.contains("expected: &mut *mut T,")
            && page_map.contains(
                "crate::core_types::AtomicPtrField(unsafe { _mi_page_map.add(idx.get()) });"
            )
            && page_map.contains("atomic_submap.load(crate::core_types::MemoryOrder::Acquire)")
            && page_map.contains("atomic_submap.compare_exchange_acq_rel(&mut expect, new_sub)")
            && !page_map.contains("mi_atomic_load_ptr_acquire(_mi_page_map.add(idx))")
            && !page_map.contains(
                "mi_atomic_cas_strong_acq_rel(\n                        _mi_page_map.add(idx).cast::<usize>()"
            ),
        "page-map submap slot atomics should use AtomicPtrField"
    );
    assert!(
        internal.contains(
            "crate::core_types::AtomicPtrField(ptr::addr_of!((*theap).heap).cast_mut())\n            .load(crate::core_types::MemoryOrder::Acquire)"
        ) && internal.contains(
            "crate::core_types::AtomicPtrField(_mi_page_map_base().add(idx))\n            .load(crate::core_types::MemoryOrder::Acquire)"
        ) && !internal.contains("crate::atomic::mi_atomic_load_ptr_acquire(&(*theap).heap)")
            && !internal.contains("crate::atomic::mi_atomic_load_ptr_acquire(_mi_page_map_base().add(idx))"),
        "internal thread-heap and page-map pointer loads should use AtomicPtrField"
    );
    assert!(
        arena_rs.contains(
            "unsafe fn mi_memid_create_arena(\n    arena: *mut mi_arena_t,\n    range: crate::core_types::SliceRange,\n) -> mi_memid_t"
        ) && arena_rs.contains("memid: unsafe { mi_memid_create_arena(arena, range) },")
            && arena_rs.contains("let Some((arena, range)) = (unsafe { mi_arena_range_from_memid(memid) }) else")
            && arena_rs.contains("unsafe { *slice_index = range.start_usize() };")
            && arena_rs.contains("unsafe { *slice_count = range.count_usize() };")
            && scalar_types.contains("pub(crate) fn as_u32(self) -> Option<u32>")
            && scalar_types.contains("pub(crate) const fn from_usize_const(count: usize) -> Option<Self>")
            && arena_types.contains("pub(crate) fn start_u32(self) -> u32")
            && arena_types.contains("pub(crate) const fn count_u32(self) -> u32")
            && memid_rs.contains("slice_index: range.start_u32()")
            && memid_rs.contains("slice_count: range.count_u32()")
            && arena_rs.contains("SliceCount::from_usize_const(mi_slice_count_of_size(")
            && !arena_rs.contains("mi_slice_count_of_size(MI_SMALL_PAGE_SIZE) as u32")
            && !arena_rs.contains("mi_slice_count_of_size(MI_MEDIUM_PAGE_SIZE) as u32")
            && !arena_rs.contains("mi_slice_count_of_size(MI_LARGE_PAGE_SIZE) as u32")
            && !memid_rs.contains("range.start().0 as u32")
            && !arena_rs.contains(
                "unsafe fn mi_memid_create_arena(\n    arena: *mut mi_arena_t,\n    slice_index: usize,\n    slice_count: usize,"
            )
            && !arena_rs.contains("SliceRange::from_raw(slice_index as u32, slice_count as u32)"),
        "arena memid construction should receive an already-typed SliceRange"
    );
    assert!(
        arena_rs.contains(
            "unsafe fn mi_arena_try_alloc_at(\n    arena: ArenaPtr,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(") -> Option<(*mut c_void, crate::core_types::ArenaAllocation)>")
            && arena_types.contains(
                "pub(crate) struct ArenaAllocation {\n    pub(crate) arena: ArenaPtr,\n    pub(crate) range: SliceRange,\n    pub(crate) memid: crate::internal::mi_memid_t,\n}"
            )
            && arena_rs.contains("let mut allocation = crate::core_types::ArenaAllocation {")
            && arena_rs.contains("unsafe { *memid = allocation.assume_init().memid };")
            && arena_rs.contains("mi_arena_try_alloc_at(arena, slice_count, commit, tseq)")
            && !arena_rs.contains(
                "unsafe fn mi_arena_try_alloc_at(\n    arena: *mut mi_arena_t,\n    slice_count: crate::core_types::SliceCount,"
            )
            && !arena_rs.contains("mi_arena_try_alloc_at(arena, slice_count.get() as usize"),
        "arena allocation helper should accept typed SliceCount and return ArenaAllocation"
    );
    assert!(
        arena_rs.contains(
            "unsafe fn mi_arenas_try_alloc(\n    heap: BorrowedHeap,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains("    alignment: crate::core_types::Alignment,")
            && arena_rs.contains("    req_arena: Option<ArenaPtr>,")
            && arena_rs.contains("    numa_node: crate::core_types::NumaNode,")
            && arena_rs.contains("let Some(subproc) = (unsafe { heap.ptr().subproc() }) else")
            && arena_rs.contains("heap: HeapPtr::new(heap),")
            && arena_rs.contains("crate::core_types::NumaNode::new(numa_node),")
            && arena_rs.contains("req_arena: ArenaPtr::new(req_arena),")
            && arena_rs.contains("let Some(heap_ptr) = HeapPtr::new(heap) else")
            && arena_rs.contains("BorrowedHeap::new_unchecked(heap_ptr),")
            && !arena_rs.contains(
                "unsafe fn mi_arenas_try_alloc(\n    heap: *mut mi_heap_t,"
            ),
        "arena try-alloc should receive typed heap, requested arena, slice count, and NUMA node below raw entry points"
    );
    assert!(
        arena_types.contains("#[repr(transparent)]\npub(crate) struct SharedArena(pub(crate) ArenaPtr);")
            && arena_types.contains("#[repr(transparent)]\npub(crate) struct ExclusiveArena(pub(crate) ArenaPtr);")
            && arena_types.contains("#[repr(transparent)]\npub(crate) struct ParentArena(pub(crate) ArenaPtr);")
            && arena_types.contains("impl SharedArena {\n    /// # Safety\n    /// `arena` must point to initialized arena metadata.\n    #[inline(always)]\n    pub(crate) unsafe fn new(arena: ArenaPtr) -> Option<Self> {\n        if unsafe { arena.is_exclusive() } {")
            && arena_types.contains("impl ExclusiveArena {\n    /// # Safety\n    /// `arena` must point to initialized arena metadata.\n    #[inline(always)]\n    pub(crate) unsafe fn new(arena: ArenaPtr) -> Option<Self> {\n        if unsafe { arena.is_exclusive() } {")
            && arena_types.contains("impl ParentArena {\n    /// # Safety\n    /// `arena` must point to initialized arena metadata.\n    #[inline(always)]\n    pub(crate) unsafe fn new(arena: ArenaPtr) -> Option<Self> {\n        if unsafe { arena.parent().is_none() } {"),
        "arena role wrappers should have checked constructors based on raw arena state"
    );
    assert!(
        arena_rs.contains(
            "unsafe fn mi_arena_pages_size(\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_info_slices_needed(\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_bitmap_init(\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_bbitmap_init(\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arena_initialize(\n    subproc: SubprocPtr,\n    start: OsMemPtr,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains("let slice_count_usize = slice_count.get() as usize;")
            && arena_rs.contains("let Some(typed_slice_count) = crate::core_types::SliceCount::from_usize(slice_count)")
            && arena_rs.contains("mi_arena_initialize(\n                SubprocPtr::new_unchecked(subproc),\n                OsMemPtr::new_unchecked(start),\n                typed_slice_count,")
            && !arena_rs.contains("unsafe fn mi_arena_pages_size(slice_count: usize")
            && !arena_rs.contains("unsafe fn mi_arena_info_slices_needed(slice_count: usize")
            && !arena_rs.contains("unsafe fn mi_arena_bitmap_init(slice_count: usize")
            && !arena_rs.contains("unsafe fn mi_arena_bbitmap_init(slice_count: usize"),
        "arena metadata sizing and initialization should receive typed SliceCount"
    );
    assert!(
        arena_rs.contains(
            "unsafe fn mi_arenas_page_try_find_abandoned(\n    theap: *mut mi_theap_t,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arenas_page_alloc_fresh_area(\n    theap: *mut mi_theap_t,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arenas_page_alloc_fresh(\n    theap: *mut mi_theap_t,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains(
            "unsafe fn mi_arenas_page_regular_alloc(\n    theap: *mut mi_theap_t,\n    slice_count: crate::core_types::SliceCount,"
        ) && arena_rs.contains("let Some(slice_count) = crate::core_types::SliceCount::from_usize(slice_count)")
            && arena_rs.contains("let block_size_typed = unsafe { crate::core_types::BlockSize::new_unchecked(block_size) };")
            && arena_rs.contains(
                "let block_alignment_typed =\n        unsafe { crate::core_types::MaybeAlignment::new_unchecked(block_alignment) };",
            )
            && arena_rs.contains("const MI_SMALL_PAGE_SLICE_COUNT: crate::core_types::SliceCount")
            && arena_rs.contains("const MI_MEDIUM_PAGE_SLICE_COUNT: crate::core_types::SliceCount")
            && arena_rs.contains("const MI_LARGE_PAGE_SLICE_COUNT: crate::core_types::SliceCount")
            && arena_rs.contains("mi_arenas_page_regular_alloc(theap, MI_SMALL_PAGE_SLICE_COUNT, block_size_typed)")
            && arena_rs.contains("mi_arenas_page_regular_alloc(theap, MI_MEDIUM_PAGE_SLICE_COUNT, block_size_typed)")
            && arena_rs.contains("mi_arenas_page_regular_alloc(theap, MI_LARGE_PAGE_SLICE_COUNT, block_size_typed)")
            && !arena_rs.contains("unsafe fn mi_arenas_page_try_find_abandoned(\n    theap: *mut mi_theap_t,\n    slice_count: usize,")
            && !arena_rs.contains("unsafe fn mi_arenas_page_alloc_fresh_area(\n    theap: *mut mi_theap_t,\n    slice_count: usize,")
            && !arena_rs.contains("unsafe fn mi_arenas_page_alloc_fresh(\n    theap: *mut mi_theap_t,\n    slice_count: usize,")
            && !arena_rs.contains("unsafe fn mi_arenas_page_regular_alloc(\n    theap: *mut mi_theap_t,\n    slice_count: usize,"),
        "arena page allocation helpers should carry typed SliceCount after raw entry-point validation"
    );
    assert!(
        stats_rs.contains("#[inline(never)]\npub extern \"C\" fn _mi_clock_now()")
            && stats_rs.contains("#[inline(never)]\npub extern \"C\" fn _mi_clock_start()")
            && stats_rs.contains("#[inline(never)]\npub extern \"C\" fn _mi_clock_end(")
            && stats_rs.contains("let t0 = _mi_clock_now();")
            && stats_rs.contains("let diff = _mi_clock_now() - t0;")
            && stats_rs.contains("_mi_clock_now() - start - CLOCK_DIFF.load")
            && os_rs.contains("let start_t = _mi_clock_start();")
            && os_rs.contains("let mut elapsed = _mi_clock_end(start_t);")
            && arena_rs.contains("let expire = crate::stats::_mi_clock_now() + delay as i64;")
            && arena_rs.contains("let now = crate::stats::_mi_clock_now();"),
        "clock helpers should keep C's timing helper boundaries and main callers"
    );
    assert!(
        atomic_types.contains("pub(crate) struct AtomicI64Field(pub(crate) *mut i64);")
            && atomic_types.contains("pub(crate) unsafe fn compare_exchange_acq_rel(self, expected: &mut i64, desired: i64) -> bool")
            && arena_rs.matches("crate::core_types::AtomicI64Field(").count() == 7
            && arena_rs
                .matches(".compare_exchange_acq_rel(&mut expire0, expire)")
                .count()
                == 2
            && arena_rs.contains(".load(crate::core_types::MemoryOrder::Relaxed)")
            && arena_rs.contains(".load(crate::core_types::MemoryOrder::Acquire)")
            && arena_rs
                .matches(".store(0, crate::core_types::MemoryOrder::Release)")
                .count()
                == 2
            && arena_rs.contains(
                "now + ((delay as mi_msecs_t) / 10),\n                    crate::core_types::MemoryOrder::Release,"
            )
            && !arena_rs.contains("mi_atomic_casi64_strong_acq_rel")
            && !arena_rs.contains("mi_atomic_loadi64_explicit")
            && !arena_rs.contains("mi_atomic_loadi64_acquire")
            && !arena_rs.contains("mi_atomic_storei64_release")
            && !arena_rs.contains("mi_memory_order_e::mi_memory_order_relaxed"),
        "arena purge expiry should use typed atomic i64 field wrappers"
    );
    assert!(
            atomic_types.contains("pub(crate) struct AtomicUsizeField(pub(crate) *mut usize);")
            && atomic_types.contains("desired: usize,")
            && atomic_types.contains(
                "atomic.compare_exchange(*expected, desired, Ordering::AcqRel, Ordering::Acquire)"
            )
            && arena_rs
                .matches("ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arena_count)")
                .count()
                == 2
            && arena_rs.contains(".compare_exchange_acq_rel(&mut expected, 0)")
            && arena_rs.contains("let next_count = count + 1;")
            && arena_rs.contains(".compare_exchange_acq_rel(&mut count, next_count)")
            && !arena_rs.contains(
                "mi_atomic_cas_strong_acq_rel(\n            ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arena_count)"
            )
            && !arena_rs.contains(
                "mi_atomic_cas_strong_acq_rel(\n                ptr::addr_of_mut!((*subproc.cast::<MiSubprocPrefix>()).arena_count)"
            ),
        "arena_count compare-exchange should use AtomicUsizeField"
    );
    assert!(
        atomic_types
            .contains("pub(crate) struct AtomicAbandonedCount(pub(crate) *const AtomicUsize);")
            && atomic_types.contains("impl AtomicAbandonedCount")
            && atomic_types.contains("pub(crate) unsafe fn load_relaxed(self) -> usize")
            && atomic_types
                .contains("pub(crate) unsafe fn fetch_add_relaxed(self, value: usize) -> usize")
            && atomic_types
                .contains("pub(crate) unsafe fn fetch_sub_relaxed(self, value: usize) -> usize")
            && arena_rs.contains(
                "unsafe fn mi_heap_abandoned_count(\n    heap: *mut mi_heap_t,\n    bin: crate::core_types::Bin,"
            )
            && arena_rs.contains("abandoned_count[bin.as_usize()]")
            && arena_rs.contains("let Some(typed_bin) = crate::core_types::Bin::new(bin as u16) else")
            && arena_rs.contains("crate::mi_assert_internal!(false);\n                                break;")
            && arena_rs.contains("mi_heap_abandoned_count(heap, typed_bin).load_relaxed() > 0")
            && arena_rs.contains("mi_heap_abandoned_count(heap, bin).load_relaxed() == 0")
            && arena_rs.contains("mi_heap_abandoned_count(heap, bin).fetch_add_relaxed(1)")
            && arena_rs.contains("mi_heap_abandoned_count(heap, bin).fetch_sub_relaxed(1)")
            && !arena_rs.contains(".abandoned_count[bin].load(Ordering::Relaxed)")
            && !arena_rs.contains(".abandoned_count[bin].fetch_add(1, Ordering::Relaxed)")
            && !arena_rs.contains(".abandoned_count[bin].fetch_sub(1, Ordering::Relaxed)")
            && !arena_rs.contains(".abandoned_count[i].load(Ordering::Relaxed)"),
        "arena abandoned-count atomics should go through AtomicAbandonedCount with typed Bin indexes"
    );
    assert!(
        atomic_types
            .contains("pub(crate) struct AtomicArenaPage(pub(crate) *const AtomicPtr<c_void>);")
            && atomic_types.contains("impl AtomicArenaPage")
            && atomic_types.contains(
                "pub(crate) unsafe fn load_acquire(self) -> *mut crate::types::mi_arena_pages_t"
            )
            && atomic_types.contains(
                "pub(crate) unsafe fn store_release(self, value: *mut crate::types::mi_arena_pages_t)"
            )
            && arena_rs.contains(
                "unsafe fn mi_arena_index(arena: *mut mi_arena_t) -> crate::core_types::ArenaIndex"
            )
            && !arena_rs.contains(
                "unsafe fn mi_heap_arena_page_cell(\n    heap: *mut mi_heap_t,\n    arena_idx: crate::core_types::ArenaIndex,"
            )
            && arena_rs.contains(
                "unsafe fn mi_heap_arena_page_cell(\n    heap: BorrowedHeap,\n    arena_idx: crate::core_types::ArenaIndex,"
            )
            && arena_rs.contains(
                "unsafe fn mi_heap_ensure_arena_pages(\n    heap: BorrowedHeap,\n    arena: ArenaPtr,\n    arena_idx: ArenaIndex,"
            )
            && arena_rs.contains("arena_pages[arena_idx.get() as usize]")
            && arena_rs.matches("mi_heap_arena_page_cell(heap, arena_idx).load_acquire()").count()
                == 1
            && arena_rs
                .matches("mi_heap_arena_page_cell(borrowed_heap, arena_idx).load_acquire()")
                .count()
                == 2
            && arena_rs
                .contains("mi_heap_arena_page_cell(heap, arena_idx).store_release(arena_pages.cast())")
            && !arena_rs.contains(".arena_pages[arena_idx].load(Ordering::Acquire)")
            && !arena_rs.contains(".arena_pages[arena_idx]\n                    .store")
            && !arena_rs.contains(".arena_pages[i].load(Ordering::Acquire)"),
        "heap arena page-table atomics should use AtomicArenaPage with typed ArenaIndex values"
    );
    assert!(
        atomic_types
            .contains("pub(crate) unsafe fn store(self, value: *mut T, order: MemoryOrder)")
            && atomic_types.contains("pub(crate) unsafe fn compare_exchange_release(")
            && arena_rs
                .matches("crate::core_types::AtomicPtrField(ptr::addr_of_mut!(")
                .count()
                == 1
            && arena_rs.contains(
                "unsafe fn mi_subproc_arena_cell(\n    subproc: *mut mi_subproc_t,\n    arena_idx: ArenaIndex,"
            )
            && arena_rs.contains(
                "unsafe fn mi_arena_from_index_typed(subproc: SubprocPtr, arena_idx: ArenaIndex)"
            )
            && arena_rs
                .matches("(*subproc.cast::<MiSubprocPrefix>()).arenas[")
                .count()
                == 1
            && arena_rs.contains("let Some(arena_idx) = ArenaIndex::from_usize(idx)")
            && arena_rs
                .matches("mi_arena_from_index_typed(")
                .count()
                >= 5
            && arena_rs
                .matches(".compare_exchange_release(&mut expected, arena)")
                .count()
                == 2
            && arena_rs
                .contains(".store(ptr::null_mut(), crate::core_types::MemoryOrder::Release);")
            && !arena_rs.contains("mi_arena_from_index(subproc, i)")
            && !arena_rs.contains("mi_atomic_cas_ptr_strong_release")
            && !arena_rs.contains("mi_atomic_store_ptr_release")
            && !arena_rs.contains("mi_atomic_load_ptr_acquire"),
        "arena table pointer atomics should use AtomicPtrField with typed ArenaIndex values"
    );
    assert!(
        free_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_ptr_unalign")
            && free_rs.contains(
                "crate::mi_assert!(_mi_page_ptr_unalign(page_raw, user_raw) == user_raw.cast());"
            )
            && free_rs.contains(
                "pub(crate) unsafe fn mi_validate_block_from_ptr(\n    page: crate::core_types::PagePtr,\n    user: crate::core_types::UserPtr,\n) -> crate::core_types::PointerLookup"
            )
            && free_rs.contains("let location = mi_pointer_location(page, user, MI_SECURE > 0, true);")
            && free_rs.contains("unsafe fn mi_lookup_pointer_page(")
            && free_rs.contains("user: crate::core_types::UserPtr,")
            && free_rs.contains(") -> Option<crate::core_types::PagePtr>")
            && free_rs.contains("let registered_page = mi_lookup_pointer_page(user_ptr);")
            && free_rs.contains("unsafe fn mi_pointer_location(")
            && free_rs.contains("page: crate::core_types::PagePtr,")
            && free_rs.contains("user: crate::core_types::UserPtr,")
            && free_rs.contains(") -> crate::core_types::PointerLocation")
            && free_rs.contains("let page_raw = page.as_ptr();")
            && free_rs.contains("let user_raw = user.as_ptr();")
            && free_rs.contains("let block = if mi_page_has_interior_pointers(page_raw) || force_unalign")
            && free_rs.contains("PointerLocation::Guarded")
            && validation_types.contains("pub(crate) struct LocatedBlock")
            && validation_types.contains("pub(crate) struct GuardedBlock(BlockPtr);")
            && validation_types.contains("pub(crate) fn after_guarded_detection(block: BlockPtr) -> Self")
            && validation_types.contains("impl PointerLocation")
            && validation_types.contains("pub(crate) fn located_block(self) -> LocatedBlock")
            && validation_types.contains("Valid(ValidPointerLookup)")
            && validation_types.contains("pub(crate) struct ValidPointerLookup {\n    user: UserPtr,\n    page: PagePtr,\n    location: PointerLocation,")
            && validation_types.contains("pub(crate) fn valid(user: UserPtr, page: PagePtr, location: PointerLocation) -> Self")
            && validation_types.contains("pub(crate) struct BlockForFree {\n    block: BlockPtr,\n    was_guarded: WasGuarded,")
            && validation_types.contains("pub(crate) fn is_guarded(self) -> bool")
            && validation_types
                .contains("pub(crate) fn into_free_block(self) -> crate::core_types::FreeBlock")
            && validation_types.contains("pub(crate) struct ValidatedFree {\n    user: UserPtr,\n    page: PagePtr,\n    block: BlockForFree,")
            && validation_types.contains("pub(crate) fn prepare_free(self) -> Option<ValidatedFree>")
            && validation_types.contains("ValidatedFree { user, page, block }")
            && validation_types.contains("pub(crate) fn page(self) -> PagePtr")
            && validation_types.contains("pub(crate) fn block(self) -> BlockForFree")
            && free_rs.contains("/// # Safety\n/// `lookup` must come from validating the same page/user pair that is about to\n/// be freed by the generic free path.")
            && free_rs.contains("/// the block; this boundary consumes the lookup and performs any required\n/// guarded-block unprotection before returning the freeable block witness.")
            && free_rs.contains("unsafe fn mi_block_for_free_from_lookup(")
            && free_rs.contains("let validated = lookup.prepare_free()?;")
            && free_rs.contains("unsafe fn mi_prepare_validated_free(")
            && free_rs.contains("mi_prepare_validated_free(lookup).map(crate::core_types::ValidatedFree::block)")
            && free_rs.contains("let block = block_wrapper.block().as_ptr();")
            && free_rs.contains("let was_guarded = block_wrapper.is_guarded();")
            && free_rs.contains("let page = validated.page().as_ptr();")
            && free_rs.contains("validated.block()")
            && free_rs.contains("let path = page_tid.classify_free_path(current_tid);")
            && free_rs.contains("let owner = validated.page().classify_for_free(current_tid);")
            && free_rs.contains("crate::core_types::PageFreeOwner::Local(local_page)")
            && free_rs.contains("owner: crate::core_types::PageFreeOwner,")
            && free_rs.contains("crate::core_types::PageFreeOwner::Foreign(foreign_page)")
            && free_rs.contains("mi_free_block_local(\n                    local_page,")
            && free_rs.contains("mi_free_block_mt(foreign_page, validated.block());")
            && page_states_rs.contains("pub(crate) fn remote_free_page(self) -> ForeignPage")
            && free_rs.contains("mi_free_block_mt(abandoned_page.remote_free_page(), validated.block());")
            && !free_rs.contains("let delta = page_tid.xor_current(current_tid);")
            && !source_item(
                "src/allocator/free.rs",
                &free_rs,
                "pub(crate) unsafe fn mi_free_ex_slow("
            )
            .contains("LocalPage::new_unchecked")
            && !source_item(
                "src/allocator/free.rs",
                &free_rs,
                "pub(crate) unsafe fn mi_free_ex_slow("
            )
            .contains("ForeignPage::new_unchecked")
            && free_rs.contains("GuardedBlock::after_guarded_detection(block)")
            && free_rs.contains("GuardedBlock::after_guarded_detection(block_ptr)")
            && free_rs.contains("if let Some(validated) = mi_prepare_validated_free(mi_validate_pointer(p, msg))")
            && free_rs.contains("let location = mi_pointer_location(page_ptr, user_ptr, false, false);")
            && free_rs.contains("PointerLookup::valid(user_ptr, page_ptr, location)")
            && free_rs.matches("mi_block_check_unguard(validated.page(), validated.block(), validated.user())").count() == 1
            && !free_rs.contains("let Some((validated, located))")
            && !free_rs.contains("let block_raw = if mi_page_has_interior_pointers(page_raw)")
            && alloc_rs.contains("let location = valid.location();")
            && alloc_rs.contains("let block = location.block();")
            && free_rs.matches("let page = crate::core_types::PagePtr::new_unchecked(page);").count() == 2
            && free_rs.matches("let user = crate::core_types::UserPtr::new_unchecked(p);").count() == 2
            && free_rs
                .matches("mi_prepare_validated_free(mi_validate_block_from_ptr(page, user))")
                .count()
                == 2
            && free_rs.contains("LocalPage::new_unchecked(validated.page())")
            && free_rs.contains("ForeignPage::new_unchecked(validated.page())")
            && !free_rs.contains("mi_block_for_free_from_lookup(mi_validate_block_from_ptr(page, user))")
            && !free_rs.contains("mi_block_for_free_from_lookup(page, p, mi_validate_block_from_ptr(page, p))")
            && !validation_types.contains("Valid {\n        user: UserPtr")
            && !validation_types.contains("pub(crate) block: BlockPtr")
            && !validation_types.contains("pub(crate) page: PagePtr")
            && !validation_types.contains("pub(crate) struct GuardedBlock(pub(crate) BlockPtr);")
            && !validation_types.contains("pub(crate) fn new(user: UserPtr")
            && !validation_types.contains("pub(crate) fn new(block: BlockPtr")
            && !free_rs.contains("BlockForFree::new")
            && !free_rs.contains("ValidatedFree::new")
            && !free_rs.contains("validated.block,")
            && !free_rs.contains("PointerLocation::BlockStart(b) => (b, false)")
            && !free_rs.contains("pub unsafe fn mi_validate_block_from_ptr(\n    page: *const mi_page_t,\n    p: *mut c_void,\n) -> *mut mi_block_t"),
        "free validation should keep _mi_page_ptr_unalign while returning PointerLookup and BlockForFree"
    );
    assert!(
        !heap_rs.contains("fn _mi_theap_cached_set(")
            && !heap_rs.contains("fn _mi_theap_create(")
            && !heap_rs.contains("fn _mi_theap_free(")
            && !heap_rs.contains("fn _mi_heap_move_pages(")
            && !heap_rs.contains("fn _mi_heap_destroy_pages("),
        "heap internals should call crate-local theap helpers directly"
    );
    assert!(
        arena_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_heap_move_pages")
            && arena_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_heap_destroy_pages"
            )
            && heap_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_heap_force_destroy"
            )
            && heap_rs.contains("_mi_heap_move_pages(heap_raw, mi_heap_main());")
            && heap_rs.contains("_mi_heap_destroy_pages(heap.as_ptr());")
            && heap_rs.contains("unsafe { mi_heap_force_destroy_owned(heap) };"),
        "heap delete/destroy should keep C's heap page movement and force-destroy helper boundaries"
    );
    assert!(
        arena_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_alloc_aligned"
        ) && arena_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_alloc")
            && arena_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_contain")
            && arena_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_unsafe_destroy_all"
            )
            && arena_rs.contains("mi_arenas_try_alloc(")
            && arena_rs.contains("mi_arena_os_alloc_aligned(")
            && arena_rs.contains("_mi_arenas_alloc_aligned(")
            && arena_rs.contains("mi_arenas_contain_ex(InteriorUserPtr::new(p.cast_mut()), None)")
            && arena_rs.contains("static MI_ARENAS_CONTAIN_KEEP")
            && arena_rs.contains("mi_arenas_unsafe_destroy(subproc)")
            && theap_rs.contains("let theap = _mi_arenas_alloc(")
            && init_rs.contains("_mi_arenas_unsafe_destroy_all(subproc)"),
        "arena allocation, containment, and unsafe destroy helpers should keep C's local call boundaries"
    );
    assert!(
        arena_rs.contains("struct ArenaAllocTypedArgs {\n    heap: Option<HeapPtr>,\n    layout: crate::core_types::LayoutRequest,\n    align_offset: crate::core_types::AlignmentOffset,")
            && arena_rs.contains("unsafe fn mi_arenas_alloc_aligned_typed(args: ArenaAllocTypedArgs)")
            && arena_rs.contains("let size = layout.requested();")
            && arena_rs.contains("let alignment = layout.alignment();")
            && arena_rs.contains("arena_slice_count: Option<crate::core_types::SliceCount>,")
            && arena_rs.contains("fn mi_arena_slice_count_for_request(\n    size: crate::core_types::RequestedSize,")
            && arena_rs.contains("crate::core_types::SliceCount::from_usize(mi_slice_count_of_size(size.get()))")
            && arena_rs.contains("arena_slice_count: mi_arena_slice_count_for_request(layout.requested())")
            && arena_rs.contains("crate::core_types::LayoutRequest::aligned_at(")
            && arena_rs.contains("crate::core_types::AlignmentOffset::new(align_offset)")
            && arena_rs.contains("struct ArenaAllocArgs {\n    heap: Option<HeapPtr>,\n    size: crate::core_types::RequestedSize,")
            && arena_rs.contains("unsafe fn mi_arenas_alloc_typed(args: ArenaAllocArgs)")
            && arena_rs.contains("crate::core_types::LayoutRequest::aligned(size.get(), alignment)")
            && arena_rs.contains("let size = crate::core_types::RequestedSize::new(size);")
            && arena_rs.contains("crate::core_types::AlignmentOffset::new(0)")
            && arena_rs.contains("req_arena: ArenaPtr::new(req_arena)")
            && arena_rs.contains("tseq: crate::core_types::ThreadSeq::new(tseq)")
            && arena_rs.contains("numa_node: crate::core_types::NumaNode::new(numa_node)"),
        "arena allocation ABI wrappers should parse raw size/alignment into typed layout domains before using arena internals"
    );
    let reabandon_arena = source_item(
        "src/allocator/arena.rs",
        &arena_rs,
        "pub(crate) unsafe fn _mi_arenas_page_try_reabandon_to_mapped_arena",
    );
    assert!(
        arena_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_page_try_reabandon_to_mapped"
        )
            && arena_rs.contains(
                "pub(crate) unsafe fn _mi_arenas_page_try_reabandon_to_mapped_arena(\n    page: crate::core_types::OwnedAbandonedPage,\n    _arena_memid: crate::core_types::ArenaMemId,"
            )
            && arena_rs.contains("unsafe fn mi_page_memid_is_arena(page: *const mi_page_t) -> bool")
            && arena_rs.contains("crate::core_types::MemId::view(unsafe { (*page).memid })")
            && arena_rs.contains("crate::core_types::MemIdView::Arena(arena_memid)")
            && !arena_rs.contains("(*page).memid.memkind == mi_memkind_t::MI_MEM_ARENA")
            && !arena_rs.contains("(*page).memid.memkind != mi_memkind_t::MI_MEM_ARENA")
            && arena_rs.contains("_mi_page_associated_theap_peek(crate::core_types::PagePtr::new_unchecked(page))")
            && arena_rs.contains("_mi_arenas_page_abandon(page, theap)")
            && !reabandon_arena.contains("mi_page_memid_is_arena(page)")
            && !reabandon_arena.contains("MemId::view")
            && free_rs.contains("crate::core_types::MemId::view((*page).memid)")
            && free_rs.contains("crate::core_types::MemIdView::Arena(arena_memid)")
            && !free_rs.contains(
                "(*page).memid.memkind != crate::internal::mi_memkind_t::MI_MEM_ARENA"
            )
            && free_rs.contains("_mi_arenas_page_try_reabandon_to_mapped_arena(")
            && !free_rs.contains("_mi_arenas_page_try_reabandon_to_mapped(page)"),
        "free path should validate arena memids and carry the witness into the crate-local reabandon helper"
    );
    assert!(
        arena_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arena_id_none")
            && arena_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arena_from_id")
            && arena_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arena_memid_is_suitable"
            )
            && arena_rs.contains("static MI_ARENA_ID_NONE_KEEP")
            && arena_rs.contains("static MI_ARENA_FROM_ID_KEEP")
            && arena_rs.contains("unsafe { _mi_arena_id_none() }")
            && arena_rs.contains("let arena = unsafe { _mi_arena_from_id(arena_id) };")
            && heap_rs.contains("(*heap).exclusive_arena = _mi_arena_from_id(exclusive_arena_id);")
            && free_rs.contains("_mi_arena_memid_is_suitable("),
        "arena id conversion and memid suitability helpers should keep C's local call boundaries"
    );
    assert!(
        !init_rs.contains("fn _mi_theap_collect_abandon(")
            && !init_rs.contains("fn _mi_theap_create(")
            && !init_rs.contains("fn _mi_theap_free(")
            && !init_rs.contains("fn _mi_heap_force_destroy("),
        "thread initialization internals should call crate-local theap lifecycle helpers directly"
    );
    assert!(
        theap_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_collect_abandon"
        ) && theap_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_free"),
        "thread teardown should keep C's heap collect/free call boundaries instead of inlining them into _mi_thread_done"
    );
    assert!(
        theap_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_init")
            && theap_rs.contains("let Some(theap) = ThreadHeapPtr::new(theap) else")
            && theap_rs.contains("let Some(heap) = HeapPtr::new(heap) else")
            && theap_rs.contains("let Some(tld) = TldPtr::new(tld) else")
            && theap_rs.contains(
                "unsafe fn _mi_theap_init_typed(theap: ThreadHeapPtr, heap: HeapPtr, tld: TldPtr)"
            )
            && theap_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_create")
            && theap_rs.contains("unsafe fn _mi_theap_create_typed(heap: HeapPtr, tld: TldPtr) -> Option<ThreadHeapPtr>")
            && theap_rs.contains("unsafe { _mi_theap_create_typed(heap, tld) }\n        .map(ThreadHeapPtr::as_ptr)")
            && theap_rs.contains("_mi_theap_init_typed(theap, heap, tld);\n        Some(theap)"),
        "_mi_theap_create and _mi_theap_init should parse raw metadata pointers once and use typed wrappers below the ABI shim"
    );
    assert!(
        theap_rs.contains("_mi_memcpy_aligned(\n            theap_raw.cast(),")
            && internal.contains("unsafe fn mi_cpu_movsb_max() -> usize")
            && internal.contains("\"mov {max}, qword ptr [rip + _mi_cpu_movsb_max]\"")
            && internal.contains("if n <= mi_cpu_movsb_max()")
            && internal.contains("\"rep movsb\"")
            && internal.contains("let movsb_max = mi_cpu_movsb_max();")
            && internal.contains("if n > movsb_max && n.is_multiple_of(size_of::<usize>())")
            && internal.contains("if n <= movsb_max")
            && internal.contains("\"rep movsq\"")
            && internal.contains("pub unsafe fn _mi_memcpy_aligned"),
        "_mi_theap_init should copy the empty template through C's _mi_memcpy_aligned path"
    );
    let theap_init_start = theap_rs
        .find("pub(crate) unsafe extern \"C\" fn _mi_theap_init")
        .expect("find _mi_theap_init");
    let theap_init_end = theap_rs[theap_init_start..]
        .find("pub(crate) unsafe extern \"C\" fn _mi_theap_random_next")
        .map(|offset| theap_init_start + offset)
        .expect("find _mi_theap_random_next after _mi_theap_init");
    let theap_init_body = &theap_rs[theap_init_start..theap_init_end];
    assert!(
        theap_init_body.contains(
            "(*theap_raw).cookie = _mi_random_next(ptr::addr_of_mut!((*theap_raw).random)) | 1;"
        ) && !theap_init_body.contains("_mi_theap_random_next(theap)"),
        "_mi_theap_init should match C's optimized artifact by calling _mi_random_next directly for the cookie"
    );
    assert!(
        atomic_types.contains(
            "pub(crate) unsafe fn fetch_add(self, value: usize, order: MemoryOrder) -> usize"
        ) && atomic_types.contains(
            "pub(crate) unsafe fn fetch_sub(self, value: usize, order: MemoryOrder) -> usize"
        ) && atomic_types.contains("pub(crate) unsafe fn store_seq_cst(self, value: usize)")
            && theap_init_body.contains("theap.refcount_cell().store_seq_cst(1);")
            && theap_init_body.contains("theap.set_tld(Some(tld));")
            && theap_init_body.contains("theap.set_heap(Some(heap));")
            && !theap_init_body.contains(
                "mi_atomic_store_explicit(\n        ptr::addr_of_mut!((*theap).refcount),"
            )
            && !theap_init_body.contains("(*theap).refcount = 1;"),
        "_mi_theap_init should initialize C's thread-heap metadata through typed wrapper accessors"
    );
    assert!(
        atomic_types.contains("pub(crate) struct AtomicGuard<'a>")
            && atomic_types.contains(
                "pub(crate) unsafe fn try_acquire(field: AtomicUsizeField) -> Option<Self>"
            )
            && atomic_types.contains("impl Drop for AtomicGuard<'_>")
            && atomic_types.contains("self.field.store(0, Ordering::Release)")
            && atomic_rs.contains("_guard: crate::core_types::AtomicGuard<'static>")
            && atomic_rs.contains("crate::core_types::AtomicGuard::try_acquire(field)")
            && !atomic_rs.contains("guard: *mut mi_atomic_guard_t,\n}")
            && !atomic_rs.contains("mi_atomic_store_release(self.guard.cast::<usize>(), 0);"),
        "atomic guard scopes should acquire and release through typed AtomicGuard Drop"
    );
    assert!(
        atomic_types.contains(
            "pub(crate) struct MiLockPtr(pub(crate) NonNull<crate::types::mi_lock_t_pub>)"
        ) && atomic_types.contains("pub(crate) struct MiLockGuard<'a>")
            && atomic_types.contains("pub(crate) unsafe fn acquire(lock: MiLockPtr) -> Self")
            && atomic_types.contains("crate::atomic::mi_lock_acquire(lock.0.as_ptr())")
            && atomic_types.contains("impl Drop for MiLockGuard<'_>")
            && atomic_types.contains("crate::atomic::mi_lock_release(self.lock.0.as_ptr())")
            && ptr_rs.contains("pub(crate) unsafe fn arena_reserve_lock(self) -> MiLockPtr")
            && ptr_rs.contains("pub(crate) unsafe fn heaps_lock(self) -> MiLockPtr")
            && arena_rs.contains("MiLockGuard::acquire(subproc.arena_reserve_lock())")
            && init_rs.contains(
                "let lock = MiLockPtr(NonNull::new_unchecked(ptr::addr_of_mut!(subprocs_lock)));"
            )
            && init_rs.contains("let _lock = MiLockGuard::acquire(lock);")
            && init_rs.contains(
                "let heaps_lock = unsafe { MiLockGuard::acquire(subproc.heaps_lock()) };"
            )
            && init_rs.contains("drop(heaps_lock);")
            && options_rs.contains("let _lock = MiLockGuard::acquire(lock);")
            && options_rs.contains(
                "Manual lock handling keeps this translated callback buffer path close to"
            )
            && options_rs
                .contains("Intentionally holds the output buffer lock while invoking `out`"),
        "subproc and options locks should acquire and release through typed MiLockGuard Drop"
    );
    assert!(
        atomic_types.contains(
            "pub(crate) unsafe fn exchange_acq_rel(self, value: *mut T) -> *mut T"
        ) && theap_rs.contains(
            "crate::core_types::AtomicPtrField(ptr::addr_of_mut!((*theap).heap))\n            .exchange_acq_rel(ptr::null_mut())"
        ) && !theap_rs.contains("mi_atomic_exchange_ptr_acq_rel"),
        "_mi_theap_free should clear the heap pointer through AtomicPtrField"
    );
    assert!(
        stats_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_stats_merge_into"),
        "heap cleanup should keep C's stats merge call boundary instead of inlining it into _mi_theap_free"
    );
    assert!(
        stats_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_increase_mt")
            && stats_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_decrease_mt"
            ),
        "theap lifecycle should keep C's multithreaded stats update helper boundaries as crate-local calls"
    );
    assert!(
        stats_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_counter_increase_mt"
        ) && stats_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_adjust_increase_mt"
        ) && stats_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_adjust_decrease_mt"
        ),
        "multithreaded stats counters and adjusters should keep C's helper boundaries"
    );
    assert!(
        stats_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_increase")
            && stats_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_decrease")
            && stats_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_counter_increase"
            )
            && stats_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_adjust_increase"
            )
            && stats_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn __mi_stat_adjust_decrease"
            )
            && stats_rs.contains("static MI_STAT_ADJUST_INCREASE_KEEP"),
        "thread-local stats counters and adjusters should keep C's helper boundaries"
    );
    assert!(
        internal.contains("pub(crate) use crate::stats::{"),
        "internal stats helper re-exports should stay crate-local so hot call sites are direct local calls"
    );
    assert!(
        init_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc()")
            && init_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc_to_id")
            && init_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc_main")
            && init_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc_from_id"
            )
            && init_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc_heap_main"
            )
            && init_rs.contains("static mut subproc_main: MiSubprocPrefix")
            && init_rs.contains("static mut subprocs: *mut mi_subproc_t =")
            && init_rs.contains("core::arch::global_asm!(\".hidden subproc_main\")")
            && init_rs.contains("#[cfg_attr(target_os = \"linux\", unsafe(no_mangle))]")
            && init_rs.contains("unsafe fn mi_subproc_main_addr()")
            && init_rs.contains("unsafe fn mi_subproc_main_addr_typed() -> MainSubproc")
            && init_rs.contains(
                "MainSubproc::new_unchecked(SubprocPtr::new_unchecked(mi_subproc_main_addr()))"
            )
            && init_rs.contains(
                "unsafe fn mi_subproc_heap_main_typed(subproc: BorrowedSubproc) -> Option<HeapPtr>"
            )
            && init_rs.contains("fn _mi_subproc_to_id_typed(subproc: Option<SubprocPtr>)")
            && init_rs.contains("unsafe fn _mi_subproc_typed() -> BorrowedSubproc")
            && init_rs.contains("unsafe fn _mi_subproc_from_id_typed(subproc_id: mi_subproc_id_t)")
            && init_rs.contains(
                "unsafe fn _mi_subproc_heap_main_typed(subproc: BorrowedSubproc) -> MainHeap"
            )
            && init_rs.contains("\"lea {main}, [rip + subproc_main]\"")
            && !init_rs.contains("mi_subproc_main_ptr")
            && init_rs.contains("crate::prim::_mi_theap_default_raw()")
            && init_rs.contains("static MI_SUBPROC_TO_ID_KEEP")
            && init_rs.contains("static MI_SUBPROC_MAIN_KEEP")
            && init_rs.contains("static MI_SUBPROC_FROM_ID_KEEP")
            && init_rs.contains("static MI_SUBPROC_HEAP_MAIN_KEEP")
            && prim_rs.contains("pub unsafe fn _mi_theap_default_raw()"),
        "subproc helpers should stay as C's small local helper boundaries and inspect the raw default TLS cell"
    );
    let subproc_main_start = init_rs
        .find("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc_main")
        .expect("find _mi_subproc_main");
    let subproc_main_end = init_rs[subproc_main_start..]
        .find("#[used]\nstatic MI_SUBPROC_MAIN_KEEP")
        .map(|offset| subproc_main_start + offset)
        .expect("find _mi_subproc_main keep marker");
    let subproc_main_body = &init_rs[subproc_main_start..subproc_main_end];
    assert!(
        subproc_main_body.contains("unsafe { mi_subproc_main_addr_typed().ptr().as_ptr() }")
            && !subproc_main_body.contains("mi_subproc_main_init()"),
        "_mi_subproc_main should use the typed main-subproc helper without process initialization"
    );
    let subproc_start = init_rs
        .find("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc()")
        .expect("find _mi_subproc");
    let subproc_end = init_rs[subproc_start..]
        .find("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc_from_id")
        .map(|offset| subproc_start + offset)
        .expect("find _mi_subproc_from_id marker");
    let subproc_body = &init_rs[subproc_start..subproc_end];
    assert!(
        subproc_body.contains("unsafe { _mi_subproc_typed().ptr().as_ptr() }")
            && subproc_body.contains("let subproc = if theap.is_null() {")
            && subproc_body.contains("unsafe { mi_subproc_main_addr_typed().ptr() }")
            && subproc_body.contains("let tld = unsafe { TldPtr::new_unchecked((*theap).tld) };")
            && subproc_body.contains("unsafe { tld.subproc() }")
            && subproc_body.contains("unsafe { BorrowedSubproc::new(subproc) }")
            && !subproc_body.contains("_mi_subproc_main()")
            && !subproc_body.contains("mi_subproc_main_init()"),
        "_mi_subproc should parse the raw default theap into typed subproc wrappers without process initialization"
    );
    let mi_heap_main_start = init_rs
        .find("pub unsafe extern \"C\" fn mi_heap_main")
        .expect("find mi_heap_main");
    let mi_heap_main_end = init_rs[mi_heap_main_start..]
        .find("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_is_heap_main")
        .map(|offset| mi_heap_main_start + offset)
        .expect("find _mi_is_heap_main marker");
    let mi_heap_main_body = &init_rs[mi_heap_main_start..mi_heap_main_end];
    assert!(
        mi_heap_main_body.contains("crate::prim::_mi_theap_default_raw()")
            && mi_heap_main_body.contains("unsafe { mi_subproc_main_addr() }")
            && mi_heap_main_body
                .contains("let mut heap = unsafe { mi_subproc_heap_main_raw(subproc) };")
            && mi_heap_main_body.contains("mi_heap_main_init();")
            && mi_heap_main_body.contains("crate::mi_assert_internal!(!heap.is_null());")
            && !mi_heap_main_body.contains("_mi_subproc()")
            && !mi_heap_main_body.contains("_mi_subproc_heap_main"),
        "mi_heap_main should match C's direct raw-default-theap and subproc->heap_main fast path"
    );
    let is_heap_main_start = init_rs
        .find("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_is_heap_main")
        .expect("find _mi_is_heap_main");
    let is_heap_main_end = init_rs[is_heap_main_start..]
        .find("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_is_theap_main")
        .map(|offset| is_heap_main_start + offset)
        .expect("find _mi_is_theap_main marker");
    let is_heap_main_body = &init_rs[is_heap_main_start..is_heap_main_end];
    assert!(
        is_heap_main_body.contains("let subproc = unsafe { mi_heap_subproc_raw(heap) };")
            && is_heap_main_body
                .contains("let mut main_heap = unsafe { mi_subproc_heap_main_raw(subproc) };")
            && is_heap_main_body.contains("mi_heap_main_init();")
            && is_heap_main_body
                .contains("main_heap = unsafe { mi_subproc_heap_main_raw(subproc) };")
            && !is_heap_main_body.contains("_mi_subproc_heap_main")
            && !is_heap_main_body.contains("if heap.is_null()"),
        "_mi_is_heap_main should match C's valid-heap contract and inline subproc->heap_main load"
    );
    let is_theap_main_start = init_rs
        .find("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_is_theap_main")
        .expect("find _mi_is_theap_main");
    let is_theap_main_end = init_rs[is_theap_main_start..]
        .find("#[used]\nstatic MI_IS_THEAP_MAIN_KEEP")
        .map(|offset| is_theap_main_start + offset)
        .expect("find MI_IS_THEAP_MAIN_KEEP marker");
    let is_theap_main_body = &init_rs[is_theap_main_start..is_theap_main_end];
    assert!(
        is_theap_main_body.contains("crate::internal::mi_theap_is_initialized(theap)")
            && is_theap_main_body
                .contains("let heap = unsafe { crate::internal::_mi_theap_heap(theap) };")
            && is_theap_main_body.contains("let subproc = unsafe { mi_heap_subproc_raw(heap) };")
            && is_theap_main_body
                .contains("let mut main_heap = unsafe { mi_subproc_heap_main_raw(subproc) };")
            && is_theap_main_body.contains("mi_heap_main_init();")
            && !is_theap_main_body.contains("_mi_is_heap_main")
            && !is_theap_main_body.contains("_mi_subproc_heap_main"),
        "_mi_is_theap_main should match C's nullable-theap guard and inline subproc->heap_main load"
    );
    let mi_thread_init_start = init_rs
        .find("pub unsafe extern \"C\" fn mi_thread_init")
        .expect("find mi_thread_init");
    let mi_thread_init_end = init_rs[mi_thread_init_start..]
        .find("#[unsafe(no_mangle)]\n/// # Safety\n/// The returned opaque identifier")
        .map(|offset| mi_thread_init_start + offset)
        .expect("find mi_subproc_new marker");
    let mi_thread_init_body = &init_rs[mi_thread_init_start..mi_thread_init_end];
    assert!(
        mi_thread_init_body.contains("let heap = unsafe { mi_heap_main() };")
            && mi_thread_init_body.contains("__mi_stat_increase_mt(")
            && !mi_thread_init_body.contains("if !heap.is_null()"),
        "mi_thread_init should trust mi_heap_main after thread-heap initialization like C"
    );
    let subproc_heap_main_start = init_rs
        .find("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc_heap_main")
        .expect("find _mi_subproc_heap_main");
    let subproc_heap_main_end = init_rs[subproc_heap_main_start..]
        .find("#[used]\nstatic MI_SUBPROC_HEAP_MAIN_KEEP")
        .map(|offset| subproc_heap_main_start + offset)
        .expect("find _mi_subproc_heap_main keep marker");
    let subproc_heap_main_body = &init_rs[subproc_heap_main_start..subproc_heap_main_end];
    assert!(
        subproc_heap_main_body.contains(
            "let subproc = unsafe { BorrowedSubproc::new(SubprocPtr::new_unchecked(subproc)) };"
        ) && subproc_heap_main_body
            .contains("unsafe { _mi_subproc_heap_main_typed(subproc).ptr().as_ptr() }")
            && subproc_heap_main_body
                .contains("let heap = unsafe { mi_subproc_heap_main_typed(subproc) };")
            && !subproc_heap_main_body.contains("subproc.is_null()")
            && !subproc_heap_main_body.contains("ptr::null_mut()"),
        "_mi_subproc_heap_main should enter the typed subproc wrapper path without a Rust-only null fast-path branch"
    );
    let thread_done_start = init_rs
        .find("pub(crate) unsafe extern \"C\" fn _mi_thread_done")
        .expect("find _mi_thread_done");
    let thread_done_end = init_rs[thread_done_start..]
        .find("#[inline(never)]\npub(crate) extern \"C\" fn _mi_preloading")
        .map(|offset| thread_done_start + offset)
        .expect("find _mi_preloading marker");
    let thread_done_body = &init_rs[thread_done_start..thread_done_end];
    assert!(
        thread_done_body.contains("let subproc = unsafe { (*tld).subproc };")
            && thread_done_body
                .contains("let mut heap = unsafe { mi_subproc_heap_main_raw(subproc) };")
            && thread_done_body.contains("mi_heap_main_init();")
            && thread_done_body.contains("heap = unsafe { mi_subproc_heap_main_raw(subproc) };")
            && !thread_done_body.contains("_mi_subproc_heap_main((*tld).subproc)")
            && !thread_done_body.contains("let heap = unsafe { _mi_subproc_heap_main"),
        "_mi_thread_done should match C's inline subproc->heap_main load instead of calling _mi_subproc_heap_main"
    );
    assert!(
        threadlocal_rs.contains("#[inline(never)]\npub extern \"C\" fn _mi_thread_locals_init(")
            && threadlocal_rs.contains("static mut mi_thread_locals_lock: mi_lock_t")
            && threadlocal_rs.contains("static mi_thread_locals_initialized: AtomicBool")
            && threadlocal_rs
                .contains(".compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)")
            && threadlocal_rs
                .contains("#[inline(never)]\npub extern \"C\" fn _mi_thread_locals_done(")
            && threadlocal_rs
                .contains("#[inline(never)]\npub extern \"C\" fn _mi_thread_local_set(")
            && threadlocal_rs
                .contains("#[inline(never)]\npub extern \"C\" fn _mi_thread_local_get(")
            && threadlocal_rs
                .contains("#[inline(never)]\npub extern \"C\" fn _mi_thread_locals_thread_done(")
            && threadlocal_rs
                .contains("#[inline(never)]\npub extern \"C\" fn _mi_thread_local_free(")
            && threadlocal_rs.contains("static MI_THREAD_LOCALS_INIT_KEEP")
            && threadlocal_rs.contains("static MI_THREAD_LOCALS_DONE_KEEP")
            && threadlocal_rs.contains("static MI_THREAD_LOCAL_SET_KEEP")
            && threadlocal_rs.contains("static MI_THREAD_LOCAL_GET_KEEP")
            && threadlocal_rs.contains("static MI_THREAD_LOCALS_THREAD_DONE_KEEP")
            && threadlocal_rs.contains("static MI_THREAD_LOCAL_FREE_KEEP")
            && threadlocal_rs.contains("pub extern \"C\" fn _mi_thread_local_create()")
            && threadlocal_rs.contains("_mi_thread_locals_init();"),
        "dynamic thread-local helpers should keep C's local helper boundaries"
    );
    let thread_locals_thread_done_start = threadlocal_rs
        .find("#[inline(never)]\npub extern \"C\" fn _mi_thread_locals_thread_done")
        .expect("find _mi_thread_locals_thread_done");
    let thread_locals_thread_done_end = threadlocal_rs[thread_locals_thread_done_start..]
        .find("#[used]\nstatic MI_THREAD_LOCALS_THREAD_DONE_KEEP")
        .map(|offset| thread_locals_thread_done_start + offset)
        .expect("find _mi_thread_locals_thread_done keep marker");
    let thread_locals_thread_done_body =
        &threadlocal_rs[thread_locals_thread_done_start..thread_locals_thread_done_end];
    assert!(
        thread_locals_thread_done_body.contains("let tls = thread_locals_ptr();")
            && thread_locals_thread_done_body.contains("!tls.is_null() && (*tls).count > 0")
            && thread_locals_thread_done_body.contains("mi_free(tls.cast());")
            && thread_locals_thread_done_body
                .contains("set_thread_locals_ptr(thread_locals_empty_ptr());")
            && !thread_locals_thread_done_body.contains("free_thread_locals("),
        "_mi_thread_locals_thread_done should match C's direct mi_free after the count guard"
    );
    assert!(
        init_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_cached_set(")
            && init_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_default_safe("
            )
            && init_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_main_safe("
            )
            && init_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_empty_get("
            )
            && init_rs.contains("static MI_THEAP_CACHED_SET_KEEP")
            && init_rs.contains("static MI_THEAP_DEFAULT_SAFE_KEEP")
            && init_rs.contains("static MI_THEAP_MAIN_SAFE_KEEP")
            && init_rs.contains("static MI_THEAP_EMPTY_GET_KEEP"),
        "thread-heap accessors should keep C's local helper boundaries"
    );
    assert!(
        init_rs.contains("#[inline(never)]\npub(crate) extern \"C\" fn _mi_thread_id(")
            && init_rs.contains("static MI_THREAD_ID_KEEP"),
        "_mi_thread_id should keep C's local helper boundary"
    );
    assert!(
        alloc_rs.contains(
            "#[inline(always)]\npub(crate) unsafe extern \"C\" fn _mi_theap_malloc_zero_ex"
        ) && alloc_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_malloc_zero")
            && alloc_rs.contains("static MI_THEAP_MALLOC_ZERO_EX_KEEP")
            && alloc_rs.contains("static MI_THEAP_MALLOC_ZERO_KEEP"),
        "_mi_theap_malloc_zero_ex should keep C's force-inline body and both allocation helpers should retain local definitions"
    );
    assert!(
        alloc_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn mi_theap_try_new")
            && alloc_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_try_new")
            && alloc_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_heap_try_new")
            && alloc_rs.contains("mi_heap_try_new_typed(heap, size, false)")
            && alloc_rs.contains("mi_try_new(size, true)")
            && alloc_rs.contains("static MI_THEAP_TRY_NEW_KEEP")
            && alloc_rs.contains("static MI_TRY_NEW_KEEP")
            && alloc_rs.contains("static MI_HEAP_TRY_NEW_KEEP"),
        "C++ new retry paths should keep C's separate theap/default-heap/explicit-heap helper boundaries"
    );
    assert!(
        page_rs.contains(
            "#[inline(never)]\npub(crate) unsafe fn _mi_theap_page_reclaim(\n    page: crate::core_types::OwnedAbandonedPage,\n    target: crate::core_types::ThreadHeapLocal,"
        )
            && page_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_theap_page_reclaim_raw")
            && page_rs.contains("static MI_THEAP_PAGE_RECLAIM_KEEP")
            && page_rs.contains("mi_theap_page_reclaim_raw;")
            && page_rs.contains("_mi_theap_page_reclaim(wrapper, theap_wrapper);")
            && page_rs
                .contains("let wrapper = crate::core_types::OwnedAbandonedPage::new_unchecked(page_ptr);")
            && !page_rs.contains("pub unsafe extern \"C\" fn _mi_theap_page_reclaim")
            && atomic_types.contains("pub(crate) unsafe fn load(self) -> PageThreadId")
            && atomic_types.contains("pub(crate) unsafe fn store_owner(self, owner: PageOwnerId)")
            && atomic_types.contains("pub(crate) unsafe fn compare_exchange_owner(\n        self,\n        expected: &mut PageThreadId,")
            && internal.contains("let atomic_tid = crate::core_types::AtomicPageThreadId(\n            crate::core_types::AtomicUsizeField(")
            && internal.contains("atomic_tid.load().raw()")
            && internal.contains("let mut old = atomic_tid.load();")
            && internal.contains("atomic_tid.compare_exchange_owner(&mut old, new)")
            && internal.contains("let new = old.with_owner(crate::core_types::PageOwnerId::AbandonedMapped);")
            && internal.contains("let new = old.with_owner(crate::core_types::PageOwnerId::Abandoned);")
            && page_rs.contains("let Some(flag) = crate::core_types::PageFlags::new(newflag) else")
            && page_rs.contains("let old = atomic_tid.set_flags(flag, set);")
            && page_rs.contains("let tid = atomic_tid.load();")
            && page_rs.contains("let mut xtid_old = atomic_tid.load();")
            && page_rs.contains("let xtid = xtid_old.with_owner(owner);")
            && page_rs.contains("atomic_tid.compare_exchange_owner(&mut xtid_old, xtid)")
            && arena_rs.contains("unsafe { atomic_tid.store_owner(crate::core_types::PageOwnerId::AbandonedMapped) };")
            && !internal.contains("atomic_tid.load_relaxed().raw()")
            && !internal.contains("atomic_tid.compare_exchange_release(&mut old, new)")
            && !page_rs.contains("atomic_tid.compare_exchange_release(&mut xtid_old, xtid)")
            && !arena_rs.contains("atomic_tid.set_owner_release(crate::core_types::PageOwnerId::AbandonedMapped)")
            && !internal.contains("mi_atomic_or_relaxed(&mut (*page).xthread_id")
            && !internal.contains("mi_atomic_and_relaxed(&mut (*page).xthread_id")
            && !page_rs.contains("mi_atomic_cas_weak_release(&mut (*page).xthread_id")
            && !page_rs.contains("mi_atomic_or_relaxed(&mut (*page).xthread_id")
            && !page_rs.contains("mi_atomic_and_relaxed(&mut (*page).xthread_id")
            && !arena_rs.contains("MI_THREADID_ABANDONED_MAPPED | flags")
            && page_rs.contains(
                "let atomic_tf = crate::core_types::AtomicThreadFree(crate::core_types::AtomicUsizeField(\n            ptr::addr_of_mut!((*page).xthread_free),\n        ));"
            )
            && page_rs.contains("let remote = atomic_tf.collect_acq_rel();")
            && page_rs.contains("remote.collect_into_local(page_wrapper, local);")
            && !page_rs.contains("atomic_tf.take_head_preserving_owner()")
            && atomic_types.contains("pub(crate) fn push_remote_acq_rel(")
            && atomic_types.contains(
                "pub(crate) unsafe fn collect_acq_rel(self) -> RemoteFreeList"
            )
            && atomic_types.contains("pub(crate) unsafe fn claim_acq_rel(self) -> bool")
            && atomic_types.contains(
                "pub(crate) unsafe fn release_acq_rel(self, expected: &mut ThreadFreeWord) -> bool"
            )
            && atomic_types.contains("fn field(self) -> AtomicUsizeField")
            && atomic_types.contains("self.field().compare_exchange_weak(")
            && atomic_types.contains("self.field().fetch_or(1, Ordering::AcqRel)")
            && atomic_types.contains("AtomicUsize::from_ptr(self.0)")
            && !atomic_types.contains("self.0.cast::<core::sync::atomic::AtomicUsize>()")
            && !atomic_types.contains("as_atomic(")
            && !atomic_types.contains("atomic_ref(")
            && !atomic_types.contains("-> &'static Atomic")
            && atomic_types.contains("Atomic access to a raw `usize` field.")
            && atomic_types.contains("Atomic access to a raw `i64` field.")
            && atomic_types.contains("Atomic access to a raw pointer field.")
            && internal.contains("crate::core_types::ThreadFreeWord(tf).head_raw()")
            && internal.contains("crate::core_types::ThreadFreeWord(tf).is_owned()")
            && internal.contains("crate::core_types::ThreadFreeWord::from_parts(")
            && internal.contains("pub unsafe fn mi_page_thread_free(page: *const mi_page_t) -> *mut mi_block_t")
            && internal.contains("atomic_tf.load_relaxed().head_raw()")
            && internal.contains("pub unsafe fn mi_page_is_owned(page: *const mi_page_t) -> bool")
            && internal.contains("atomic_tf.load_relaxed().is_owned()")
            && internal.contains("let atomic_tf = crate::core_types::AtomicThreadFree(crate::core_types::AtomicUsizeField(\n            ptr::addr_of_mut!((*page).xthread_free),")
            && internal.contains("atomic_tf.claim_acq_rel()")
            && !internal.contains("atomic_tf.claim_ownership()")
            && arena_rs.contains("(*page).xthread_free = crate::core_types::ThreadFreeWord::from_parts(\n            None,\n            crate::core_types::ThreadFreeOwner::Unowned,")
            && !arena_rs.contains("(*page).xthread_free = 0")
            && free_rs.contains(
                "let atomic_tf = crate::core_types::AtomicThreadFree(crate::core_types::AtomicUsizeField(\n            ptr::addr_of_mut!((*page).xthread_free),\n        ));"
            )
            && free_rs.contains("match atomic_tf.push_remote_acq_rel(&mut tf_old, bp)")
            && free_rs.contains("while !atomic_tf.release_acq_rel(&mut tf_expect)")
            && free_rs.contains("tf_expect = atomic_tf.load_relaxed();")
            && !free_rs.contains("atomic_tf.compare_exchange_push(")
            && !free_rs.contains("atomic_tf.compare_exchange_release_ownership(")
            && !free_rs.contains("mi_tf_create(expected_thread_free")
            && arena_rs.contains(
                "let atomic_tf =\n        crate::core_types::AtomicThreadFree(crate::core_types::AtomicUsizeField(unsafe {\n            ptr::addr_of_mut!((*page).xthread_free)\n        }));"
            )
            && arena_rs.contains("let mut tf_old = atomic_tf.load_relaxed();")
            && arena_rs.contains("while tf_old.head().is_some()")
            && arena_rs.contains("if unsafe { atomic_tf.release_acq_rel(&mut tf_old) }")
            && !arena_rs.contains("atomic_tf.compare_exchange_release_ownership(")
            && !arena_rs.contains("mi_tf_create(ptr::null_mut(), false)")
            && page_rs.contains(
                "unsafe fn mi_page_queue_push_at_end_typed(\n    theap: crate::core_types::ThreadHeapLocal,\n    queue: crate::core_types::NormalPageQueue,\n    page: crate::core_types::ActivePage,"
            )
            && page_rs.contains(
                "mi_page_queue_push_at_end_typed(\n            crate::core_types::ThreadHeapLocal::new_unchecked"
            )
            && page_rs.contains("crate::core_types::NormalPageQueue")
            && page_rs.contains("crate::core_types::ActivePage"),
        "_mi_theap_page_reclaim should keep C's local abandoned-page reclaim helper boundary while collecting remote frees through typed wrappers"
    );
    assert!(
        page_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_page_free_list_extend")
            && page_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_page_extend_free")
            && page_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn mi_page_fresh_alloc")
            && page_rs.contains("static MI_PAGE_FREE_LIST_EXTEND_KEEP")
            && page_rs.contains("static MI_PAGE_EXTEND_FREE_KEEP")
            && page_rs.contains("static MI_PAGE_FRESH_ALLOC_KEEP")
            && page_rs.contains("mi_page_free_list_extend(page, bsize, extend);")
            && page_rs.contains("if !mi_page_extend_free(theap, page)")
            && page_rs.contains("let block_size = mi_page_queue_block_size_raw(pq);")
            && page_rs.contains("let page = mi_page_fresh_alloc(theap, pq, block_size, 0);")
            && page_rs
                .contains("crate::mi_assert_internal!(block_size == mi_page_block_size(page));")
            && page_rs
                .contains("let page = mi_page_fresh_alloc(theap, pq, block_size, page_alignment);"),
        "fresh-page allocation and page free-list extension should keep C's local helper boundaries"
    );
    assert!(
        free_rs.contains(
            "unsafe fn mi_abandoned_page_try_reclaim(\n    page: crate::core_types::OwnedAbandonedPage,"
        )
            && free_rs.contains(
                "#[inline(never)]\npub unsafe extern \"C\" fn mi_page_usable_aligned_size_of"
            )
            && free_rs.contains("static MI_ABANDONED_PAGE_TRY_RECLAIM_KEEP")
            && free_rs.contains("static MI_PAGE_USABLE_ALIGNED_SIZE_OF_KEEP")
            && free_rs.contains("&& mi_abandoned_page_try_reclaim(page, reclaim_on_free)")
            && free_rs.contains("_mi_theap_page_reclaim(\n            page,")
            && free_rs.contains("crate::core_types::OwnedAbandonedPage::new_unchecked")
            && free_rs.contains("unsafe fn mi_free_try_collect_mt_typed")
            && !free_rs.contains("_mi_theap_page_reclaim(theap, page)")
            && free_rs.contains("mi_page_usable_aligned_size_of_typed(page, p)"),
        "abandoned-page reclaim should route internal abandoned-page collection through typed page/heap witnesses"
    );
    assert!(
        theap_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_area_visit_blocks"
        ) && theap_rs.contains("static MI_THEAP_AREA_VISIT_BLOCKS_KEEP")
            && theap_rs.contains("visitor: BlockVisitCallback")
            && theap_rs.contains("arg: BlockVisitCallbackArg")
            && theap_rs.contains("let heap = HeapPtr::new_unchecked(mi_page_heap(page));")
            && callbacks_rs
                .contains("pub(crate) unsafe fn invoke(self, heap: HeapPtr, arg: HeapVisitCallbackArg) -> bool")
            && callbacks_rs.contains("heap: HeapPtr,\n        area: *const mi_heap_area_t,")
            && callbacks_rs.contains("fun(heap.as_ptr(), area, block, block_size, arg.as_ptr())")
            && init_rs.contains("ok = visitor.invoke(HeapPtr::new_unchecked(heap), arg);")
            && theap_rs.contains("mi_get_fast_divisor(bsize, &mut magic, &mut shift);")
            && arena_rs.contains(
                "_mi_theap_area_visit_blocks(ptr::addr_of!(area), page, (*vinfo).visitor, (*vinfo).arg)"
            )
            && !arena_rs.contains("unsafe fn mi_theap_area_visit_blocks("),
        "heap and arena block visitation should share C's theap area block visitor helper"
    );
    assert!(
        theap_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_theap_area_visitor")
            && theap_rs.contains(
                "static MI_THEAP_AREA_VISITOR_KEEP: mi_theap_area_visit_fun = mi_theap_area_visitor;"
            )
            && theap_rs.contains("Some(mi_theap_area_visitor)"),
        "mi_theap_area_visitor should keep C's local theap area visitor callback boundary"
    );
    assert!(
        arena_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_page_alloc")
            && arena_rs.contains("static MI_ARENAS_PAGE_ALLOC_KEEP")
            && page_rs.contains(
                "let page = _mi_arenas_page_alloc(theap, block_size, page_alignment.get());",
            ),
        "arena page allocation should keep C's _mi_arenas_page_alloc local helper boundary"
    );
    assert!(
        stats_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_heap_print_visitor")
            && stats_rs
                .contains("#[inline(never)]\nunsafe extern \"C\" fn mi_heap_aggregate_visitor")
            && stats_rs.contains("static MI_HEAP_PRINT_VISITOR_KEEP: mi_heap_visit_fun = mi_heap_print_visitor;")
            && stats_rs.contains(
                "static MI_HEAP_AGGREGATE_VISITOR_KEEP: mi_heap_visit_fun = mi_heap_aggregate_visitor;"
            )
            && stats_rs.contains("Some(mi_heap_print_visitor as mi_heap_visit_fun)")
            && stats_rs.contains("Some(mi_heap_aggregate_visitor as mi_heap_visit_fun)"),
        "heap stats visitors should keep C's local callback helper boundaries"
    );
    assert!(
        prim_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn mi_process_attach()")
            && prim_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn mi_process_detach()")
            && prim_rs.contains(
                "pub(crate) static _MI_PROCESS_ATTACH_CTOR: unsafe extern \"C\" fn() = mi_process_attach;"
            )
            && prim_rs.contains(
                "pub(crate) static _MI_PROCESS_DETACH_DTOR: unsafe extern \"C\" fn() = mi_process_detach;"
            ),
        "ELF process lifecycle hooks should keep C's constructor and destructor helper names"
    );
    assert!(
        theap_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_incref")
            && theap_rs.contains("unsafe fn _mi_theap_incref_typed(theap: ThreadHeapPtr)")
            && theap_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_decref")
            && theap_rs.contains("#[inline(always)]\nunsafe fn mi_theap_decref_inline(theap: ThreadHeapPtr)")
            && theap_rs.contains("mi_theap_decref_inline(ThreadHeapPtr::new_unchecked(theap));\n        true")
            && theap_rs.contains("unsafe fn _mi_theap_delete_typed(theap: ThreadHeapPtr, acquire_tld_theaps_lock: bool)")
            && theap_rs.contains("match theap.state()")
            && theap_rs.contains("ThreadHeapState::Initialized(theap) | ThreadHeapState::Main(theap)")
            && theap_rs.contains("ThreadHeapState::Uninitialized(_) | ThreadHeapState::Deleted(_) => return")
            && !theap_rs.contains("    _mi_theap_decref(theap);\n    true"),
        "_mi_theap_free should inline C's decref tail while incref/decref/delete use typed thread-heap wrappers below their raw shims"
    );
    assert!(
        theap_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_random_next")
            && theap_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_delete")
            && theap_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_create")
            && !theap_rs.contains("static MI_THEAP_RANDOM_NEXT_KEEP")
            && theap_rs.contains("static MI_THEAP_CREATE_KEEP")
            && theap_rs.contains("static MI_THEAP_DELETE_KEEP"),
        "theap random/create/delete helpers should keep C's local helper boundaries without a random-next function pointer anchor"
    );
    assert!(
        heap_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_heap_init_theap")
            && heap_rs.contains("static MI_HEAP_INIT_THEAP_KEEP")
            && heap_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_heap_theap_get_or_init"
            )
            && heap_rs.contains("static MI_HEAP_THEAP_GET_OR_INIT_KEEP")
            && heap_rs.contains("theap = unsafe { mi_heap_init_theap(heap) };"),
        "heap theap initialization should keep C's local helper boundaries"
    );
    assert!(
        init_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_options_init")
            && init_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_guarded_init"
            )
            && init_rs.contains("core::sync::atomic::compiler_fence"),
        "_mi_theap_init should keep C's options and guarded-init helper call boundaries"
    );
    assert!(
        init_rs.contains("#[inline(always)]\nunsafe fn _mi_thread_init_theap_default()"),
        "thread default-heap initialization should stay inlined like C's static helper"
    );
    let thread_default_start = init_rs
        .find("#[inline(always)]\nunsafe fn _mi_thread_init_theap_default()")
        .expect("find _mi_thread_init_theap_default");
    let thread_default_end = init_rs[thread_default_start..]
        .find("\n}\n\n#[unsafe(no_mangle)]")
        .map(|offset| thread_default_start + offset)
        .expect("find end of _mi_thread_init_theap_default");
    let thread_default_body = &init_rs[thread_default_start..thread_default_end];
    assert!(
        thread_default_body.contains("if mi_is_main_thread_fast() {")
            && thread_default_body.contains("let tld = mi_tld_alloc_non_main(")
            && !thread_default_body.contains("_mi_is_main_thread()"),
        "thread default-heap initialization should inline C's main-thread predicate body and avoid a second main-thread check on the non-main branch"
    );
    assert!(
        init_rs.contains("crate::threadlocal::_mi_thread_locals_init();")
            && init_rs.find("crate::threadlocal::_mi_thread_locals_init();")
                < init_rs.find("let _ = crate::page_map::_mi_page_map_init();"),
        "process initialization should initialize dynamic TLS locks before page-map/TLS key creation"
    );
    let tld_alloc_start = init_rs
        .find("#[inline(always)]\nfn mi_tld_alloc()")
        .expect("find mi_tld_alloc");
    let tld_alloc_end = init_rs[tld_alloc_start..]
        .find("\n}\n\n#[inline(never)]")
        .map(|offset| tld_alloc_start + offset)
        .expect("find end of mi_tld_alloc");
    let tld_alloc_body = &init_rs[tld_alloc_start..tld_alloc_end];
    assert!(
        tld_alloc_body.contains("ptr::addr_of_mut!(subproc_main)")
            && !tld_alloc_body.contains("mi_subproc_main_init()")
            && !tld_alloc_body.contains("mi_subproc_main_ptr()"),
        "thread-local data allocation should use the initialized main subproc directly like C's mi_tld_alloc"
    );
    assert!(
        prim_rs.contains("pub(crate) use unix::{")
            && !prim_rs.contains("pub fn _mi_prim_mem_init(")
            && !prim_rs.contains("pub fn _mi_prim_alloc(")
            && !prim_rs.contains("pub fn _mi_prim_thread_yield(")
            && !prim_rs.contains("pub fn _mi_theap_empty_get(")
            && !prim_rs.contains("pub fn _mi_heap_theap_get_or_init(")
            && !prim_rs.contains("pub fn _mi_heap_theap_get_peek("),
        "Unix primitive helpers should be routed through crate paths, not extern declarations"
    );
    assert!(
        !init_rs.contains("#[unsafe(no_mangle)]\npub static _mi_page_empty"),
        "_mi_page_empty should stay crate-local so page-empty tables use relative relocations"
    );
    assert!(
        init_rs.contains("#[unsafe(no_mangle)]\npub(crate) static _mi_theap_empty")
            && init_rs.contains("core::arch::global_asm!(\".hidden _mi_theap_empty\")")
            && !init_rs.contains("#[unsafe(no_mangle)]\npub static _mi_theap_empty"),
        "_mi_theap_empty needs a stable hidden local link name for TLS initialization without becoming a public dynamic ABI symbol"
    );
    assert!(
        init_rs.contains(
            "core::arch::global_asm!(\".hidden _mi_cpu_movsb_max\", \".hidden _mi_cpu_stosb_max\")"
        ) && init_rs.contains("#[unsafe(no_mangle)]\npub(crate) static mut _mi_cpu_movsb_max")
            && init_rs.contains("#[unsafe(no_mangle)]\npub(crate) static mut _mi_cpu_stosb_max")
            && !init_rs.contains("#[unsafe(no_mangle)]\npub static mut _mi_cpu_movsb_max")
            && !init_rs.contains("#[unsafe(no_mangle)]\npub static mut _mi_cpu_stosb_max"),
        "CPU copy thresholds should use stable hidden C names like C without becoming public dynamic ABI symbols"
    );
    assert!(
        init_rs.contains("fn mi_detect_cpu_features()")
            && init_rs.contains("if mi_cpuid(cpu_info.as_mut_ptr(), 0, 0)")
            && init_rs.contains("amd = cpu_info[2] == 0x444d_4163;")
            && init_rs.contains("if mi_cpuid(cpu_info.as_mut_ptr(), 7, 0)")
            && init_rs.contains("fsrm = (cpu_info[3] & (1 << 4)) != 0;")
            && init_rs.contains("if mi_cpuid(cpu_info.as_mut_ptr(), 7, 1)")
            && init_rs.contains("fsrs = (cpu_info[1] & (1 << 11)) != 0;")
            && init_rs.contains("if mi_cpuid(cpu_info.as_mut_ptr(), 1, 0)")
            && init_rs.contains("_mi_cpu_has_popcnt = (cpu_info[2] & (1 << 23)) != 0")
            && init_rs.contains("if fsrm {\n            unsafe { _mi_cpu_movsb_max = 127 };")
            && init_rs.contains(
                "if fsrs || (amd && fsrm) {\n            unsafe { _mi_cpu_stosb_max = 127 };"
            ),
        "CPU feature detection should initialize C's popcnt and short rep movsb/stosb thresholds"
    );
    assert!(
        internal.contains("\"rep stosb\"")
            && internal.contains("pub unsafe fn mi_rep_stosb(dst: *mut c_void, val: u8, n: usize)")
            && internal.contains("if n <= mi_cpu_stosb_max() {\n                mi_rep_stosb(dst, val as u8, n);\n                return;\n            }"),
        "_mi_memset should keep C's CPU-thresholded rep stosb shape instead of always lowering to external memset"
    );
    for internal_static in [
        "_mi_cpu_movsb_max",
        "_mi_cpu_stosb_max",
        "_mi_theap_default_slot",
        "_mi_theap_default_expansion_slot",
        "_mi_theap_cached_slot",
        "_mi_theap_cached_expansion_slot",
        "mi_tls_raw_index_default",
        "mi_tls_raw_index_cached",
        "_mi_theap_default_key",
        "_mi_theap_cached_key",
        "__mi_theap_main",
        "_mi_process_is_initialized",
        "_mi_cpu_has_popcnt",
    ] {
        assert!(
            !init_rs.contains(&format!(
                "#[unsafe(no_mangle)]\npub static mut {internal_static}"
            )),
            "{internal_static} should stay crate-local instead of a dynamic storage export"
        );
    }
    assert!(
        !prim_rs.contains("pub static mut __mi_theap_default")
            && !prim_rs.contains("pub static mut __mi_theap_cached"),
        "unused fallback TLS globals should not be exported as dynamic storage symbols"
    );
    assert!(
        !internal.contains("pub static mut _mi_cpu_movsb_max")
            && !internal.contains("pub static mut _mi_cpu_stosb_max")
            && !prim_rs.contains("pub static mut _mi_process_is_initialized")
            && !prim_rs.contains("pub static mut _mi_theap_default_slot")
            && !prim_rs.contains("pub static mut _mi_theap_cached_slot")
            && !prim_rs.contains("pub static mut _mi_theap_default_expansion_slot")
            && !prim_rs.contains("pub static mut _mi_theap_cached_expansion_slot")
            && !prim_rs.contains("pub static mut _mi_theap_default_key")
            && !prim_rs.contains("pub static mut _mi_theap_cached_key"),
        "internal storage should be routed through crate paths, not C-linkage extern statics"
    );
    for main_static in [
        "pub static _mi_theap_empty_wrong",
        "pub static mut theap_main",
        "pub static mut heap_main",
    ] {
        assert!(
            !init_rs.contains(main_static),
            "{main_static} should stay crate-local instead of a dynamic export"
        );
    }
    assert!(
        !free_rs.contains("#[unsafe(no_mangle)]\npub unsafe extern \"C\" fn _mi_free_generic"),
        "_mi_free_generic should not be a dynamic export"
    );
    assert!(
        free_rs.contains("#[inline(never)]")
            && free_rs.contains("pub(crate) unsafe extern \"C\" fn _mi_free_generic(")
            && free_rs.contains("if is_local {\n            mi_free_generic_local(page, p);")
            && free_rs.contains("} else {\n            mi_free_generic_mt(page, p);")
            && free_rs.contains("static MI_FREE_GENERIC_KEEP")
            && free_rs.contains("    _mi_free_generic;"),
        "_mi_free_generic should keep C's local runtime-integration dispatch boundary"
    );
    assert!(
        !page_rs.contains("#[unsafe(no_mangle)]\npub unsafe extern \"C\" fn _mi_page_retire"),
        "_mi_page_retire should not be a dynamic export"
    );
    assert!(
        !page_rs.contains("#[unsafe(no_mangle)]\npub unsafe extern \"C\" fn _mi_malloc_generic"),
        "_mi_malloc_generic should not be a dynamic export"
    );
}

#[test]
fn generic_malloc_refill_helper_is_force_inlined() {
    let page_rs =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/allocator/page.rs"))
            .expect("read src/allocator/page.rs");

    assert!(
        page_rs.contains("#[inline(always)]\nunsafe fn mi_find_page("),
        "mi_find_page should stay force-inlined into _mi_malloc_generic like C's generic path"
    );
}

#[test]
fn release_page_stat_writes_follow_c_mimalloc_gates() {
    let page_rs =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/allocator/page.rs"))
            .expect("read src/allocator/page.rs");

    for (stat, needle) in [
        (
            "pages_retire",
            "if MI_STAT > 0 {\n                __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_retire), 1);",
        ),
        (
            "pages_extended",
            "if MI_STAT > 0 {\n            __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_extended), 1);",
        ),
        (
            "page_committed",
            "if MI_STAT > 0 {\n            __mi_stat_increase(\n                ptr::addr_of_mut!((*theap).stats.page_committed),",
        ),
    ] {
        assert!(
            page_rs.contains(needle),
            "{stat} updates should stay behind MI_STAT > 0 like C mimalloc"
        );
    }
}

#[test]
fn public_option_domain_is_transparent_not_rust_enum() {
    let options_rs =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/allocator/options.rs"))
            .expect("read src/allocator/options.rs");

    assert!(
        options_rs.contains("#[repr(transparent)]\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub struct mi_option_t(i32);")
            && options_rs.contains("pub const fn from_raw(raw: i32) -> Self")
            && options_rs.contains("pub const fn as_i32(self) -> i32")
            && options_rs.contains("pub type mi_option_e = mi_option_t;")
            && !options_rs.contains("pub enum mi_option_e")
            && !options_rs.contains("transmute::<i32, mi_option_t>"),
        "public mi_option_t must allow unknown C integer values without Rust enum UB"
    );
}
