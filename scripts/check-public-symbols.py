#!/usr/bin/env python3
"""Check exported Rust symbols against vendored mimalloc public ABI."""

from __future__ import annotations

import argparse
import dataclasses
import difflib
import os
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
MANIFEST_DIR = ROOT / "docs/abi-manifests"

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

LIBC_OVERRIDE_SYMBOLS = {
    "__libc_calloc",
    "__libc_cfree",
    "__libc_free",
    "__libc_malloc",
    "__libc_memalign",
    "__libc_pvalloc",
    "__libc_realloc",
    "__libc_valloc",
    "__posix_memalign",
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


@dataclasses.dataclass(frozen=True)
class DynamicManifest:
    artifact: str
    features: str
    path: pathlib.Path

    @property
    def build_command(self) -> str:
        return (
            "cargo rustc --lib "
            f"--features {self.features} "
            f"--target-dir target/symbol-check/{self.artifact} "
            "-- --crate-type cdylib"
        )


DYNAMIC_MANIFESTS = [
    DynamicManifest(
        artifact="c-abi",
        features="c-abi",
        path=MANIFEST_DIR / "c-abi-dynamic.tsv",
    ),
    DynamicManifest(
        artifact="c-abi-secure",
        features="c-abi secure",
        path=MANIFEST_DIR / "c-abi-secure-dynamic.tsv",
    ),
    DynamicManifest(
        artifact="c-abi-guarded",
        features="c-abi guarded",
        path=MANIFEST_DIR / "c-abi-guarded-dynamic.tsv",
    ),
    DynamicManifest(
        artifact="c-abi-track-valgrind",
        features="c-abi track-valgrind",
        path=MANIFEST_DIR / "c-abi-track-valgrind-dynamic.tsv",
    ),
    DynamicManifest(
        artifact="c-abi-track-asan",
        features="c-abi track-asan",
        path=MANIFEST_DIR / "c-abi-track-asan-dynamic.tsv",
    ),
    DynamicManifest(
        artifact="interpose",
        features="interpose",
        path=MANIFEST_DIR / "interpose-dynamic.tsv",
    ),
]


def feature_list() -> list[str]:
    features = os.environ.get("MIMALLOC_CHECK_FEATURES", "").split()
    return features


def feature_args() -> list[str]:
    features = feature_list()
    if not features:
        return []
    return ["--features", " ".join(features)]


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
    feature_key = "-".join(feature_list()) or "default"
    feature_key = re.sub(r"[^A-Za-z0-9_.-]+", "-", feature_key)
    target_dir = ROOT / "target/symbol-check-static" / feature_key
    subprocess.run(
        [
            "cargo",
            "rustc",
            "--lib",
            "--target-dir",
            str(target_dir),
            *feature_args(),
            "--",
            "--crate-type",
            "staticlib",
        ],
        cwd=ROOT,
        check=True,
    )
    libs = sorted(
        [
            *target_dir.glob("debug/deps/librimalloc-*.a"),
            *target_dir.glob("debug/deps/librimalloc.a"),
            *target_dir.glob("debug/librimalloc.a"),
        ],
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not libs:
        raise SystemExit("no Rust staticlib artifact found")
    return libs[0]


def rust_cdylib(features: str, artifact_name: str) -> pathlib.Path:
    target_dir = ROOT / f"target/symbol-check/{artifact_name}"
    subprocess.run(
        [
            "cargo",
            "rustc",
            "--lib",
            "--features",
            features,
            "--target-dir",
            str(target_dir),
            "--",
            "--crate-type",
            "cdylib",
        ],
        cwd=ROOT,
        check=True,
    )
    libs = sorted(
        [
            *target_dir.glob("debug/deps/librimalloc*.so"),
            *target_dir.glob("debug/librimalloc.so"),
        ],
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not libs:
        raise SystemExit("no Rust cdylib artifact found")
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


def dynamic_symbol_entries(cdylib: pathlib.Path) -> list[tuple[str, str, str, str, str]]:
    readelf = subprocess.run(
        ["readelf", "-W", "--dyn-syms", str(cdylib)],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    entries: dict[str, tuple[str, str, str, str, str]] = {}
    for line in readelf.stdout.splitlines():
        parts = line.split()
        if len(parts) < 8:
            continue
        index, _value, _size, kind, bind, visibility, section, name = parts[:8]
        if not index.endswith(":"):
            continue
        if section == "UND" or bind not in {"GLOBAL", "WEAK"} or visibility != "DEFAULT":
            continue
        if kind not in {"FUNC", "OBJECT", "NOTYPE"}:
            continue
        symbol, version = parse_symbol_version(name)
        entries[symbol] = (symbol, bind, visibility, version, kind)
    return [entries[symbol] for symbol in sorted(entries)]


def parse_symbol_version(name: str) -> tuple[str, str]:
    symbol, separator, version = name.partition("@")
    if not separator:
        return name, "-"
    return symbol, version.lstrip("@") or "-"


def dynamic_symbols(cdylib: pathlib.Path) -> set[str]:
    return {entry[0] for entry in dynamic_symbol_entries(cdylib)}


def render_dynamic_manifest(manifest: DynamicManifest) -> str:
    entries = dynamic_symbol_entries(rust_cdylib(manifest.features, manifest.artifact))
    lines = [
        "# rimalloc dynamic ABI manifest",
        f"# artifact: {manifest.artifact}",
        f"# build-command: {manifest.build_command}",
        "# columns: symbol\tbinding\tvisibility\tversion\ttype",
        "symbol\tbinding\tvisibility\tversion\ttype",
    ]
    lines.extend("\t".join(entry) for entry in entries)
    return "\n".join(lines) + "\n"


def manifest_diff(expected: str, actual: str, path: pathlib.Path) -> list[str]:
    return list(
        difflib.unified_diff(
            expected.splitlines(),
            actual.splitlines(),
            fromfile=str(path),
            tofile="current",
            lineterm="",
        )
    )


def check_dynamic_manifests(write: bool) -> int:
    MANIFEST_DIR.mkdir(parents=True, exist_ok=True)
    failed = False
    for manifest in DYNAMIC_MANIFESTS:
        actual = render_dynamic_manifest(manifest)
        if write:
            manifest.path.write_text(actual)
            print(f"wrote {manifest.path.relative_to(ROOT)}")
            continue
        if not manifest.path.exists():
            print(f"missing dynamic ABI manifest: {manifest.path}", file=sys.stderr)
            failed = True
            continue
        expected = manifest.path.read_text()
        diff = manifest_diff(expected, actual, manifest.path)
        if diff:
            print(
                f"dynamic ABI manifest differs: {manifest.path.relative_to(ROOT)}",
                file=sys.stderr,
            )
            for line in diff:
                print(line, file=sys.stderr)
            failed = True
    if failed:
        return 1
    if not write:
        print(f"ok: {len(DYNAMIC_MANIFESTS)} dynamic ABI manifests match")
    return 0


def self_test_dynamic_manifest_diff() -> int:
    manifest = DYNAMIC_MANIFESTS[0]
    actual = render_dynamic_manifest(manifest)
    injected = actual + "zz_injected_symbol\tGLOBAL\tDEFAULT\t-\tFUNC\n"
    diff = manifest_diff(injected, actual, manifest.path)
    if not diff:
        print("dynamic manifest self-test failed to detect injected symbol", file=sys.stderr)
        return 1
    print("ok: dynamic manifest diff self-test detects an injected extra symbol")
    return 0


def check_dynamic_mi_exports() -> int:
    expected = set(public_header_symbols())
    exported = {
        symbol
        for symbol in dynamic_symbols(rust_cdylib("c-abi", "c-abi"))
        if symbol.startswith("mi_")
    }
    missing = sorted(expected - exported)
    extra = sorted(exported - expected)
    if missing:
        print("missing dynamic public mi_* symbols:", file=sys.stderr)
        for symbol in missing:
            print(symbol, file=sys.stderr)
    if extra:
        print("unexpected dynamic mi_* symbols:", file=sys.stderr)
        for symbol in extra:
            print(symbol, file=sys.stderr)
    if missing or extra:
        return 1
    print(f"ok: c-abi cdylib exports exactly {len(expected)} public header mi_* symbols")
    return 0


def report_symbols(message: str, symbols: list[str]) -> None:
    print(message, file=sys.stderr)
    for symbol in symbols:
        print(symbol, file=sys.stderr)


def check_dynamic_export_groups() -> int:
    c_abi = dynamic_symbols(rust_cdylib("c-abi", "c-abi"))
    cxx = dynamic_symbols(rust_cdylib("cxx-overrides", "cxx-overrides"))
    interpose = dynamic_symbols(rust_cdylib("interpose", "interpose"))

    failed = False
    c_abi_extra = sorted(
        (MALLOC_FAMILY_SYMBOLS | CXX_OPERATOR_OVERRIDE_SYMBOLS | LIBC_OVERRIDE_SYMBOLS) & c_abi
    )
    if c_abi_extra:
        report_symbols("c-abi cdylib unexpectedly exports override symbols:", c_abi_extra)
        failed = True

    cxx_malloc_extra = sorted((MALLOC_FAMILY_SYMBOLS | LIBC_OVERRIDE_SYMBOLS) & cxx)
    if cxx_malloc_extra:
        report_symbols(
            "cxx-overrides cdylib unexpectedly exports malloc-family symbols:",
            cxx_malloc_extra,
        )
        failed = True

    cxx_missing = sorted(CXX_OPERATOR_OVERRIDE_SYMBOLS - cxx)
    if cxx_missing:
        report_symbols("cxx-overrides cdylib is missing C++ operator symbols:", cxx_missing)
        failed = True

    interpose_missing = sorted(
        (MALLOC_OVERRIDE_SYMBOLS | CXX_OPERATOR_OVERRIDE_SYMBOLS | LIBC_OVERRIDE_SYMBOLS)
        - interpose
    )
    if interpose_missing:
        report_symbols(
            "interpose cdylib is missing allocator override symbols:",
            interpose_missing,
        )
        failed = True

    if failed:
        return 1

    print("ok: c-abi cdylib excludes malloc-family and C++ override symbols")
    print("ok: cxx-overrides cdylib exports C++ overrides without malloc-family symbols")
    print("ok: interpose cdylib exports malloc-family and C++ override symbols")
    return 0


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
    parser.add_argument(
        "--dynamic-mi",
        action="store_true",
        help="check c-abi cdylib dynamic mi_* exports against public headers",
    )
    parser.add_argument(
        "--dynamic-groups",
        action="store_true",
        help="check dynamic cdylib export groups for c-abi, cxx-overrides, and interpose",
    )
    parser.add_argument(
        "--dynamic-manifests",
        action="store_true",
        help="check dynamic cdylib exports against checked manifests",
    )
    parser.add_argument(
        "--write-dynamic-manifests",
        action="store_true",
        help="rewrite checked dynamic cdylib export manifests",
    )
    parser.add_argument(
        "--dynamic-manifest-self-test",
        action="store_true",
        help="verify dynamic manifest diffing fails on an injected extra symbol",
    )
    args = parser.parse_args()

    if args.dynamic_mi:
        return check_dynamic_mi_exports()
    if args.dynamic_groups:
        return check_dynamic_export_groups()
    if args.dynamic_manifests:
        return check_dynamic_manifests(write=False)
    if args.write_dynamic_manifests:
        return check_dynamic_manifests(write=True)
    if args.dynamic_manifest_self_test:
        return self_test_dynamic_manifest_diff()

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
