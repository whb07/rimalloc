use std::fs;
#[cfg(all(target_os = "linux", not(debug_assertions)))]
use std::mem::align_of;
use std::mem::{offset_of, size_of};
use std::path::Path;

#[cfg(all(target_os = "linux", not(debug_assertions)))]
use mechanical_mimalloc::atomic::mi_lock_t;
use mechanical_mimalloc::types::mi_page_t;
#[cfg(not(feature = "guarded"))]
use mechanical_mimalloc::types::mi_theap_t;

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

#[test]
fn debug_assert_macros_are_release_gated_like_c_mimalloc() {
    let internal_rs = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/internal.rs");
    let source = fs::read_to_string(internal_rs).expect("read src/internal.rs");

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
    let page_rs = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/page.rs");
    let source = fs::read_to_string(page_rs).expect("read src/page.rs");
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
            "if MI_STAT > 0 {\n            __mi_stat_increase(\n                ptr::addr_of_mut!((*theap).stats.malloc_huge),"
        ) && body.contains(
            "__mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.malloc_huge_count), 1);"
        ),
        "release huge allocation stats must compile out when MI_STAT == 0, matching C mi_theap_stat_* macros"
    );
}

#[test]
fn preloading_state_matches_c_mimalloc_lifecycle() {
    let init_rs = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/init.rs");
    let source = fs::read_to_string(init_rs).expect("read src/init.rs");
    let preload_fn = source
        .find("pub(crate) extern \"C\" fn _mi_preloading() -> bool")
        .expect("find _mi_preloading");
    let thread_init = source
        .find("pub unsafe extern \"C\" fn mi_thread_init()")
        .expect("find mi_thread_init");
    let auto_init = source
        .find("pub(crate) unsafe extern \"C\" fn _mi_auto_process_init()")
        .expect("find _mi_auto_process_init");
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
        source[preload_fn..].contains("OS_PRELOADING.load(Ordering::Relaxed)")
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
        source[process_init..process_done].contains("mi_process_init_inner()")
            && !source[process_init..process_done].contains("_mi_auto_process_init()"),
        "mi_process_init should not clear preloading; only _mi_auto_process_init should"
    );
    assert!(
        source[process_done..].contains("OS_PRELOADING.store(true, Ordering::Relaxed);"),
        "mi_process_done should restore preloading after allocator teardown"
    );
}

