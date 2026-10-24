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
