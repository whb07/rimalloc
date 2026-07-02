use super::*;

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
