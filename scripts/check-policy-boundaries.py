#!/usr/bin/env python3
"""Keep Stage 2 compatibility decisions inside allocator policy modules."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
POLICY_ROOT = ROOT / "src" / "allocator" / "policy"
SCOPED_OPTIONS = (
    "mi_option_generic_collect",
    "mi_option_page_reclaim_on_free",
    "mi_option_page_full_retain",
    "mi_option_page_max_candidates",
    "mi_option_page_max_reclaim",
    "mi_option_page_cross_thread_max_reclaim",
)
OPTION_READ_RE = re.compile(
    r"(?:_mi_option_get_fast|mi_option_get|mi_option_get_clamp)\s*\([^;]*?"
    + r"(?:" + "|".join(SCOPED_OPTIONS) + r")",
    re.DOTALL,
)
OLD_CONSTANT_RE = re.compile(r"\b(?:MI_MAX_EXTEND_SIZE|MI_MIN_EXTEND|MI_RETIRE_CYCLES)\b")


def main() -> int:
    failures: list[str] = []
    for path in sorted((ROOT / "src" / "allocator").rglob("*.rs")):
        if path.is_relative_to(POLICY_ROOT):
            continue
        text = path.read_text(encoding="utf-8")
        if match := OPTION_READ_RE.search(text):
            line = text.count("\n", 0, match.start()) + 1
            failures.append(f"{path.relative_to(ROOT)}:{line}: direct scoped option read")
        if match := OLD_CONSTANT_RE.search(text):
            line = text.count("\n", 0, match.start()) + 1
            failures.append(f"{path.relative_to(ROOT)}:{line}: old hardcoded policy constant")
    if failures:
        raise AssertionError("policy boundary check failed:\n" + "\n".join(failures))
    print("ok: compatibility policy boundaries are centralized")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
