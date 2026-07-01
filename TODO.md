# TODO: Rust Idiom and Crate Standardization Backlog

Source: Rust idiom review of the current `mechanical_mimalloc` crate, using the
project's Rust coding, public API, and type/performance design standards.

Goal: make the Rust library feel consistently Rusty without weakening mimalloc
parity, C ABI compatibility, allocator hot paths, or the intentionally raw
compatibility surfaces.

> **Highlighted principle:** Do not mechanically "rustify" C ABI and raw
> compatibility code. Instead, make the safe Rust facade idiomatic, keep raw/C
> layers explicit, and use internal newtypes/typestate to encode allocator
> invariants with zero or measured overhead.

## Rules for this backlog

- Treat `raw`, C ABI symbols, feature flags, macros, MSRV, and supported targets
  as compatibility-sensitive surfaces.
- Keep Tier S safe Rust API narrow unless a separate design document and tests
  justify expansion.
- Preserve hot-path behavior. Any allocator-core or strict-type change needs
  benchmark or assembly evidence required by `docs/performance.md` and
  `docs/strict-types.md`.
- Do not broaden public API while polishing style unless the API inventory,
  compatibility docs, doctests, and release checklist are updated together.
- For Rust code changes, run project-required verification before declaring
  completion:
  - `cargo fmt --all`
  - `cargo clippy --workspace --all-targets --all-features`
  - `cargo test --workspace --all-targets --all-features`

## Current strengths to preserve

- The crate root already exposes a deliberately small Rust-facing surface.
- Implementation modules are private by default.
- `raw` is already separated from the safe facade and documented as unsafe.
- `unsafe_callbacks` is separated from ordinary safe APIs.
- The crate already uses strict lints, `unsafe_op_in_unsafe_fn = "deny"`, and
  `missing_safety_doc = "deny"`.
- The project already has extensive release, ABI, unsafe-audit, fuzzing,
  hardening, benchmark, and artifact validation infrastructure.
- Existing safe wrappers such as `Allocation`, `Heap`, `GlobalOptions`, and
  `StatsSnapshot` are the correct direction for a Rusty API.

## Highlighted issues and comments

### Issue: Safe Rust style policy is spread across docs instead of one guide

> **Comment:** The project has many excellent release and safety policies, but
> contributors need a single "how Rust code should look here" guide. Without a
> compact style guide, new code can drift toward translated C naming, raw
> pointer conventions, or ad hoc constructor/error naming.

- [x] Add `docs/rust-style.md` or a dedicated `CONTRIBUTING.md` section.
- [x] Define where C-style names are allowed: `raw`, `ffi`, `platform`, and
      mechanical translation internals only.
- [x] Define safe-layer naming conventions: `new`, `try_new`, `from_raw`,
      `as_ptr`, `as_non_null`, `len`, `is_empty`, `layout`, `into_raw`.
- [x] Define unchecked constructor naming: `unsafe fn new_unchecked(...)`.
- [x] Define error naming: `ThingError`, `TryThingError`, `ThingErrorKind`.
- [x] Define accessor return conventions: `&str`, `&[T]`, `Option<&T>`, raw
      pointers only at unsafe/raw boundaries.
- [x] Define when `Copy` is appropriate for newtypes.
- [x] Define safety-comment format and audit-tag expectations.
- [x] Link the style guide from README, `CONTRIBUTING.md`, and
      `docs/rust-safety-model.md`.
  - Evidence: `docs/rust-style.md` now defines the crate boundary rule,
    Rust-facing naming, unchecked constructors, typed error names, accessor and
    borrowing conventions, `Copy`/`Clone` rules, public representation policy,
    unsafe comment format with `SAFETY[AUDIT:*]`, hot-path wrapper evidence
    expectations, and macro/trait defaults. README, `CONTRIBUTING.md`, and
    `docs/rust-safety-model.md` link to it.
- Verify: `scripts/check-docs.sh`.

### Issue: Safe facade and raw facade need an explicit naming firewall

> **Comment:** Rust users should never need to guess whether a function is a
> safe Rust abstraction or a C-shaped compatibility function. C names such as
> `mi_malloc` belong in `raw`/ABI surfaces; safe APIs should read like ordinary
> Rust ownership APIs.

- [x] Audit crate-root exports and `src/safe` for C-shaped names that could be
      renamed or hidden before stabilization.
