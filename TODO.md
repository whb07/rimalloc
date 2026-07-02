# TODO: Next Audit Granularity and Harness Backlog

Fresh-eyes review after completing the previous TODO backlog on `master` at
`985da34`. The previous backlog is implemented, validated, committed, and
pushed. The next mechanical translation debt is in remaining broad unsafe audit
operation IDs and release-audit modules that are still close to the module cap.

Current review evidence:

- `scripts/check-unsafe-audit.py` covers 82 source files, 5,893 unsafe
  mentions, and 123 operation IDs. The migration allowlist remains empty.
- `scripts/check-unsafe-audit-evidence.py` covers those 123 operation IDs with
  306 automated evidence rows.
- `strict-type-allowlist.txt` still has 26 wildcard entries left: 4
  raw-pointer-parameter entries, 13 bare-domain `usize` entries, 4
  pointer/usize round-trip entries, 2 metadata-reference entries, 2 atomic-cast
  entries, and 1 heap-allocating-type entry.
- No remaining path has more than one strict-type wildcard entry. The residual
  path-level entries are public/raw ABI, platform ABI, proof-fixture, or
  typed-domain constructor boundaries such as `src/allocator/heap.rs`,
  `src/platform/mod.rs`, `src/raw/macros.rs`, `src/types/layout.rs`,
  `src/safe/allocation.rs`, and `src/types/atomic.rs`.
- The largest remaining unsafe audit marker clusters are
  `init-thread-tls-boundary` with 129 markers,
  `windows-primitive-boundary` with 114 markers,
  `arena-page-lifecycle-boundary` with 107 markers,
  `arena-allocation-boundary` with 105 markers,
  `allocation-abi-entry-boundary` with 104 markers,
  `internal-atomic-facade` with 103 markers,
  `page-allocation-boundary` with 102 markers, and
  `init-subproc-boundary` with 102 markers.
- The release-audit root harness is 247 lines under a 300-line root cap. The
  module cap is still 1,000 lines; the largest modules are
  `unsafe_performance/lifecycle_boundaries.rs` at 922 lines,
  `unsafe_performance/platform_primitive_boundaries.rs` at 895 lines,
  `unsafe_performance/raw_facade.rs` at 852 lines,
  `unsafe_performance/page_arena_behavior.rs` at 724 lines,
  `release_policy/docs_and_linking.rs` at 633 lines,
  `release_policy/api_surface.rs` at 517 lines,
  `datatype_acceptance/strict_type.rs` at 512 lines, and
  `release_policy/release_process.rs` at 504 lines.
- The previous raw macro, allocation/aligned/stats/page-map/OS, typed pointer,
  and root release-audit aggregate splits are complete and guarded by focused
  release-audit tests.

## Issue 1: Split init thread/TLS and subproc audit clusters

> **Comment:** `init-thread-tls-boundary` has 129 markers and
> `init-subproc-boundary` has 102 markers. They mix thread lifecycle, default
> and cached thread-heap TLS, platform TLS slot/key setup, TLD allocation/free,
> subproc identity, main-subproc state, and heap/subproc association.

- [ ] Split `init-thread-tls-boundary` into thread lifecycle, thread-heap TLS,
      platform TLS slot/key, and TLD allocation/free operation IDs.
- [ ] Split `init-subproc-boundary` into subproc identity/registry,
      main-subproc lifecycle, heap association, and subproc stats/count
      operation IDs.
- [ ] Update `UNSAFE_AUDIT.md`, evidence rows, and focused release-audit guards
      for each new init operation ID.

Verify: lifecycle stats exports, release fast-path audit lifecycle modules,
unsafe audit scripts, unsafe evidence scripts, full clippy, and full tests.

## Issue 2: Split page and arena allocation lifecycle clusters

> **Comment:** `arena-page-lifecycle-boundary` has 107 markers,
> `arena-allocation-boundary` has 105 markers, and `page-allocation-boundary`
> has 102 markers. They cover distinct allocation, page queue, arena reserve,
> huge-page, free/abandon, and stats invariants.

- [ ] Split page allocation markers by free-list refill/search, queue
      transition, huge-page allocation, and stats/public helper responsibility.
- [ ] Split arena allocation markers by slice allocation, arena initialization,
      reserve growth, OS fallback, public reserve/manage API, and huge-page
      reservation responsibility.
- [ ] Split arena page lifecycle markers by fresh/regular/singleton page setup,
      free/abandon/reabandon/unabandon shims, heap arena-page publication, and
      page ownership transitions.

Verify: page/arena release-audit modules, original arena/static tests,
allocation boundary tests, unsafe audit scripts, unsafe evidence scripts, full
clippy, and full tests.

## Issue 3: Split platform and atomic facade audit clusters

> **Comment:** `windows-primitive-boundary` has 114 markers,
> `internal-atomic-facade` has 103 markers, and `unix-primitive-backend` still
> has 90 markers. These modules mix memory primitives, process/env helpers,
> TLS/thread helpers, lock/once facades, atomic field wrappers, and
> platform-specific compatibility shims.

- [ ] Split `internal-atomic-facade` into lock/once, atomic field, pointer-sized
      atomic, remote-free/thread-free, and platform lock shim operation IDs.
- [ ] Split `windows-primitive-boundary` into virtual memory, process info,
      stderr/env, TLS/FLS lifecycle, large/huge-page, NUMA, and ETW operation
      IDs.
- [ ] Split `unix-primitive-backend` where existing helper groups already
      separate mmap/syscall, process info, stderr/env, random, thread/TLS,
      huge-page, and NUMA behavior.

Verify: platform primitive release-audit modules, atomic ordering audit, unsafe
audit scripts, unsafe evidence scripts, full clippy, and full tests.

## Issue 4: Split remaining large release-audit modules and lower the module cap

> **Comment:** The root harness is now small, but the largest focused modules
> are still near the 1,000-line cap. The next cap reduction should happen only
> after moving coherent checks into submodules.

- [ ] Split `unsafe_performance/lifecycle_boundaries.rs` into process lifecycle,
      thread/TLS, options/guarded, random/CPU, and platform hook modules.
- [ ] Split `unsafe_performance/platform_primitive_boundaries.rs` into Windows,
      Unix/WASI/Emscripten, macOS, and shared primitive-policy modules.
- [ ] Split `unsafe_performance/raw_facade.rs` into raw override, libc/POSIX,
      C++ compatibility, header forwarding, and bootstrap/query modules.
- [ ] Lower the release-audit module cap below 1,000 after the largest modules
      are comfortably below the new cap.

Verify: release fast-path audit, full formatting, full clippy, and full tests.

## Suggested implementation order

1. [ ] Split init thread/TLS and subproc audit clusters.
2. [ ] Split page and arena allocation lifecycle clusters.
3. [ ] Split platform and atomic facade audit clusters.
4. [ ] Split remaining large release-audit modules and lower the module cap.

## Definition of done for this backlog

- [ ] Remaining unsafe audit operation IDs above 100 markers are split into
      narrower review units with separate docs, evidence, and release-audit
      guards.
- [ ] Release-audit modules are split enough to lower the 1,000-line module cap
      without weakening coverage.
- [ ] `TODO.md` and its release-audit guard reflect the current measured counts.
- [ ] For every completed item, run `cargo fmt --all`,
      `cargo clippy --workspace --all-targets --all-features`, and
      `cargo test --workspace --all-targets --all-features`; also run the
      relevant audit scripts before committing and pushing to `origin/master`.
