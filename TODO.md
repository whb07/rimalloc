# TODO: Fresh Rust Idiom and Release-Readiness Backlog

Fresh-eyes review performed after completing the first Rust idiom backlog.
The crate is materially less mechanical now, but the next issues are still
visible in source, docs, and validation scripts.

Review evidence:

- `scripts/check-unsafe-audit.py` passes, but reports 64 legacy source/test
  files still on the dated unsafe migration allowlist.
- `Cargo.toml` keeps broad package-level clippy allowances to support translated
  code while local modules slowly become idiomatic.
- `src/internal/mod.rs` still exposes internal helper macros with
  `#[macro_export]`, which leaks C-shaped implementation helpers at crate root.
- Large translated modules such as `src/allocator/arena.rs`,
  `src/platform/windows.rs`, and `src/internal/mod.rs` remain hard to review as
  Rust modules.
- Safe object ownership APIs such as `MiBox<T>` and `HeapBox<'heap, T>` are
  deliberately deferred in docs and release checks.
- Generated analysis files and benchmark outputs need a single reproducible
  owner so repository state stays clean during repeated audit work.

## Issue: Unsafe migration allowlist is still too broad

> **Comment:** The audit is no longer missing coverage, but a large dated
> allowlist still means many unsafe operations are accepted because the file is
> legacy translated code. The next step is shrinking that exception surface by
> subsystem, starting with safe-facing and typed-wrapper modules.

- [x] Split `docs/unsafe-migration-allowlist.tsv` into production, test, and
      generated/translation categories, or add an equivalent category column.
- [x] Remove `src/safe/allocation.rs`, `src/safe/heap.rs`, `src/safe/mod.rs`,
      `src/safe/stats.rs`, and `src/ffi/callbacks.rs` from the allowlist by
      tagging remaining unsafe operations with specific `SAFETY[AUDIT:*]`
      evidence.
- [x] Burn down typed-wrapper test files separately from production typed
      wrappers so test scaffolding does not hide production unsafe debt.
- [x] Add an audit test that caps the allowlist count and requires the cap to
      decrease when entries are removed.
- [x] Keep hot allocator files allowlisted only with explicit performance and
      parity rationale until they can be migrated safely.
  - Evidence: `docs/unsafe-migration-allowlist.tsv` now has a required
    `category` column with `production`, `test`, and `translation` entries.
    The safe-facing files `src/safe/allocation.rs`, `src/safe/heap.rs`,
    `src/safe/mod.rs`, and `src/safe/stats.rs` were removed after their unsafe
    operations were covered by nearby `SAFETY[AUDIT:*]` comments;
    `src/ffi/callbacks.rs` was already fully tagged and absent from the
    allowlist. `scripts/check-unsafe-audit.py` enforces the category set,
    requires all categories to be present, and caps the remaining allowlist at
    64 entries so future removals must lower the cap. Hot allocator allowlist
    reasons now call out parity plus assembly/benchmark or behavior evidence.
- Verify: `scripts/check-unsafe-audit.py`, `scripts/check-unsafe-audit-evidence.py`,
  full clippy, and full tests.

## Issue: Internal macro exports leak implementation details

> **Comment:** `#[macro_export]` makes macros part of the crate-root namespace.
> That is appropriate for documented Tier R C-header compatibility macros, but
> many helper macros in `src/internal/mod.rs` are mechanical implementation
> details.

- [x] Inventory all `#[macro_export]` macros and classify them as Tier R public
      compatibility macros or crate-internal helpers.
- [x] Replace internal-only exported helper macros with crate-local macros or
      small functions where that preserves hot-path codegen.
- [x] Keep documented C-header parity macros feature-gated behind
      `c-header-macros`.
- [x] Add an audit test that rejects new root-exported macros unless they appear
      in the documented Tier R allowlist.
- [x] Run API and ABI inventory checks after each macro visibility change.
  - Evidence: `scripts/check-api-inventory.py` now classifies only the two
    documented Tier R macro groups as root-exported macros, verifies each
    macro is gated by `c-header-macros`, and rejects exports from any source
    file other than `src/ffi/abi.rs` or `src/ffi/headers.rs`. Internal
    assertion helpers are crate-local under `crate::internal`, the atomic guard
    helper is crate-local under `crate::atomic`, unused translated macro shims
    were removed, and `MI_UNUSED`/`MI_UNUSED_RELEASE` call sites now use the
    private no-op functions. `scripts/check-feature-abi.sh`,
    `cargo test --test header_macro_expansion --all-features`, and the focused
    release audit passed. `cargo semver-checks` was not installed in this
    environment, so the explicit public API diff command could not be run.
