use super::*;

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
