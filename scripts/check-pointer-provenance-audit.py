#!/usr/bin/env python3
"""Check that pointer provenance and arithmetic audit coverage is current."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
AUDIT = ROOT / "POINTER_PROVENANCE_AUDIT.md"

PATTERN = re.compile(
    r"\.add\(|\.sub\(|\.offset\(|offset_from|wrapping_add|wrapping_sub|"
    r"checked_add|checked_sub|checked_mul|_mi_align_(?:up|down)|"
    r"\bas usize\b|\bas \*mut\b|\bas \*const\b"
)

AUDITED_RUST_FILES = {
    "src/abi.rs",
    "src/alloc.rs",
    "src/alloc_aligned.rs",
    "src/alloc_override.rs",
    "src/alloc_posix.rs",
    "src/arena.rs",
    "src/arena_meta.rs",
    "src/atomic.rs",
    "src/bench_diagnostics.rs",
    "src/bitmap.rs",
    "src/bits.rs",
    "src/core_types/arena.rs",
    "src/core_types/atomic_wrappers.rs",
    "src/core_types/callbacks.rs",
    "src/core_types/freelist.rs",
    "src/core_types/freelist_wrappers.rs",
    "src/core_types/guarded.rs",
    "src/core_types/layout.rs",
    "src/core_types/memid.rs",
    "src/core_types/padding.rs",
    "src/core_types/ptr.rs",
    "src/core_types/scalar.rs",
    "src/core_types/track.rs",
    "src/free.rs",
    "src/headers.rs",
    "src/init.rs",
    "src/internal.rs",
    "src/libc.rs",
    "src/options.rs",
    "src/os.rs",
    "src/page.rs",
    "src/page_map.rs",
    "src/prim.rs",
    "src/prim/osx.rs",
    "src/prim/unix.rs",
    "src/prim/wasi.rs",
    "src/prim/windows.rs",
    "src/random.rs",
    "src/safe.rs",
    "src/stats.rs",
    "src/theap.rs",
    "src/threadlocal.rs",
    "src/types.rs",
}

REQUIRED_AUDIT_SNIPPETS = [
    "Miri was checked for this toolchain but is not available",
    "Pointer-to-integer casts are used for C-equivalent alignment checks",
    "`.add`, `.sub`, and `offset_from` are used only within allocator-owned arrays",
    "Count/size multiplication that accepts public input goes through checked",
    "Wrapping arithmetic is limited to bit operations",
    "`scripts/check-behavioral-parity.py`",
    "`scripts/check-unix-syscall-parity.py`",
]


def files_with_pointer_arithmetic() -> set[str]:
    files: set[str] = set()
    for path in (ROOT / "src").rglob("*.rs"):
        if PATTERN.search(path.read_text()):
            files.add(path.relative_to(ROOT).as_posix())
    return files


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def main() -> int:
    audit_text = AUDIT.read_text()
    found = files_with_pointer_arithmetic()
    require(found == AUDITED_RUST_FILES, f"pointer audit file set mismatch: {sorted(found ^ AUDITED_RUST_FILES)}")

    for path in sorted(AUDITED_RUST_FILES):
        require(path in audit_text, f"{path} is not mentioned in {AUDIT.name}")

    for snippet in REQUIRED_AUDIT_SNIPPETS:
        require(snippet in audit_text, f"missing pointer audit invariant: {snippet}")

    mentions = sum(len(PATTERN.findall((ROOT / path).read_text())) for path in found)
    print(f"ok: pointer provenance audit covers {len(found)} source files and {mentions} pointer/arithmetic markers")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