#[test]
fn stderr_output_routes_through_platform_primitive() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let options_rs = fs::read_to_string(root.join("src/options.rs")).expect("read src/options.rs");
    let os_rs = fs::read_to_string(root.join("src/os.rs")).expect("read src/os.rs");
    let unix_rs = fs::read_to_string(root.join("src/prim/unix.rs")).expect("read src/prim/unix.rs");

    assert!(
        options_rs.contains("use crate::prim::_mi_prim_out_stderr;")
            && !options_rs.contains("use crate::os::_mi_prim_out_stderr;"),
        "default stderr output should route through the platform primitive layer"
    );
    assert!(
        unix_rs.contains("fn write(fd: c_int, buf: *const c_void, count: usize) -> ssize_t;")
            && unix_rs.contains("let n = write(")
            && unix_rs.contains("2,\n            bytes.as_ptr().add(written)")
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
    let alloc_rs = fs::read_to_string(root.join("src/alloc.rs")).expect("read src/alloc.rs");
    let atomic_rs = fs::read_to_string(root.join("src/atomic.rs")).expect("read src/atomic.rs");
    let free_rs = fs::read_to_string(root.join("src/free.rs")).expect("read src/free.rs");
    let init_rs = fs::read_to_string(root.join("src/init.rs")).expect("read src/init.rs");
    let libc_rs = fs::read_to_string(root.join("src/libc.rs")).expect("read src/libc.rs");
    let options_rs = fs::read_to_string(root.join("src/options.rs")).expect("read src/options.rs");
    let prim_rs = fs::read_to_string(root.join("src/prim.rs")).expect("read src/prim.rs");
    let random_rs = fs::read_to_string(root.join("src/random.rs")).expect("read src/random.rs");
    let stats_rs = fs::read_to_string(root.join("src/stats.rs")).expect("read src/stats.rs");

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
            && init_rs.contains("static SUBPROC_MAIN_INIT: mi_atomic_once_t")
            && init_rs.contains("static PROCESS_INIT: mi_atomic_once_t")
            && init_rs.contains("static TLS_SLOTS_INIT: mi_atomic_once_t")
            && init_rs.contains("mi_atomic_do_once_enter(ptr::addr_of!(SUBPROC_MAIN_INIT)")
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
            && options_rs.contains("mi_lock_acquire(addr_of_mut!(options_lock))")
            && options_rs.contains("mi_lock_release(addr_of_mut!(options_lock))")
            && options_rs.contains("mi_lock_acquire(addr_of_mut!(mi_error_handler_lock))")
            && options_rs.contains("mi_lock_release(addr_of_mut!(mi_error_handler_lock))")
            && options_rs.contains("#[link_name = \"abort\"]"),
        "options state should use mimalloc's internal lock and C abort primitives"
    );
    #[cfg(all(target_os = "linux", not(debug_assertions)))]
    {
        assert_eq!(
            size_of::<mi_lock_t>(),
            40,
            "Linux mi_lock_t should match the pthread_mutex_t-backed C layout"
        );
        assert_eq!(
            align_of::<mi_lock_t>(),
            align_of::<usize>(),
            "Linux mi_lock_t should keep pthread mutex alignment"
        );
    }
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
            && alloc_rs.contains("core::str::from_utf8_unchecked(CStr::from_ptr(msg).to_bytes())")
            && free_rs.contains("core::str::from_utf8_unchecked(CStr::from_ptr(msg).to_bytes())"),
        "pointer validation messages should borrow static C message names without lossy Rust decoding"
    );
}