- [x] Ensure new safe APIs use Rust verbs such as `allocate`, `resize`,
      `options`, `snapshot`, `reset`, or `with_*` instead of `mi_*` names.
- [x] Keep `mi_*`, `_mi_*`, malloc-family, and C++ operator names out of Tier S.
- [x] Add a docs check or inventory rule that flags new crate-root `mi_*`
      exports unless classified as Tier R/Tier C.
- [x] Update `docs/api-inventory.md` with the naming firewall policy.
  - Evidence: `StatsSnapshot` safe accessors now use
    `requested_bytes_total`, `normal_bytes_total`, and `huge_bytes_total`
    instead of malloc-family public method names. `docs/api-inventory.md`
    documents the naming firewall and the stats counter mapping.
    `scripts/check-api-inventory.py` now fails new Tier S crate-root items or
    public safe-facade methods that use `mi_*`, `_mi_*`, malloc-family, or C++
    operator spellings outside Tier R/Tier C classification.
- Verify: `scripts/check-api-inventory.py` and `scripts/check-raw-facade.sh`.

### Issue: Public API representation should remain private by default

> **Comment:** Public fields freeze representation and make future allocator or
> layout changes harder. Rust-facing structs should expose invariants through
> constructors and accessors unless literal construction is intentionally part
> of the contract.

- [x] Audit all Tier S and Tier U public structs for public fields.
- [x] For each public field, classify it as one of:
  - intentionally stable DTO-like field;
  - C ABI layout field;
  - accidental representation leak.
- [x] Convert accidental public fields to private fields with accessors before
      a stable release.
- [x] Add compile-fail examples where external construction must remain
      impossible.
- [x] Document intentionally public fields in `docs/api-inventory.md`.
  - Evidence: the audit found no Tier S/Tier U public representation fields to
    convert; unit structs and private-field wrappers remain intentional.
    `scripts/check-api-inventory.py` now fails public fields in `src/safe` and
    Tier U callback guards unless a future inventory exception is deliberately
    added. `Allocation` and `StatsSnapshot` include compile-fail examples
    proving external representation construction remains impossible.
    `docs/api-inventory.md` documents the representation firewall.
- Verify: public API snapshot diff plus `cargo test --doc --features std`.

### Issue: Public enums need deliberate exhaustiveness policy

> **Comment:** Adding a variant to a public exhaustive enum is a breaking
> change. Error kinds, option keys, event categories, and classifications that
> may grow should be `#[non_exhaustive]` before release.

- [x] Audit every public Tier S/Tier U enum.
- [x] Decide whether each enum is closed/exhaustive or growable.
- [x] Add `#[non_exhaustive]` to growable public enums before stabilization.
- [x] Keep exhaustive enums only where downstream exhaustive matching is a
      deliberate part of the API.
- [x] Add doctests showing intended match style for growable enums.
- [x] Update API inventory with exhaustiveness decisions.
  - Evidence: all current Tier S enums are growable and already marked
    `#[non_exhaustive]`; there are no current Tier U enums. `OptionKey` and
    `TryAllocError` docs now show downstream wildcard match style.
    `docs/api-inventory.md` records the enum exhaustiveness policy and current
    decisions. `scripts/check-api-inventory.py` now fails any public enum in
    `src/safe` that lacks `#[non_exhaustive]` unless a future inventory
    exception is deliberately added.
- Verify: `scripts/check-api-inventory.py` and public API snapshot diff.

### Issue: Error APIs should be uniformly typed and inspectable

> **Comment:** Library callers should be able to match expected failures. Avoid
> string errors, integer status codes, null return conventions, and panics in
> the safe facade.

- [x] Audit Tier S fallible functions for consistent `Result<T, E>` usage.
- [x] Ensure expected absence uses `Option<T>` or `Option<&T>`.
- [x] Ensure safe APIs do not expose C-style error/status integers unless they
      are wrapped in typed Rust errors.
- [x] Ensure fallible constructors are named `try_new`, `parse`, or implemented
      as `TryFrom` where appropriate.
