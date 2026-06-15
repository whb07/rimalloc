#!/usr/bin/env python3
"""Check exported Rust symbols against vendored mimalloc public ABI."""

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
CLASSIFICATION_FILE = ROOT / "symbols/public-symbol-classification.tsv"

# These names and the Itanium C++ operators are actual externally visible
# override symbols in the C `MI_MALLOC_OVERRIDE` artifact on Linux.
MALLOC_OVERRIDE_SYMBOLS = {
    "aligned_alloc",
    "calloc",
    "cfree",
    "free",
    "malloc",
    "malloc_size",
    "malloc_usable_size",
    "memalign",
    "posix_memalign",
    "pvalloc",
    "realloc",
    "reallocarray",
    "valloc",
}

CXX_OPERATOR_OVERRIDE_SYMBOLS = {
    "_ZdaPv",
    "_ZdaPvRKSt9nothrow_t",
    "_ZdaPvSt11align_val_t",
    "_ZdaPvSt11align_val_tRKSt9nothrow_t",
    "_ZdaPvm",
    "_ZdaPvmSt11align_val_t",
    "_ZdlPv",
    "_ZdlPvRKSt9nothrow_t",
    "_ZdlPvSt11align_val_t",
    "_ZdlPvSt11align_val_tRKSt9nothrow_t",
    "_ZdlPvm",
    "_ZdlPvmSt11align_val_t",
    "_Znam",
    "_ZnamRKSt9nothrow_t",
    "_ZnamSt11align_val_t",
    "_ZnamSt11align_val_tRKSt9nothrow_t",
    "_Znwm",
    "_ZnwmRKSt9nothrow_t",
    "_ZnwmSt11align_val_t",
    "_ZnwmSt11align_val_tRKSt9nothrow_t",
}

MALLOC_FAMILY_SYMBOLS = MALLOC_OVERRIDE_SYMBOLS | {
    "cfree",
    "posix_memalign",
    "_posix_memalign",
    "memalign",
    "valloc",
    "pvalloc",
    "aligned_alloc",
    "reallocarray",
    "reallocarr",
    "reallocf",
    "strdup",
    "strndup",
    "realpath",
    "malloc_size",
    "malloc_usable_size",
    "malloc_good_size",
}

STATIC_MALLOC_EXTRA_SYMBOLS = {
    "__posix_memalign",
    "malloc_good_size",
    "reallocf",
    "strdup",
    "strndup",
}


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
        [
            *(ROOT / "target/debug/deps").glob("libmechanical_mimalloc-*.a"),
            *(ROOT / "target/debug/deps").glob("libmechanical_mimalloc.a"),
            *(ROOT / "target/debug").glob("libmechanical_mimalloc.a"),
        ],
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

    readelf = subprocess.run(
        ["readelf", "-Ws", str(staticlib)],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    hidden: set[str] = set()
    for line in readelf.stdout.splitlines():
        parts = line.split()
        if len(parts) >= 8 and parts[4] == "GLOBAL" and parts[5] == "HIDDEN":
            hidden.add(parts[7])
    return symbols - hidden


def is_relevant_export(symbol: str) -> bool:
    if symbol.startswith(("mi_", "_mi_")):
        return True
    if symbol in MALLOC_FAMILY_SYMBOLS:
        return True
    # Itanium operator new/delete symbols are relevant, while Rust and libstd
    # symbols also start with _Z and are filtered out by excluding namespaces.
    return symbol.startswith("_Z") and not symbol.startswith("_ZN")


def classified_extra_symbols(exported: set[str]) -> set[str]:
    exact: set[str] = set()
    prefixes: list[str] = []
    if not CLASSIFICATION_FILE.exists():
        raise SystemExit(f"missing classification file: {CLASSIFICATION_FILE}")

    for lineno, raw_line in enumerate(CLASSIFICATION_FILE.read_text().splitlines(), start=1):
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        parts = raw_line.split("\t")
        if len(parts) != 5:
            raise SystemExit(
                f"{CLASSIFICATION_FILE}:{lineno}: expected 5 tab-separated fields"
            )
        match_kind, symbol, classification, _source, rationale = parts
        if not symbol or not classification or not rationale:
            raise SystemExit(f"{CLASSIFICATION_FILE}:{lineno}: empty required field")
        if match_kind == "exact":
            exact.add(symbol)
        elif match_kind == "prefix":
            prefixes.append(symbol)
        else:
            raise SystemExit(
                f"{CLASSIFICATION_FILE}:{lineno}: match kind must be exact or prefix"
            )

    return {
        symbol
        for symbol in exported
        if symbol in exact or any(symbol.startswith(prefix) for prefix in prefixes)
    }


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
        expected_abi = (
            set(expected)
            | MALLOC_OVERRIDE_SYMBOLS
            | STATIC_MALLOC_EXTRA_SYMBOLS
            | CXX_OPERATOR_OVERRIDE_SYMBOLS
        )
        missing_abi = sorted(symbol for symbol in expected_abi if symbol not in exported)
        if missing_abi:
            print("missing public ABI symbols:", file=sys.stderr)
            for symbol in missing_abi:
                print(symbol, file=sys.stderr)
            return 1

        relevant = {symbol for symbol in exported if is_relevant_export(symbol)}
        explained = expected_abi | classified_extra_symbols(exported)
        unexplained = sorted(relevant - explained)
        if unexplained:
            print("unexplained relevant exported symbols:", file=sys.stderr)
            for symbol in unexplained:
                print(symbol, file=sys.stderr)
            return 1

        extra_count = len(relevant - expected_abi)
        print(f"ok: {len(expected)} public header symbols exported")
        print(
            "ok: "
            f"{len(expected_abi)} public ABI symbols present; "
            f"{extra_count} relevant extra exports classified"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
