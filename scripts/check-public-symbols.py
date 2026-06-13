#!/usr/bin/env python3
"""Check exported Rust symbols against vendored mimalloc public headers."""

from __future__ import annotations

import argparse
import pathlib
import re
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
PUBLIC_HEADERS = [
    ROOT / "mimalloc-3.3.2/include/mimalloc.h",
    ROOT / "mimalloc-3.3.2/include/mimalloc-stats.h",
]


def public_header_symbols() -> list[str]:
    decl = re.compile(r"mi_decl(?:_nodiscard)?[^;#]*?\b(mi_[A-Za-z0-9_]+)\s*\(")
    symbols: list[str] = []
    for header in PUBLIC_HEADERS:
        text = header.read_text()
        text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
        text = re.sub(r"//.*", "", text)
        for match in decl.finditer(text):
            symbol = match.group(1)
            if symbol not in symbols:
                symbols.append(symbol)
    return symbols


def rust_staticlib() -> pathlib.Path:
    subprocess.run(
        ["cargo", "rustc", "--lib", "--", "--crate-type", "staticlib"],
        cwd=ROOT,
        check=True,
    )
    libs = sorted(
        (ROOT / "target/debug/deps").glob("libmechanical_mimalloc-*.a"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not libs:
        raise SystemExit("no Rust staticlib artifact found")
    return libs[0]


def exported_symbols(staticlib: pathlib.Path) -> set[str]:
    nm = subprocess.run(
        ["nm", "-g", "--defined-only", str(staticlib)],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    symbols: set[str] = set()
    for line in nm.stdout.splitlines():
        parts = line.split()
        if parts:
            symbols.add(parts[-1])
    return symbols


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--missing-only",
        action="store_true",
        help="only check that public C header declarations are exported",
    )
    args = parser.parse_args()

    expected = public_header_symbols()
    exported = exported_symbols(rust_staticlib())
    missing = [symbol for symbol in expected if symbol not in exported]

    if missing:
        print("missing public header symbols:", file=sys.stderr)
        for symbol in missing:
            print(symbol, file=sys.stderr)
        return 1

    if args.missing_only:
        print(f"ok: {len(expected)} public header symbols exported")
    else:
        print(f"ok: {len(expected)} public header symbols exported")
        print("extra-symbol classification is not implemented by this check")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
