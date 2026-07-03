use super::*;

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
        "src/types/atomic/thread_free.rs",
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
