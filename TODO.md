# Exemplary Rust library improvement plan

This is the ordered implementation plan for turning `mechanical_mimalloc` into
an exemplary Rust systems library while preserving mimalloc 3.3.2 behavior,
the supported C ABI, allocator performance, `no_std` operation, and the
mechanical relationship with upstream.

The plan is intentionally explicit enough for a junior engineer to execute one
item at a time. Complete the phases in order. Within a phase, complete one task
before starting the next unless the task explicitly says that work may proceed
in parallel.

Public-API compatibility tooling, public-API snapshots, and SemVer automation
are deliberately out of scope for this plan. Do not add them as part of these
tasks.

## How to use this plan

For every task:

1. Read the named source files and their existing tests before editing.
2. Write down the invariant or problem in one sentence in the commit or pull
   request description.
3. Add or identify a test that fails if the invariant is broken.
4. Make the smallest production change that satisfies the task.
5. Run the task-specific validation.
6. Run the common Rust validation gate.
7. Inspect the diff and remove unrelated formatting, renames, or cleanup.
8. Record the production simplification achieved: removed raw callers,
   unchecked constructors, broad re-exports, internal C ABIs, synthetic-use
   helpers, or justified exceptions. Counts are supporting evidence, not the
   goal.
9. Check the task only after every completion condition is true.

Do not combine unrelated tasks in one change. A task that discovers no safe or
useful production change may end as `NO CHANGE`, but the checklist must record
what was inspected and why no change was justified.

## Non-negotiable engineering rules

- Preserve mimalloc's allocation, failure, alignment, initialization,
  reclamation, statistics, option, and thread-lifecycle behavior.
- Preserve the supported C and C++ ABI symbols and layouts. ABI changes are not
  authorized by this plan.
- Keep C-shaped names and raw-pointer conventions inside `raw`, `ffi`,
  `platform`, or clearly mechanical translation boundaries.
- Keep the crate-root safe facade idiomatic Rust with private representation,
  ownership types, lifetimes, `Option`, and typed `Result` values.
- Do not weaken atomic ordering, pointer validation, corruption detection, or
  ownership checks for readability or speed.
- Remove a validation check only when a stronger invariant makes the check
  redundant, the invariant is encoded at the boundary, and tests prove it.
- Do not replace a typed wrapper with a primitive merely to make the Rust source
  resemble C. Compare emitted code before judging wrapper cost.
- Do not add an abstraction for one call site. Prefer a direct helper or an
  inherent method unless multiple real users need a shared abstraction.
- Do not add broad `allow` attributes. Use the narrowest possible allowance and
  explain why the code must retain the flagged shape.
- Do not add audit rows, caps, or source-shape tests as a substitute for reducing
  unsafe code, complexity, or runtime work.
- Do not split files only to meet a line-count target. A new module must own a
  coherent invariant or lifecycle.
- Do not add production allocation, formatting, dynamic dispatch, locks,
  syscalls, or environment reads to allocator hot paths.
- Keep diagnostics behind `bench-diagnostics` and prove they compile out of the
  default release artifact.
- Never claim a performance improvement from a single run or from source
  appearance. Use assembly, normalized counters, balanced measurements, and
  held-out workloads.

## Common validation gate

