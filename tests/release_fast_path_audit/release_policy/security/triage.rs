use std::fs;
use std::path::Path;

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