- [x] Add `# Errors` rustdoc sections for any missing fallible methods.
- [x] Add tests that assert safe failure preserves ownership where applicable.
  - Evidence: the audit found safe fallible APIs consistently return typed
    `Result<T, E>` values, expected absence uses `Option`, and ordinary safe
    failure does not use null pointers, integer status codes, or panics.
    Existing allocation and heap resize tests assert failed resizes preserve
    ownership. `docs/api-inventory.md` now records the safe error API policy,
    and `scripts/check-safe-docs.py` checks that policy text in addition to
    requiring `# Errors` and `# Panics` sections on public safe `Result`
    methods.
- Verify: `scripts/check-safe-docs.py` and full Rust tests.

### Issue: Constructor and conversion patterns should be standardized

> **Comment:** The crate already uses many strict internal types. Consistent
> constructor names make invariants easier to review and unsafe boundaries
> easier to recognize.

- [x] Audit strict scalar, pointer, and typestate wrappers for constructor
      naming consistency.
- [x] Prefer:
  - `new(...) -> Option<Self>` or `Result<Self, E>` for checked construction;
  - `try_new(...) -> Result<Self, E>` when an error type is useful;
  - `unsafe fn new_unchecked(...) -> Self` for unchecked invariant-bearing
    construction;
  - `from_raw`/`from_non_null` for pointer wrappers;
  - `as_ptr`/`as_non_null` for pointer access.
- [x] Avoid adding more ambiguous names such as `make`, `create`, `init`, or
      `get` unless they match existing domain vocabulary.
- [x] Add a small table of constructor conventions to the Rust style guide.
  - Evidence: the audit covered `src/types` and `src/safe` constructor and
    accessor names. Strict scalar, pointer, heap/page typestate, arena,
    freelist, padding, callback, and safe allocation wrappers already use the
    intended `new`, `new_unchecked`, `from_raw`, `from_non_null`, `as_ptr`, and
    `as_non_null` patterns. The only broad `get()` usage is the established
    crate-private scalar/counter accessor pattern and safe domain reads such as
    `GlobalOptions::get` and `MainHeap::get`; the style guide now documents
    that distinction. `scripts/check-safe-docs.py` keeps the constructor table
    and scalar `get()` exception present.
- Verify: targeted source audit and full Rust tests.

### Issue: Boolean and nullable state clusters should become typed states where practical

> **Comment:** Allocator internals often inherit C-style boolean combinations.
> Where a state has mutually exclusive cases, enums or typestate witnesses make
> invalid states harder to represent and review.

- [x] Audit `src/types`, `src/allocator`, and `src/safe` for clusters such as
      `bool + Option<T>` or multiple booleans representing allocator state.
- [x] Convert reviewable non-hot-path clusters to enums.
- [x] For hot paths, require assembly or benchmark evidence before and after
      any typestate conversion.
- [x] Prefer state-specific data inside enum variants rather than parallel
      optional fields.
- [x] Avoid wildcard `_` matches inside defining modules when exhaustive
      handling should catch future variants.
  - Evidence: the audit found no reviewable non-hot-path cluster that should
    be converted now; raw C layout fields and callback ABI option slots should
    remain compatibility-shaped, and heap/page ownership, validation, free
    paths, memory IDs, and callback wrappers already use typed states. The
    practical cleanup was `LocalFreeMode`: the former `track_stats`/`check_full`
    boolean pair is now a small enum with named variants and an inline
    accessor, removing boolean literal clusters from free call sites while
    keeping the mode crate-private and `Copy`. The defining module uses
    exhaustive matches over `FreePathKind`; no wildcard arm hides future
    free-path variants. Focused unit coverage checks the valid mode states.
    Because this touches the free path, release `mi_free` assembly was captured
    under `target/strict-type-assembly/`, and `scripts/check-bench-smoke.sh`
    passed threshold validation with
    `target/criterion/benchmark-thresholds-smoke.csv`.
- Verify: relevant unit tests plus benchmark/assembly evidence for hot paths.

### Issue: Borrowing conventions should be documented and enforced in new code

> **Comment:** Idiomatic Rust APIs borrow by default and take ownership only
> when they store or consume. This matters for allocation-free safe helpers and
> no-std compatibility.

- [x] Document default parameter choices:
  - read text: `&str`;
  - read bytes: `&[u8]`;
  - read path: `&Path`;
  - read sequence: `&[T]`;
  - store text/path/bytes: owned `String`, `PathBuf`, `Vec<T>`, or `Box<[T]>`.
