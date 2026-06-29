#!/usr/bin/env python3
"""Check that direct Rust atomic ordering usage stays covered by the audit."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
AUDIT = ROOT / "ATOMIC_ORDERING_AUDIT.md"
ORDERING_ID_RE = re.compile(r"ORDERING\[AUDIT:([a-z0-9_.-]+)\]")
PATH_ALIASES = {
    "src/platform/os.rs": ["src/os.rs"],
    "src/safe/mod.rs": ["src/safe.rs"],
    "src/types/atomic.rs": ["src/core_types/atomic_wrappers.rs"],
    "src/types/atomic/concurrency_models.rs": [
        "src/core_types/atomic_wrappers/concurrency_models.rs"
    ],
    "src/types/atomic/tests.rs": ["src/core_types/atomic_wrappers/tests.rs"],
}

ATOMIC_RS_REQUIRED_SNIPPETS = [
    "mi_memory_order_e::mi_memory_order_relaxed => Ordering::Relaxed",
    "mi_memory_order_e::mi_memory_order_seq_cst => Ordering::SeqCst",
    "_ => Ordering::Acquire",
    "_ => Ordering::Release",
    "mi_memory_order_e::mi_memory_order_acq_rel => Ordering::AcqRel",
    "unsafe { atomic_i64(p) }.fetch_add(add, Ordering::Relaxed)",
    "atomic.compare_exchange_weak(current, x, Ordering::Release, Ordering::Relaxed)",
    "unsafe { atomic_i64(p) }.fetch_add(i, Ordering::AcqRel)",
    "match unsafe { atomic_i64(p) }.compare_exchange(",
    "unsafe { atomic_isize(p) }.fetch_add(add, Ordering::AcqRel)",
    "unsafe { atomic_isize(p) }.fetch_sub(sub, Ordering::AcqRel)",
    ".compare_exchange(expected, 1, Ordering::AcqRel, Ordering::Acquire)",
    ".mutex.store(0, Ordering::Release)",
    "let once_tid = unsafe { (*once).tid.load(Ordering::Acquire) }",
    ".compare_exchange(expected, current_tid, Ordering::AcqRel, Ordering::Acquire)",
    "(*once).tid.store(1, Ordering::Release)",
]

C_ATOMIC_REQUIRED_SNIPPETS = [
    "#define mi_atomic_load_acquire(p)",
    "#define mi_atomic_load_relaxed(p)",
    "#define mi_atomic_store_release(p,x)",
    "#define mi_atomic_store_relaxed(p,x)",
    "#define mi_atomic_exchange_release(p,x)",
    "#define mi_atomic_exchange_acq_rel(p,x)",
    "#define mi_atomic_cas_weak_release(p,exp,des)",
    "#define mi_atomic_cas_weak_acq_rel(p,exp,des)",
    "#define mi_atomic_cas_strong_release(p,exp,des)",
    "#define mi_atomic_cas_strong_acq_rel(p,exp,des)",
    "#define mi_atomic_add_relaxed(p,x)",
    "#define mi_atomic_add_acq_rel(p,x)",
    "#define mi_atomic_and_acq_rel(p,x)",
    "#define mi_atomic_or_acq_rel(p,x)",
]


def rust_files_with_orderings() -> set[str]:
    files: set[str] = set()
    for path in (ROOT / "src").rglob("*.rs"):
        text = path.read_text()
        if "Ordering::" in text:
            files.add(path.relative_to(ROOT).as_posix())
    return files


def audit_path_candidates(path: str) -> list[str]:
    candidates = [path]
    candidates.extend(PATH_ALIASES.get(path, []))
    prefixes = {
        "src/allocator/": "src/",
        "src/internal/": "src/",
        "src/platform/": "src/prim/",
        "src/raw/": "src/alloc_override.rs",
    }
    for old, new in prefixes.items():
        if path.startswith(old):
            if new.endswith(".rs"):
                candidates.append(new)
            else:
                candidates.append(new + path.removeprefix(old))
    return candidates


def audit_mentions_path(audit_text: str, path: str) -> bool:
    return any(candidate in audit_text for candidate in audit_path_candidates(path))


def audit_id_counts(text: str) -> dict[str, int]:
    counts: dict[str, int] = {}
    for audit_id in ORDERING_ID_RE.findall(text):
        counts[audit_id] = counts.get(audit_id, 0) + 1
    return counts


def source_audit_ids() -> set[str]:
    ids: set[str] = set()
    for path in (ROOT / "src").rglob("*.rs"):
        ids.update(ORDERING_ID_RE.findall(path.read_text()))
    return ids


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def main() -> int:
    audit_text = AUDIT.read_text()
    found = rust_files_with_orderings()
    doc_ids = audit_id_counts(audit_text)
    ids_in_source = source_audit_ids()
    missing = sorted(path for path in found if not audit_mentions_path(audit_text, path))
    require(
        not missing,
        f"{AUDIT.name} is missing atomic-ordering source entries: {', '.join(missing)}",
    )
    for path in sorted(found):
        require(audit_mentions_path(audit_text, path), f"{path} is not mentioned in {AUDIT.name}")

    atomic_rs = (ROOT / "src/internal/atomic.rs").read_text()
    for snippet in ATOMIC_RS_REQUIRED_SNIPPETS:
        require(snippet in atomic_rs, f"missing audited Rust atomic snippet: {snippet}")

    c_atomic = (ROOT / "mimalloc-3.3.2/include/mimalloc/atomic.h").read_text()
    for snippet in C_ATOMIC_REQUIRED_SNIPPETS:
        require(snippet in c_atomic, f"missing C atomic baseline snippet: {snippet}")

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

    ordering_mentions = len(re.findall(r"Ordering::", "\n".join((ROOT / path).read_text() for path in found)))
    print(
        f"ok: atomic ordering audit covers {len(found)} source files and "
        f"{ordering_mentions} direct Ordering uses; {len(ids_in_source)} operation IDs checked"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
