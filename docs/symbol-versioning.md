# Symbol and Versioning Policy

This page defines the Tier C symbol policy for release signoff. It applies to
the explicit C ABI artifacts built from the tagged repository checkout and to
the checked manifests under `docs/abi-manifests/`.

Default Cargo builds produce an `rlib` for Rust integration. C ABI symbol
policy applies only when an explicit `cdylib` or preview `staticlib` artifact is
built with the documented feature set.

## Public C Header Symbols

The public C header symbol set is the ordered set of `mi_*` function
declarations extracted from:

- `mimalloc-3.3.2/include/mimalloc.h`
- `mimalloc-3.3.2/include/mimalloc-stats.h`

`scripts/check-public-symbols.py --dynamic-mi` generates this set from the
vendored headers and verifies that the clean `c-abi` cdylib exports exactly
those public-header `mi_*` symbols. Header-private helpers, translated Rust
helpers, and extension candidates are not part of the default dynamic C ABI
just because their spelling starts with `mi_`.

Intentional changes to the public C header symbol set require all of the
following:

- a vendored-header update or an explicit extension ABI decision;
- regenerated checked dynamic manifests with
  `scripts/check-public-symbols.py --write-dynamic-manifests`;
- `scripts/check-abi-symbols.sh` passing after regeneration;
- a changelog ABI marker when the change affects a previous release baseline.

## Malloc-Family Interposition Symbols

Malloc-family symbols are override symbols, not clean C ABI symbols. The clean
`c-abi` cdylib must not export them. The `interpose` cdylib must export the
malloc-family and libc override groups required by the Linux interposition
mode.

The policy source of truth is `scripts/check-public-symbols.py`, which defines
and checks `MALLOC_OVERRIDE_SYMBOLS`, `LIBC_OVERRIDE_SYMBOLS`, and
`MALLOC_FAMILY_SYMBOLS`. The current release policy treats these names as
interpose-only dynamic ABI:

- `malloc`, `calloc`, `realloc`, `free`, `cfree`
- `aligned_alloc`, `posix_memalign`, `memalign`, `valloc`, `pvalloc`
- `malloc_size`, `malloc_usable_size`
- libc aliases such as `__libc_malloc`, `__libc_free`, and
  `__posix_memalign` when required by the interposition artifact

Some malloc-family compatibility spellings are audited for static archive or
platform compatibility, including `malloc_good_size`, `reallocarray`,
`reallocarr`, `reallocf`, `strdup`, `strndup`, and `realpath`. Their presence
outside an interposition artifact must be classified by
`symbols/public-symbol-classification.tsv` or rejected by the symbol checks.

## C++ Operator Symbols

C++ operator new/delete symbols are override symbols. They are not part of the
clean `c-abi` cdylib. They are expected in `cxx-overrides` and `interpose`
cdylibs when the C++ shim is linked.

`scripts/check-public-symbols.py` defines `CXX_OPERATOR_OVERRIDE_SYMBOLS` as the
Itanium ABI operator set:

- `_Znwm`, `_ZnwmRKSt9nothrow_t`, `_ZnwmSt11align_val_t`,
  `_ZnwmSt11align_val_tRKSt9nothrow_t`
- `_Znam`, `_ZnamRKSt9nothrow_t`, `_ZnamSt11align_val_t`,
  `_ZnamSt11align_val_tRKSt9nothrow_t`
- `_ZdlPv`, `_ZdlPvRKSt9nothrow_t`, `_ZdlPvSt11align_val_t`,
  `_ZdlPvSt11align_val_tRKSt9nothrow_t`, `_ZdlPvm`,
  `_ZdlPvmSt11align_val_t`
- `_ZdaPv`, `_ZdaPvRKSt9nothrow_t`, `_ZdaPvSt11align_val_t`,
  `_ZdaPvSt11align_val_tRKSt9nothrow_t`, `_ZdaPvm`,
  `_ZdaPvmSt11align_val_t`

`scripts/check-public-symbols.py --dynamic-groups` verifies the artifact split:

- `c-abi` excludes malloc-family and C++ override symbols;
- `cxx-overrides` exports C++ override symbols without malloc-family symbols;
- `interpose` exports malloc-family, libc override, and C++ override symbols.

`scripts/check-release-cxx-artifacts.py` is the release consumer check for C++
operator behavior.

## Extension ABI Naming Rules

There is no supported extension ABI in the current release policy.

Rust-only `mi_*` helpers and platform helper symbols classified as
`rust-extension` or `platform-helper-extension` in
`symbols/public-symbol-classification.tsv` are audit classifications only. They
are hidden from the default dynamic C ABI and are retained only for internal
Rust reachability, static/no-`c-abi` compatibility audits, and pre-release
migration visibility.

A future extension ABI must use one of these decisions before release:

- promote a symbol only after documenting ownership, nullability, threading,
  error behavior, and stability in this page and `docs/abi.md`;
- prefer a `rimalloc_*` prefix for Rust-specific extensions so they
  cannot be confused with upstream mimalloc public-header symbols;
- avoid adding new public `mi_*` extension spellings unless they intentionally
  become part of the mimalloc-compatible C ABI contract.

Unclassified exported `mi_*`, `_mi_*`, malloc-family, libc override, or C++
operator symbols are release blockers.

## Static Archive Internal-Symbol Policy

Static archives are preview-only. The current static archive surface is broader
than the production dynamic C ABI because translated modules can still require
C-shaped internal linkage.

The TSV classification file defines the current internal static archive escape
hatches:

- `internal-export-required`: internal `_mi_*` and related helpers needed by
  translated code, not stable public API;
- `rust-extension`: Rust-only `mi_*` helpers retained for audits and migration
  visibility, not default dynamic ABI;
- `platform-helper-extension`: platform helper `mi_*` symbols retained for
  audits and internal reachability, not default dynamic ABI.

Preview static archives must keep preview-marked artifact names and release
notes. They cannot be promoted until the static symbol contract, collision
tests, and checked static manifests are complete.

## Release-to-Release ABI Diff Expectations

Checked dynamic manifests live under `docs/abi-manifests/`. They record each
tracked cdylib symbol's name, binding, visibility, version, type, and build
command. The current manifest set covers:

- `c-abi-dynamic.tsv`
- `c-abi-secure-dynamic.tsv`
- `c-abi-guarded-dynamic.tsv`
- `c-abi-track-valgrind-dynamic.tsv`
- `c-abi-track-asan-dynamic.tsv`
- `interpose-dynamic.tsv`

Release candidates must compare the current manifests with the previous release
baseline:

```sh
scripts/check-abi-diff.py --previous-ref <previous-tag>
```

or, for an unpacked previous release asset directory:

```sh
scripts/check-abi-diff.py --previous-dir <previous-release>/abi-manifests
```

The ABI diff check classifies added, removed, possible-renamed,
visibility-changed, binding-changed, version-changed, and type-changed symbols.
Every intentional ABI difference must be recorded in `CHANGELOG.md` with:

```text
ABI-CHANGE: <manifest> <symbol>
```

Use `ABI-CHANGE: <manifest> *` only for an explicitly reviewed whole-manifest
baseline change.

## Required Checks

Run the symbol policy checks before release signoff:

```sh
scripts/check-abi-symbols.sh
scripts/check-public-symbols.py --dynamic-mi
scripts/check-public-symbols.py --dynamic-groups
scripts/check-public-symbols.py --dynamic-manifests
scripts/check-abi-diff.py --previous-ref <previous-tag>
```

For local policy drift review, the symbol scripts compare built artifacts with
the checked manifests and classification TSV.
