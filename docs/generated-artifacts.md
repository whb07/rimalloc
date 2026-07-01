# Generated Artifact Hygiene

Generated files are either checked-in release evidence with a reproducible
owner, or local-only scratch output that stays ignored. Do not commit ad hoc
analysis files unless the command that regenerates them is documented here and
the path is covered by an audit or release check.

## Checked-In Evidence

| Path | Owner command | Verification |
| --- | --- | --- |
| `docs/abi-manifests/*.tsv` | `scripts/check-public-symbols.py --write-dynamic-manifests` | `scripts/check-abi-symbols.sh` |
| `benchmarks/thresholds.json` | Maintained with benchmark policy updates | `scripts/check-bench-smoke.sh` and `scripts/check-bench-full.sh` |
| `NO_ALLOCATION_CRITICAL_PATHS.json` | Maintained with no-allocation audit updates | `scripts/check-no-allocation-audit.py` |
| `docs/unsafe-migration-allowlist.tsv` | Maintained with unsafe migration work | `scripts/check-unsafe-audit.py` |
| `docs/unsafe-audit-evidence.tsv` | Maintained with unsafe evidence updates | `scripts/check-unsafe-audit-evidence.py` |

## Local-Only Outputs

These paths are generated during local development or release dry runs and must
remain out of commits:

- `target/`, including `target/criterion`, `target/bench-full/`, package
  archives, and `target/release-artifacts/`.
- `fuzz/target/`.
- `mimalloc-bench/`, used for local native benchmark checkouts and outputs.
- `RUST_DEPENDENCY_EDGES.csv`, used for ad hoc dependency graph experiments.
- `parse_bench*.py`, used for temporary benchmark parser experiments.
- Python caches, debugger dumps, patch scratch files, and profiling coverage
  files such as `*.profraw`, `*.profdata`, `*.gcda`, and `*.gcno`.

If a local artifact becomes release evidence, move it under a stable checked-in
path, add the regeneration command to the table above, and add or update the
script that verifies it.

## Release Outputs

Release artifacts are generated from a clean checkout with:

```sh
scripts/build-release-artifacts.py
```

The generated payload lives under `target/release-artifacts/`; release docs
define which files are attached to the release. Benchmark threshold CSVs for
release candidates are regenerated with:

```sh
scripts/check-bench-full.sh
```

They stay in `target/bench-full/.../criterion/` unless a release report
explicitly links or attaches them as external evidence.

## Hygiene Check

Run the centralized guard before committing audit or benchmark work:

```sh
scripts/check-generated-artifacts.py
```

The checker verifies required local-only ignore rules, reports untracked
generated-looking files outside approved paths, and ensures this document keeps
the owner commands for checked-in generated evidence.
