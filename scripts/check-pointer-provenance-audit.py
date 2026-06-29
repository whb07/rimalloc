#!/usr/bin/env python3
"""Check that pointer provenance and arithmetic audit coverage is current."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
AUDIT = ROOT / "POINTER_PROVENANCE_AUDIT.md"
PROVENANCE_ID_RE = re.compile(r"PROVENANCE\[AUDIT:([a-z0-9_.-]+)\]")
PATH_ALIASES = {
    "src/allocator/aligned.rs": ["src/alloc_aligned.rs"],
    "src/allocator/thread_heap.rs": ["src/theap.rs"],
    "src/ffi/callbacks.rs": ["src/unsafe_callbacks.rs"],
    "src/internal/guarded.rs": ["src/core_types/guarded.rs"],
    "src/internal/mod.rs": ["src/internal.rs"],
    "src/platform/macos.rs": ["src/prim/osx.rs"],
    "src/platform/mod.rs": ["src/prim.rs"],
    "src/platform/os.rs": ["src/os.rs"],
    "src/raw/macros.rs": ["src/alloc_override.rs"],
    "src/raw/mod.rs": ["src/raw.rs"],
    "src/raw/posix.rs": ["src/alloc_posix.rs"],
    "src/safe/allocation.rs": ["src/safe/allocations.rs"],
    "src/safe/heap.rs": ["src/safe/heaps.rs"],
    "src/safe/mod.rs": ["src/safe.rs"],
    "src/types/atomic.rs": ["src/core_types/atomic_wrappers.rs"],
    "src/types/atomic/tests.rs": ["src/core_types/atomic_wrappers/tests.rs"],
    "src/types/constants.rs": ["src/types.rs"],
    "src/types/mod.rs": ["src/types.rs"],
    "src/types/track.rs": ["src/track.rs"],
}

PATTERN = re.compile(
    r"\.add\(|\.sub\(|\.offset\(|offset_from|wrapping_add|wrapping_sub|"
    r"checked_add|checked_sub|checked_mul|_mi_align_(?:up|down)|"
    r"\bas usize\b|\bas \*mut\b|\bas \*const\b"
)

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


def audit_path_candidates(path: str) -> list[str]:
    candidates = [path]
    candidates.extend(PATH_ALIASES.get(path, []))
    prefixes = {
        "src/allocator/": "src/",
        "src/ffi/": "src/",
        "src/internal/": "src/",
        "src/platform/": "src/prim/",
        "src/types/": "src/core_types/",
    }
    for old, new in prefixes.items():
        if path.startswith(old):
            candidates.append(new + path.removeprefix(old))
    return candidates


def audit_mentions_path(audit_text: str, path: str) -> bool:
    return any(candidate in audit_text for candidate in audit_path_candidates(path))


def audit_id_counts(text: str) -> dict[str, int]:
    counts: dict[str, int] = {}
    for audit_id in PROVENANCE_ID_RE.findall(text):
        counts[audit_id] = counts.get(audit_id, 0) + 1
    return counts


def source_audit_ids() -> set[str]:
    ids: set[str] = set()
    for path in (ROOT / "src").rglob("*.rs"):
        ids.update(PROVENANCE_ID_RE.findall(path.read_text()))
    return ids


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def main() -> int:
    audit_text = AUDIT.read_text()
    found = files_with_pointer_arithmetic()
    doc_ids = audit_id_counts(audit_text)
    ids_in_source = source_audit_ids()
    missing = sorted(path for path in found if not audit_mentions_path(audit_text, path))
    require(
        not missing,
        f"{AUDIT.name} is missing pointer-provenance source entries: {', '.join(missing)}",
    )

    for path in sorted(found):
        require(audit_mentions_path(audit_text, path), f"{path} is not mentioned in {AUDIT.name}")

    for snippet in REQUIRED_AUDIT_SNIPPETS:
        require(snippet in audit_text, f"missing pointer audit invariant: {snippet}")

    missing_doc_ids = sorted(audit_id for audit_id in ids_in_source if audit_id not in doc_ids)
    duplicate_doc_ids = sorted(audit_id for audit_id, count in doc_ids.items() if count != 1)
    require(
        not missing_doc_ids,
        f"{AUDIT.name} is missing operation ID entries: {', '.join(missing_doc_ids)}",
    )
    require(
        not duplicate_doc_ids,
        f"{AUDIT.name} must list each operation ID exactly once: {', '.join(duplicate_doc_ids)}",
    )

    mentions = sum(len(PATTERN.findall((ROOT / path).read_text())) for path in found)
    print(
        f"ok: pointer provenance audit covers {len(found)} source files and "
        f"{mentions} pointer/arithmetic markers; {len(ids_in_source)} operation IDs checked"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
