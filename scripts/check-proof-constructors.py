#!/usr/bin/env python3
"""Compile valid proof handoffs and reject the corresponding invalid callers."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
PROBES = [
    (
        "allocation-loan-handoff",
        "src/types/local_page.rs",
        "unsafe fn probe(p: AvailablePage, r: PaddedSize) { unsafe { "
        "p.with_allocation_access(r, |p, r| { let (b, p) = p.pop(r); "
        "let _ = b.track_undefined().finish(&p, Zeroed::Yes); }); } }",
        None,
    ),
    (
        "allocation-rejects-cross-page-request",
        "src/types/local_page.rs",
        "unsafe fn probe(a: AvailablePage, b: AvailablePage, r: PaddedSize) { unsafe { "
        "a.with_allocation_access(r, |a, _| b.with_allocation_access(r, |_, r| "
        "{ let _ = a.pop(r); })); } }",
        "E0521",
    ),
    (
        "allocation-storage-cannot-escape",
        "src/types/local_page.rs",
        "unsafe fn probe(p: AvailablePage, r: PaddedSize) { let mut escaped = None; unsafe { "
        "p.with_allocation_access(r, |p, r| { escaped = Some(p.pop(r).0); }); } "
        "let _ = escaped; }",
        "E0521",
    ),
    (
        "allocation-storage-consumed-on-finish",
        "src/types/local_page.rs",
        "unsafe fn probe(p: AvailablePage, r: PaddedSize) { unsafe { "
        "p.with_allocation_access(r, |p, r| { let (b, p) = p.pop(r); "
        "let b = b.track_undefined(); let _ = b.finish(&p, Zeroed::Yes); "
        "let _ = b.finish(&p, Zeroed::Yes); }); } }",
        "E0382",
    ),
    (
        "allocation-storage-requires-tracking-transition",
        "src/types/local_page.rs",
        "unsafe fn probe(p: AvailablePage, r: PaddedSize) { unsafe { "
        "p.with_allocation_access(r, |p, r| { let (b, p) = p.pop(r); "
        "let _ = b.finish(&p, Zeroed::Yes); }); } }",
        "E0599",
    ),
    (
        "local-field-loan",
        "src/types/local.rs",
        "unsafe fn probe(h: ThreadHeapLocal) { unsafe { h.with_local_access(|a| { "
        "a.stats().malloc_normal.current += 1; }); } }",
        None,
    ),
    (
        "local-field-loans-cannot-overlap",
        "src/types/local.rs",
        "unsafe fn probe(h: ThreadHeapLocal) { unsafe { h.with_local_access(|a| { "
        "let stats = a.stats(); let _random = a.random(); "
        "stats.malloc_normal.current += 1; }); } }",
        "E0499",
    ),
    (
        "local-field-loan-cannot-escape",
        "src/types/local.rs",
        "unsafe fn probe(h: ThreadHeapLocal) { let mut escaped = None; "
        "unsafe { h.with_local_access(|a| { escaped = Some(a.stats()); }); } "
        "let _ = escaped; }",
        "E0521",
    ),
    (
        "scoped-free-handoff",
        "src/allocator/free.rs",
        "unsafe fn probe(v: crate::core_types::ValidatedFree) { unsafe { "
        "v.with_local(|page, block| mi_free_block_local(page, block, "
        "crate::core_types::LocalFreeMode::TrackOnly)); } }",
        None,
    ),
    (
        "scoped-free-rejects-independent-page-and-block",
        "src/allocator/free.rs",
        "unsafe fn probe(p: crate::core_types::LocalPage, "
        "b: crate::core_types::BlockForFree) { unsafe { "
        "mi_free_block_local(p, b, crate::core_types::LocalFreeMode::TrackOnly); } }",
        "E0308",
    ),
    (
        "scoped-free-rejects-cross-request-pairing",
        "src/allocator/free.rs",
        "unsafe fn probe(a: crate::core_types::ValidatedFree, "
        "b: crate::core_types::ValidatedFree) { unsafe { "
        "a.with_local(|page, _| b.with_local(|_, block| "
        "mi_free_block_local(page, block, crate::core_types::LocalFreeMode::TrackOnly))); } }",
        "E0521",
    ),
    (
        "compact-chunk-access-requires-unsafe",
        "src/internal/bitmap.rs",
        "fn probe(p: *const mi_bitmap_t) { let _ = mi_bitmap_chunk(p, 0); }",
        "E0133",
    ),
    (
        "metadata-request-from-bounded-constructor",
        "src/allocator/arena_meta.rs",
        "fn probe(size: usize) { let _ = MetaRequest::new(size); }",
        None,
    ),
    (
        "metadata-request-fields-confined-to-owner",
        "src/allocator/mod.rs",
        "fn probe() { let _ = arena_meta::MetaRequest { size: 0, "
        "block_count: core::num::NonZeroU32::MIN }; }",
        "E0603",
    ),
    (
        "validated-pointer-handoff",
        "src/types/validation.rs",
        "unsafe fn probe(u: UserPtr, p: PagePtr, l: PointerLocation) { "
        "let _ = unsafe { PointerLookup::after_validation(u, p, l) }; }",
        None,
    ),
    (
        "validated-pointer-requires-unsafe",
        "src/types/validation.rs",
        "fn probe(u: UserPtr, p: PagePtr, l: PointerLocation) { "
        "let _ = PointerLookup::after_validation(u, p, l); }",
        "E0133",
    ),
    (
        "old-safe-pointer-constructor-absent",
        "src/types/validation.rs",
        "fn probe(u: UserPtr, p: PagePtr, l: PointerLocation) { "
        "let _ = PointerLookup::valid(u, p, l); }",
        "E0599",
    ),
    (
        "huge-alignment-from-checked-alignment",
        "src/types/layout.rs",
        "fn probe(a: Alignment) { let _ = MaybeHugeAlignment::for_overalloc(a); }",
        None,
    ),
    (
        "huge-alignment-rejects-raw-integer",
        "src/types/layout.rs",
        "fn probe(a: usize) { let _ = MaybeHugeAlignment::for_overalloc(a); }",
        "E0308",
    ),
    (
        "old-raw-alignment-constructor-absent",
        "src/types/layout.rs",
        "fn probe() { let _ = MaybeHugeAlignment::new(1); }",
        "E0599",
    ),
    (
        "flags-require-restricted-alignment",
        "src/types/layout.rs",
        "fn probe(a: MaybeAlignment) { let _ = MallocGenericFlags::new(Zeroed::No, a); }",
        "E0308",
    ),
]


def main():
    with tempfile.TemporaryDirectory(prefix="mimalloc-proof-checks-") as directory:
        checkout = Path(directory)
        shutil.copytree(ROOT / "src", checkout / "src")
        for name in ["Cargo.toml", "Cargo.lock", "build.rs", "rust-toolchain.toml"]:
            shutil.copy2(ROOT / name, checkout / name)
        env = dict(os.environ, CARGO_TARGET_DIR=str(ROOT / "target/proof-constructors"))
        for name, relative, code, expected in PROBES:
            path = checkout / relative
            original = path.read_text()
            first_line = original.count("\n") + 1
            try:
                path.write_text(original + "\n#[allow(dead_code)] mod proof_probe {\nuse super::*;\n" + code + "\n}\n")
                run = subprocess.run(
                    ["cargo", "check", "--locked", "--lib", "--features", "std", "--message-format=json"],
                    cwd=checkout, env=env, capture_output=True, text=True,
                )
                errors = [json.loads(line)["message"] for line in run.stdout.splitlines()
                          if line.startswith('{"reason":"compiler-message"')]
                matched = [error for error in errors if error.get("code")
                           and error["code"]["code"] == expected
                           and any(span["is_primary"] and span["file_name"] == relative
                                   and span["line_start"] >= first_line for span in error["spans"])]
                if (expected is None and run.returncode) or (expected and (not run.returncode or not matched)):
                    raise RuntimeError(f"{name}: unexpected compiler result\n{run.stdout}\n{run.stderr}")
                print(f"{name}: passed", flush=True)
            finally:
                path.write_text(original)


if __name__ == "__main__":
    main()