- Verify: `scripts/check-api-inventory.py`, macro expansion tests, public API
  diff, full clippy, and full tests.

## Issue: Clippy allowances should move from crate policy to local rationale

> **Comment:** A broad package lint allowlist keeps the translated code
> buildable, but it makes it harder to see where Rust code is intentionally
> non-idiomatic. High-signal lints should be re-enabled globally once localized
> exceptions are justified.

- [x] Rank package-level clippy allowances by likely signal in safe, typed, and
      platform code.
- [x] Move high-signal allowances from `Cargo.toml` to the smallest module or
      item that still needs them, with a short reason when the exception is
      intentional.
- [x] Add or tighten tests that count package-level allowances and selected
      module-level allowances.
- [x] Avoid enabling lints that churn hot translated allocator paths without
      improving safety, API clarity, or generated code.
- [x] Re-run release fast-path audits after each lint reduction.
  - Evidence: `clippy::unnecessary_wraps` had no current violations and was
    removed from the package allowance set. `clippy::needless_pass_by_ref_mut`
    found a small high-signal cluster in bitmap/random helpers; those APIs now
    take shared references where mutation happens through atomics or the source
    context is read-only. `tests/release_fast_path_audit.rs` now rejects both
    lints returning to the package allowance set. Focused clippy runs with
    `-D clippy::needless_pass_by_ref_mut` and `-D clippy::unnecessary_wraps`
    passed.
- Verify: `cargo clippy --workspace --all-targets --all-features`,
  `tests/release_fast_path_audit.rs`, and full tests.

## Issue: Large translated modules need clearer Rust boundaries

> **Comment:** Some files still read like direct C translation units. Splitting
> them by invariant or ownership boundary will make safety review easier without
> changing public behavior.

- [x] Identify the largest translated modules and record intended submodule
      boundaries before moving code.
- [x] Split `src/internal/mod.rs` helper clusters into focused modules without
      changing public crate-root exports.
- [x] Split allocator/platform modules only when the move preserves hot-path
      inlining and does not obscure C parity.
- [x] Add module-level docs for new modules that state the invariant they own.
- [x] Update audit scripts and path aliases when files move.
  - Evidence: the largest translated files remain `src/allocator/arena.rs`,
    `src/allocator/page.rs`, `src/internal/bitmap.rs`,
    `src/allocator/alloc.rs`, `src/allocator/init.rs`,
    `src/platform/windows.rs`, and `src/internal/mod.rs`. The smallest
    high-value split was the raw memory-origin identifier cluster:
    `src/internal/memid.rs` now owns the C-compatible `mi_memid_t` layout,
    `mi_memkind_t`, and construction/classification helpers, while
    `src/internal/mod.rs` re-exports the module so existing translated call
    paths keep using `crate::internal::*`. No allocator/platform hot-path file
    was moved in this step; `#[inline]` helper attributes and call paths were
    preserved. `free_path_internals_are_not_dynamic_exports` now verifies the
    new module doc boundary and the `mod.rs` re-export. Unsafe inventory and
    audit docs were refreshed for the moved source path, and
    `scripts/capture-release-assembly.sh` captured the fixed release hot-path
    symbol set after the split.
- Verify: module inventory tests, API inventory checks, release assembly capture,
  full clippy, and full tests.

## Issue: Safe object ownership APIs remain unresolved

> **Comment:** The docs correctly avoid pretending byte buffers are object
> ownership. A Rust library will feel incomplete until typed object ownership is
> either implemented with `Drop` correctness or explicitly postponed with a
> stable design note.

- [x] Write the `MiBox<T>` and `HeapBox<'heap, T>` design, including allocation
      failure, initialization, `Drop`, zero-sized types, unsized values, and
      heap lifetime behavior.
- [x] Decide whether typed object ownership is in scope before the next release.
- [x] If in scope, implement fallible constructors without exposing raw
      ownership invariants.
- [x] If out of scope, document the reason and add release checks that keep the
      deferred status explicit.
