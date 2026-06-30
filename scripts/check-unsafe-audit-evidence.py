#!/usr/bin/env python3
"""Check that every unsafe audit operation ID has evidence."""

from __future__ import annotations

import pathlib
import re


ROOT = pathlib.Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / "docs/unsafe-audit-evidence.tsv"
UNSAFE_AUDIT = ROOT / "UNSAFE_AUDIT.md"
SAFETY_ID_RE = re.compile(r"SAFETY\[AUDIT:([a-z0-9_.-]+)\]")
HEADER = "audit_id\ttype\tpath\tdetail"
AUTOMATED_TYPES = {"test", "script", "miri", "sanitizer", "benchmark", "assembly"}
ALLOWED_TYPES = AUTOMATED_TYPES | {"manual"}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def source_audit_ids() -> set[str]:
    ids: set[str] = set()
    for path in (ROOT / "src").rglob("*.rs"):
        ids.update(SAFETY_ID_RE.findall(path.read_text()))
    return ids


def evidence_rows() -> list[tuple[int, str, str, str, str]]:
    require(EVIDENCE.exists(), f"missing unsafe audit evidence file: {EVIDENCE}")
    lines = EVIDENCE.read_text().splitlines()
    require(lines and lines[0] == HEADER, f"{EVIDENCE} must start with {HEADER!r}")

    rows: list[tuple[int, str, str, str, str]] = []
    seen: set[tuple[str, str, str]] = set()
    for line_number, line in enumerate(lines[1:], start=2):
        if not line or line.startswith("#"):
            continue
        fields = line.split("\t")
        require(len(fields) == 4, f"{EVIDENCE}:{line_number}: expected 4 tab-separated fields")
        audit_id, evidence_type, path, detail = fields
        require(audit_id, f"{EVIDENCE}:{line_number}: audit_id is required")
        require(evidence_type in ALLOWED_TYPES, f"{EVIDENCE}:{line_number}: invalid type {evidence_type!r}")
        require(path, f"{EVIDENCE}:{line_number}: path is required")
        require(detail.strip(), f"{EVIDENCE}:{line_number}: detail is required")

        key = (audit_id, evidence_type, path)
        require(key not in seen, f"{EVIDENCE}:{line_number}: duplicate evidence row for {audit_id}")
        seen.add(key)
        rows.append((line_number, audit_id, evidence_type, path, detail))
    return rows


def main() -> int:
    ids_in_source = source_audit_ids()
    rows = evidence_rows()
    ids_with_automated_evidence: set[str] = set()
    ids_in_evidence: set[str] = set()

    for line_number, audit_id, evidence_type, path, _detail in rows:
        require(
            audit_id in ids_in_source,
            f"{EVIDENCE}:{line_number}: evidence references unknown source audit ID {audit_id!r}",
        )
        full_path = ROOT / path
        require(full_path.exists(), f"{EVIDENCE}:{line_number}: evidence path does not exist: {path}")
        if evidence_type == "test":
            require(
                path.startswith(("tests/", "src/")) and path.endswith(".rs"),
                f"{EVIDENCE}:{line_number}: test evidence must reference tests/*.rs or src/*.rs",
            )
        elif evidence_type == "script":
            require(
                path.startswith("scripts/") and full_path.is_file(),
                f"{EVIDENCE}:{line_number}: script evidence must reference a scripts/ file",
            )
        elif evidence_type == "manual":
            require(
                path in {"UNSAFE_AUDIT.md", "docs/unsafe-audit.md"} or path.startswith("docs/"),
                f"{EVIDENCE}:{line_number}: manual evidence must reference audit documentation",
            )

        ids_in_evidence.add(audit_id)
        if evidence_type in AUTOMATED_TYPES:
            ids_with_automated_evidence.add(audit_id)

    missing = sorted(ids_in_source - ids_in_evidence)
    missing_automated = sorted(ids_in_source - ids_with_automated_evidence)
    require(not missing, f"unsafe audit IDs missing evidence entries: {', '.join(missing)}")
    require(
        not missing_automated,
        "unsafe audit IDs missing automated evidence entries: " + ", ".join(missing_automated),
    )
    require(
        "docs/unsafe-audit-evidence.tsv" in UNSAFE_AUDIT.read_text(),
        "UNSAFE_AUDIT.md must reference docs/unsafe-audit-evidence.tsv",
    )

    print(
        f"ok: unsafe audit evidence covers {len(ids_in_source)} operation IDs "
        f"with {len(rows)} evidence entries"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