- [x] Audit Tier S methods for owned parameters used only for read-only work.
- [x] Audit return types for `&Vec<T>` or concrete collection leaks; prefer
      `&[T]` or iterators.
- [x] Avoid spreading `Cow` unless it avoids meaningful allocation and keeps
      the API clear.
  - Evidence: `docs/rust-style.md` now records the borrowed-input defaults for
    text, bytes, paths, and sequences, plus the ownership rule for stored
    text/path/bytes. The Tier S safe facade audit found no owned `String`,
    `Vec<T>`, `PathBuf`, or `Cow` parameters used only for read-only work, and
    no public `&Vec<T>`, `&String`, or `&PathBuf` return or parameter leaks.
    Allocation and heap APIs expose slices (`&[u8]`, `&mut [u8]`, and
    `&[MaybeUninit<u8>]`) or typed ownership values instead. `scripts/check-safe-docs.py`
    now keeps the style-guide borrowing phrases present and rejects borrowed
    concrete collections or `Cow` in public safe signatures.
- Verify: clippy plus targeted API review.

### Issue: Clones should be intentional and visible

> **Comment:** Cloning to satisfy the borrow checker can hide allocation and
> ownership mistakes. In allocator code, accidental cloning is also a signal to
> re-check ownership design.

- [x] Audit safe-layer and internal helper clones.
- [x] Remove clones that can be replaced by borrowing or moving.
- [x] For intentional non-trivial clones, add a short comment or name such as
      `snapshot` to explain the reason.
- [x] Keep `Copy` only on small scalar/pointer wrappers where duplication is
      cheap and semantically invisible.
  - Evidence: the safe/internal/allocator helper scan found no `.clone()` or
    `clone_from()` calls to remove. The only non-test owned string conversions
    found are Windows platform environment-string boundary conversions outside
    the safe/internal helper layer. Current `Copy` derives remain on small
    scalar, pointer, handle, enum, or witness types. The only public safe
    non-`Copy` `Clone` type is `StatsSnapshot`; its docs now state that cloning
    duplicates the captured counters without calling mimalloc or refreshing
    statistics. `scripts/check-safe-docs.py` now rejects future public safe
    non-`Copy` `Clone` structs unless their rustdoc documents clone semantics.
- Verify: full Rust tests and benchmarks if hot code changes.

### Issue: Trait use should stay minimal and capability-focused

> **Comment:** Traits are public contracts. Broad or premature traits make
> downstream compatibility and optimization harder. Prefer inherent methods for
> types owned by this crate.

- [x] Audit public traits, if any, for downstream implementation expectations.
- [x] Seal traits that external users should not implement.
- [x] Keep traits one to three methods where practical.
- [x] Avoid adding blanket impls without explicit coherence and inference
      review.
- [x] Decide and test object-safety if `dyn Trait` is part of a public API.
  - Evidence: the Tier S/Tier U audit found no public Rust-facing traits to
    seal, size, make object-safe, or review for blanket impl coherence. The
    only crate-root public trait implementation is `MiMalloc`'s expected
    `core::alloc::GlobalAlloc` impl. `docs/api-inventory.md` now records that
    there are no current Tier S or Tier U public traits and requires explicit
    downstream-implementation, sealing, blanket-impl, and object-safety review
    before adding one. `scripts/check-api-inventory.py` now includes public
    traits in crate-root drift detection and rejects public traits in the safe
    facade until they are deliberately inventoried.
- Verify: API inventory and compile-fail/object-safety tests where applicable.

### Issue: Unsafe style should be standardized beyond mechanical correctness

> **Comment:** The project has strong unsafe audit tooling. The remaining polish
> is consistency: small unsafe blocks, local invariant comments, and explicit
> safe wrappers around unsafe primitives.

- [x] Update the Rust style guide with the required safety-comment format.
- [x] Prefer small unsafe blocks around the exact unsafe operation.
- [x] Avoid broad unsafe regions that mix pointer operations with ordinary
      control flow.
- [x] Ensure public unsafe functions include caller obligations, pointer
      validity, aliasing, lifetime, threading, and ownership requirements.
