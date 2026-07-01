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

PRIVATE_RAW_IMPLEMENTATION_MODULES = {
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
}

C_HEADER_MACRO_TIERS = {
    "tier-r-typed-allocation",
    "tier-r-header-override",
}
C_HEADER_MACROS = set().union(
    *(EXPECTED_EXPORTED_MACROS_BY_TIER[tier] for tier in C_HEADER_MACRO_TIERS)
)
EXPECTED_MACRO_SOURCE_BY_TIER = {
    "tier-r-typed-allocation": pathlib.Path("src/ffi/abi.rs"),
    "tier-r-header-override": pathlib.Path("src/ffi/headers.rs"),
}
C_SHAPED_NAME = re.compile(
    r"^(?:_?mi_|malloc|calloc|realloc|free|operator)",
    flags=re.IGNORECASE,
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def root_public_items(lib_rs: str) -> set[str]:
    items = set(re.findall(r"^pub mod\s+([A-Za-z0-9_]+)\s*;", lib_rs, flags=re.M))
    items.update(re.findall(r"^pub struct\s+([A-Za-z0-9_]+)\b", lib_rs, flags=re.M))
    items.update(re.findall(r"^pub enum\s+([A-Za-z0-9_]+)\b", lib_rs, flags=re.M))
    items.update(re.findall(r"^pub trait\s+([A-Za-z0-9_]+)\b", lib_rs, flags=re.M))
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


def raw_modules(raw_rs: str) -> set[str]:
    default = set()
    pending_attr = False
    for line in raw_rs.splitlines():
        stripped = line.strip()
        if pending_attr:
            if stripped.endswith("]"):
                pending_attr = False
            continue
        if stripped.startswith("#["):
            if not stripped.endswith("]"):
                pending_attr = True
            continue
        match = re.match(r"pub mod\s+([A-Za-z0-9_]+)\s*(?:\{|;)", line)
        if not match:
            continue
        name = match.group(1)
        default.add(name)
    return default


def exported_macros() -> dict[str, tuple[pathlib.Path, bool]]:
    macros = {}
    pending_c_header_cfg = False
    pending_export: tuple[pathlib.Path, bool] | None = None
    for path in sorted((ROOT / "src").rglob("*.rs")):
        for line in path.read_text().splitlines():
            stripped = line.strip()
            if stripped == '#[cfg(feature = "c-header-macros")]':
                pending_c_header_cfg = True
                continue
            if stripped == "#[macro_export]":
                pending_export = (path, pending_c_header_cfg)
                pending_c_header_cfg = False
                continue
            if pending_export is None:
                if stripped and not stripped.startswith("#[") and not stripped.startswith("//"):
                    pending_c_header_cfg = False
                continue
            match = re.match(r"macro_rules!\s+([A-Za-z0-9_]+)\b", stripped)
            if match:
                export_path, gated_by_c_header_macros = pending_export
                macros[match.group(1)] = (
                    export_path.relative_to(ROOT),
                    gated_by_c_header_macros,
                )
                pending_export = None
                continue
            if stripped and not stripped.startswith("#[") and not stripped.startswith("//"):
                pending_export = None
                pending_c_header_cfg = False
    return macros


def public_safe_methods() -> dict[str, pathlib.Path]:
    methods = {}
    for path in sorted((ROOT / "src/safe").glob("*.rs")):
        for match in re.finditer(
            r"^\s+pub\s+(?:const\s+)?(?:unsafe\s+)?fn\s+([A-Za-z0-9_]+)\b",
            path.read_text(),
            flags=re.M,
        ):
            methods[match.group(1)] = path.relative_to(ROOT)
    return methods


def public_safe_traits() -> dict[str, pathlib.Path]:
    traits = {}
    for path in sorted((ROOT / "src/safe").glob("*.rs")):
        for match in re.finditer(
            r"^\s*pub\s+(?:unsafe\s+)?trait\s+([A-Za-z0-9_]+)\b",
            path.read_text(),
            flags=re.M,
        ):
            traits[match.group(1)] = path.relative_to(ROOT)
    return traits


def public_struct_field_leaks() -> list[str]:
    leaks = []
    for path in [
        *sorted((ROOT / "src/safe").glob("*.rs")),
        ROOT / "src/ffi/callbacks.rs",
    ]:
        lines = path.read_text().splitlines()
        line_index = 0
        while line_index < len(lines):
            line = lines[line_index]
            match = re.match(r"^pub struct\s+([A-Za-z0-9_]+)(.*)", line)
            if not match:
                line_index += 1
                continue
            name = match.group(1)
            tail = match.group(2)
            if "(" in tail and re.search(r"\(\s*pub\s+", tail):
                leaks.append(f"{path.relative_to(ROOT)}:{line_index + 1}:{name}")
            if "{" not in tail:
                line_index += 1
                continue
            line_index += 1
            while line_index < len(lines) and not lines[line_index].startswith("}"):
                if re.match(r"^\s+pub\s+[A-Za-z_]", lines[line_index]):
                    leaks.append(f"{path.relative_to(ROOT)}:{line_index + 1}:{name}")
                line_index += 1
            line_index += 1
    return leaks


def public_enum_exhaustiveness_errors() -> list[str]:
    errors = []
    for path in sorted((ROOT / "src/safe").glob("*.rs")):
        lines = path.read_text().splitlines()
        for index, line in enumerate(lines):
            match = re.match(r"^pub enum\s+([A-Za-z0-9_]+)\b", line)
            if not match:
                continue
            previous = lines[index - 1].strip() if index > 0 else ""
            if previous != "#[non_exhaustive]":
                errors.append(f"{path.relative_to(ROOT)}:{index + 1}:{match.group(1)}")
    return errors


def check_naming_firewall(actual_root: set[str]) -> None:
    bad_tier_s = sorted(item for item in TIER_S_ROOT_ITEMS if C_SHAPED_NAME.match(item))
    require(
        not bad_tier_s,
        "Tier S crate-root items must use Rust-facing names; C-shaped names "
        f"belong in Tier R/Tier C: {bad_tier_s}",
    )
    bad_safe_methods = sorted(
        f"{path}:{name}"
        for name, path in public_safe_methods().items()
        if C_SHAPED_NAME.match(name)
    )
    require(
        not bad_safe_methods,
        "public safe-facade methods must use Rust-facing names; C-shaped names "
        "belong in Tier R/Tier C:\n" + "\n".join(bad_safe_methods),
    )
    bad_root = sorted(
        item
        for item in actual_root
        if C_SHAPED_NAME.match(item) and item not in TIER_R_ROOT_ITEMS
    )
    require(
        not bad_root,
        "crate-root C-shaped public items must be classified as Tier R/Tier C: "
        f"{bad_root}",
    )


def check_representation_firewall() -> None:
    leaks = public_struct_field_leaks()
    require(
        not leaks,
        "Tier S/Tier U public structs must not expose representation fields "
        "without an explicit inventory exception:\n" + "\n".join(leaks),
    )


def check_enum_exhaustiveness() -> None:
    errors = public_enum_exhaustiveness_errors()
    require(
        not errors,
        "public Tier S/Tier U enums must be marked #[non_exhaustive] unless "
        "docs/api-inventory.md records an exhaustive-enum exception:\n"
        + "\n".join(errors),
    )


def check_public_trait_policy() -> None:
    traits = public_safe_traits()
    require(
        not traits,
        "Tier S/Tier U public traits require explicit inventory, sealing, "
        "object-safety, and blanket-impl review before exposure:\n"
        + "\n".join(f"{path}:{name}" for name, path in sorted(traits.items())),
    )


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
    compatibility_doc = (ROOT / "docs/compatibility.md").read_text()

    actual_root = root_public_items(lib_rs)
    require(
        actual_root == EXPECTED_ROOT_ITEMS,
        f"crate-root public inventory drift: missing={sorted(EXPECTED_ROOT_ITEMS - actual_root)} "
        f"extra={sorted(actual_root - EXPECTED_ROOT_ITEMS)}",
    )
    check_naming_firewall(actual_root)
    check_representation_firewall()
    check_enum_exhaustiveness()
    check_public_trait_policy()
    for unsafe_callback_item in [
        "CallbackError",
        "DeferredFreeCallbackGuard",
        "ErrorCallbackGuard",
    ]:
        require(
            unsafe_callback_item not in actual_root,
            f"{unsafe_callback_item} must not be reintroduced as a crate-root Tier S item",
        )

    actual_raw = raw_modules(raw_rs)
    require(
        actual_raw == EXPECTED_RAW_DEFAULT_MODULES,
        f"default raw facade module inventory drift: missing={sorted(EXPECTED_RAW_DEFAULT_MODULES - actual_raw)} "
        f"extra={sorted(actual_raw - EXPECTED_RAW_DEFAULT_MODULES)}",
    )
    require(
        actual_raw.isdisjoint(PRIVATE_RAW_IMPLEMENTATION_MODULES),
        "private raw implementation modules must not be public raw modules: "
        f"{sorted(actual_raw & PRIVATE_RAW_IMPLEMENTATION_MODULES)}",
    )

    expected_macros = set().union(*EXPECTED_EXPORTED_MACROS_BY_TIER.values())
    macro_inventory = exported_macros()
    actual_macros = set(macro_inventory)
    require(
        actual_macros == expected_macros,
        "c-header-macros feature macro inventory drift: "
        f"missing={sorted(expected_macros - actual_macros)} "
        f"extra={sorted(actual_macros - expected_macros)}",
    )
    actual_default_macros = {
        name for name, (_, gated) in macro_inventory.items() if not gated
    }
    expected_default_macros = expected_macros - C_HEADER_MACROS
    require(
        actual_default_macros == expected_default_macros,
        "default macro inventory drift: "
        f"missing={sorted(expected_default_macros - actual_default_macros)} "
        f"extra={sorted(actual_default_macros - expected_default_macros)}",
    )
    for macro_name in sorted(C_HEADER_MACROS):
        require(
            macro_inventory[macro_name][1],
            f"c-header macro {macro_name}! must be gated by feature c-header-macros",
        )
    for macro_tier, expected_path in EXPECTED_MACRO_SOURCE_BY_TIER.items():
        for macro_name in sorted(EXPECTED_EXPORTED_MACROS_BY_TIER[macro_tier]):
            actual_path = macro_inventory[macro_name][0]
            require(
                actual_path == expected_path,
                f"{macro_tier} macro {macro_name}! must be exported from "
                f"{expected_path}, found {actual_path}",
            )
    for macro_name in sorted(expected_default_macros):
        require(
            not macro_inventory[macro_name][1],
            f"default macro {macro_name}! must not be gated by feature c-header-macros",
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
        "Implementation modules such as allocation, page, heap, arena, OS, primitive,",
        "Naming firewall: Tier S crate-root items and public safe-facade methods use",
        "`mi_*`, `_mi_*`, malloc-family, and C++ operator spellings",
        "Representation firewall: Tier S and Tier U structs keep fields private",
        "All current Tier S enums are growable and marked `#[non_exhaustive]`",
        "There are no current Tier U enums.",
        "There are no current Tier S or Tier U public traits.",
        "`requested_bytes_total`, `normal_bytes_total`, and `huge_bytes_total`",
        "`unsafe_callbacks`: Tier U unsafe Rust callback guard namespace.",
        "API diff review is required for style refactors",
        "classify any public change as Tier S,",
        "`cargo semver-checks check-release`",
        "The raw Rust module path surface is unstable until explicitly versioned.",
        "symbols/public-symbol-classification.tsv",
    ]:
        require(phrase in api_doc, f"docs/api-inventory.md missing policy phrase: {phrase}")
    for phrase in [
        "For style refactors touching `src/lib.rs`, `src/safe`,",
        "Classify every public change as Tier S, Tier U, Tier R, or Tier C.",
        "`cargo semver-checks check-release`",
    ]:
        require(
            phrase in compatibility_doc,
            f"docs/compatibility.md missing API diff policy phrase: {phrase}",
        )
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
        f"{len(actual_default_macros)} default macros / "
        f"{len(actual_macros)} Tier R c-header-macros feature macros"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
