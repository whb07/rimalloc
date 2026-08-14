#!/usr/bin/env python3
"""Reject broken relative links in repository Markdown documentation."""

from __future__ import annotations

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
MARKDOWN_LINK_RE = re.compile(r"\[[^\]]+\]\((?P<target>[^)]+)\)")


def main() -> None:
    failures: list[str] = []
    documents = [*ROOT.glob("*.md"), *(ROOT / "docs").rglob("*.md")]
    for path in documents:
        text = path.read_text(encoding="utf-8")
        relative = path.relative_to(ROOT)
        for match in MARKDOWN_LINK_RE.finditer(text):
            target = match.group("target").strip().strip("<>").split("#", 1)[0]
            if not target or "://" in target or target.startswith(("mailto:", "/")):
                continue
            linked = (path.parent / target).resolve()
            try:
                linked.relative_to(ROOT)
            except ValueError:
                failures.append(f"{relative}: repository link escapes the tree: {target}")
                continue
            if not linked.exists():
                failures.append(f"{relative}: broken repository link: {target}")
    if failures:
        raise SystemExit("documentation link check failed:\n" + "\n".join(failures))
    print(f"ok: checked relative links in {len(documents)} Markdown documents")


if __name__ == "__main__":
    main()