- [x] Keep `SAFETY[AUDIT:*]` tags mapped to tests or manual proof.
  - Evidence: `docs/rust-style.md` already defines the
    `SAFETY[AUDIT:<operation-id>]` comment format, requires small unsafe
    blocks around exact operations, rejects broad unsafe regions that mix
    pointer operations with ordinary control flow, and requires public unsafe
    functions to document caller obligations in `# Safety` sections.
    `scripts/check-safe-docs.py` now keeps those style-guide phrases present
    and continues to require `# Safety` docs on public unsafe functions.
    `scripts/check-unsafe-audit.py` verified unsafe source coverage, operation
    ID documentation, and the dated migration allowlist; it reported 72 unsafe
    source files, 5,967 unsafe mentions, 10 checked operation IDs, and 68
    legacy allowlist files. `scripts/check-unsafe-audit-evidence.py` verified
    21 evidence entries covering all 10 operation IDs.
- Verify: `scripts/check-unsafe-audit.py` and
  `scripts/check-unsafe-audit-evidence.py`.

### Issue: Module organization should make the intended API obvious

> **Comment:** A Rust crate feels consistent when module `mod.rs` files expose a
> curated surface and implementation paths stay private. This is especially
> important for a translated allocator where internal module names may look
> public-worthy but are not stable contracts.

- [x] Audit each top-level module for accidental public items.
- [x] Keep `pub use` only for intended API items.
- [x] Prefer `pub(crate)` or private modules for translated implementation
      details.
- [x] Avoid adding crate-root compatibility aliases except where translated
      code requires them.
- [x] Document any necessary crate-root aliases as internal compatibility only.
  - Evidence: the module audit found the crate root exposing only the intended
    `raw` Tier R module, `unsafe_callbacks` Tier U namespace, `MiMalloc`, and
    curated Tier S safe re-exports. `allocator`, `ffi`, `internal`, `platform`,
    and `safe` remain private modules; `types` remains `pub(crate)`.
    `src/safe/mod.rs` re-exports only the intended safe facade types, while
    translated implementation modules stay private or `pub(crate)`. The
    necessary crate-root compatibility aliases remain documented in `src/lib.rs`
    as crate-private aliases for translated mimalloc code. `docs/api-inventory.md`
    classifies implementation modules as Tier I, and `scripts/check-api-inventory.py`
    now keeps that policy text present while enforcing crate-root inventory,
    default raw-facade modules, private raw implementation modules, and the
    Tier S/U API snapshot.
- Verify: `scripts/check-api-inventory.py` and public API snapshot diff.

### Issue: Safe API docs should use Rust examples first

> **Comment:** Users should see `MiMalloc`, `Allocation`, `Heap`,
> `GlobalOptions`, and `StatsSnapshot` before raw `mi_*` calls. Raw examples
> should be lower in docs and visibly unsafe.

- [x] Audit README and rustdoc example order.
- [x] Ensure each Tier S type has a short safe example.
- [x] Keep raw examples behind explicit `unsafe` blocks.
- [x] Keep compile-fail examples proving raw APIs and macros require unsafe.
- [x] Avoid showing raw/C examples in places meant for safe Rust onboarding.
  - Evidence: README and crate rustdoc both lead with `MiMalloc`,
    `Allocation`, `Heap`, `GlobalOptions`, and `StatsSnapshot` before showing
    raw C-shaped calls. Tier S types have short safe examples in rustdoc,
    including allocation, initialized allocation, heap, heap-tied allocation,
    main heap, options, stats, process info, and typed errors. Raw examples in
    README and crate rustdoc are behind explicit `unsafe` blocks, and crate
    rustdoc keeps compile-fail examples proving raw calls, typed raw allocation
    macros, header override macros, implementation modules, and internal
    typestate witnesses are not safe public API. `scripts/check-safe-docs.py`
    now checks that README and crate rustdoc keep safe examples before raw
    facade examples and that the README keeps the safe facade example present.
- Verify: `cargo test --doc --features std` and `scripts/check-docs.sh`.

### Issue: Macro surface should remain exceptional, not ordinary API style

> **Comment:** Macros are harder to type-check, document, and evolve than
> functions. They are appropriate for C-header compatibility, but new Rusty APIs
> should usually be functions, methods, or typed wrappers.

- [x] Keep `#[macro_export]` limited to documented Tier R C-header parity.
- [x] Prefer private `macro_rules!` or functions for internal single-crate use.
- [x] Reject new public macros unless API inventory and compatibility review
      classify them.
