#!/usr/bin/env python3
"""Check that the unsafe audit covers every Rust source file using unsafe."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
AUDIT = ROOT / "UNSAFE_AUDIT.md"
ALLOWLIST = ROOT / "docs/unsafe-migration-allowlist.tsv"
SAFETY_ID_RE = re.compile(r"SAFETY\[AUDIT:([a-z0-9_.-]+)\]")
ALLOWLIST_HEADER = "path\tcategory\towner\tremove_by\treason"
ALLOWLIST_ENTRY_CAP = 23
ALLOWLIST_CATEGORIES = {"production", "test", "translation"}
PATH_ALIASES = {
    "src/allocator/aligned.rs": ["src/alloc_aligned.rs"],
    "src/allocator/thread_heap.rs": ["src/theap.rs"],
    "src/allocator/tracking.rs": ["src/track.rs"],
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
    "src/types/atomic/concurrency_models.rs": [
        "src/core_types/atomic_wrappers/concurrency_models.rs"
    ],
    "src/types/atomic/tests.rs": ["src/core_types/atomic_wrappers/tests.rs"],
    "src/types/constants.rs": ["src/types.rs"],
    "src/types/mod.rs": ["src/types.rs"],
    "src/types/track.rs": ["src/track.rs"],
}

REQUIRED_AUDIT_SNIPPETS = [
    "Public `extern \"C\"` APIs accept the same nullable and invalid-pointer inputs",
    "Metadata pointers are obtained from owning allocator structures",
    "Raw pointer arithmetic is limited to C-equivalent block/page/arena layout",
    "Cross-thread metadata mutation goes through the audited atomic helpers",
    "Callback invocation and process/thread lifecycle entry points",
    "`ATOMIC_ORDERING_AUDIT.md`",
    "scripts/check-valgrind-tracking-parity.py",
    "scripts/check-asan-tracking-parity.py",
]


def unsafe_source_files() -> set[str]:
    files: set[str] = set()
    for path in (ROOT / "src").rglob("*.rs"):
        text = path.read_text()
        if re.search(r"\bunsafe\b", text):
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
    for audit_id in SAFETY_ID_RE.findall(text):
        counts[audit_id] = counts.get(audit_id, 0) + 1
    return counts


def source_audit_ids() -> set[str]:
    ids: set[str] = set()
    for path in (ROOT / "src").rglob("*.rs"):
        ids.update(SAFETY_ID_RE.findall(path.read_text()))
    return ids


def has_recent_safety_id(lines: list[str], index: int) -> bool:
    start = max(0, index - 5)
    return any(SAFETY_ID_RE.search(lines[line]) for line in range(start, index + 1))


def unsafe_operations(rel: str, text: str) -> list[str]:
    lines = text.splitlines()
    offenders: list[str] = []
    patterns = [
        re.compile(r"\bunsafe\s*\{"),
        re.compile(r"\bunsafe\s+extern\b"),
        re.compile(r"\bunsafe\s+impl\b"),
        re.compile(r"#\[\s*unsafe\s*\("),
        re.compile(r"\bglobal_asm!\s*\("),
    ]
    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("//") or stripped.startswith("///"):
            continue
        if any(pattern.search(line) for pattern in patterns) and not has_recent_safety_id(
            lines, index
        ):
            offenders.append(f"{rel}:{index + 1}: {stripped}")
    return offenders


def migration_allowlist() -> dict[str, tuple[str, str, str, str]]:
    require(ALLOWLIST.exists(), f"missing unsafe migration allowlist: {ALLOWLIST}")
    lines = ALLOWLIST.read_text().splitlines()
    require(lines and lines[0] == ALLOWLIST_HEADER, f"{ALLOWLIST} must start with {ALLOWLIST_HEADER!r}")

    entries: dict[str, tuple[str, str, str, str]] = {}
    date_re = re.compile(r"^20[0-9]{2}-[01][0-9]-[0-3][0-9]$")
    for line_number, line in enumerate(lines[1:], start=2):
        if not line or line.startswith("#"):
            continue
        fields = line.split("\t")
        require(
            len(fields) == 5,
            f"{ALLOWLIST}:{line_number}: expected 5 tab-separated fields",
        )
        path, category, owner, remove_by, reason = fields
        require(path.startswith("src/") and path.endswith(".rs"), f"{ALLOWLIST}:{line_number}: invalid path {path!r}")
        require(
            category in ALLOWLIST_CATEGORIES,
            f"{ALLOWLIST}:{line_number}: invalid category {category!r}",
        )
        require((ROOT / path).exists(), f"{ALLOWLIST}:{line_number}: missing source path {path}")
        require(owner.strip(), f"{ALLOWLIST}:{line_number}: owner is required")
        require(date_re.match(remove_by) is not None, f"{ALLOWLIST}:{line_number}: remove_by must be YYYY-MM-DD")
        require(reason.strip(), f"{ALLOWLIST}:{line_number}: reason is required")
        require(path not in entries, f"{ALLOWLIST}:{line_number}: duplicate path {path}")
        entries[path] = (category, owner, remove_by, reason)
    return entries


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def main() -> int:
    audit_text = AUDIT.read_text()
    found = unsafe_source_files()
    doc_ids = audit_id_counts(audit_text)
    ids_in_source = source_audit_ids()
    allowlist = migration_allowlist()
    require(
        len(allowlist) == ALLOWLIST_ENTRY_CAP,
        f"{ALLOWLIST.name} entry cap must match current migration count: "
        f"{len(allowlist)} entries, cap is {ALLOWLIST_ENTRY_CAP}",
    )
    missing = sorted(path for path in found if not audit_mentions_path(audit_text, path))
    require(
        not missing,
        f"{AUDIT.name} is missing unsafe source entries: {', '.join(missing)}",
    )

    for path in sorted(found):
        require(audit_mentions_path(audit_text, path), f"{path} is not mentioned in {AUDIT.name}")

    for snippet in REQUIRED_AUDIT_SNIPPETS:
        require(snippet in audit_text, f"missing unsafe audit invariant: {snippet}")

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

    untagged_by_file: dict[str, list[str]] = {}
    for rel in sorted(found):
        offenders = unsafe_operations(rel, (ROOT / rel).read_text())
        if offenders:
            untagged_by_file[rel] = offenders

    stale_allowlist = sorted(path for path in allowlist if path not in untagged_by_file)
    require(
        not stale_allowlist,
        f"{ALLOWLIST.name} has stale entries with no untagged unsafe operations: "
        + ", ".join(stale_allowlist),
    )

    missing_allowlist = sorted(path for path in untagged_by_file if path not in allowlist)
    require(
        not missing_allowlist,
        "unsafe operations need SAFETY[AUDIT:<id>] markers or allowlist entries: "
        + "; ".join(
            offender
            for path in missing_allowlist
            for offender in untagged_by_file[path][:3]
        ),
    )

    unsafe_mentions = sum(len(re.findall(r"\bunsafe\b", (ROOT / path).read_text())) for path in found)
    print(
        f"ok: unsafe audit covers {len(found)} source files and "
        f"{unsafe_mentions} unsafe mentions; {len(ids_in_source)} operation IDs checked; "
        f"{len(allowlist)} legacy files remain under dated migration allowlist"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
