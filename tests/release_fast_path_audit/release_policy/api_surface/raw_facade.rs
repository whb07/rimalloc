use std::fs;
use std::path::Path;

#[path = "raw_facade/macro_policy.rs"]
mod macro_policy;
#[path = "raw_facade/nullability.rs"]
mod nullability;
#[path = "raw_facade/private_imports.rs"]
mod private_imports;

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