- [x] Keep feature-gated macro docs and compile-fail unsafe tests current.
  - Evidence: `#[macro_export]` remains limited to the documented Tier R
    C-header compatibility macros plus the existing internal-compatibility
    macros retained for translated code. Other single-crate helper macros stay
    private `macro_rules!` definitions in their implementation modules.
    `docs/api-inventory.md` classifies macro tiers, and
    `scripts/check-api-inventory.py` rejects macro inventory drift, verifies
    `c-header-macros` gating for C-header macros, and requires every exported
    macro to be documented in the inventory. `tests/header_macro_expansion.rs`
    verifies the public macro inventory, downstream expansion, feature gating,
    and compile-fail unsafe boundary behavior.
- Verify: macro inventory checks and doctests.

### Issue: Feature flags should stay additive and predictable

> **Comment:** Cargo features are API. Enabling a feature should not silently
> remove Rust APIs or change artifact meaning except where explicitly documented
> as an artifact mode.

- [x] Audit feature combinations for additive behavior.
- [x] Document invalid feature combinations with `compile_error!` if any exist.
- [x] Avoid adding heavy default features.
- [x] Use `cargo hack` or existing equivalent scripts for feature powerset
      checks where feasible.
- [x] Keep feature-specific ABI checks aligned with release artifacts.
  - Evidence: the feature audit found additive behavior across the current
    feature surface. `default = []` remains intentionally empty, and heavy or
    artifact-oriented modes such as `bench`, `compare`, `bench-diagnostics`,
    `c-abi`, `cxx-overrides`, `tls-shim`, and `interpose` remain opt-in. No
    invalid feature combination was found, so no `compile_error!` gate is
    currently warranted. `scripts/check-features.sh` completed the explicit
    feature matrix, the interpose cdylib build, and a `cargo hack` powerset of
    128 additive combinations for non-artifact features. The safe API feature
    matrix passed via `scripts/check-safe-api-features.sh`, and
    `scripts/check-feature-abi.sh` passed default, `secure`, `guarded`,
    `track-valgrind`, `track-asan` export checks, plus dynamic ABI manifests.
- Verify: `scripts/check-feature-abi.sh` and feature matrix checks.

### Issue: Public API diffing should be used for style refactors too

> **Comment:** Idiom polish can accidentally move, rename, or expose public
> items. Public API snapshots should be reviewed even when a change is described
> as cleanup.

- [x] Require public API snapshot review for any change touching `src/lib.rs`,
      `src/safe`, `src/ffi/callbacks.rs`, `src/raw`, or public macros.
- [x] Classify changes as Tier S, Tier U, Tier R, or Tier C before merge.
- [x] Add changelog entries for any intentional compatibility-sensitive change.
- [x] Run SemVer tooling before publishing a release.
  - Evidence: `docs/api-inventory.md` now requires Tier S/U snapshot review
    for style refactors touching `src/lib.rs`, `src/safe`,
    `src/ffi/callbacks.rs`, `src/raw`, or exported macros, with public changes
    classified as Tier S, Tier U, Tier R, or Tier C. `docs/compatibility.md`
    adds the same checkpoint requirements and changelog rule.
    `docs/release.md` documents the release API diff command, the style-only
    refactor trigger, and `cargo semver-checks check-release` as the
    publishing SemVer tool gate. `scripts/check-api-inventory.py` now enforces
    those policy phrases.
- Verify: `scripts/check-api-inventory.py` and release API diff command.

### Issue: Performance-sensitive type changes need layout and codegen evidence

> **Comment:** Newtypes and typestates are idiomatic only if they preserve the
> allocator's hot-path expectations. Rusty design should not introduce hidden
> branches, formatting, allocation, stack spills, or panic paths.

- [x] For any new hot-path wrapper, measure type size and generated assembly
      where required.