#[test]
fn default_linux_malloc_override_uses_c_tls_fast_path() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = fs::read_to_string(root.join("Cargo.toml")).expect("read Cargo.toml");
    let lib_rs = fs::read_to_string(root.join("src/lib.rs")).expect("read src/lib.rs");
    let tls_shim = fs::read_to_string(root.join("src/tls_shim.c")).expect("read src/tls_shim.c");
    let build_rs = fs::read_to_string(root.join("build.rs")).expect("read build.rs");
    let aligned_rs =
        fs::read_to_string(root.join("src/alloc_aligned.rs")).expect("read src/alloc_aligned.rs");
    let alloc_rs = fs::read_to_string(root.join("src/alloc.rs")).expect("read src/alloc.rs");
    let free_rs = fs::read_to_string(root.join("src/free.rs")).expect("read src/free.rs");
    let override_rs =
        fs::read_to_string(root.join("src/alloc_override.rs")).expect("read src/alloc_override.rs");
    let prim_rs = fs::read_to_string(root.join("src/prim.rs")).expect("read src/prim.rs");
    let init_rs = fs::read_to_string(root.join("src/init.rs")).expect("read src/init.rs");
    let threadlocal_rs =
        fs::read_to_string(root.join("src/threadlocal.rs")).expect("read src/threadlocal.rs");

    assert!(
        cargo_toml.contains("crate-type = [\"rlib\", \"cdylib\"]")
            && cargo_toml.contains("lto = \"fat\"")
            && cargo_toml.contains("codegen-units = 1"),
        "release builds should produce the benchmark cdylib directly with one optimized codegen unit"
    );
    assert!(
        lib_rs.contains(
            "#![cfg_attr(all(not(test), target_os = \"linux\", not(debug_assertions)), no_std)]"
        ) && lib_rs.contains("#[panic_handler]")
            && lib_rs.contains("#[link_name = \"abort\"]")
            && lib_rs.contains("extern \"C\" fn rust_eh_personality()"),
        "Linux release cdylibs should avoid linking Rust std panic/backtrace machinery"
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
            && tls_shim.contains("_mi_tls_recurse_get"),
        "default Linux allocator fast paths should read hidden initial-exec TLS cells like C"
    );
    assert!(
        prim_rs.contains("fn mi_linux_x64_theap_default_get()")
            && prim_rs.contains("fn mi_linux_x64_theap_default_set(theap: *mut mi_theap_t)")
            && prim_rs.contains("fn mi_linux_x64_theap_cached_get()")
            && prim_rs.contains("fn mi_linux_x64_theap_cached_set(theap: *mut mi_theap_t)")
            && prim_rs.contains("__mi_theap_default@GOTTPOFF")
            && prim_rs.contains("__mi_theap_cached@GOTTPOFF")
            && init_rs.contains("fn mi_linux_x64_theap_main_get()")
            && init_rs.contains("fn mi_linux_x64_theap_main_set(theap: *mut mi_theap_t)")
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
        build_rs.contains("target_arch == \"x86_64\"")
            && build_rs.contains("cargo:rustc-cfg=mi_c_malloc_fast"),
        "assembly malloc fast override must be limited to the supported target"
    );
    assert!(
        override_rs.contains("#[unsafe(naked)]")
            && override_rs.contains("pub unsafe extern \"C\" fn mi_cxx_new_fast_64")
            && override_rs.contains("pub unsafe extern \"C\" fn mi_cxx_new_array_fast_64")
            && override_rs.contains("core::arch::naked_asm!")
            && override_rs.matches("\"endbr64\"").count() >= 5
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
            && override_rs.contains(".fill 0x540,1,0x90")
            && override_rs.contains("page_map = sym crate::page_map::_mi_page_map")
            && override_rs.contains("retire = sym crate::page::_mi_page_retire")
            && override_rs.contains("collect_mt = sym crate::free::mi_free_try_collect_mt")
            && override_rs.contains("generic_local = sym crate::free::mi_free_generic_local")
            && override_rs.contains("generic_mt = sym crate::free::mi_free_generic_mt")
            && override_rs.contains("\"cmp rdx, 3\"")
            && override_rs.contains("\"lock cmpxchg qword ptr [rdx], rcx\"")
            && override_rs.contains("jmp mi_new"),
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
            && override_rs.contains(
                "\"sub word ptr [rdi + 0x10], 1\",\n            \"mov qword ptr [rdi + 0x18], rsi\",\n            \"je 7f\",\n            \"3:\",\n            \"ret\","
            )
            && override_rs.contains("\"cmp byte ptr [rdi + 0x16], 0\",\n            \"jne 3b\",")
            && override_rs.contains(".set mi_free, free")
            && free_rs.contains("cfg_attr(not(mi_c_malloc_fast), unsafe(no_mangle))"),
        "Unix free/mi_free exports must keep the C-like aligned assembly fast override and hot local-free branch layout"
    );
    assert_eq!(offset_of!(mi_page_t, free), 0x08);
    assert_eq!(offset_of!(mi_page_t, used), 0x10);
    assert_eq!(offset_of!(mi_page_t, local_free), 0x18);
    assert_eq!(offset_of!(mi_page_t, xthread_free), 0x20);
    assert_eq!(offset_of!(mi_page_t, block_size), 0x28);
    assert_eq!(offset_of!(mi_page_t, page_start), 0x30);
    #[cfg(any(
        feature = "secure",
        debug_assertions,
        feature = "track-valgrind",
        feature = "track-asan"
    ))]
    {
        assert_eq!(size_of::<mi_page_t>(), 0x88);
        assert_eq!(offset_of!(mi_page_t, keys), 0x38);
        assert_eq!(offset_of!(mi_page_t, theap), 0x48);
        assert_eq!(offset_of!(mi_page_t, memid), 0x70);
    }
    #[cfg(not(any(
        feature = "secure",
        debug_assertions,
        feature = "track-valgrind",
        feature = "track-asan"
    )))]
    {
        assert_eq!(size_of::<mi_page_t>(), 0x78);
        assert_eq!(offset_of!(mi_page_t, theap), 0x38);
        assert_eq!(offset_of!(mi_page_t, memid), 0x60);
    }
    #[cfg(not(feature = "guarded"))]
    assert_eq!(offset_of!(mi_theap_t, pages_free_direct), 0x110);
}

