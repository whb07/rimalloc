use std::fs;
use std::path::Path;

use super::super::super::source_item;

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
