pub(super) fn assert_no_path_wide_allowlist_entries(allowlist_rs: &str) {
    for platform_path in [
        "src/allocator/heap.rs",
        "src/allocator/threadlocal.rs",
        "src/platform/windows.rs",
        "src/platform/unix.rs",
        "src/platform/wasi.rs",
        "src/platform/os.rs",
        "src/platform/mod.rs",
        "src/platform/emscripten.rs",
        "src/platform/macos.rs",
        "src/allocator/aligned.rs",
        "src/allocator/arena.rs",
        "src/allocator/tracking.rs",
        "src/internal/mod.rs",
        "src/internal/block_link.rs",
        "src/internal/atomic.rs",
        "src/allocator/stats.rs",
        "src/allocator/thread_heap.rs",
        "src/allocator/alloc.rs",
        "src/allocator/alloc/cxx_new.rs",
        "src/allocator/alloc/string_path.rs",
        "src/allocator/free.rs",
        "src/allocator/arena/os_memory.rs",
        "src/allocator/arena/purge.rs",
        "src/allocator/init.rs",
        "src/allocator/page_map.rs",
        "src/allocator/page/stats_bin.rs",
        "src/allocator/options.rs",
        "src/bitmap.rs",
        "src/internal/bitmap.rs",
        "src/internal/bitmap/chunkmap.rs",
        "src/internal/page_flags.rs",
        "src/internal/random.rs",
        "src/raw/macros.rs",
        "src/raw/macros/header_forwarding.rs",
        "src/types/arena.rs",
        "src/types/atomic.rs",
        "src/types/atomic/tests.rs",
        "src/types/callbacks.rs",
        "src/types/freelist.rs",
        "src/types/guarded.rs",
        "src/types/layout.rs",
        "src/types/memid.rs",
        "src/types/page_queues.rs",
        "src/types/page_queues/tests.rs",
        "src/types/ptr.rs",
        "src/types/ptr/tests.rs",
        "src/types/scalar.rs",
        "src/ffi/libc.rs",
        "src/safe/mod.rs",
        "src/safe/allocation.rs",
        "src/safe/heap.rs",
    ] {
        assert!(
            !allowlist_rs.lines().any(|line| {
                let mut fields = line.split('|');
                let _kind = fields.next();
                let path = fields.next();
                let symbol = fields.next();
                path == Some(platform_path) && symbol == Some("*")
            }),
            "{platform_path} should not retain path-wide strict-type allowlist entries"
        );
    }
}
