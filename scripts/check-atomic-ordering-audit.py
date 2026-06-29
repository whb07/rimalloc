#!/usr/bin/env python3
"""Check that direct Rust atomic ordering usage stays covered by the audit."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
AUDIT = ROOT / "ATOMIC_ORDERING_AUDIT.md"

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


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def main() -> int:
    audit_text = AUDIT.read_text()
    found = rust_files_with_orderings()
    missing = sorted(path for path in found if path not in audit_text)
    require(
        not missing,
        f"{AUDIT.name} is missing atomic-ordering source entries: {', '.join(missing)}",
    )
    for path in sorted(found):
        require(path in audit_text, f"{path} is not mentioned in {AUDIT.name}")

    atomic_rs = (ROOT / "src/atomic.rs").read_text()
    for snippet in ATOMIC_RS_REQUIRED_SNIPPETS:
        require(snippet in atomic_rs, f"missing audited Rust atomic snippet: {snippet}")

    c_atomic = (ROOT / "mimalloc-3.3.2/include/mimalloc/atomic.h").read_text()
    for snippet in C_ATOMIC_REQUIRED_SNIPPETS:
        require(snippet in c_atomic, f"missing C atomic baseline snippet: {snippet}")

    ordering_mentions = len(re.findall(r"Ordering::", "\n".join((ROOT / path).read_text() for path in found)))
    print(f"ok: atomic ordering audit covers {len(found)} source files and {ordering_mentions} direct Ordering uses")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