- [x] Add compile-fail tests for heap lifetime misuse and private representation
      construction for any new object ownership types.
  - Evidence: `docs/object-ownership.md` now records the release-scoped
    decision: `MiBox<T>` and `HeapBox<'heap, T>` are out of scope before the
    next release. It still specifies the future API shape for allocation
    failure, initialization, constructor panic cleanup, `Drop`, destructor
    panic behavior, zero-sized values, unsized-value deferral, and heap
    lifetimes. `docs/safe-api.md`, `docs/rust-safety-model.md`, and
    `docs/api-inventory.md` now link that decision. The release checklist and
    `safe_byte_allocations_keep_object_ownership_separate` audit test reject
    object ownership names or constructors unless the design is completed with
    the required compile-fail coverage. No new object ownership types were
    added, so the compile-fail item is enforced by the existing absence guard
    rather than type-specific tests.
- Verify: safe API docs, compile-fail doctests, full clippy, full tests, and API
  snapshot review.

## Issue: Generated artifact hygiene is not centralized

> **Comment:** Repeated audit, benchmark, and dependency-analysis work creates
> local artifacts. The repo should make it obvious which files are generated,
> which are checked in as evidence, and which must remain ignored.

- [x] Inventory generated analysis outputs such as dependency graphs, benchmark
      CSVs, Criterion output, fuzz metadata, and release artifact manifests.
- [x] Move reproducible generated evidence under documented paths or add it to
      ignore rules when it is local-only.
- [x] Add a script or test that reports unexpected generated artifacts outside
      approved paths.
- [x] Document the command that regenerates each checked-in evidence file.
- [x] Keep untracked local scratch files out of commits unless the evidence is
      intentionally promoted.
  - Evidence: `docs/generated-artifacts.md` now owns the checked-in evidence
    table, local-only generated paths, release artifact outputs, and the
    command for each generated evidence family. `scripts/check-generated-artifacts.py`
    verifies required ignore rules, reports generated-looking untracked files
    outside approved paths, and checks that the policy doc still names the
    owner commands. Local scratch files such as `RUST_DEPENDENCY_EDGES.csv` and
    `parse_bench*.py` are ignored rather than promoted. The hygiene gate is
    wired into `scripts/check-local.sh`, release docs, and
    `tests/release_fast_path_audit.rs`.
- Verify: artifact hygiene script, `git status --short`, release checklist, and
  full tests when Rust code changes.

## Issue: Feature and no-std validation should cover safe-facing examples

> **Comment:** The crate has strong feature docs, but safe-facing examples and
> doctests mostly run with `std`. Feature/no-std validation should prove the
> documented safe and raw surfaces stay honest across supported build modes.

- [ ] Add focused checks for default `no_std` build behavior and documented
      `std`-only examples.
- [ ] Ensure safe facade docs clearly mark any examples or APIs that require
      `std`, tests, or process-global runtime state.
- [ ] Run the feature matrix after changing docs, examples, safe APIs, raw
      macros, or callback guards.
- [ ] Keep `c-header-macros`, tracking, secure, guarded, and interpose feature
      behavior additive unless docs state otherwise.
- [ ] Add release checklist entries for no-std doctest/build coverage if they
      are not already enforced.
- Verify: feature matrix scripts, doctests, full clippy, full tests, and docs
  generation.

## Suggested implementation order

1. [x] Add the unsafe allowlist category/cap and remove safe-facing allowlist
       entries first.
2. [x] Inventory root-exported macros and prevent new internal macro leaks.
3. [x] Localize the highest-signal clippy allowances.
4. [x] Centralize generated artifact hygiene.
5. [x] Document or implement the safe object ownership decision.
6. [x] Split the smallest high-value translated module cluster.
7. [ ] Expand feature/no-std validation around safe-facing examples.

## Definition of done for this backlog

- [x] The unsafe migration allowlist is categorized, capped, and smaller than
      the original 68-entry baseline.
- [x] Root-exported macros are limited to documented public compatibility
      surfaces.
- [x] Package-level clippy allowances are reduced or explicitly justified by
      local exceptions.
- [x] At least one translated module boundary is improved without API or ABI
      drift.
- [x] Safe typed object ownership has either a tested implementation or a clear
      release-scoped design decision.
- [x] Generated artifact ownership is documented and unexpected local artifacts
      are detectable.
- [ ] Feature/no-std docs and validation cover the safe-facing examples.
