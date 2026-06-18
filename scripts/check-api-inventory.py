#!/usr/bin/env python3
"""Verify the documented public API inventory stays in sync with source."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]

EXPECTED_ROOT_ITEMS = {
    "raw",
    "MiMalloc",
    "Allocation",
    "AllocError",
    "LayoutError",
    "TryAllocError",
}

EXPECTED_RAW_MODULES = {
    "abi",
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
    "headers",
    "heap",
    "init",
    "internal",
    "libc",
    "memory",
    "mimalloc",
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
}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def root_public_items(lib_rs: str) -> set[str]:
    items = set(re.findall(r"^pub mod\s+([A-Za-z0-9_]+)\s*;", lib_rs, flags=re.M))
    items.update(re.findall(r"^pub struct\s+([A-Za-z0-9_]+)\b", lib_rs, flags=re.M))
    items.update(re.findall(r"^pub enum\s+([A-Za-z0-9_]+)\b", lib_rs, flags=re.M))
    items.update(re.findall(r"^pub fn\s+([A-Za-z0-9_]+)\b", lib_rs, flags=re.M))
    items.update(re.findall(r"^pub type\s+([A-Za-z0-9_]+)\b", lib_rs, flags=re.M))

    for group in re.findall(r"^pub use\s+[A-Za-z0-9_:]+::\{([^}]+)\};", lib_rs, flags=re.M):
        items.update(item.strip().split(" as ")[-1] for item in group.split(","))

    return items


def raw_modules(raw_rs: str) -> set[str]:
    return set(re.findall(r"^pub mod\s+([A-Za-z0-9_]+)\s*\{", raw_rs, flags=re.M))


def main() -> int:
    lib_rs = (ROOT / "src/lib.rs").read_text()
    raw_rs = (ROOT / "src/raw.rs").read_text()
    api_doc = (ROOT / "docs/api-inventory.md").read_text()

    actual_root = root_public_items(lib_rs)
    require(
        actual_root == EXPECTED_ROOT_ITEMS,
        f"crate-root public inventory drift: missing={sorted(EXPECTED_ROOT_ITEMS - actual_root)} "
        f"extra={sorted(actual_root - EXPECTED_ROOT_ITEMS)}",
    )

    actual_raw = raw_modules(raw_rs)
    require(
        actual_raw == EXPECTED_RAW_MODULES,
        f"raw facade module inventory drift: missing={sorted(EXPECTED_RAW_MODULES - actual_raw)} "
        f"extra={sorted(actual_raw - EXPECTED_RAW_MODULES)}",
    )

    for item in sorted(EXPECTED_ROOT_ITEMS):
        require(item in api_doc, f"docs/api-inventory.md does not mention crate-root item {item}")

    for phrase in [
        "Crate root safe API: Rust SemVer surface.",
        "`raw`: unstable low-level Rust path surface until explicitly versioned.",
        "C ABI symbols and layouts: C compatibility surface.",
        "`core_types`: private design surface, not public SemVer.",
        "symbols/public-symbol-classification.tsv",
    ]:
        require(phrase in api_doc, f"docs/api-inventory.md missing policy phrase: {phrase}")

    require(
        (ROOT / "symbols/public-symbol-classification.tsv").exists(),
        "missing symbols/public-symbol-classification.tsv",
    )

    print(
        f"ok: public API inventory covers {len(actual_root)} crate-root items "
        f"and {len(actual_raw)} raw facade modules"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
