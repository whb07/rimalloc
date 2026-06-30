#!/usr/bin/env python3
"""Verify the documented public API inventory stays in sync with source."""

from __future__ import annotations

import argparse
import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
TIER_SU_SNAPSHOT = ROOT / "docs/api-snapshots/tier-s-u.txt"

EXPECTED_ROOT_ITEMS = {
    "raw",
    "unsafe_callbacks",
    "MiMalloc",
    "Allocation",
    "AllocError",
    "GlobalOptions",
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

TIER_U_ROOT_ITEMS = {"unsafe_callbacks"}
TIER_R_ROOT_ITEMS = {"raw"}
TIER_S_ROOT_ITEMS = EXPECTED_ROOT_ITEMS - TIER_U_ROOT_ITEMS - TIER_R_ROOT_ITEMS

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

EXPECTED_EXPORTED_MACROS_BY_TIER = {
    "tier-r-typed-allocation": {
        "mi_calloc_tp",
        "mi_heap_calloc_tp",
        "mi_heap_malloc_tp",
        "mi_heap_mallocn_tp",
        "mi_heap_reallocn_tp",
        "mi_heap_recalloc_tp",
        "mi_heap_zalloc_tp",
        "mi_malloc_tp",
        "mi_mallocn_tp",
        "mi_reallocn_tp",
        "mi_zalloc_tp",
    },
    "tier-r-header-override": {
        "_aligned_free",
        "_aligned_malloc",
        "_aligned_offset_realloc",
        "_aligned_recalloc",
        "_dupenv_s",
        "_expand",
        "_recalloc",
        "_strdup",
        "_wcsdup",
        "cfree",
        "malloc",
        "malloc_usable_size",
        "memalign",
        "operatordelete",
        "operatordelete_aligned",
        "operatordelete_aligned_nothrow",
        "operatordelete_array",
        "operatordelete_array_aligned",
        "operatordelete_array_aligned_nothrow",
        "operatordelete_array_nothrow",
        "operatordelete_array_sized",
        "operatordelete_array_sized_aligned",
        "operatordelete_nothrow",
        "operatordelete_sized",
        "operatordelete_sized_aligned",
        "operatornew",
        "operatornew_aligned",
        "operatornew_aligned_nothrow",
        "operatornew_array",
        "operatornew_array_aligned",
        "operatornew_array_aligned_nothrow",
        "operatornew_array_nothrow",
        "operatornew_nothrow",
        "posix_memalign",
        "realpath",
        "realloc",
        "reallocarray",
        "reallocf",
        "strdup",
        "valloc",
    },
    "internal-compatibility": {
        "MI_INIT128",
        "MI_INIT16",
        "MI_INIT256",
        "MI_INIT32",
        "MI_INIT4",
        "MI_INIT5",
        "MI_INIT6",
        "MI_INIT64",
        "MI_INIT74",
        "MI_INIT8",
        "MI_UNUSED",
        "MI_UNUSED_RELEASE",
        "_mi_memzero_var",
        "mi_assert",
        "mi_assert_expensive",
        "mi_assert_internal",
        "mi_atomic_do_once",
        "mi_atomic_guard",
        "mi_debug_assert",
        "mi_debug_assert_eq",
        "mi_heap_stat_adjust_decrease",
        "mi_heap_stat_counter_increase",
        "mi_heap_stat_decrease",
        "mi_os_stat_counter_increase",
        "mi_os_stat_decrease",
        "mi_subproc_stat_adjust_decrease",
        "mi_subproc_stat_counter_increase",
        "mi_subproc_stat_decrease",
        "mi_theap_stat_adjust_decrease",
        "mi_theap_stat_counter_increase",
        "mi_theap_stat_decrease",
        "mi_trace_message",
    },
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


def exported_macros() -> dict[str, pathlib.Path]:
    macros = {}
    pending_export: pathlib.Path | None = None
    for path in sorted((ROOT / "src").rglob("*.rs")):
        for line in path.read_text().splitlines():
            stripped = line.strip()
            if stripped == "#[macro_export]":
                pending_export = path
                continue
            if pending_export is None:
                continue
            match = re.match(r"macro_rules!\s+([A-Za-z0-9_]+)\b", stripped)
            if match:
                macros[match.group(1)] = pending_export.relative_to(ROOT)
                pending_export = None
                continue
            if stripped and not stripped.startswith("#[") and not stripped.startswith("//"):
                pending_export = None
    return macros


def tier_s_u_snapshot(actual_root: set[str]) -> str:
    require(
        TIER_S_ROOT_ITEMS | TIER_U_ROOT_ITEMS <= actual_root,
        "Tier S/U snapshot source items are not all present in crate root",
    )
    lines = [
        "# Tier S/U Rust API Snapshot",
        "# Generated by scripts/check-api-inventory.py --write-tier-su-snapshot",
        "# Tier R raw facade and Tier C ABI changes are classified separately.",
        "",
        "## Tier S",
    ]
    lines.extend(f"- {item}" for item in sorted(TIER_S_ROOT_ITEMS))
    lines.extend(["", "## Tier U"])
    lines.extend(f"- {item}" for item in sorted(TIER_U_ROOT_ITEMS))
    lines.append("")
    return "\n".join(lines)


def check_tier_s_u_snapshot(actual_root: set[str]) -> None:
    expected = tier_s_u_snapshot(actual_root)
    require(TIER_SU_SNAPSHOT.exists(), f"missing {TIER_SU_SNAPSHOT.relative_to(ROOT)}")
    actual = TIER_SU_SNAPSHOT.read_text()
    require(
        actual == expected,
        f"{TIER_SU_SNAPSHOT.relative_to(ROOT)} is stale; run "
        "scripts/check-api-inventory.py --write-tier-su-snapshot",
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--write-tier-su-snapshot",
        action="store_true",
        help="rewrite the checked Tier S/U Rust API snapshot after review",
    )
    args = parser.parse_args()

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

    expected_macros = set().union(*EXPECTED_EXPORTED_MACROS_BY_TIER.values())
    actual_macros = set(exported_macros())
    require(
        actual_macros == expected_macros,
        "exported macro inventory drift: "
        f"missing={sorted(expected_macros - actual_macros)} "
        f"extra={sorted(actual_macros - expected_macros)}",
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
    for macro_tier, macros in EXPECTED_EXPORTED_MACROS_BY_TIER.items():
        require(
            f"Macro tier: `{macro_tier}`" in api_doc,
            f"docs/api-inventory.md missing macro tier heading {macro_tier}",
        )
        for macro_name in sorted(macros):
            require(
                f"`{macro_name}!`" in api_doc,
                f"docs/api-inventory.md must classify exported macro {macro_name}!",
            )

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
        "scripts/check-api-inventory.py",
    ]:
        require(phrase in abi_doc, f"docs/abi.md missing module taxonomy phrase: {phrase}")
    require(
        "raw::{alloc, free, heap, stats, options, ...}" not in abi_doc,
        "docs/abi.md still describes broad implementation modules as default raw facade",
    )
    if args.write_tier_su_snapshot:
        TIER_SU_SNAPSHOT.parent.mkdir(parents=True, exist_ok=True)
        TIER_SU_SNAPSHOT.write_text(tier_s_u_snapshot(actual_root))
    else:
        check_tier_s_u_snapshot(actual_root)

    print(
        f"ok: public API inventory covers {len(actual_root)} crate-root items "
        f"{len(actual_raw)} default raw facade modules, and "
        f"{len(actual_unstable)} unstable raw-internals modules, and "
        f"{len(actual_macros)} exported macros"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