Run these commands after every Rust source change:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-targets --all-features
```

For documentation-only changes, run:

```bash
scripts/check-fmt.sh
scripts/check-docs.sh
```

For changes that touch raw pointers, unsafe code, layout, FFI, atomics,
thread-local state, or allocator policy, also run the relevant commands named by
the task. A task is not complete if a required command fails. If the failure is
environmental, record the command, full error, and required follow-up instead of
checking the task.

## Completed foundation — remove audit theater

Completed in `4c51636`:

- [x] Deleted the 193-file `release_fast_path_audit` source/prose inspection
      harness instead of preserving it as a coverage metric.
- [x] Deleted unsafe evidence ledgers, migration and strict-type allowlists,
      marker IDs, count caps, and their enforcement scripts.
- [x] Replaced capability-linearity text inspection with compiler assertions in
      `src/types/linearity_tests.rs`.
- [x] Reduced `scripts/check-strict-types.py` to architectural boundaries Rust
      visibility cannot directly express; it has no debt ledger or count cap.
- [x] Made `scripts/check-unsafe-inventory.py` an advisory lexical report rather
      than a checked-in snapshot or release gate.
- [x] Removed documentation-wording assertions while retaining checks for real
      public items, safety sections, ABI symbols, links, and boundaries.
- [x] Removed benchmark execution from `scripts/check-local.sh`; benchmark smoke
      and performance qualification remain explicit opt-in commands.
- [x] Updated CI, specifications, and policy documentation to use compiler,
      runtime, layout, symbol, concurrency, Miri, and assembly evidence.

Do not recreate these systems under new names. New checks must identify a
concrete defect that compiler, runtime, artifact, or narrowly scoped
architectural evidence can catch.

---

# Phase 0 — Establish a trustworthy starting point

## 0.1 Record the current engineering baseline

Purpose: make later improvement claims measurable without creating another
permanent audit ledger. This is a post-cleanup baseline: use `4c51636` as the
minimum reference revision, or the later commit that contains this plan update.

### Work

- [ ] Record the current commit with `git rev-parse HEAD` in the change
      description and confirm it descends from `4c51636`.
- [ ] Record the current count of production Rust files and lines under `src/`.
- [ ] Record the number of production files above 1,000 and 1,500 lines.
- [ ] Record lexical counts for `unsafe fn`, `unsafe {`, `unsafe impl`,
      `#[inline(always)]`, `dyn Trait`, `Vec`, `Box`, and `.clone()` under
      `src/`. Label these as lexical counts, not AST-accurate facts.
- [ ] Record the current crate-wide lint exceptions and narrow architectural
      boundary exceptions that are relevant to planned work.
- [ ] Record the current ignored-test count and the reason for every ignored
      production-behavior test.
- [ ] Put the measurements directly in this task's completion note or the
      associated change description. Do not create a new inventory document.

### Validation

- [ ] Re-run every counting command from a clean tree and confirm it gives the
      recorded value.
- [ ] Run `scripts/check-unsafe-inventory.py` for advisory counts and require
      `scripts/check-strict-types.py` to pass.
- [ ] Confirm `git diff --check` reports no whitespace errors.

### Completion condition

This task is complete when later work can report an exact before/after change
for file size, unsafe surface, justified exceptions, ignored tests, and forced
inlining.

## 0.2 Confirm the existing correctness baseline

### Work

- [ ] Run the common Rust validation gate without modifying production code.
- [ ] Run `scripts/check-behavioral-parity.py`.
- [ ] Run `scripts/check-concurrency-models.sh`.
- [ ] Run `scripts/check-layout.sh`.
- [ ] Run `scripts/check-ffi-boundary.py`.
- [ ] Run `tests/no_allocation_critical_paths.rs` and the default no-std build.
- [ ] Save the exact failing command and output for any pre-existing failure.
      Do not repair failures in this baseline task.

### Validation

- [ ] Confirm the working tree is unchanged after the baseline run, excluding
      ignored build artifacts.
- [ ] Confirm every pre-existing failure has an owner and a later task in this
      plan, or add a narrowly scoped task before starting Phase 1.

### Completion condition

There must be a known-good baseline, or an explicit list of pre-existing
failures that prevents later changes from being blamed incorrectly.

---

# Phase 1 — Make the architecture easy to navigate

## 1.1 Write a maintainer map for the largest modules

Purpose: identify real invariant boundaries before moving code.

### Work

- [ ] For each production Rust file above 1,500 lines, list its distinct
      responsibilities, important state it owns, and callers.
- [ ] Mark each responsibility as one of: raw ABI, safe facade, allocation hot
      path, lifecycle, policy, platform primitive, diagnostics, or formatting.
- [ ] Identify responsibilities already represented by an existing child
      module; prefer moving code there instead of creating a new module.
- [ ] Propose a module extraction only when the extracted code owns one
      invariant, lifecycle, or boundary.
- [ ] Add the final module ownership map to `ARCHITECTURE.md`; keep it concise
      and link to existing detailed documents instead of duplicating them.

### Validation

- [ ] Verify every proposed module name corresponds to an actual responsibility
      and at least two related functions or one complete state lifecycle.
- [ ] Run the documentation-only validation commands.
- [ ] Have `rg` confirm that every source path named in the map exists.

### Completion condition

Another engineer must be able to locate allocation, free, page lifecycle,
arena lifecycle, initialization, options, statistics, platform, raw, and safe
facade code from `ARCHITECTURE.md` without searching the entire repository.

## 1.2 Decompose `src/allocator/init.rs` by lifecycle

### Work

- [ ] Identify the process initialization/teardown functions.
- [ ] Identify thread initialization/teardown and TLS association functions.
- [ ] Identify subprocess registry and subprocess lifecycle functions.
- [ ] Identify CPU detection, random initialization, and platform hook glue.
- [ ] Move one coherent group at a time into an `init/` child module. Reuse
      `init/subproc_registry.rs` where it already owns the behavior.
- [ ] Keep raw export entrypoints and the high-level initialization sequence in
      `init.rs`; move implementation details behind crate-private functions.
- [ ] Preserve function visibility unless a moved item can safely become more
      private.
- [ ] Remove imports and aliases made unused by each extraction.

### Validation

- [ ] Add or retain focused tests for process initialization, thread init/done,
      TLS reset, subprocess creation/destruction, and repeated initialization.
- [ ] Run `scripts/check-release-thread-stress.py` if present; otherwise run the
      repository's documented release thread-stress entrypoint.
- [ ] Run `scripts/check-behavioral-parity.py` and
      `scripts/check-layer-boundaries.py`.
- [ ] Run the common Rust validation gate.
- [ ] Compare `git diff --stat` and confirm this is code movement plus only the
      minimal visibility/import changes required by the move.

### Completion condition

Each new module owns a complete lifecycle, no initialization order changes, and
the parent file is materially easier to scan without duplicating logic.

## 1.3 Decompose allocator policy and reporting modules

Complete these subitems independently.

### Options

- [ ] Separate option storage/initialization from diagnostic output,
      formatting, and callback invocation in `src/allocator/options.rs`.
- [ ] Keep option reads on their current fast path and preserve option
      mutability and environment semantics.
- [ ] Validate with option export tests, callback tests,
      `scripts/check-policy-boundaries.py`, and the common gate.

### Statistics

- [ ] Separate counter mutation/aggregation, JSON/text rendering, and process
      information collection in `src/allocator/stats.rs`.
- [ ] Keep all formatting bounded and allocation-free in allocator paths.
- [ ] Validate with stats export tests, safe stats tests,
      `tests/no_allocation_critical_paths.rs`, and the common gate.

### Arena

- [ ] Move remaining arena responsibilities into existing `arena/` child
      modules where their invariant already belongs: geometry, OS memory,
      abandoned ownership, purge, metadata, or page ownership.
- [ ] Leave orchestration and raw compatibility entrypoints in
      `src/allocator/arena.rs`.
- [ ] Validate with arena/type tests, layout checks, behavioral parity, and the
      common gate.

### Validation

- [ ] Run the option, callback, statistics, arena, and safe-facade focused tests
      named above after each independent extraction.
- [ ] Run `scripts/check-policy-boundaries.py`,
      `tests/no_allocation_critical_paths.rs`, `scripts/check-layout.sh`, and
      `scripts/check-behavioral-parity.py` after all three module families are
      complete.
- [ ] Run the common Rust validation gate.
- [ ] Compare the pre/post module dependency graph with `rg` and confirm that
      no new cross-layer import or circular responsibility was introduced.

### Completion condition

- [ ] Confirm every resulting file has a describable single responsibility.
- [ ] Confirm no duplicate helper, wrapper, or validation path was introduced.
- [ ] Confirm no file was split solely to satisfy a size target.

## 1.4 Remove accidental crate-root coupling

### Work

- [ ] Inventory crate-root compatibility aliases in `src/lib.rs` and identify
      all internal users of each alias.
- [ ] For new Rust-native modules, replace broad crate-root aliases with the
      owning module path or a narrow import.
- [ ] Retain aliases required by mechanical translation or raw compatibility;
      document why they remain next to the alias.
- [ ] Do not rename C-shaped functions inside compatibility layers in this task.
- [ ] Do not change crate-root public exports.

### Validation

- [ ] Run `scripts/check-layer-boundaries.py`,
      `scripts/check-safe-boundary.py`, and
      `scripts/check-raw-facade.sh`.
- [ ] Use `rg` to prove removed aliases have no remaining users.
- [ ] Run the common Rust validation gate.

### Completion condition

New Rust-native code depends on owning modules, while mechanical compatibility
aliases remain explicitly identified and contained.

## 1.5 Remove synthetic reachability and broad internal re-exports

### Work

- [ ] Replace broad allocator re-exports in `src/lib.rs` with narrow owning
      module imports where no compatibility boundary requires the alias.
- [ ] Replace wildcard re-exports in `src/internal/mod.rs` with explicit imports
      at the actual call sites.
- [ ] Remove the `src/types/mod.rs::__lint_touch` chain and every subordinate
      `__lint_touch()` helper.
- [ ] Delete helpers, imports, and constants that become genuinely unused after
      synthetic reachability is removed.
- [ ] Retain a compatibility alias only when an actual raw, FFI, assembly, or
      mechanical translation consumer requires it; state that reason locally.
- [ ] Do not replace one broad re-export with another prelude or facade module.

### Validation

- [ ] Run Clippy with warnings denied after each module family is disconnected
      from the synthetic-use chain.
- [ ] Use `rg` to confirm no `__lint_touch` identifier remains.
- [ ] Run layer, raw-facade, API-boundary, and common Rust validation checks.
- [ ] Compare release assembly for any allocation/free path whose imports or
      helper reachability changed.

### Completion condition

The crate contains no fake-use machinery, Rust-native modules import their
actual owners, and every remaining compatibility alias has a real consumer.

---

# Phase 2 — Reduce the real unsafe trust surface

## 2.1 Rank unsafe hotspots by risk, not raw count

### Work

- [ ] Rank files containing unsafe code using these factors: aliasing risk,
      pointer arithmetic, lifetime extension, union access, atomic publication,
      FFI entry, and number of callers.
- [ ] Select the five highest-risk domains. Expected candidates include free
      routing, page metadata, arena ownership, thread lifecycle, and callback
      state; use the evidence rather than assuming this exact order.
- [ ] For each selected domain, identify the smallest safe or typed primitive
      that could own repeated unsafe operations.
- [ ] Reject proposed wrappers that merely move an unsafe block without reducing
      the number of callers that must understand its invariant.
- [ ] Record the ranked list in this task's completion note, not a new ledger.

### Validation

- [ ] Confirm every proposed primitive has at least two current unsafe callers
      or encloses one complete high-risk state transition.
- [ ] Confirm each proposed safe API can state and enforce its invariant without
      accepting an unchecked primitive that recreates the original risk.
- [ ] Run `scripts/check-unsafe-inventory.py` before making production changes.

### Completion condition

The next unsafe reductions must be ordered by potential undefined behavior and
review burden, not by whichever file is easiest to edit.

## 2.2 Centralize pointer arithmetic and metadata field access

Work through one pointer domain at a time: page, block, heap/thread heap, arena,
subprocess, and OS allocation.

### Work

- [ ] Find direct pointer arithmetic and direct raw metadata field access
      outside the owning `src/types/` or boundary module.
- [ ] Reuse an existing checked pointer wrapper before creating a new wrapper.
- [ ] Add a wrapper method only when it can validate bounds, alignment,
      provenance, initialization, or ownership once for multiple callers.
- [ ] Return typed values or `Option`/`Result` from checked operations; do not
      encode failure with a new sentinel.
- [ ] Keep raw accessors crate-private and avoid `Deref`/`DerefMut` for
      allocator metadata wrappers.
- [ ] Replace callers in one domain, then remove the superseded unsafe helper or
      raw field access.

### Validation

- [ ] Add boundary tests for null, zero, minimum, maximum, overflow,
      misalignment, and one-past-end cases relevant to the domain.
- [ ] Run `scripts/check-strict-types.py` and the focused provenance tests.
- [ ] Run the relevant focused Miri command from `scripts/check-miri-all.sh`.
- [ ] Run the common Rust validation gate.
- [ ] Record the before/after unsafe-site count and require a net reduction or a
      clearly smaller set of callers that must uphold the invariant.

### Completion condition

Each pointer invariant has one owner, callers cannot bypass it accidentally,
and the change reduces the code that must be manually trusted.

## 2.3 Narrow unsafe functions and blocks

### Work

- [ ] Find `unsafe fn` bodies containing ordinary control flow around only one
      or two unsafe operations.
- [ ] Keep the function unsafe only if callers must uphold an invariant. A
      function is not unsafe merely because its implementation uses unsafe.
- [ ] Move validation and ordinary calculations outside the smallest possible
      `unsafe` block.
- [ ] Replace whole-function unsafe scopes with operation-level unsafe blocks.
- [ ] Ensure every remaining unsafe function has a `# Safety` section naming
      pointer validity, lifetime, aliasing, initialization, ownership,
      threading, and unwind requirements that apply.
- [ ] Ensure every unsafe block or impl has a nearby `SAFETY:` comment
      that explains this exact operation, not only the surrounding subsystem.

### Validation

- [ ] Run Clippy's `missing_safety_doc` and `unsafe_op_in_unsafe_fn` gates and
      `scripts/check-safe-docs.py`.
- [ ] Run Clippy with all features and confirm `unsafe_op_in_unsafe_fn` remains
      denied.
- [ ] Record a net reduction in broad unsafe scope. Do not count splitting one
      broad block into many equivalent blocks as risk reduction.
- [ ] Run the common Rust validation gate.

### Completion condition

A reviewer can see the exact unsafe operation and its proof without reasoning
through unrelated control flow.

## 2.4 Remove redundant unsafe marker layers

### Work

- [ ] Find raw-pointer-to-wrapper-to-raw-pointer round trips inside one function
      or one proven ownership epoch.
- [ ] Determine whether the wrapper can be carried through the complete
      operation instead of reconstructed.
- [ ] Preserve linear capability consumption on state transitions.
- [ ] Do not make a linear capability `Copy` to simplify call sites.
- [ ] Remove superseded unchecked constructors that no longer have callers.
- [ ] Remove narrow boundary exceptions made obsolete by the production
      reduction.

### Validation

- [ ] Add compiler assertions or compile-fail tests showing linear capabilities
      cannot be duplicated or forged.
- [ ] Run capability, strict-type, layout, and focused behavioral tests.
- [ ] Capture release assembly for any changed allocation/free hot path.
- [ ] Confirm wrapper traffic does not survive in release code, or record and
      justify any remaining instruction.
- [ ] Run the common Rust validation gate.

### Completion condition

Typed state travels through the operation without repeated unsafe
reconstruction, and obsolete unsafe constructors and allowances are gone.

## 2.5 Finish domain-type migration one invariant at a time

Work in this order unless hotspot evidence justifies a change: sizes and
alignments, bins and indexes, page/block counts, ownership words, then metadata
pointers. Migrate cold validation and diagnostic paths before allocation/free
hot paths.

### Work

- [ ] Find Rust-native functions that accept `usize`, integer flags, or raw
      metadata pointers even though an existing domain type represents the
      invariant.
- [ ] Convert one complete caller/callee slice at a time so the typed value
      survives through the operation instead of being immediately unwrapped.
- [ ] Add a newtype only when it rejects an invalid state, prevents domain
      mixing, or gives one owner to checked arithmetic.
- [ ] Keep primitive conversion at raw, FFI, platform, or mechanical ingress
      and egress boundaries.
- [ ] Keep metadata wrappers private, `#[repr(transparent)]` only when layout is
      required, and free of `Deref`/`DerefMut`.
- [ ] Remove superseded primitive overloads and unchecked constructors after
      their last caller is migrated.

### Validation

- [ ] Add boundary and property tests for every constructor or checked
      arithmetic operation introduced.
- [ ] Run strict-type, layout, Miri, and common Rust validation checks.
- [ ] Capture assembly before changing a hot path and reject surviving wrapper
      traffic, extra branches, or extra calls.
- [ ] Record fewer primitive/raw parameters or fewer callers that must prove the
      invariant; adding a wrapper alone is not completion.

### Completion condition

Rust-native signatures express their domains, primitives are confined to real
boundaries, and the number of call sites manually preserving each invariant is
smaller.

## 2.6 Translate failures at the raw and ABI edges

### Work

- [ ] Identify internal functions that encode expected failure as null, a
      sentinel integer, errno mutation, formatted diagnostics, or abort.
- [ ] Return an existing typed `Result` or `Option` internally; introduce a new
      error enum only when callers need to distinguish actionable cases.
- [ ] Translate typed failure into null, errno, C diagnostics, or required abort
      behavior once at the owning raw or exported C ABI boundary.
- [ ] Keep corruption and impossible-by-invariant failures distinct from
      ordinary allocation, layout, and input failures.
- [ ] Keep hot-path errors allocation-free and avoid owned strings or formatting
      before the ABI boundary.
- [ ] Remove duplicate error translation and errno writes from internal
      callees.

### Validation

- [ ] Add Rust tests for each typed error and C-facing tests for the exact null,
      errno, diagnostic, and preservation contract.
- [ ] Run allocation-boundary, oversize, FFI panic/no-unwind, behavioral parity,
      and common Rust checks.
- [ ] Confirm each migrated failure has one translation point and no new unwind
      path crosses an ABI boundary.

### Completion condition

Rust-native code communicates ordinary failure through types, while C-shaped
failure behavior exists only at the boundary that promises it.

## 2.7 Remove unnecessary internal C calling conventions

### Work

- [ ] Inventory `extern "C"` functions and classify each as exported ABI,
      foreign callback, assembly/linker target, or internal-only function.
- [ ] Convert internal-only functions to the Rust ABI and give them typed
      parameters where an existing wrapper applies.
- [ ] Narrow visibility after confirming no foreign, assembly, linker-script,
      or dynamic-symbol consumer exists.
- [ ] Preserve symbol names and calling conventions for actual ABI contracts.
- [ ] Do not infer export status from a C-shaped name; inspect built artifacts
      and call sites.

### Validation

- [ ] Inspect dynamic and static symbols before and after each conversion.
- [ ] Run ABI-symbol, FFI-boundary, static-link, release-consumer, and common
      Rust checks.
- [ ] Record the internal `extern "C"` functions removed and the real ABI
      functions intentionally retained.

### Completion condition

Every remaining C calling convention has a foreign, assembly, linker, or
export contract; ordinary internal calls use Rust signatures.

---

# Phase 3 — Make zero-cost claims precise and enforceable

## 3.1 Classify every `#[inline(always)]`

### Work

- [ ] Produce a list of every production `#[inline(always)]` grouped by file and
      function.
- [ ] Classify each site as: required hot-path wrapper, required ABI shim,
      measured codegen fix, compile-time accessor, or unproven.
- [ ] Put the reason next to retained non-obvious forced-inline attributes.
- [ ] For unproven sites, compare three release variants: current,
      ordinary `#[inline]`, and no attribute.
- [ ] Change one coherent function family at a time. Do not perform a blanket
      replacement.

### Validation

- [ ] Use `scripts/capture-release-assembly.sh` for before/after artifacts.
- [ ] Compare function bytes, instructions, calls, branches, stack references,
      and complete artifact `.text` size.
- [ ] Retain `#[inline(always)]` only when it removes a real call/wrapper cost or
      preserves a measured hot-path improvement without harmful code growth.
- [ ] Run focused correctness tests and the common gate for each batch.
- [ ] For a changed allocator hot path, follow `docs/performance.md` before
      accepting the change.

### Completion condition

Every forced-inline site has evidence or an obvious trivial-wrapper reason;
the attribute is no longer a general style default.

## 3.2 Prove strict wrappers have the intended representation

### Work

- [ ] Inventory pointer, scalar, layout, queue, freelist, atomic, and typestate
      wrappers used on allocation/free hot paths.
- [ ] For each transparent wrapper, test size and alignment equality with its
      wrapped representation.
- [ ] For optional non-null wrappers, test the expected niche size.
- [ ] For linear capability types, test that they are stack-only and are not
      `Copy` or `Clone` unless duplication is semantically harmless.
- [ ] For public owned allocations, document intentional metadata such as
      `Layout` and heap identity instead of calling the whole public type
      representation-free.
- [ ] Remove `#[repr(transparent)]` where no layout contract is required, or
      document the real layout reason where it remains.

### Validation

- [ ] Run `scripts/check-layout.sh` in default, `secure`, `guarded`, and tracking
      configurations supported by the script.
- [ ] Run the strict-type layout and niche tests.
- [ ] Capture assembly for free-list pop, local free, remote free, small page
      lookup, and layout construction.
- [ ] Confirm constructors/accessors add no calls, branches, spills, loads, or
      stores beyond the underlying operation.
- [ ] Run the common Rust validation gate.

### Completion condition

Every “zero-cost” wrapper claim names the exact cost being eliminated:
representation, dispatch, allocation, or emitted instructions.

## 3.3 Keep allocator critical paths allocation- and dispatch-free

### Work

- [ ] Review allocator, internal, platform, and strict-type modules for `Vec`,
      `Box`, `String`, formatting allocation, `Arc`, `Rc`, trait objects, and
      standard synchronization types.
- [ ] Confirm every occurrence is test-only, diagnostics-only, a safe facade
      boundary, or explicitly justified outside a critical path.
- [ ] Replace accidental intermediate collections with direct loops or caller
      buffers.
- [ ] Replace accidental dynamic dispatch in hot code with an enum, generic, or
      direct function only when a real occurrence exists.
- [ ] Do not introduce a custom collection or abstraction when no accidental
      allocation exists.

### Validation

- [ ] Run `tests/no_allocation_critical_paths.rs` and the default no-std build.
- [ ] Inspect the default release dynamic symbol table for unexpected allocator
      or formatting dependencies.
- [ ] Run the common Rust validation gate.

### Completion condition

Allocation/free critical paths require no Rust heap allocation or runtime
dispatch beyond the allocator mechanism itself.

---

# Phase 4 — Improve ordinary Rust style without rustifying the ABI

## 4.1 Remove panic-prone production shortcuts

### Work

- [ ] Search non-test production code for `unwrap`, `expect`, `panic!`, indexing
      that can fail, unchecked arithmetic, and `unreachable!`.
- [ ] Classify each occurrence as input failure, allocator corruption,
      impossible-by-type state, FFI abort policy, or static invariant.
- [ ] Return `Option` or a typed error for expected failure.
- [ ] Keep abort/assert behavior where mimalloc compatibility requires process
      termination; document that boundary explicitly.
- [ ] Use checked arithmetic at untrusted or overflow-sensitive boundaries.
- [ ] Do not add error allocation or formatting to a hot path.

### Validation

- [ ] Add regression tests for every converted expected-failure path.
- [ ] Run `tests/ffi_panic_audit.rs`, oversize allocation tests, allocation
      boundary tests, and behavioral parity.
- [ ] Run `scripts/check-no-unwind-artifact.py`.
- [ ] Run the common Rust validation gate.

### Completion condition

Ordinary invalid input cannot panic, and every remaining abort/panic is an
explicit compatibility or invariant boundary.

## 4.2 Make ownership and borrowing obvious at function boundaries

### Work

- [ ] Review Rust-native helpers for owned parameters used only for reading.
- [ ] Accept references, slices, or typed borrowed handles where ownership is
      not transferred.
- [ ] Review returned references and ensure their lifetime is tied to the
      correct heap, page, or owner.
- [ ] Keep consuming parameters for linear state transitions; do not borrow a
      capability that must be invalidated.
- [ ] Review `Copy` and `Clone` implementations and retain them only for cheap,
      semantically duplicable values.
- [ ] Make intentional snapshot copies visible through naming or a short
      comment.

### Validation

- [ ] Add compile-fail docs or tests for values that must not outlive a heap or
      cross a thread boundary.
- [ ] Run the safe facade, capability-linearity, and trait expectation tests.
- [ ] Confirm `.clone()` and `Copy` counts do not grow without an explicit
      reason.
- [ ] Run the common Rust validation gate.

### Completion condition

Each Rust-native signature communicates whether it borrows, mutates, consumes,
or transfers allocator state.

## 4.3 Simplify complex functions with pure decision helpers

### Work

- [ ] Find long functions that mix policy decisions, pointer mutation, atomic
      operations, statistics, and error reporting.
- [ ] Extract pure decision logic only when it can take typed values and return
      a small enum or typed result.
- [ ] Keep pointer mutation and atomic publication in the state-owning module.
- [ ] Use exhaustive matches for internal closed state machines.
- [ ] Do not introduce a trait, context object, or generic framework for a
      single decision.
- [ ] Name helpers after allocator concepts, not vague verbs such as `handle`,
      `process`, or `do_work`.

### Validation

- [ ] Add table-driven tests comparing the extracted decision with the previous
      formula over boundary values.
- [ ] Run behavioral parity and relevant policy tests.
- [ ] Capture assembly if the changed function is on an allocation/free hot
      path; reject extra calls or branches that survive release optimization.
- [ ] Run the common Rust validation gate.

### Completion condition

Policy is testable without raw pointers, mutation remains close to its owner,
and emitted hot-path work does not increase.

## 4.4 Tighten lint exceptions

### Work

- [ ] Review every crate-wide Clippy `allow` in `Cargo.toml`.
- [ ] Remove an allowance if the current crate passes without it.
- [ ] If only a few compatibility sites need an allowance, move it to those
      sites with a reason.
- [ ] Retain crate-wide allowances only when the warning is pervasive because
      of mechanical C compatibility and local annotations would add noise.
- [ ] Pay special attention to `inline_always`, `too_many_lines`,
      `wildcard_imports`, pointer casts, numeric casts, and pass-by-value.
- [ ] Do not rewrite correct compatibility code merely to satisfy a stylistic
      lint.

### Validation

- [ ] Remove one allowance at a time and run full-feature Clippy.
- [ ] Confirm the final exception list contains a short explanation for every
      non-obvious crate-wide allowance.
- [ ] Run the common Rust validation gate.

### Completion condition

The lint policy describes intentional compatibility constraints rather than
hiding ordinary Rust quality issues.

## 4.5 Replace C compatibility idioms inside Rust-native code

### Work

- [ ] Classify uses of `MI_UNUSED`, `MI_UNUSED_RELEASE`, `MI_DEBUG`, assertion
      wrappers, `mi_likely`/`mi_unlikely`, and numeric tag constants as raw
      compatibility or Rust-native implementation.
- [ ] Replace unused-value shims in Rust-native code with removal, an
      underscore binding, or `let _ = value` as appropriate.
- [ ] Replace debug/assertion wrappers only when Rust assertions preserve the
      required debug level, message, abort/no-unwind behavior, and evaluation
      semantics.
- [ ] Replace numeric tags with closed internal enums when exhaustive matching
      prevents invalid states; preserve integer constants at layout/ABI edges.
- [ ] Treat branch hints as a code-generation decision: remove or replace them
      only after checking support on the pinned toolchain and emitted assembly.
- [ ] Keep compatibility macros where mechanical parity is clearer and safer
      than an abstraction.

### Validation

- [ ] Add behavioral tests for assertion levels, tag conversion, and any
      compatibility fallback changed.
- [ ] Run no-unwind, behavioral parity, full-feature Clippy, and common Rust
      checks.
- [ ] Compare optimized assembly for every changed hot-path assertion or branch
      hint.
- [ ] Record the C idioms removed from Rust-native modules; renaming or moving a
      shim is not completion.

### Completion condition

Rust-native code uses Rust control flow and domain types, while retained C
idioms are confined to boundaries where they preserve a concrete contract.

---

# Phase 5 — Strengthen concurrency and lifecycle correctness

## 5.1 Resolve the thread-lifecycle sanitizer finding

Purpose: investigate the recorded TSan race between `_mi_theap_create` and
`_mi_thread_done` instead of treating it as permanent background noise.

### Work

- [ ] Reproduce the race using the exact command and artifact configuration
      recorded in `PERFORMANCE_WORKLOG.md`.
- [ ] Confirm whether current HEAD still reproduces it in at least three runs.
- [ ] Reduce the reproducer to the smallest thread create/allocate/free/exit
      sequence that retains the report.
- [ ] Map every reported access to its owning state and intended atomic or TLS
      synchronization.
- [ ] Compare the lifecycle with C mimalloc 3.3.2 and document whether the race
      is a translation defect, a sanitizer false positive, or inherited design.
- [ ] If it is a defect, add a regression test before changing synchronization.
- [ ] Make the smallest fix that establishes a clear happens-before relation.
- [ ] Do not add a global lock or strengthen unrelated atomics as a shortcut.

### Validation

- [ ] The minimized reproducer passes at least ten TSan runs after the fix.
- [ ] Run release thread stress, behavioral parity, Loom models, ASan, and
      Valgrind checks documented for thread lifecycle.
- [ ] Compare allocation/free assembly and counters to ensure the fix does not
      add synchronization to unrelated hot paths.
- [ ] Run the common Rust validation gate.

### Completion condition

The race is fixed with a proven synchronization edge or documented with a
minimal false-positive proof and an automated suppression scoped to that exact
boundary.

## 5.2 Review atomic APIs by protocol

### Work

- [ ] Group atomic operations by protocol: remote free publication, ownership
      claim/release, once initialization, arena bitmap, page-map publication,
      statistics, and locks.
- [ ] For each protocol, write the state transition and required visibility in
      a short module comment next to its implementation.
- [ ] Ensure raw C memory-order enums are converted once at the compatibility
      boundary and do not propagate into Rust-native code.
- [ ] Ensure atomic wrapper methods name the operation, such as `publish`,
      `detach`, `claim`, or `release`, rather than only `load`/`store` when they
      carry a protocol invariant.
- [ ] Remove duplicate raw atomic access that bypasses an existing protocol
      wrapper.
- [ ] Do not weaken or strengthen ordering without a protocol proof and C
      comparison.

### Validation

- [ ] Run the focused atomic protocol tests.
- [ ] Add or update a Loom model for every changed publication/ownership
      protocol.
- [ ] Run `scripts/check-concurrency-models.sh` and release thread stress.
- [ ] Compare release assembly for changed atomic hot paths and confirm no
      unexpected fences or locked instructions were introduced.
- [ ] Run the common Rust validation gate.

### Completion condition

Atomic orderings are understandable from the protocol they implement, and
Rust-native code cannot casually bypass that protocol.

## 5.3 Make process-global callback tests deterministic

### Work

- [ ] Inventory the ignored callback tests in `src/safe/tests.rs`.
- [ ] Move process-global callback tests into a dedicated integration test
      binary or otherwise guarantee serial execution and process isolation.
- [ ] Test registration, nested LIFO restoration, allocator reentry, recursive
      suppression, panic containment, and drop restoration.
- [ ] Ensure a failed test restores global callback state before process exit
      where possible.
- [ ] Remove `#[ignore]` only after isolation makes the test deterministic.

### Validation

- [ ] Run the isolated callback test binary at least twenty times.
- [ ] Run it once with the test harness default thread count and once with
      `--test-threads=1`.
- [ ] Run FFI panic audit and the common Rust validation gate.
- [ ] Confirm the ignored-test count decreases and no process-global test leaks
      state into another test.

### Completion condition

Callback safety behavior runs in normal CI rather than depending on permanently
ignored tests.

---

# Phase 6 — Improve tests without creating audit theater

## 6.1 Expand property tests around unsafe boundaries

### Work

- [ ] Add property coverage for layout products, alignment, size-class
      boundaries, page capacity, slice ranges, and pointer classification.
- [ ] Add randomized valid state-transition sequences for page queues,
      local/remote free lists, reclaim, and reabandon behavior.
- [ ] Include zero, one, maximum valid, first invalid, and arithmetic-overflow
      values in every numeric domain.
- [ ] Check conservation properties: no duplicate block, no lost block, used
      count matches ownership, and every allocation is freed at most once.
- [ ] Use deterministic seeds and print the seed on failure.
- [ ] Keep property-test allocation and randomness outside allocator timed or
      no-allocation paths.

### Validation

- [ ] Demonstrate each new property catches one deliberately introduced local
      defect, then revert the defect.
- [ ] Run property tests in default and `secure` modes.
- [ ] Run the relevant fuzz-smoke and Miri scripts.
- [ ] Run the common Rust validation gate.

### Completion condition

The most dangerous unsafe transitions are checked over domains and sequences,
not only a handful of hand-written examples.

## 6.2 Make test ownership and naming consistent

### Work

- [ ] Keep private invariant tests beside their owning module or in that
      module's `tests.rs`.
- [ ] Keep public behavior, FFI linking, and downstream-usage tests under
      `tests/`.
- [ ] Rename vague tests so the name states the condition and expected result.
- [ ] Split a test file only when the new file owns a coherent behavior family.
- [ ] Remove duplicate fixtures created by previous file splits; retain one
      focused helper in the nearest shared test module.
- [ ] Do not move tests solely for cosmetic consistency.

### Validation

- [ ] Run `cargo test -- --list` and confirm names are unique and descriptive.
- [ ] Confirm each production invariant named in `ARCHITECTURE.md` has an
      identifiable test owner.
- [ ] Run the common Rust validation gate.

### Completion condition

An engineer can find the test for an invariant from the module that owns it,
and public behavior is tested as a downstream user would observe it.

---

# Phase 7 — Validate features, `no_std`, and supported boundaries

## 7.1 Make the feature matrix explicit and additive

### Work

- [ ] Enumerate supported combinations for `std`, `secure`, `guarded`,
      `track-valgrind`, `track-asan`, `bench-diagnostics`, `c-abi`, and
      `interpose` using `docs/features.md` as the owner.
- [ ] Identify intentionally precedence-based combinations such as Valgrind
      versus ASan tracking.
- [ ] Confirm enabling a feature does not silently remove unrelated Rust APIs.
- [ ] Confirm benchmark/diagnostic features do not alter default production
      behavior when disabled.
- [ ] Remove stale feature conditionals that cannot occur in a supported build.
- [ ] Do not add new features for internal implementation choices.

### Validation

- [ ] Run `scripts/check-features.sh` and `scripts/check-feature-abi.sh`.
- [ ] Run layout and focused allocation tests for every executable feature
      combination supported by the scripts.
- [ ] Compare default release symbols and assembly before/after diagnostics are
      compiled out.
- [ ] Run the common Rust validation gate.

### Completion condition

Every supported feature combination has documented behavior and an automated
compile or execution check.

## 7.2 Tighten `no_std` separation

### Work

- [ ] Search allocator-core modules for accidental `std` imports or APIs.
- [ ] Keep `std::error::Error` implementations, examples, tooling, and
      diagnostics behind `std`.
- [ ] Ensure safe facade functionality that does not require `std` remains
      available in the default Linux `no_std` build.
- [ ] Keep error values allocation-free and format through `core::fmt`.
- [ ] Confirm platform primitives, not `std`, own required OS interaction.

### Validation

- [ ] Run the repository default `no_std` check and `scripts/check-features.sh`.
- [ ] Run `scripts/check-safe-api-features.sh`.
- [ ] Run `scripts/check-strict-types.py`, the default no-std build, and
      `tests/no_allocation_critical_paths.rs`.
- [ ] Run the common Rust validation gate.

### Completion condition

The core allocator and safe facade use `std` only where the feature explicitly
permits it, with no accidental dependency hidden by tests.

## 7.3 Contain source-present unsupported platforms

### Work

- [ ] Confirm Linux x86_64 GNU remains the only maintained release target unless
      project policy explicitly changes.
- [ ] Ensure source-present platform modules are clearly gated and cannot imply
      unsupported production status.
- [ ] Keep shared platform interfaces narrow and avoid forcing the maintained
      target through lowest-common-denominator abstractions.
- [ ] Fix cross-platform compile failures only when they can be validated by an
      available target or CI environment.
- [ ] Do not advertise a platform based on source compilation alone.

### Validation

- [ ] Run the platform status and primitive-boundary tests.
- [ ] Run `scripts/check-unix-syscall-parity.py` on the maintained target.
- [ ] Run available cross-target compile checks without claiming runtime
      support.
- [ ] Run the common Rust validation gate on the maintained target.

### Completion condition

Platform abstractions are honest, narrow, and optimized for verified targets.

## 7.4 Evaluate and narrow the platform operation boundary

Do not assume that one `Platform` trait is the desired result. First determine
whether platform operations actually share a stable typed contract and whether
an abstraction reduces unsafe callers or duplicated invariants.

### Work

- [ ] Group maintained-target platform operations by responsibility: virtual
      memory, commit/decommit, protection, process information, environment,
      clocks, NUMA, and thread lifecycle.
- [ ] Identify raw arguments and repeated validation that can move behind a
      narrow typed operation shared by at least two real callers.
- [ ] Compare `cfg`-selected modules, direct functions, a sealed static-dispatch
      trait, and small operation-specific interfaces.
- [ ] Reject a broad trait that introduces dynamic dispatch, a context object,
      lowest-common-denominator semantics, or unused methods.
- [ ] Implement only the smallest boundary that reduces duplicated unsafe code
      or makes a platform invariant unrepresentable to callers.
- [ ] Keep unsupported platform implementations isolated; do not distort the
      maintained Linux path to make source-present platforms look uniform.

### Validation

- [ ] Test typed operation boundaries on the maintained target and compile-check
      only those additional targets with available toolchains.
- [ ] Run syscall parity, platform primitive, feature, layout, and common Rust
      checks.
- [ ] Compare optimized code for allocation/free paths that cross the platform
      boundary.
- [ ] Record fewer unsafe platform callers or duplicated validation sites. A
      new trait or module by itself is not completion.

### Completion condition

Platform code has the narrowest useful statically dispatched boundary; keeping
`cfg`-selected functions is an acceptable result when it is simpler and equally
safe.

---

# Phase 8 — Simplify documentation and governance

## 8.1 Assign one owner document per policy

### Work

- [ ] Group tracked Markdown files by topic: architecture, safety,
      performance, testing, ABI/FFI, features/platforms, release, and upstream
      parity.
- [ ] Select one authoritative document for each topic.
- [ ] Replace duplicated explanations in secondary files with a short summary
      and link to the owner.
- [ ] Delete obsolete reports only when their historical evidence is preserved
      in an authoritative report or version control.
- [ ] Keep experiment history in `PERFORMANCE_WORKLOG.md`; do not copy it into
      policy documents.
- [ ] Update README links after consolidation.

### Validation

- [ ] Run `scripts/check-docs.sh`, `scripts/check-generated-artifacts.py`, and
      doc tests.
- [ ] Use `rg` to confirm deleted document names have no remaining references.
- [ ] Confirm each policy topic has exactly one named authoritative owner in
      `ARCHITECTURE.md` or README.
- [ ] Confirm the tracked documentation count or duplicated text decreases.

### Completion condition

Documentation remains thorough but no policy must be updated in several places
to stay consistent.

## 8.2 Reduce remaining production exceptions

### Work

- [x] Delete the strict-type and unsafe-migration debt allowlists and their count
      caps.
- [ ] Review crate-wide Clippy allowances and narrow FFI/layer boundary
      exceptions against current production code.
- [ ] Remove an exception when the production site no longer exists or the code
      can be made clearer without changing compatibility or codegen.
- [ ] Keep exceptions only for concrete raw ABI, platform ABI, upstream
      translation, or measured performance constraints.
- [ ] Do not split one exception into several entries and count that as progress
      unless the trusted production surface also shrinks.

### Validation

- [ ] Run Clippy, `scripts/check-strict-types.py`, and the relevant FFI/layer
      boundary checks.
- [ ] Record the production change and exception removed; raw counts are
      supporting context only.
- [ ] Require every claimed improvement to remove an unsafe/raw/dead-code site,
      narrow a boundary, or eliminate an allowance.
- [ ] Run the common Rust validation gate if production code changes.

### Completion condition

Exception removal follows real code reduction and never becomes the primary
deliverable.

## 8.3 Review comments for proof value

### Work

- [ ] Remove comments that restate syntax or narrate obvious control flow.
- [ ] Keep comments that explain aliasing, lifetime, ownership, memory ordering,
      upstream parity, platform behavior, or non-obvious performance choices.
- [ ] Update stale source paths and line references.
- [ ] Ensure safety comments explain why the operation is valid at that exact
      point.
- [ ] Ensure performance comments name the evidence artifact or general
      mechanism, not an unexplained claim such as “faster.”

### Validation

- [ ] Run `scripts/check-safe-docs.py`, docs, and generated-artifact checks.
- [ ] Review the diff and confirm production code behavior is unchanged.
- [ ] Run documentation-only validation if no Rust code changed; otherwise run
      the common Rust gate.

### Completion condition

Comments carry information the type system and code cannot express, and safety
proofs remain local and reviewable.

---

# Phase 9 — Close measured performance gaps without weakening design

Do not begin this phase until structural and safety refactors above are complete
and a fresh baseline has been captured. Follow `docs/performance.md` and record
every experiment in `PERFORMANCE_WORKLOG.md`.

## 9.1 Re-establish the post-refactor Rust/C baseline

### Work

- [ ] Build clean current Rust and C mimalloc 3.3.2 release artifacts in
      separate target directories.
- [ ] Record commit, toolchains, build commands, hashes, dynamic dependencies,
      host state, and allocator order.
- [ ] Run the repository's balanced full comparison protocol in both allocator
      orders.
- [ ] Capture resource evidence and release assembly.
- [ ] Compare against the accepted B02 evidence without treating runs from a
      different host or configuration as directly interchangeable.

### Validation

- [ ] Every reported number resolves to an immutable artifact hash.
- [ ] Both allocator orders agree for every claimed stable difference.
- [ ] No structural refactor introduced a material hot-path or resource
      regression.
- [ ] The full correctness and release gates pass on the measured artifact.

### Completion condition

There is a trustworthy baseline for the exact post-refactor source before any
new optimization candidate is attempted.

## 9.2 Reduce low-contention remote-free overhead

Current evidence reports remaining Rust/C throughput gaps in low-contention
1+1 remote-free cases, especially 64–2048-byte objects.

### Work

- [ ] Reproduce the gap with balanced, normalized measurements.
- [ ] Compare Rust and C allocation work separately from remote publication,
      detachment, scan, collection, reclaim, and ownership release.
- [ ] Use counters and assembly to identify one repeated load, validation,
      branch, division, scan, or state reconstruction on the common path.
- [ ] Confirm whether the work is required safety behavior, a compiler artifact,
      or redundant under an existing typed ownership invariant.
- [ ] Make only one mechanism change per candidate.
- [ ] Preserve deep validation in debug/secure modes if ordinary release can
      rely on a proven trusted boundary.

### Validation

- [ ] Add a focused correctness test for the exact invariant used by the
      optimization.
- [ ] Run Loom, Miri/fuzz, release thread stress, ASan, TSan, and Valgrind checks
      relevant to the changed path.
- [ ] Show the targeted emitted work disappears in release assembly.
- [ ] Show two balanced comparisons improve normalized instructions/cycles and
      wall time in the same direction.
- [ ] Guard saturated xmalloc, Larson, glibc-thread, mstress, alloc-test,
      Redis, RocksDB, and resource behavior.
- [ ] Run the common Rust validation gate.

### Completion condition

Keep a change only when it removes general allocator work, preserves every
ownership/corruption invariant, and has no repeatable held-out regression.

## 9.3 Investigate the remaining `glibc-thread` difference

### Work

- [ ] Reproduce the approximately 1% gap at high resolution before changing
      code.
- [ ] Profile allocator symbols separately from the workload loop and C++
      boundary.
- [ ] Compare call frequency and cost for allocation, delete/free, page retire,
      refill, collection, and option reads.
- [ ] Identify one common-path difference confirmed by both counters and
      annotated assembly.
- [ ] Reject branch-layout or forced-inline guesses without a demonstrated
      instruction or cache effect.
- [ ] Make one candidate change and record `KEEP`, `REJECT`, or `NO CHANGE`.

### Validation

- [ ] Use balanced baseline/candidate order and enough samples to resolve a 1%
      effect from host noise.
- [ ] Guard local allocation/free, low-contention remote free, Larson,
      fragmentation, and memory/resource probes.
- [ ] Confirm no `.text`, branch, instruction, or RSS regression is hidden by a
      noisy wall-time result.
- [ ] Run the full correctness gate before keeping a candidate.

### Completion condition

The gap is either reduced by a general improvement or documented as measured
noise/compiler behavior with no unjustified source churn.

## 9.4 Enforce performance neutrality for the safe facade

### Work

- [ ] Compare the safe `MiMalloc`, `Allocation`, and heap allocation paths with
      their corresponding raw entrypoints at the call and assembly level.
- [ ] Separate intentional safe metadata and error mapping from accidental
      calls, branches, or allocation.
- [ ] Confirm zero-sized allocation handling remains outside raw allocator calls.
- [ ] Confirm initialized wrappers do not add copying beyond requested zeroing
      or explicit clone operations.
- [ ] Optimize only accidental wrapper overhead; retain metadata required for
      ownership and correct `Drop`.

### Validation

- [ ] Run safe facade allocation, resize, zeroing, heap lifetime, failure, and
      global allocator tests.
- [ ] Inspect release assembly for representative sizes and alignments.
- [ ] Run the repository's public-versus-raw evidence protocol when performance
      measurements are authorized.
- [ ] Run the common Rust validation gate.

### Completion condition

The safe facade charges only for its documented semantics—ownership metadata,
failure classification, initialization, and drop—not accidental abstraction
machinery.

---

# Phase 10 — Final exemplary-library signoff

## 10.1 Perform a clean-tree senior review

### Work

- [ ] Confirm every production module has a clear owner and responsibility.
- [ ] Confirm public safe types have private fields, correct ownership, typed
      errors, appropriate thread traits, and complete rustdoc examples.
- [ ] Confirm raw pointers are confined to raw/FFI/platform/mechanical or
      narrowly trusted internal boundaries.
- [ ] Confirm unsafe scopes and constructors are minimal and locally justified.
- [ ] Confirm no production `unwrap`, `expect`, or panic handles ordinary input
      failure.
- [ ] Confirm no accidental allocation, dynamic dispatch, broad lock, or
      formatting exists on allocator critical paths.
- [ ] Confirm every remaining forced-inline attribute and lint allowance has a
      concrete reason.
- [ ] Confirm documentation has one owner per policy and no stale generated
      claims.
- [ ] Confirm the working tree contains no stale TODO edits, generated evidence,
      or uncommitted production change.

### Validation

- [ ] Re-run the Phase 0 measurements and report exact before/after changes.
- [ ] Require a real reduction in unsafe review burden, oversized mixed-purpose
      modules, unjustified forced inlining, ignored tests, or allowances. More
      documentation alone is not completion.
- [ ] Review every changed line and trace it to a task in this plan.

### Completion condition

A senior reviewer can understand the boundaries, trust surface, ownership
model, and performance evidence without relying on institutional memory.

## 10.2 Run the final correctness and release qualification

### Work and validation

- [ ] Run `cargo fmt --all`.
- [ ] Run `cargo clippy --workspace --all-targets --all-features`.
- [ ] Run `cargo test --workspace --all-targets --all-features`.
- [ ] Run `scripts/check-local.sh` and every additional release check documented
      in `docs/release.md`.
- [ ] Run behavioral parity, feature, layout, ABI/FFI boundary, no-unwind,
      no-allocation, strict-type, safety-documentation, and policy checks.
- [ ] Run Loom/concurrency models.
- [ ] Run the supported Miri suite.
- [ ] Run ASan, TSan, and Valgrind stress checks on the maintained target.
- [ ] Run package and documentation builds from a clean tree.
- [ ] Run the full performance/resource qualification only after all
      correctness checks pass.
- [ ] Preserve exact commands, commit, toolchains, artifact hashes, and any
      environmental limitation.
- [ ] Fix every source-caused failure before checking this task. Do not waive a
      failure because unrelated checks passed.

### Completion condition

All mandatory checks pass on the same clean source revision, performance is at
or near the accepted C baseline without unexplained resource regressions, and
no required evidence depends on an uncommitted working tree.

## 10.3 Close the plan with measured results

### Work

- [ ] Replace every remaining unchecked task with a completed result, a linked
      follow-up approved by the maintainer, or a documented `NO CHANGE` backed by
      evidence.
- [ ] Summarize unsafe-site, justified-exception, forced-inline, ignored-test,
      large-file, code-size, and performance changes from the Phase 0 baseline.
- [ ] List any intentionally retained translation debt and the compatibility or
      performance reason it remains.
- [ ] Confirm no completed item claims success solely because a cap, ledger, or
      document grew.
- [ ] Move historical experiment detail to `PERFORMANCE_WORKLOG.md`; keep this
      file focused on remaining work and final outcomes.

### Validation

- [ ] Verify every cited path, command, commit, and artifact exists.
- [ ] Verify the final summary agrees with the clean-tree measurements.
- [ ] Run documentation checks after the final update.

### Completion condition

The library is not merely well-audited: it has a smaller and clearer trusted
core, idiomatic safe boundaries, evidence-backed zero-cost abstractions,
deterministic tests, and measured parity with the allocator it translates.
