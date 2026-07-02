use super::*;

pub(super) struct UnsafePerformanceSources {
    pub(super) unsafe_allowlist: String,
    pub(super) page_map: String,
    pub(super) internal: String,
    pub(super) internal_memid: String,
    pub(super) alloc_rs: String,
    pub(super) arena_rs: String,
    pub(super) arena_debug_rs: String,
    pub(super) arena_heap_pages_rs: String,
    pub(super) arena_ids_rs: String,
    pub(super) arena_prefix_rs: String,
    pub(super) arena_meta_rs: String,
    pub(super) atomic_rs: String,
    pub(super) bitmap_rs: String,
    pub(super) free_rs: String,
    pub(super) libc_rs: String,
    pub(super) memid_rs: String,
    pub(super) callbacks_rs: String,
    pub(super) freelist_types: String,
    pub(super) arena_types: String,
    pub(super) page_types: String,
    pub(super) scalar_types: String,
    pub(super) atomic_types: String,
    pub(super) ptr_rs: String,
    pub(super) validation_types: String,
    pub(super) page_states_rs: String,
    pub(super) page_queues_rs: String,
    pub(super) os_rs: String,
    pub(super) options_rs: String,
    pub(super) page_rs: String,
    pub(super) page_bins_rs: String,
    pub(super) page_counts_rs: String,
    pub(super) deferred_rs: String,
    pub(super) page_flags_rs: String,
    pub(super) page_validation_rs: String,
    pub(super) random_rs: String,
    pub(super) stats_rs: String,
    pub(super) theap_rs: String,
    pub(super) init_rs: String,
    pub(super) threadlocal_rs: String,
    pub(super) alloc_aligned_rs: String,
    pub(super) heap_rs: String,
    pub(super) prim_rs: String,
    pub(super) prim_unix_rs: String,
}

impl UnsafePerformanceSources {
    pub(super) fn load(root: &Path) -> Self {
        Self {
            unsafe_allowlist: read_source(root, "docs/unsafe-migration-allowlist.tsv"),
            page_map: read_source(root, "src/allocator/page_map.rs"),
            internal: read_source(root, "src/internal/mod.rs"),
            internal_memid: read_source(root, "src/internal/memid.rs"),
            alloc_rs: read_source(root, "src/allocator/alloc.rs"),
            arena_rs: read_source(root, "src/allocator/arena.rs"),
            arena_debug_rs: read_source(root, "src/allocator/arena/debug.rs"),
            arena_heap_pages_rs: read_source(root, "src/allocator/arena/heap_pages.rs"),
            arena_ids_rs: read_source(root, "src/allocator/arena/ids.rs"),
            arena_prefix_rs: read_source(root, "src/allocator/arena/prefix.rs"),
            arena_meta_rs: read_source(root, "src/allocator/arena_meta.rs"),
            atomic_rs: read_source(root, "src/internal/atomic.rs"),
            bitmap_rs: read_source(root, "src/internal/bitmap.rs"),
            free_rs: read_source(root, "src/allocator/free.rs"),
            libc_rs: read_source(root, "src/ffi/libc.rs"),
            memid_rs: read_source(root, "src/types/memid.rs"),
            callbacks_rs: read_source(root, "src/types/callbacks.rs"),
            freelist_types: read_source(root, "src/types/freelist_wrappers.rs"),
            arena_types: read_source(root, "src/types/arena.rs"),
            page_types: read_source(root, "src/types/page.rs"),
            scalar_types: read_source(root, "src/types/scalar.rs"),
            atomic_types: read_source(root, "src/types/atomic.rs"),
            ptr_rs: read_source(root, "src/types/ptr.rs"),
            validation_types: read_source(root, "src/types/validation.rs"),
            page_states_rs: read_source(root, "src/types/page_states.rs"),
            page_queues_rs: read_source(root, "src/types/page_queues.rs"),
            os_rs: read_source(root, "src/platform/os.rs"),
            options_rs: read_source(root, "src/allocator/options.rs"),
            page_rs: read_source(root, "src/allocator/page.rs"),
            page_bins_rs: read_source(root, "src/allocator/page/bins.rs"),
            page_counts_rs: read_source(root, "src/allocator/page/counts.rs"),
            deferred_rs: read_source(root, "src/allocator/page/deferred.rs"),
            page_flags_rs: read_source(root, "src/allocator/page/flags.rs"),
            page_validation_rs: read_source(root, "src/allocator/page/validation.rs"),
            random_rs: read_source(root, "src/internal/random.rs"),
            stats_rs: read_source(root, "src/allocator/stats.rs"),
            theap_rs: read_source(root, "src/allocator/thread_heap.rs"),
            init_rs: read_source(root, "src/allocator/init.rs"),
            threadlocal_rs: read_source(root, "src/allocator/threadlocal.rs"),
            alloc_aligned_rs: read_source(root, "src/allocator/aligned.rs"),
            heap_rs: read_source(root, "src/allocator/heap.rs"),
            prim_rs: read_source(root, "src/platform/mod.rs"),
            prim_unix_rs: read_source(root, "src/platform/unix.rs"),
        }
    }
}

fn read_source(root: &Path, rel: &str) -> String {
    fs::read_to_string(root.join(rel)).unwrap_or_else(|err| panic!("read {rel}: {err}"))
}

pub(super) fn source_has_inline_never_function(source: &str, signature: &str) -> bool {
    let mut saw_inline_never = false;

    for line in source.lines().map(str::trim_start) {
        if line == "#[inline(never)]" {
            saw_inline_never = true;
            continue;
        }
        if saw_inline_never && line.starts_with("//") {
            continue;
        }
        if saw_inline_never && line.starts_with(signature) {
            return true;
        }
        saw_inline_never = false;
    }

    false
}