#[test]
fn aligned_os_hints_are_randomized_like_c_release_builds() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let os_rs = fs::read_to_string(root.join("src/os.rs")).expect("read src/os.rs");
    let init_rs = fs::read_to_string(root.join("src/init.rs")).expect("read src/init.rs");

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
        fs::read_to_string(root.join("src/alloc_override.rs")).expect("read src/alloc_override.rs");

    assert!(
        override_rs.contains("export_name = \"_Znwm\"")
            && override_rs.contains("export_name = \"_Znam\"")
            && override_rs.contains("not(mi_c_malloc_fast)")
            && override_rs.contains("export_name = \"_ZnwmRKSt9nothrow_t\"")
            && override_rs.contains("export_name = \"_ZnamRKSt9nothrow_t\""),
        "64-bit C++ operator new wrappers must export Itanium ABI symbols"
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
        build_rs.contains("cargo:rustc-link-arg=-Wl,--version-script="),
        "Linux builds should pass the allocator export script through the link step"
    );
    assert!(
        build_rs.contains("mi_*;") && build_rs.contains("local:\n    *;"),
        "version script should export public mi_* APIs while hiding unmatched internals"
    );
    assert!(
        !build_rs.contains("_Z*;")
            && build_rs.contains("_Zda*;")
            && build_rs.contains("_Zdl*;")
            && build_rs.contains("_Zna*;")
            && build_rs.contains("_Znw*;")
            && build_rs.contains("malloc*;")
            && build_rs.contains("free*;")
            && build_rs.contains("realloc*;"),
        "version script must keep allocator and C++ ABI override names exported"
    );
}

