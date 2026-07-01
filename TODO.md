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

- [ ] Inventory all `#[macro_export]` macros and classify them as Tier R public
      compatibility macros or crate-internal helpers.
- [ ] Replace internal-only exported helper macros with crate-local macros or
      small functions where that preserves hot-path codegen.
- [ ] Keep documented C-header parity macros feature-gated behind
      `c-header-macros`.
- [ ] Add an audit test that rejects new root-exported macros unless they appear
      in the documented Tier R allowlist.
- [ ] Run API and ABI inventory checks after each macro visibility change.
- Verify: `scripts/check-api-inventory.py`, macro expansion tests, public API
  diff, full clippy, and full tests.

## Issue: Clippy allowances should move from crate policy to local rationale

> **Comment:** A broad package lint allowlist keeps the translated code
> buildable, but it makes it harder to see where Rust code is intentionally
> non-idiomatic. High-signal lints should be re-enabled globally once localized
> exceptions are justified.

- [ ] Rank package-level clippy allowances by likely signal in safe, typed, and
      platform code.
- [ ] Move high-signal allowances from `Cargo.toml` to the smallest module or
      item that still needs them, with a short reason when the exception is
      intentional.
- [ ] Add or tighten tests that count package-level allowances and selected
      module-level allowances.
- [ ] Avoid enabling lints that churn hot translated allocator paths without
      improving safety, API clarity, or generated code.
- [ ] Re-run release fast-path audits after each lint reduction.
- Verify: `cargo clippy --workspace --all-targets --all-features`,
  `tests/release_fast_path_audit.rs`, and full tests.

## Issue: Large translated modules need clearer Rust boundaries

> **Comment:** Some files still read like direct C translation units. Splitting
> them by invariant or ownership boundary will make safety review easier without
> changing public behavior.

- [ ] Identify the largest translated modules and record intended submodule
      boundaries before moving code.
- [ ] Split `src/internal/mod.rs` helper clusters into focused modules without
      changing public crate-root exports.
- [ ] Split allocator/platform modules only when the move preserves hot-path
      inlining and does not obscure C parity.
- [ ] Add module-level docs for new modules that state the invariant they own.
- [ ] Update audit scripts and path aliases when files move.
- Verify: module inventory tests, API inventory checks, release assembly capture,
  full clippy, and full tests.

## Issue: Safe object ownership APIs remain unresolved

> **Comment:** The docs correctly avoid pretending byte buffers are object
> ownership. A Rust library will feel incomplete until typed object ownership is
> either implemented with `Drop` correctness or explicitly postponed with a
> stable design note.

- [ ] Write the `MiBox<T>` and `HeapBox<'heap, T>` design, including allocation
      failure, initialization, `Drop`, zero-sized types, unsized values, and
      heap lifetime behavior.
- [ ] Decide whether typed object ownership is in scope before the next release.
- [ ] If in scope, implement fallible constructors without exposing raw
      ownership invariants.
- [ ] If out of scope, document the reason and add release checks that keep the
      deferred status explicit.
- [ ] Add compile-fail tests for heap lifetime misuse and private representation
      construction for any new object ownership types.
- Verify: safe API docs, compile-fail doctests, full clippy, full tests, and API
  snapshot review.

## Issue: Generated artifact hygiene is not centralized

> **Comment:** Repeated audit, benchmark, and dependency-analysis work creates
> local artifacts. The repo should make it obvious which files are generated,
> which are checked in as evidence, and which must remain ignored.

- [ ] Inventory generated analysis outputs such as dependency graphs, benchmark
      CSVs, Criterion output, fuzz metadata, and release artifact manifests.
- [ ] Move reproducible generated evidence under documented paths or add it to
      ignore rules when it is local-only.
- [ ] Add a script or test that reports unexpected generated artifacts outside
      approved paths.
- [ ] Document the command that regenerates each checked-in evidence file.
- [ ] Keep untracked local scratch files out of commits unless the evidence is
      intentionally promoted.
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
2. [ ] Inventory root-exported macros and prevent new internal macro leaks.
3. [ ] Localize the highest-signal clippy allowances.
4. [ ] Centralize generated artifact hygiene.
5. [ ] Document or implement the safe object ownership decision.
6. [ ] Split the smallest high-value translated module cluster.
7. [ ] Expand feature/no-std validation around safe-facing examples.

## Definition of done for this backlog

- [x] The unsafe migration allowlist is categorized, capped, and smaller than
      the original 68-entry baseline.
- [ ] Root-exported macros are limited to documented public compatibility
      surfaces.
- [ ] Package-level clippy allowances are reduced or explicitly justified by
      local exceptions.
- [ ] At least one translated module boundary is improved without API or ABI
      drift.
- [ ] Safe typed object ownership has either a tested implementation or a clear
      release-scoped design decision.
- [ ] Generated artifact ownership is documented and unexpected local artifacts
      are detectable.
- [ ] Feature/no-std docs and validation cover the safe-facing examples.