- [x] Keep `Copy` wrappers small and pointer/scalar-like.
- [x] Box rare large enum variants if enum size matters.
- [x] Avoid dynamic dispatch in allocator hot loops unless measured.
- [x] Avoid allocation in no-allocation critical paths.
  - Evidence: `docs/strict-types.md`, `docs/performance.md`, and
    `docs/release.md` already require size/layout, assembly, and benchmark
    evidence for new strict wrappers on allocator hot paths. The current
    pointer/scalar and typestate wrappers are covered by
    `src/types/layout_tests.rs`, `scripts/check-layout.sh`, and the release
    fast-path audit. `scripts/check-strict-types.py` now skips non-production
    fuzz harnesses, recognizes moved module allowlist aliases, and rejects
    dynamic dispatch in allocator, internal, platform, and strict-type modules
    unless an explicit allowlist entry records measured justification.
    `scripts/capture-release-assembly.sh` now builds the `c-abi` cdylib
    artifact it inspects and captures the tracked hot symbols. Validation ran
    `scripts/check-strict-types.py`, `scripts/check-layout.sh`,
    `scripts/check-no-allocation-audit.py`,
    `scripts/capture-release-assembly.sh`, and `scripts/check-bench-smoke.sh`.
    Full benchmark runs remain the release-candidate gate for actual hot-path
    behavior changes.
- Verify: `scripts/capture-release-assembly.sh`, benchmark smoke/full checks,
  and no-allocation audit scripts.

### Issue: Tests should enforce Rust API misuse boundaries

> **Comment:** Compile-fail tests are a good way to keep APIs Rusty. They prove
> unsafe requirements, heap lifetimes, privacy, and typestate invariants remain
> enforced by the type system.

- [x] Keep or add compile-fail tests for raw APIs requiring unsafe.
- [x] Keep or add compile-fail tests for heap allocation lifetime misuse.
- [x] Add compile-fail tests when private representation prevents external
      construction.
- [x] Add trait/static assertion tests for expected `Send`, `Sync`, `Copy`,
      `Clone`, and `Debug` behavior.
- [x] Prefer public-behavior integration tests for Tier S and unit tests for
      private invariants.
  - Evidence: `src/lib.rs`, `src/safe/allocation.rs`, `src/safe/heap.rs`,
    `src/safe/stats.rs`, and `src/ffi/callbacks.rs` include compile-fail
    examples for raw unsafe calls, hidden implementation modules, internal
    typestate witnesses, uninitialized safe reads, move-only allocations,
    unsafe initialization, heap lifetimes, heap move/drop misuse, private
    representation construction, callback guard lifetimes, and negative
    `Send`/`Sync` heap behavior. Trait/static assertion tests cover the
    allocator facade traits, safe allocation `Send`/`Sync`, and error
    `Copy`/`Clone`/`Debug` behavior. `scripts/check-safe-docs.py` now protects
    the misuse-boundary compile-fail snippets from being removed.
- Verify: doctests, full Rust tests, and any trybuild-equivalent checks.

## Suggested implementation order

1. [x] Add the Rust style guide and link it from contributor docs.
2. [x] Add inventory checks for the safe/raw naming firewall.
3. [x] Audit public enums for `#[non_exhaustive]` decisions.
4. [x] Audit public structs for accidental public fields.
5. [x] Standardize constructor/accessor naming in safe and strict-type code.
6. [x] Audit errors and fallible APIs for consistent typed `Result` usage.
7. [x] Audit borrowing/clone patterns in `src/safe` first.
8. [x] Extend compile-fail tests for privacy, unsafe boundaries, and lifetimes.
9. [x] Gradually reduce clippy allowances in `src/safe` and `src/types` only
       where the signal is high and hot-path evidence supports the change.
10. [x] Use public API and ABI diffs for every cleanup that touches exported
        surfaces.

## Definition of done for Rust idiom standardization

- [x] A crate-specific Rust style guide exists and is linked from contributor
      and safety docs.
- [x] Safe Rust APIs use Rust naming, typed errors, RAII ownership, private
      representation, and documented constructors/accessors.
- [x] Raw/C ABI APIs remain explicitly unsafe, C-shaped, and compatibility
      classified.
- [x] Public enums have deliberate exhaustive or `#[non_exhaustive]` policy.
- [x] Public structs expose fields only when literal construction or ABI layout
      is intentionally part of the contract.
- [x] New internal strict wrappers have hot-path performance evidence where
      required.
- [x] Unsafe blocks and public unsafe functions follow one documented comment
      and audit-tag style.
- [x] Compile-fail and trait tests enforce ownership, lifetime, unsafe, and
      privacy boundaries.
- [x] Public API snapshots, ABI manifests, docs, changelog, and release
      checklist stay aligned after idiom-polish changes.
