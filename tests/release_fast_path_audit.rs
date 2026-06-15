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
        source[process_init..process_done].contains("mi_process_init_inner()")
            && !source[process_init..process_done].contains("_mi_auto_process_init()"),
        "mi_process_init should not clear preloading; only _mi_auto_process_init should"
    );
    assert!(
        source[process_init_inner..process_init].contains("_mi_process_is_initialized = true;"),
        "mi_process_init_inner should set process initialization state like C's mi_process_init_once"
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
    let tls_recurse_shim = fs::read_to_string(root.join("src/tls_recurse_shim.c"))
        .expect("read src/tls_recurse_shim.c");
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
        build_rs.contains("target_arch == \"x86_64\"")
            && build_rs.contains("cargo:rustc-cfg=mi_c_malloc_fast")
            && build_rs.contains("src/tls_shim.c")
            && build_rs.contains("src/tls_recurse_shim.c")
            && build_rs.contains(".arg(&tls_obj)\n        .arg(&recurse_obj)"),
        "assembly malloc fast override must be limited to the supported target"
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
            && override_rs.contains(".fill 0x540,1,0x90")
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
            && override_rs.contains(
                "\"sub word ptr [rdi + 0x10], 1\",\n            \"mov qword ptr [rdi + 0x18], rsi\",\n            \"je 7f\",\n            \"3:\",\n            \"ret\","
            )
            && override_rs.contains("\"cmp byte ptr [rdi + 0x16], 0\",\n            \"jne 3b\",")
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
    let atomic_rs = fs::read_to_string(root.join("src/atomic.rs")).expect("read src/atomic.rs");
    let bitmap_rs = fs::read_to_string(root.join("src/bitmap.rs")).expect("read src/bitmap.rs");
    let free_rs = fs::read_to_string(root.join("src/free.rs")).expect("read src/free.rs");
    let libc_rs = fs::read_to_string(root.join("src/libc.rs")).expect("read src/libc.rs");
    let os_rs = fs::read_to_string(root.join("src/os.rs")).expect("read src/os.rs");
    let options_rs = fs::read_to_string(root.join("src/options.rs")).expect("read src/options.rs");
    let page_rs = fs::read_to_string(root.join("src/page.rs")).expect("read src/page.rs");
    let random_rs = fs::read_to_string(root.join("src/random.rs")).expect("read src/random.rs");
    let stats_rs = fs::read_to_string(root.join("src/stats.rs")).expect("read src/stats.rs");
    let theap_rs = fs::read_to_string(root.join("src/theap.rs")).expect("read src/theap.rs");
    let init_rs = fs::read_to_string(root.join("src/init.rs")).expect("read src/init.rs");
    let threadlocal_rs =
        fs::read_to_string(root.join("src/threadlocal.rs")).expect("read src/threadlocal.rs");
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
    assert!(
        theap_rs
            .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_heap_area_init(")
            && theap_rs.contains("let bsize = mi_page_block_size(page);")
            && theap_rs.contains("(*area).reserved = (*page).reserved as usize * bsize;")
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
            && heap_rs.contains("if !_mi_is_heap_main(heap) {")
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
            && page_rs.contains("(*theap).heartbeat = (*theap).heartbeat.wrapping_add(1);")
            && page_rs.contains("mi_atomic_load_ptr_acquire(ptr::addr_of!(deferred_free))")
            && page_rs.contains("fun(force, (*theap).heartbeat, arg);")
            && page_rs.contains("_mi_deferred_free(theap, false);")
            && theap_rs.contains("_mi_deferred_free(theap, force);"),
        "deferred free callback dispatch should keep C's _mi_deferred_free helper boundary"
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
            "pub(crate) unsafe extern \"C\" fn _mi_page_abandon",
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
        ("src/libc.rs", &libc_rs, "pub(crate) fn _mi_snprintf"),
        (
            "src/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_vsnprintf",
        ),
        ("src/libc.rs", &libc_rs, "pub extern \"C\" fn _mi_toupper"),
        (
            "src/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strnicmp",
        ),
        (
            "src/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_streq",
        ),
        (
            "src/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strlcpy",
        ),
        (
            "src/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strlcat",
        ),
        (
            "src/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strlen",
        ),
        (
            "src/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strnlen",
        ),
        (
            "src/libc.rs",
            &libc_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_strnstr",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_free",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_mem_init",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_alloc",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_commit",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_reuse",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_decommit",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_reset",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_protect",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_alloc_huge_os_pages",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub unsafe extern \"C\" fn _mi_prim_numa_node",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_numa_node_count",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_clock_now",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_process_info",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_out_stderr",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_random_buf",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_getenv",
        ),
        (
            "src/libc.rs",
            &libc_rs,
            "pub(crate) extern \"C\" fn _mi_popcount_generic",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_init_auto_done",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_done_auto_done",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_associate_default_theap",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_is_in_threadpool",
        ),
        (
            "src/prim/unix.rs",
            &prim_unix_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_prim_thread_yield",
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
            "pub(crate) extern \"C\" fn _mi_os_random_weak",
        ),
        (
            "src/random.rs",
            &random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_split",
        ),
        (
            "src/random.rs",
            &random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_next",
        ),
        (
            "src/random.rs",
            &random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_init",
        ),
        (
            "src/random.rs",
            &random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_init_weak",
        ),
        (
            "src/random.rs",
            &random_rs,
            "pub(crate) unsafe extern \"C\" fn _mi_random_reinit_if_weak",
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
            "src/threadlocal.rs",
            &threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_locals_init",
        ),
        (
            "src/threadlocal.rs",
            &threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_locals_done",
        ),
        (
            "src/threadlocal.rs",
            &threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_local_set",
        ),
        (
            "src/threadlocal.rs",
            &threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_local_get",
        ),
        (
            "src/threadlocal.rs",
            &threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_locals_thread_done",
        ),
        (
            "src/threadlocal.rs",
            &threadlocal_rs,
            "pub extern \"C\" fn _mi_thread_local_free",
        ),
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
            "pub(crate) unsafe extern \"C\" fn __mi_stat_increase_mt",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_decrease_mt",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub extern \"C\" fn _mi_stats_init",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_counter_increase_mt",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_increase_mt",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_decrease_mt",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_increase",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_decrease",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_counter_increase",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_increase",
        ),
        (
            "src/stats.rs",
            &stats_rs,
            "pub(crate) unsafe extern \"C\" fn __mi_stat_adjust_decrease",
        ),
        (
            "src/options.rs",
            &options_rs,
            "pub(crate) fn _mi_raw_message",
        ),
        ("src/options.rs", &options_rs, "pub(crate) fn _mi_message"),
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
            && free_rs.contains("(*padding).delta = new_delta as u32;")
            && alloc_aligned_rs.contains("#[cfg(mi_c_malloc_fast)]")
            && alloc_aligned_rs.contains("\"call _mi_padding_shrink\"")
            && alloc_aligned_rs.contains("fn mi_padding_shrink_call(")
            && alloc_aligned_rs.contains("_mi_padding_shrink(page, block, min_size);")
            && alloc_aligned_rs
                .contains("mi_padding_shrink_call(page, p.cast::<mi_block_t>(), adjust + size);"),
        "aligned allocation should keep C's _mi_padding_shrink helper boundary"
    );
    assert!(
        page_rs.contains("#[inline(never)]\npub extern \"C\" fn mi_bin(size: usize) -> usize")
            && page_rs
                .contains("#[inline(never)]\npub extern \"C\" fn _mi_bin(size: usize) -> usize")
            && page_rs.contains(
                "#[inline(never)]\npub extern \"C\" fn _mi_bin_size(bin: usize) -> usize"
            )
            && page_rs.contains("mi_bin(size)")
            && page_rs.contains("_mi_theap_empty")
            && page_rs.contains(".pages[bin].block_size")
            && page_rs.contains("_mi_bin_size(mi_bin(size + MI_PADDING_SIZE))")
            && stats_rs.contains("_mi_bin_size(bin)"),
        "bin lookup helpers should keep C's mi_bin/_mi_bin/_mi_bin_size boundaries and empty-theap table dependency"
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
            && heap_rs.contains("let mut theap = _mi_heap_theap_get_peek(heap);")
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
            && options_rs.contains("let default_out = mi_out_get_default(Some(&mut default_arg));")
            && options_rs.contains("unsafe { default_out(message, default_arg) };")
            && options_rs.contains("unsafe { out(message, arg) };")
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
            && prim_unix_rs.contains("_mi_strnicmp(name, s, len) == 0")
            && prim_unix_rs.contains("if _mi_preloading() {\n        return false;")
            && prim_unix_rs.contains("s = getenv(buf.as_ptr());")
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
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_strnicmp(",
        "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_streq(",
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
            "#[inline(never)]\npub(crate) fn _mi_error_message_fmt(err: c_int, args: fmt::Arguments<'_>)"
        ) && options_rs.contains(
            "#[inline(never)]\n#[cfg_attr(mi_internal_c_probe, unsafe(no_mangle))]\npub(crate) unsafe extern \"C\" fn _mi_error_message(err: c_int, fmt: *const c_char)"
        ) && options_rs.contains(
            "static MI_ERROR_MESSAGE_FMT_KEEP: for<'a> fn(c_int, fmt::Arguments<'a>) = _mi_error_message_fmt;"
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
            && page_rs.contains("mi_page_queue_remove(pq, page);")
            && page_rs.contains("mi_page_set_theap(page, ptr::null_mut());")
            && page_rs.contains("_mi_arenas_page_free(page, theap);")
            && page_rs.contains("_mi_arenas_collect(false, false, (*theap).tld);")
            && page_rs.contains("_mi_page_free(page, pq);")
            && theap_rs.contains("_mi_page_free(page, pq);"),
        "page release should keep C's local _mi_page_free helper boundary"
    );
    assert!(
        page_rs.contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_page_abandon(")
            && page_rs.contains("_mi_page_free_collect(page, false);")
            && page_rs.contains("if mi_page_all_free(page) {")
            && page_rs.contains("_mi_page_free(page, pq);")
            && page_rs.contains("mi_page_queue_remove(pq, page);")
            && page_rs.contains("mi_page_set_theap(page, ptr::null_mut());")
            && page_rs.contains("_mi_arenas_page_abandon(page, theap);")
            && page_rs.contains("_mi_arenas_collect(false, false, (*theap).tld);")
            && page_rs.contains("_mi_page_abandon(page, pq);")
            && theap_rs.contains("_mi_page_abandon(page, pq);"),
        "page abandon should keep C's local helper boundary and collect/free/reclaim sequence"
    );
    assert!(
        page_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_free_collect(")
            && page_rs.contains(
                "#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_free_collect_partly("
            )
            && page_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_page_unfull(")
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
            && page_rs
                .contains("crate::mi_assert_internal!(!force || (*page).local_free.is_null());")
            && free_rs.contains("_mi_page_unfull(page);")
            && free_rs.contains("_mi_page_free_collect(page, false);")
            && free_rs.contains("_mi_page_free_collect_partly(page, mt_free);")
            && arena_rs.contains("crate::page::_mi_page_init(theap, page)")
            && arena_rs.contains("_mi_page_free_collect(page, false)")
            && arena_rs.contains("_mi_page_stats_bin(page)")
            && theap_rs.contains("_mi_page_free_collect(page, true)")
            && theap_rs.contains("_mi_page_queue_is_valid("),
        "page lifecycle and page-queue helpers should keep C's local helper boundaries"
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
        atomic_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_atomic_once_enter")
            && atomic_rs
                .contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_atomic_once_release")
            && atomic_rs.contains("_mi_atomic_once_release(self.once);"),
        "atomic once should keep C's enter/release helper boundaries"
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
                .contains("return _mi_bitmap_forall_setc_ranges(bitmap, visit, arena, arg);")
            && arena_rs.contains("_mi_bitmap_forall_setc_rangesn("),
        "bitmap traversal should keep C's set/range traversal helper boundaries"
    );
    for marker in [
        "#[inline(never)]\npub unsafe fn mi_bitmap_try_find_and_claim(",
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
        .find("\n}\n\n#[inline(never)]\npub unsafe fn mi_bbitmap_try_find_and_clear8(")
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
        "#[inline(never)]\nfn mi_bchunk_try_find_and_clearNC(",
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
        .find("unsafe extern \"C\" fn mi_arena_try_claim_abandoned(")
        .expect("find mi_arena_try_claim_abandoned");
    let claim_end = arena_rs[claim_start..]
        .find("\n}\n\n#[allow(dead_code)]\nunsafe fn mi_heap_ensure_arena_pages(")
        .map(|offset| claim_start + offset)
        .expect("find end of mi_arena_try_claim_abandoned");
    let claim_body = &arena_rs[claim_start..claim_end];
    assert!(
        !claim_body.contains("mi_arena_page_at_slice(")
            && claim_body.contains("pages_meta.add(slice_index)")
            && claim_body.contains("slice_index * MI_ARENA_SLICE_SIZE"),
        "abandoned arena page claiming should keep C's inlined page lookup shape"
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
        theap_init_body
            .contains("(*theap).cookie = _mi_random_next(ptr::addr_of_mut!((*theap).random)) | 1;")
            && !theap_init_body.contains("_mi_theap_random_next(theap)"),
        "_mi_theap_init should match C's optimized artifact by calling _mi_random_next directly for the cookie"
    );
    assert!(
        theap_init_body.contains("mi_atomic_store_explicit(\n        ptr::addr_of_mut!((*theap).refcount),\n        1,\n        mi_memory_order_e::mi_memory_order_seq_cst,\n    );")
            && !theap_init_body.contains("(*theap).refcount = 1;"),
        "_mi_theap_init should initialize C's atomic refcount through a seq_cst atomic store"
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
        subproc_main_body.contains("unsafe { mi_subproc_main_addr() }")
            && !subproc_main_body.contains("mi_subproc_main_init()"),
        "_mi_subproc_main should match C's direct &subproc_main helper without process initialization"
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
        subproc_body.contains("unsafe { mi_subproc_main_addr() }")
            && !subproc_body.contains("_mi_subproc_main()")
            && !subproc_body.contains("mi_subproc_main_init()"),
        "_mi_subproc should use C's direct main-subproc fallback on a null raw default theap"
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
        subproc_heap_main_body.contains("let heap = unsafe { mi_subproc_heap_main_raw(subproc) };")
            && !subproc_heap_main_body.contains("subproc.is_null()")
            && !subproc_heap_main_body.contains("ptr::null_mut()"),
        "_mi_subproc_heap_main should match C's valid-subproc contract without a Rust-only null fast-path branch"
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
            && alloc_rs.contains(
                "if mi_unlikely(p.is_null()) {\n        mi_heap_try_new(heap, size, false)"
            )
            && alloc_rs.contains("if mi_unlikely(p.is_null()) {\n        mi_try_new(size, true)")
            && alloc_rs.contains("static MI_THEAP_TRY_NEW_KEEP")
            && alloc_rs.contains("static MI_TRY_NEW_KEEP")
            && alloc_rs.contains("static MI_HEAP_TRY_NEW_KEEP"),
        "C++ new retry paths should keep C's separate theap/default-heap/explicit-heap helper boundaries"
    );
    assert!(
        page_rs.contains("#[inline(never)]\npub unsafe extern \"C\" fn _mi_theap_page_reclaim")
            && page_rs.contains("static MI_THEAP_PAGE_RECLAIM_KEEP")
            && page_rs.contains("mi_page_queue_push_at_end(theap, pq, page);"),
        "_mi_theap_page_reclaim should keep C's local abandoned-page reclaim helper boundary"
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
            && page_rs.contains("let page = mi_page_fresh_alloc(theap, pq, (*pq).block_size, 0);")
            && page_rs
                .contains("let page = mi_page_fresh_alloc(theap, pq, block_size, page_alignment);"),
        "fresh-page allocation and page free-list extension should keep C's local helper boundaries"
    );
    assert!(
        free_rs.contains("#[inline(never)]\nunsafe extern \"C\" fn mi_abandoned_page_try_reclaim")
            && free_rs.contains(
                "#[inline(never)]\npub unsafe extern \"C\" fn mi_page_usable_aligned_size_of"
            )
            && free_rs.contains("static MI_ABANDONED_PAGE_TRY_RECLAIM_KEEP")
            && free_rs.contains("static MI_PAGE_USABLE_ALIGNED_SIZE_OF_KEEP")
            && free_rs.contains("&& mi_abandoned_page_try_reclaim(page, reclaim_on_free)")
            && free_rs.contains("mi_page_usable_aligned_size_of(page, p)"),
        "abandoned-page reclaim and aligned usable-size helpers should keep C's local helper boundaries"
    );
    assert!(
        theap_rs.contains(
            "#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_area_visit_blocks"
        ) && theap_rs.contains("static MI_THEAP_AREA_VISIT_BLOCKS_KEEP")
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
            && page_rs
                .contains("let page = _mi_arenas_page_alloc(theap, block_size, page_alignment);"),
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
            && theap_rs
                .contains("#[inline(never)]\npub(crate) unsafe extern \"C\" fn _mi_theap_decref")
            && theap_rs.contains("#[inline(always)]\nunsafe fn mi_theap_decref_inline")
            && theap_rs.contains("mi_theap_decref_inline(theap);\n    true")
            && !theap_rs.contains("    _mi_theap_decref(theap);\n    true"),
        "_mi_theap_free should inline C's decref tail while preserving the standalone _mi_theap_decref helper for other call sites"
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
            && heap_rs.contains("theap = mi_heap_init_theap(heap);"),
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
        internal.contains("\"rep stosb\"")
            && internal.contains("pub unsafe fn mi_rep_stosb(dst: *mut c_void, val: u8, n: usize)")
            && internal.contains("if n <= mi_cpu_stosb_max() {\n            mi_rep_stosb(dst, val as u8, n);\n            return;\n        }"),
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
            && free_rs.contains("if is_local {\n        mi_free_generic_local(page, p);")
            && free_rs.contains("} else {\n        mi_free_generic_mt(page, p);")
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
