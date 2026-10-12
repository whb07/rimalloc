#!/usr/bin/env python3
"""Compare checked ABI manifests against a previous release baseline."""

from __future__ import annotations

import argparse
import dataclasses
import difflib
import pathlib
import subprocess
import sys
import tempfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
DEFAULT_MANIFEST_DIR = ROOT / "docs/abi-manifests"
DEFAULT_CHANGELOG = ROOT / "CHANGELOG.md"


@dataclasses.dataclass(frozen=True)
class SymbolEntry:
    symbol: str
    binding: str
    visibility: str
    version: str
    kind: str

    @property
    def signature(self) -> tuple[str, str, str, str]:
        return (self.binding, self.visibility, self.version, self.kind)


@dataclasses.dataclass(frozen=True)
class AbiChange:
    manifest: str
    classification: str
    symbol: str
    detail: str

    @property
    def marker(self) -> str:
        return f"ABI-CHANGE: {self.manifest} {self.symbol}"


def manifest_paths(directory: pathlib.Path) -> set[str]:
    if not directory.exists():
        return set()
    return {
        str(path.relative_to(directory))
        for path in directory.rglob("*.tsv")
        if path.is_file()
    }


def git_manifest_paths(ref: str) -> set[str]:
    result = subprocess.run(
        ["git", "ls-tree", "-r", "--name-only", ref, "--", "docs/abi-manifests"],
        cwd=ROOT,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    prefix = "docs/abi-manifests/"
    return {
        path.removeprefix(prefix)
        for path in result.stdout.splitlines()
        if path.startswith(prefix) and path.endswith(".tsv")
    }


def read_git_manifest(ref: str, relpath: str) -> str | None:
    result = subprocess.run(
        ["git", "show", f"{ref}:docs/abi-manifests/{relpath}"],
        cwd=ROOT,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
    )
    if result.returncode != 0:
        return None
    return result.stdout


def read_dir_manifest(directory: pathlib.Path, relpath: str) -> str | None:
    path = directory / relpath
    if not path.exists():
        return None
    return path.read_text()


def parse_manifest(relpath: str, text: str) -> dict[str, SymbolEntry]:
    entries: dict[str, SymbolEntry] = {}
    for lineno, raw_line in enumerate(text.splitlines(), start=1):
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        if line == "symbol\tbinding\tvisibility\tversion\ttype":
            continue
        parts = raw_line.split("\t")
        if len(parts) != 5:
            raise SystemExit(f"{relpath}:{lineno}: expected 5 tab-separated fields")
        symbol, binding, visibility, version, kind = parts
        if not all(parts):
            raise SystemExit(f"{relpath}:{lineno}: empty ABI manifest field")
        entries[symbol] = SymbolEntry(symbol, binding, visibility, version, kind)
    return entries


def possible_renames(
    removed: list[SymbolEntry], added: list[SymbolEntry]
) -> list[tuple[SymbolEntry, SymbolEntry]]:
    matches: list[tuple[SymbolEntry, SymbolEntry]] = []
    used_added: set[str] = set()
    for old in removed:
        candidates = [
            new
            for new in added
            if new.signature == old.signature and new.symbol not in used_added
        ]
        if not candidates:
            continue
        new = max(
            candidates,
            key=lambda candidate: difflib.SequenceMatcher(
                None, old.symbol, candidate.symbol
            ).ratio(),
        )
        score = difflib.SequenceMatcher(None, old.symbol, new.symbol).ratio()
        if score >= 0.82:
            used_added.add(new.symbol)
            matches.append((old, new))
    return matches


def compare_manifest(relpath: str, previous: str | None, current: str | None) -> list[AbiChange]:
    changes: list[AbiChange] = []
    previous_entries = parse_manifest(relpath, previous) if previous is not None else {}
    current_entries = parse_manifest(relpath, current) if current is not None else {}

    removed = [
        previous_entries[symbol]
        for symbol in sorted(previous_entries.keys() - current_entries.keys())
    ]
    added = [
        current_entries[symbol]
        for symbol in sorted(current_entries.keys() - previous_entries.keys())
    ]

    for entry in removed:
        changes.append(AbiChange(relpath, "removed", entry.symbol, "symbol removed"))
    for entry in added:
        changes.append(AbiChange(relpath, "added", entry.symbol, "symbol added"))
    for old, new in possible_renames(removed, added):
        changes.append(
            AbiChange(
                relpath,
                "possible-renamed",
                f"{old.symbol}->{new.symbol}",
                "removed and added symbols have matching ABI attributes and similar names",
            )
        )

    for symbol in sorted(previous_entries.keys() & current_entries.keys()):
        old = previous_entries[symbol]
        new = current_entries[symbol]
        if old.visibility != new.visibility:
            changes.append(
                AbiChange(
                    relpath,
                    "visibility-changed",
                    symbol,
                    f"{old.visibility} -> {new.visibility}",
                )
            )
        if old.binding != new.binding:
            changes.append(
                AbiChange(relpath, "binding-changed", symbol, f"{old.binding} -> {new.binding}")
            )
        if old.version != new.version:
            changes.append(
                AbiChange(relpath, "version-changed", symbol, f"{old.version} -> {new.version}")
            )
        if old.kind != new.kind:
            changes.append(AbiChange(relpath, "type-changed", symbol, f"{old.kind} -> {new.kind}"))

    return changes


def changelog_text(path: pathlib.Path) -> str:
    if not path.exists():
        return ""
    return path.read_text()


def missing_changelog_markers(changes: list[AbiChange], changelog: str) -> list[str]:
    missing: list[str] = []
    for change in changes:
        manifest_wildcard = f"ABI-CHANGE: {change.manifest} *"
        if change.marker not in changelog and manifest_wildcard not in changelog:
            missing.append(change.marker)
    return sorted(set(missing))


def print_changes(changes: list[AbiChange]) -> None:
    for change in changes:
        print(
            f"{change.manifest}: {change.classification}: "
            f"{change.symbol} ({change.detail})"
        )


def compare_sources(
    current_dir: pathlib.Path,
    changelog: pathlib.Path,
    previous_ref: str | None,
    previous_dir: pathlib.Path | None,
    allow_missing_changelog: bool,
) -> int:
    current_paths = manifest_paths(current_dir)
    if previous_ref is not None:
        previous_paths = git_manifest_paths(previous_ref)
        read_previous = lambda relpath: read_git_manifest(previous_ref, relpath)
    elif previous_dir is not None:
        previous_paths = manifest_paths(previous_dir)
        read_previous = lambda relpath: read_dir_manifest(previous_dir, relpath)
    else:
        raise SystemExit("expected --previous-ref or --previous-dir")

    all_paths = sorted(previous_paths | current_paths)
    if not all_paths:
        raise SystemExit("no ABI manifests found to compare")

    changes: list[AbiChange] = []
    for relpath in all_paths:
        changes.extend(
            compare_manifest(
                relpath,
                read_previous(relpath),
                read_dir_manifest(current_dir, relpath),
            )
        )

    if not changes:
        print(f"ok: {len(all_paths)} ABI manifests match the previous release baseline")
        return 0

    print_changes(changes)
    if allow_missing_changelog:
        return 1

    missing = missing_changelog_markers(changes, changelog_text(changelog))
    if missing:
        print("missing ABI changelog markers:", file=sys.stderr)
        for marker in missing:
            print(marker, file=sys.stderr)
        print(
            "Add markers under the changelog ABI section, or use "
            "`ABI-CHANGE: <manifest> *` for an explicitly reviewed full-manifest change.",
            file=sys.stderr,
        )
        return 1

    print(f"ok: {len(changes)} ABI changes are classified in the changelog")
    return 0


def manifest_text(entries: list[SymbolEntry]) -> str:
    lines = [
        "# mechanical_mimalloc dynamic ABI manifest",
        "# artifact: self-test",
        "# build-command: test",
        "# columns: symbol\tbinding\tvisibility\tversion\ttype",
        "symbol\tbinding\tvisibility\tversion\ttype",
    ]
    lines.extend(
        f"{entry.symbol}\t{entry.binding}\t{entry.visibility}\t{entry.version}\t{entry.kind}"
        for entry in entries
    )
    return "\n".join(lines) + "\n"


def self_test() -> int:
    with tempfile.TemporaryDirectory() as temp:
        root = pathlib.Path(temp)
        previous = root / "previous"
        current = root / "current"
        previous.mkdir()
        current.mkdir()
        relpath = "c-abi-dynamic.tsv"
        (previous / relpath).write_text(
            manifest_text(
                [
                    SymbolEntry("mi_malloc", "GLOBAL", "DEFAULT", "-", "FUNC"),
                    SymbolEntry("mi_allocator_v1", "GLOBAL", "DEFAULT", "-", "FUNC"),
                    SymbolEntry("mi_visibility", "GLOBAL", "DEFAULT", "-", "FUNC"),
                ]
            )
        )
        (current / relpath).write_text(
            manifest_text(
                [
                    SymbolEntry("mi_added", "GLOBAL", "DEFAULT", "-", "FUNC"),
                    SymbolEntry("mi_malloc", "GLOBAL", "DEFAULT", "-", "FUNC"),
                    SymbolEntry("mi_allocator_v2", "GLOBAL", "DEFAULT", "-", "FUNC"),
                    SymbolEntry("mi_visibility", "GLOBAL", "HIDDEN", "-", "FUNC"),
                ]
            )
        )
        changelog = root / "CHANGELOG.md"
        changes = compare_manifest(
            relpath,
            read_dir_manifest(previous, relpath),
            read_dir_manifest(current, relpath),
        )
        classifications = {change.classification for change in changes}
        required = {"added", "removed", "possible-renamed", "visibility-changed"}
        if not required.issubset(classifications):
            print(
                f"ABI diff self-test missed classifications: {required - classifications}",
                file=sys.stderr,
            )
            return 1
        changelog.write_text("# Changelog\n\n## Unreleased\n\n### ABI\n")
        if not missing_changelog_markers(changes, changelog_text(changelog)):
            print("ABI diff self-test failed to require changelog markers", file=sys.stderr)
            return 1
        changelog.write_text(
            "# Changelog\n\n## Unreleased\n\n### ABI\n\n"
            f"- Intentional ABI review.\n\nABI-CHANGE: {relpath} *\n"
        )
        if missing_changelog_markers(changes, changelog_text(changelog)):
            print("ABI diff self-test failed to accept wildcard marker", file=sys.stderr)
            return 1
    print("ok: ABI diff self-test classifies changes and requires changelog markers")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    source = parser.add_mutually_exclusive_group()
    source.add_argument("--previous-ref", help="previous release git ref or tag")
    source.add_argument(
        "--previous-dir",
        type=pathlib.Path,
        help="directory containing previous release ABI manifest files",
    )
    parser.add_argument(
        "--current-dir",
        type=pathlib.Path,
        default=DEFAULT_MANIFEST_DIR,
        help="directory containing current ABI manifest files",
    )
    parser.add_argument(
        "--changelog",
        type=pathlib.Path,
        default=DEFAULT_CHANGELOG,
        help="changelog file containing ABI-CHANGE markers",
    )
    parser.add_argument(
        "--allow-missing-changelog",
        action="store_true",
        help="report ABI differences without requiring changelog markers",
    )
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="run local ABI diff classification and changelog-marker tests",
    )
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    return compare_sources(
        args.current_dir,
        args.changelog,
        args.previous_ref,
        args.previous_dir,
        args.allow_missing_changelog,
    )


if __name__ == "__main__":
    raise SystemExit(main())