#[test]
fn free_path_internals_are_not_dynamic_exports() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let page_map = fs::read_to_string(root.join("src/page_map.rs")).expect("read src/page_map.rs");
    let internal = fs::read_to_string(root.join("src/internal.rs")).expect("read src/internal.rs");
    let alloc_rs = fs::read_to_string(root.join("src/alloc.rs")).expect("read src/alloc.rs");
    let arena_rs = fs::read_to_string(root.join("src/arena.rs")).expect("read src/arena.rs");
    let arena_meta_rs =
        fs::read_to_string(root.join("src/arena_meta.rs")).expect("read src/arena_meta.rs");
    let free_rs = fs::read_to_string(root.join("src/free.rs")).expect("read src/free.rs");
    let libc_rs = fs::read_to_string(root.join("src/libc.rs")).expect("read src/libc.rs");
    let os_rs = fs::read_to_string(root.join("src/os.rs")).expect("read src/os.rs");
    let options_rs = fs::read_to_string(root.join("src/options.rs")).expect("read src/options.rs");
    let page_rs = fs::read_to_string(root.join("src/page.rs")).expect("read src/page.rs");
    let random_rs = fs::read_to_string(root.join("src/random.rs")).expect("read src/random.rs");
    let stats_rs = fs::read_to_string(root.join("src/stats.rs")).expect("read src/stats.rs");
    let theap_rs = fs::read_to_string(root.join("src/theap.rs")).expect("read src/theap.rs");
    let init_rs = fs::read_to_string(root.join("src/init.rs")).expect("read src/init.rs");
    let alloc_aligned_rs =
        fs::read_to_string(root.join("src/alloc_aligned.rs")).expect("read src/alloc_aligned.rs");
    let heap_rs = fs::read_to_string(root.join("src/heap.rs")).expect("read src/heap.rs");
    let prim_rs = fs::read_to_string(root.join("src/prim.rs")).expect("read src/prim.rs");
    let prim_unix_rs =
        fs::read_to_string(root.join("src/prim/unix.rs")).expect("read src/prim/unix.rs");

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
            && alloc_rs.contains("let page = _mi_safe_ptr_page(p);")
            && free_rs.contains("let mut page = _mi_safe_ptr_page(p);")
            && heap_rs.contains("let page: *mut mi_page_t = _mi_safe_ptr_page(p);")
            && heap_rs.contains("let page = _mi_safe_ptr_page(p);")
            && arena_rs.contains("let page = unsafe { _mi_safe_ptr_page(start) };"),
        "safe pointer page lookup should keep C's _mi_safe_ptr_page local call boundary"
    );

    for (source_name, source, internal_name) in [
        (
            "src/alloc.rs",
            &alloc_rs,
            "pub unsafe extern \"C\" fn _mi_page_malloc_zero",
        ),
        (
            "src/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_alloc",
        ),
        (
            "src/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_alloc_aligned",
        ),
        (
            "src/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_collect",
        ),
        (
            "src/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_contain",
        ),
        (
            "src/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_free",
        ),
        (
            "src/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_abandon",
        ),
        (
            "src/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_alloc",
        ),
        (
            "src/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_free",
        ),
        (
            "src/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_try_reabandon_to_mapped",
        ),
        (
            "src/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_page_unabandon",
        ),
        (
            "src/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arenas_unsafe_destroy_all",
        ),
        (
            "src/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_heap_visit_blocks",
        ),
        (
            "src/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_heap_move_pages",
        ),
        (
            "src/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_heap_destroy_pages",
        ),
        (
            "src/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arena_id_none",
        ),
        (
            "src/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arena_from_id",
        ),
        (
            "src/arena.rs",
            &arena_rs,
            "pub unsafe extern \"C\" fn _mi_arena_memid_is_suitable",
        ),
        (
            "src/arena_meta.rs",
            &arena_meta_rs,
            "pub unsafe extern \"C\" fn _mi_meta_free",
        ),
        (
            "src/arena_meta.rs",
            &arena_meta_rs,
            "pub unsafe extern \"C\" fn _mi_meta_is_meta_page",
        ),
        (
            "src/arena_meta.rs",
            &arena_meta_rs,
            "pub unsafe extern \"C\" fn _mi_meta_zalloc",
        ),
        (
            "src/free.rs",
            &free_rs,
            "pub unsafe extern \"C\" fn _mi_page_ptr_unalign",
        ),
        (
            "src/free.rs",
            &free_rs,
            "pub unsafe extern \"C\" fn _mi_padding_shrink",
        ),
        (
            "src/page.rs",
            &page_rs,
            "pub unsafe extern \"C\" fn _mi_page_abandon",
        ),
        (
            "src/page.rs",
            &page_rs,
            "pub unsafe extern \"C\" fn _mi_page_free",
        ),
        (
            "src/page.rs",
            &page_rs,
            "pub unsafe extern \"C\" fn _mi_page_init",
        ),
        (
            "src/page.rs",
            &page_rs,
            "pub unsafe extern \"C\" fn _mi_deferred_free",
        ),
        (
            "src/libc.rs",
            &libc_rs,
            "pub unsafe extern \"C\" fn _mi_vsnprintf",
        ),
        ("src/libc.rs", &libc_rs, "pub extern \"C\" fn _mi_toupper"),
        (
            "src/libc.rs",
            &libc_rs,
            "pub unsafe extern \"C\" fn _mi_strnicmp",
        ),
        (
            "src/libc.rs",
            &libc_rs,
            "pub unsafe extern \"C\" fn _mi_strlcpy",
        ),
        (
            "src/libc.rs",
            &libc_rs,
            "pub unsafe extern \"C\" fn _mi_strlcat",
        ),
        (
            "src/libc.rs",
            &libc_rs,
            "pub unsafe extern \"C\" fn _mi_strlen",
        ),
        (
            "src/libc.rs",
            &libc_rs,
            "pub unsafe extern \"C\" fn _mi_strnlen",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_free",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_mem_init",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_alloc",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_commit",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_reuse",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_decommit",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_reset",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_protect",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_alloc_huge_os_pages",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_numa_node",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_numa_node_count",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_getenv",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_thread_init_auto_done",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_thread_done_auto_done",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_thread_associate_default_theap",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_thread_is_in_threadpool",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_thread_yield",
        ),
        (
            "src/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_alloc",
        ),
        (
            "src/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_zalloc",
        ),
        (
            "src/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_free",
        ),
        (
            "src/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_free_ex",
        ),
        (
            "src/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_page_size",
        ),
        (
            "src/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_good_alloc_size",
        ),
        (
            "src/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_has_overcommit",
        ),
        (
            "src/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_has_virtual_reserve",
        ),
        (
            "src/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_virtual_address_bits",
        ),
        (
            "src/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_reuse",
        ),
        (
            "src/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_commit",
        ),
        (
            "src/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_commit_ex",
        ),
        (
            "src/os.rs",
            &os_rs,
            "pub unsafe extern \"C\" fn _mi_os_purge_ex",
        ),
        (
            "src/random.rs",
            &random_rs,
            "pub unsafe extern \"C\" fn _mi_random_split",
        ),
        (
            "src/random.rs",
            &random_rs,
            "pub unsafe extern \"C\" fn _mi_random_next",
        ),
        (
            "src/random.rs",
            &random_rs,
            "pub unsafe extern \"C\" fn _mi_random_init",
        ),
        (
            "src/random.rs",
            &random_rs,
            "pub unsafe extern \"C\" fn _mi_random_init_weak",
        ),
        (
            "src/random.rs",
            &random_rs,
            "pub unsafe extern \"C\" fn _mi_random_reinit_if_weak",
        ),
        (
            "src/alloc.rs",
            &alloc_rs,
            "pub unsafe extern \"C\" fn _mi_theap_malloc_zero_ex",
        ),
        (
            "src/alloc.rs",
            &alloc_rs,
            "pub unsafe extern \"C\" fn _mi_theap_malloc_zero",
        ),
        (
            "src/alloc.rs",
            &alloc_rs,
            "pub unsafe extern \"C\" fn _mi_theap_realloc_zero",
        ),
        (
            "src/alloc.rs",
            &alloc_rs,
            "pub unsafe extern \"C\" fn _mi_theap_malloc_guarded",
        ),
        ("src/init.rs", &init_rs, "pub extern \"C\" fn _mi_thread_id"),
        (
            "src/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_thread_done",
        ),
        (
            "src/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_guarded_init",
        ),
        (
            "src/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_options_init",
        ),
        (
            "src/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_default_set",
        ),
        (
            "src/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_cached_set",
        ),
        (
            "src/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_default_safe",
        ),
        (
            "src/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_main_safe",
        ),
        (
            "src/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_theap_empty_get",
        ),
        (
            "src/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_is_heap_main",
        ),
        (
            "src/init.rs",
            &init_rs,
            "pub unsafe extern \"C\" fn _mi_is_theap_main",
        ),
        ("src/init.rs", &init_rs, "fn _mi_subproc_to_id"),
        ("src/init.rs", &init_rs, "fn _mi_preloading"),
        ("src/init.rs", &init_rs, "fn _mi_subproc_main"),
        ("src/init.rs", &init_rs, "fn _mi_subproc"),
        ("src/init.rs", &init_rs, "fn _mi_subproc_from_id"),
        ("src/init.rs", &init_rs, "fn _mi_subproc_heap_main"),
        ("src/init.rs", &init_rs, "fn _mi_auto_process_init"),
        ("src/init.rs", &init_rs, "fn _mi_auto_process_done"),
        (
            "src/heap.rs",
            &heap_rs,
            "pub unsafe extern \"C\" fn _mi_heap_force_destroy",
        ),
        (
            "src/heap.rs",
            &heap_rs,
            "pub unsafe extern \"C\" fn _mi_heap_theap_get_peek",
        ),
        (
            "src/heap.rs",
            &heap_rs,
            "pub unsafe extern \"C\" fn _mi_heap_theap_get_or_init",
        ),
        (
            "src/page.rs",
            &page_rs,
            "pub unsafe extern \"C\" fn _mi_theap_collect_retired",
        ),
        (
            "src/theap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_collect_abandon",
        ),
        (
            "src/theap.rs",
            &theap_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_theap_init",
        ),
        (
            "src/theap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_create",
        ),
        (
            "src/theap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_incref",
        ),
        (
            "src/theap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_decref",
        ),
        (
            "src/theap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_free",
        ),
        (
            "src/theap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_delete",
        ),
        (
            "src/theap.rs",
            &theap_rs,
            "pub unsafe extern \"C\" fn _mi_theap_random_next",
        ),
        ("src/prim.rs", &prim_rs, "fn _mi_allocator_init"),
        ("src/prim.rs", &prim_rs, "fn _mi_allocator_done"),
        ("src/options.rs", &options_rs, "fn _mi_warning_message"),
        ("src/options.rs", &options_rs, "fn _mi_error_message"),
        ("src/options.rs", &options_rs, "fn _mi_option_get_fast"),
        ("src/options.rs", &options_rs, "fn _mi_assert_fail"),
        (
            "src/stats.rs",
            &stats_rs,
            "pub unsafe extern \"C\" fn _mi_stats_merge_into",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub unsafe extern \"C\" fn __mi_stat_increase_mt",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub unsafe extern \"C\" fn __mi_stat_decrease_mt",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub unsafe extern \"C\" fn __mi_stat_counter_increase_mt",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub unsafe extern \"C\" fn __mi_stat_adjust_increase_mt",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub unsafe extern \"C\" fn __mi_stat_adjust_decrease_mt",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub unsafe extern \"C\" fn __mi_stat_increase",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub unsafe extern \"C\" fn __mi_stat_decrease",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub unsafe extern \"C\" fn __mi_stat_counter_increase",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub unsafe extern \"C\" fn __mi_stat_adjust_increase",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub unsafe extern \"C\" fn __mi_stat_adjust_decrease",
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
        alloc_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_malloc_zero")
            && page_rs.contains("let p = _mi_page_malloc_zero(theap, page, size, zero);")
            && alloc_aligned_rs
                .contains("let p = _mi_page_malloc_zero(theap, page, padsize, zero);"),
        "page and aligned allocation should keep C's _mi_page_malloc_zero call boundary"
    );
    assert!(
        free_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_ptr_unalign")
            && free_rs.contains("crate::mi_assert!(_mi_page_ptr_unalign(page, p) == p.cast());")
            && free_rs.contains("let block = if mi_page_has_interior_pointers(page) {")
            && free_rs.contains("let block = _mi_page_ptr_unalign(page, p);"),
        "free and usable-size paths should keep C's _mi_page_ptr_unalign call boundary"
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
            && heap_rs.contains("_mi_heap_move_pages(heap, mi_heap_main());")
            && heap_rs.contains("_mi_heap_destroy_pages(heap);")
            && heap_rs.contains("_mi_heap_force_destroy(heap);"),
        "heap delete/destroy should keep C's heap page movement and force-destroy helper boundaries"
    );
    assert!(
        arena_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_alloc")
            && arena_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_contain")
            && arena_rs.contains(
                "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_unsafe_destroy_all"
            )
            && arena_rs.contains("_mi_arenas_alloc_aligned(")
            && arena_rs.contains("mi_arenas_contain_ex(p, ptr::null_mut())")
            && arena_rs.contains("static MI_ARENAS_CONTAIN_KEEP")
            && arena_rs.contains("mi_arenas_unsafe_destroy(subproc)")
            && theap_rs.contains("let theap = _mi_arenas_alloc(")
            && init_rs.contains("_mi_arenas_unsafe_destroy_all(subproc)"),
        "arena allocation, containment, and unsafe destroy helpers should keep C's local call boundaries"
    );
    assert!(
        arena_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_arenas_page_try_reabandon_to_mapped"
        )
            && arena_rs.contains("_mi_page_associated_theap_peek(page)")
            && arena_rs.contains("_mi_arenas_page_abandon(page, theap)")
            && free_rs.contains("_mi_arenas_page_try_reabandon_to_mapped(page)"),
        "free path should keep C's arena page reabandon-to-mapped helper boundary"
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
            && theap_rs.contains("_mi_theap_init(theap, heap, tld);\n    theap"),
        "_mi_theap_create should keep C's _mi_theap_init call boundary"
    );
    assert!(
        theap_rs.contains("_mi_memcpy_aligned(\n        theap.cast(),")
            && internal.contains("if n <= _mi_cpu_movsb_max")
            && internal.contains("\"rep movsb\"")
            && internal
                .contains("if n > _mi_cpu_movsb_max && n.is_multiple_of(size_of::<usize>())")
            && internal.contains("\"rep movsq\"")
            && internal.contains("pub unsafe fn _mi_memcpy_aligned"),
        "_mi_theap_init should copy the empty template through C's _mi_memcpy_aligned path"
    );
    assert!(
        stats_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_stats_merge_into"),
        "heap cleanup should keep C's stats merge call boundary instead of inlining it into _mi_theap_free"
    );
    assert!(
        stats_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn __mi_stat_increase_mt")
            && stats_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn __mi_stat_decrease_mt"),
        "theap lifecycle should keep C's multithreaded stats update helper boundaries"
    );
    assert!(
        stats_rs
            .contains("#[inline(never)]\npub unsafe extern \"C\" fn __mi_stat_counter_increase_mt")
            && stats_rs.contains(
                "#[inline(never)]\npub unsafe extern \"C\" fn __mi_stat_adjust_increase_mt"
            )
            && stats_rs.contains(
                "#[inline(never)]\npub unsafe extern \"C\" fn __mi_stat_adjust_decrease_mt"
            ),
        "multithreaded stats counters and adjusters should keep C's helper boundaries"
    );
    assert!(
        stats_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn __mi_stat_increase")
            && stats_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn __mi_stat_decrease")
            && stats_rs.contains(
                "#[inline(never)]\npub unsafe extern \"C\" fn __mi_stat_counter_increase"
            )
            && stats_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn __mi_stat_adjust_increase")
            && stats_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn __mi_stat_adjust_decrease")
            && stats_rs.contains("static MI_STAT_ADJUST_INCREASE_KEEP"),
        "thread-local stats counters and adjusters should keep C's helper boundaries"
    );
    assert!(
        init_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_subproc()")
            && init_rs.contains("crate::prim::_mi_theap_default_raw()")
            && prim_rs.contains("pub unsafe fn _mi_theap_default_raw()"),
        "_mi_subproc should stay as C's small local helper and inspect the raw default TLS cell"
    );
    assert!(
        theap_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_incref")
            && theap_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_decref")
            && theap_rs.contains("#[inline(always)]\nunsafe fn mi_theap_decref_inline")
            && theap_rs.contains("mi_theap_decref_inline(theap);\n    true")
            && !theap_rs.contains("    _mi_theap_decref(theap);\n    true"),
        "_mi_theap_free should inline C's decref tail while preserving the standalone _mi_theap_decref helper for other call sites"
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
    let page_rs = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/page.rs"))
        .expect("read src/page.rs");

    assert!(
        page_rs.contains("#[inline(always)]\nunsafe fn mi_find_page("),
        "mi_find_page should stay force-inlined into _mi_malloc_generic like C's generic path"
    );
}

#[test]
fn release_page_stat_writes_follow_c_mimalloc_gates() {
    let page_rs = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/page.rs"))
        .expect("read src/page.rs");

    for (stat, needle) in [
        (
            "pages_retire",
            "if MI_STAT > 0 {\n            __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_retire), 1);",
        ),
        (
            "pages_extended",
            "if MI_STAT > 0 {\n        __mi_stat_counter_increase(ptr::addr_of_mut!((*theap).stats.pages_extended), 1);",
        ),
        (
            "page_committed",
            "if MI_STAT > 0 {\n        __mi_stat_increase(\n            ptr::addr_of_mut!((*theap).stats.page_committed),",
        ),
    ] {
        assert!(
            page_rs.contains(needle),
            "{stat} updates should stay behind MI_STAT > 0 like C mimalloc"
        );
    }
}
