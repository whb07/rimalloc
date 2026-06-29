#!/usr/bin/env python3
"""Verify the documented public API inventory stays in sync with source."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]

EXPECTED_ROOT_ITEMS = {
    "raw",
    "unsafe_callbacks",
    "MiMalloc",
    "Allocation",
    "AllocError",
    "Heap",
    "HeapAllocation",
    "HeapError",
    "HeapRef",
    "InitializedAllocation",
    "InitializedHeapAllocation",
    "LayoutError",
    "MainHeap",
    "OptionError",
    "OptionKey",
    "Options",
    "ProcessInfo",
    "ResizeError",
    "ResizeErrorKind",
    "StatCount",
    "StatTotal",
    "StatsError",
    "StatsSnapshot",
    "TryAllocError",
}

EXPECTED_RAW_DEFAULT_MODULES = {
    "abi",
    "headers",
    "mimalloc",
}

EXPECTED_UNSTABLE_RAW_INTERNAL_MODULES = {
    "alloc",
    "alloc_aligned",
    "aligned",
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
        items.update(
            item.strip().split(" as ")[-1] for item in group.split(",") if item.strip()
        )
    for alias in re.findall(
        r"^pub use\s+[A-Za-z0-9_:]+\s+as\s+([A-Za-z0-9_]+)\s*;",
        lib_rs,
        flags=re.M,
    ):
        items.add(alias)

    return items


def raw_modules_by_gate(raw_rs: str) -> tuple[set[str], set[str]]:
    default = set()
    unstable = set()
    pending_cfg = False
    pending_attr = False
    for line in raw_rs.splitlines():
        stripped = line.strip()
        if stripped == '#[cfg(feature = "unstable-raw-internals")]':
            pending_cfg = True
            pending_attr = False
            continue
        if pending_attr:
            if stripped.endswith("]"):
                pending_attr = False
            continue
        if stripped.startswith("#["):
            if stripped == '#[cfg(feature = "unstable-raw-internals")]':
                pending_cfg = True
                pending_attr = False
                continue
            if not stripped.endswith("]"):
                pending_attr = True
            continue
        match = re.match(r"pub mod\s+([A-Za-z0-9_]+)\s*(?:\{|;)", line)
        if not match:
            if stripped:
                pending_cfg = False
            continue
        name = match.group(1)
        if pending_cfg:
            unstable.add(name)
        else:
            default.add(name)
        pending_cfg = False
    return default, unstable


def main() -> int:
    lib_rs = (ROOT / "src/lib.rs").read_text()
    raw_rs = (ROOT / "src/raw/mod.rs").read_text()
    abi_rs = (ROOT / "src/ffi/abi.rs").read_text()
    mimalloc_rs = (ROOT / "src/ffi/mimalloc.rs").read_text()
    api_doc = (ROOT / "docs/api-inventory.md").read_text()
    abi_doc = (ROOT / "docs/abi.md").read_text()

    actual_root = root_public_items(lib_rs)
    require(
        actual_root == EXPECTED_ROOT_ITEMS,
        f"crate-root public inventory drift: missing={sorted(EXPECTED_ROOT_ITEMS - actual_root)} "
        f"extra={sorted(actual_root - EXPECTED_ROOT_ITEMS)}",
    )
    for unsafe_callback_item in [
        "CallbackError",
        "DeferredFreeCallbackGuard",
        "ErrorCallbackGuard",
    ]:
        require(
            unsafe_callback_item not in actual_root,
            f"{unsafe_callback_item} must not be reintroduced as a crate-root Tier S item",
        )

    actual_raw, actual_unstable = raw_modules_by_gate(raw_rs)
    require(
        actual_raw == EXPECTED_RAW_DEFAULT_MODULES,
        f"default raw facade module inventory drift: missing={sorted(EXPECTED_RAW_DEFAULT_MODULES - actual_raw)} "
        f"extra={sorted(actual_raw - EXPECTED_RAW_DEFAULT_MODULES)}",
    )
    require(
        actual_unstable == EXPECTED_UNSTABLE_RAW_INTERNAL_MODULES,
        "unstable raw-internals module inventory drift: "
        f"missing={sorted(EXPECTED_UNSTABLE_RAW_INTERNAL_MODULES - actual_unstable)} "
        f"extra={sorted(actual_unstable - EXPECTED_UNSTABLE_RAW_INTERNAL_MODULES)}",
    )

    for item in sorted(EXPECTED_ROOT_ITEMS):
        require(item in api_doc, f"docs/api-inventory.md does not mention crate-root item {item}")
    for item in ["ErrorCallbackGuard", "DeferredFreeCallbackGuard"]:
        require(
            f"`unsafe_callbacks::{item}`" in api_doc,
            f"docs/api-inventory.md must classify unsafe_callbacks::{item}",
        )
        require(
            f"`unsafe_callbacks::{item}`: Tier U" in api_doc,
            f"docs/api-inventory.md must classify unsafe_callbacks::{item} as Tier U",
        )

    for phrase in [
        "Tier S: safe Rust API, Rust SemVer surface.",
        "Tier U: unsafe Rust convenience API, Rust SemVer surface, outside Tier S",
        "Tier U is public unsafe Rust API with Rust SemVer review",
        "Tier R: raw Rust facade, unsafe and unstable unless explicitly versioned.",
        "Tier C: exported C ABI and raw layout compatibility surface.",
        "Tier I: internal implementation surface, not public API.",
        "`unsafe_callbacks`: Tier U unsafe Rust callback guard namespace.",
        "The raw Rust module path surface is unstable until explicitly versioned.",
        "`unstable-raw-internals`",
        "symbols/public-symbol-classification.tsv",
    ]:
        require(phrase in api_doc, f"docs/api-inventory.md missing policy phrase: {phrase}")

    require(
        (ROOT / "symbols/public-symbol-classification.tsv").exists(),
        "missing symbols/public-symbol-classification.tsv",
    )
    require(
        'unsafe extern "C"' in abi_rs and "pub fn mi_malloc" in abi_rs,
        "src/ffi/abi.rs must own the public C header declarations",
    )
    require(
        'unsafe extern "C"' not in mimalloc_rs,
        "src/ffi/mimalloc.rs must remain an internal alias, not a C header mirror",
    )
    for phrase in [
        "## Module Taxonomy",
        "## Current File Layout",
        "C header declarations and layouts",
        "Raw C-shaped Rust facade",
        "Exported C ABI implementations",
        "Internal compatibility aliases",
        "Allocator internals",
        "raw::abi",
        "raw::mimalloc",
        "raw::headers",
        "unstable-raw-internals",
        "src/ffi/abi.rs",
        "New public C ABI declarations",
        "New exported C ABI implementations",
        "scripts/check-ffi-boundary.py",
    ]:
        require(phrase in abi_doc, f"docs/abi.md missing module taxonomy phrase: {phrase}")
    require(
        "raw::{alloc, free, heap, stats, options, ...}" not in abi_doc,
        "docs/abi.md still describes broad implementation modules as default raw facade",
    )

    print(
        f"ok: public API inventory covers {len(actual_root)} crate-root items "
        f"{len(actual_raw)} default raw facade modules, and "
        f"{len(actual_unstable)} unstable raw-internals modules"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
