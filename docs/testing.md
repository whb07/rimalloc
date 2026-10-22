# Testing And Hardening Policy

This repository uses several complementary test classes. A change does not need
to run every expensive class every time, but release claims must map to the
right evidence.

## Unit And Integration Tests

Default Rust tests cover translated API behavior, release-shape audits, feature
contracts, public ABI assumptions, and deterministic property-style allocation
sequences:

```sh
cargo test --quiet --features std
cargo test --quiet --features std --test property_alloc_sequences
cargo test --quiet --features std --test fuzz_alloc_sequences
```

The property sequence test is intentionally deterministic. It exercises bounded
random malloc, calloc, aligned allocation, heap allocation, realloc, free,
usable-size, zero-fill, alignment, preservation, and heap-containment
properties without requiring an external fuzzing engine.

Safe allocation failure handling has deterministic test-only injection at the
safe wrapper boundary. The injected path is compiled only for unit tests and
exercises `AllocError`, `ResizeError`, heap ownership preservation, and raw
overflow null-return behavior without relying on huge allocations or host
overcommit policy.

Process-global callback guard tests are marked ignored in the default parallel
unit-test run because the Tier U constructors borrow Rust closures through
process-global callback slots. `scripts/check-safety-stress.sh` runs those
ignored callback tests with `--test-threads=1`; they cover serialized
registration/restoration, nested LIFO restore, allocator reentry, deferred-free
reentry suppression, and panic containment.

Release candidates must also run:

```sh
scripts/check-release-thread-stress.py
```

This builds the release `c-abi` cdylib and runs a C consumer over sustained
same-thread allocation churn, cross-thread frees, abandoned-page visitation and
reclaim, repeated thread init/done, final stats capture/printing, and process
shutdown.

`tests/fuzz_alloc_sequences.rs` adds a byte-stream operation interpreter over a
fixed corpus. The maintained external fuzz harness in `fuzz/` uses the same
operation model outside the normal test crate. It covers malloc, zalloc,
calloc, aligned allocation, heap allocation, heap-aligned allocation, realloc,
rezalloc, collect, null realloc/free, overflow-sized request rejection,
cross-thread free, usable-size lower bounds, stats capture, zero-fill,
alignment, preservation, and heap ownership.

## Safe API / Raw Facade Split Checks

Use these checks when changing the crate-root facade, `raw`, FFI declarations,
or documentation that describes the split:

```sh
scripts/check-api-inventory.py
scripts/check-raw-facade.sh
scripts/check-ffi-boundary.py
scripts/check-docs.sh
```

`scripts/check-api-inventory.py` verifies the crate-root safe API, default raw
modules, private implementation-module absence, and public inventory docs.
`scripts/check-raw-facade.sh` verifies that `raw` does not re-export
implementation modules and that tests/examples/benches do not import removed
private raw implementation paths.

## Sanitizers

AddressSanitizer is available through:

```sh
scripts/check-sanitizers.sh
```

ThreadSanitizer is available through:

```sh
SANITIZER=thread scripts/check-sanitizers.sh
```

The sanitizer suites include `tests/thread_lifecycle.rs`, which mixes explicit
teardown with automatic thread-exit teardown while other workers reuse metadata.
The Unix unit test `explicit_thread_done_clears_destructor_before_metadata_reuse`
also checks the pthread destructor association directly. For the optimized
lifecycle configuration used in X40:

```sh
RUSTFLAGS='--cfg mi_sanitizer_runtime -Zsanitizer=thread -Zunstable-options -Cpanic=immediate-abort -C lto=off -C linker-plugin-lto=no -C embed-bitcode=no' \
  cargo +nightly test -Zbuild-std -Zpanic-abort-tests --release \
  --target-dir target/sanitizer-thread-release --target x86_64-unknown-linux-gnu \
  --features std --test thread_lifecycle
```

X40 records the exact toolchain and its temporary `-Aunused-unsafe` compatibility
flag, the reproducing C-ABI stress configuration, and the before/after runs.

The script uses a nightly toolchain, `-Zbuild-std`, aborting test panic
strategy, and the `std` feature. It sets `mi_sanitizer_runtime` so sanitizer
builds can disable libc allocator interposition exports that conflict with the
sanitizer runtimes. Sanitizer runs are expected to be Linux x86_64 GNU evidence
unless the report says otherwise. UBSan is not currently part of the maintained
signoff set; claims about that tool require adding explicit scripts and
documenting known false positives. Maintainers can also run the manual GitHub
Actions hardening workflow for ASan and TSan evidence.

`track-asan` is allocator tracking instrumentation, not a substitute for running
the sanitizer toolchain. When `track-asan` and `track-valgrind` are enabled
together by aggregate tooling, Valgrind mode takes precedence.

## Valgrind

Valgrind tracking parity is checked by:

```sh
scripts/check-valgrind-tracking-parity.py
```

This check builds C and Rust probes and compares the tracked reports. It
requires `valgrind` on the host. The normal Rust test suite does not imply
Valgrind evidence. Its Rust archive is built in a dedicated target directory
so a shared-target artifact with inactive tracking cannot satisfy the check.
Maintainers can also run the manual GitHub Actions hardening workflow for
Valgrind tracking evidence.

## Differential Tests Against C mimalloc

Behavioral parity against the vendored mimalloc 3.3.2 C baseline is covered by
the scripts listed in `PARITY.md`, especially:

```sh
scripts/check-behavioral-parity.py
scripts/check-no-unwind-artifact.py
scripts/check-cxx-operator-new-parity.py
scripts/check-release-cxx-artifacts.py
scripts/check-secure-mode-parity.py
scripts/check-guarded-mode-parity.py
scripts/check-asan-tracking-parity.py
scripts/check-valgrind-tracking-parity.py
scripts/check-unix-syscall-parity.py
```

These checks compare null behavior, usable-size behavior, alignment, realloc
preservation, heap APIs, options, stats, feature modes, C++ overrides, and
platform-specific behavior where the current supported matrix claims parity.
The Linux syscall check also forces a successful misaligned hinted mapping and
both success and failure of the next mapping attempt. An external page ledger
checks that normal teardown releases every mapped page in both Rust and C.
The same check builds optimized release and secure-level-4 release probes for
decommit protection, `needs_recommit`, and recommit/reuse. It checks Linux
mapping permissions and isolated child access to an owned primitive mapping,
then exercises allocation/collection/reuse on an initialized worker. These
C-linked release probes disable LTO so their static archives contain native
objects usable by the system C linker.
`scripts/check-no-unwind-artifact.py` builds the release `c-abi` cdylib,
compiles a C consumer that exercises allocation, overflow/error, callback, and
OOM/null paths, checks the artifact's aborting no-std personality boundary, and
runs the serialized callback panic-containment tests for `std`/test builds.
`scripts/check-behavioral-parity.py` prints the artifact modes it exercises at
startup. Use `scripts/check-behavioral-parity.py --only dynamic` for the
CI-friendly dynamic artifact-mode slice: `dlopen` loads the Rust `c-abi`
cdylib, while `LD_PRELOAD` and shutdown show-stats checks use the Rust
`interpose` cdylib. Use `--list-artifact-modes` to print the current mapping
without building artifacts.
Known unresolved baseline mismatches remain documented in `PARITY.md`.

## Fuzzing Policy

The maintained external fuzz target is:

```sh
scripts/check-fuzz-smoke.sh
```

The script builds `fuzz/Cargo.toml` and runs `fuzz/fuzz_targets/alloc_sequences.rs`
for a bounded smoke pass over `fuzz/corpus/alloc_sequences` plus deterministic
generated inputs. Release candidates should increase `--runs` locally or in a
dedicated workflow when deeper fuzzing time is available.

The fuzz target generates operation sequences over:

- malloc, calloc, zalloc, realloc, rezalloc, aligned allocation, heap allocation,
  collect, and free,
- nulls, zero sizes, overflow-sized requests, small sizes, large sizes, and
  alignment classes,
- per-heap ownership, cross-thread free, realloc preservation, zeroing,
  usable-size lower bounds, and stats consistency.

When a fuzz input fails, first classify whether it is valid API use, documented
C-compatible abort behavior, or intentionally unsupported invalid/double-free
behavior. Minimize by deleting bytes from the failing input while preserving the
failure under `cargo run --manifest-path fuzz/Cargo.toml --bin alloc_sequences -- <input>`.
Commit minimized valid reproducers under `fuzz/corpus/alloc_sequences/` with a
descriptive filename and add a regression test when the failure maps to a fixed
allocator bug.

Minimized reproducers live in one of two places:

- `fuzz/corpus/alloc_sequences/<descriptive-name>` for byte inputs accepted by
  `fuzz/fuzz_targets/alloc_sequences.rs`.
- A focused Rust regression test under `tests/` or the owning source module
  when the failure needs setup that cannot be represented as an alloc-sequence
  corpus input.

Corpus additions are required when a confirmed bug is reachable through valid
safe API use, documented raw/C ABI use, release artifact behavior, or a
hardening feature mode. Add a corpus entry or focused regression before closing
any fix for allocator-owned memory corruption, realloc preservation, zeroing,
alignment, heap ownership, cross-thread free, stats/process observation,
panic/unwind, or supported artifact-mode behavior. Add corpus coverage when the
fuzz target gains a new operation class or when a minimized security reproducer
can be safely committed after disclosure.

Do not commit corpus inputs that depend on invalid caller UB unless the harness
is being changed to reject, classify, or safely model that invalid input. A
finding is an allocator bug when well-formed safe Rust, documented raw/C ABI
use, or a documented hardening mode violates this crate's contract or diverges
from C mimalloc 3.3.2 on a maintained target without a documented exception. A
finding is invalid-caller UB when it requires forged pointers, foreign
allocations, use-after-free, unsynchronized raw shared state, invalid alignment,
or ownership violations outside the safe facade and documented C ABI contract.
Documented C-compatible abort behavior is not an allocator bug unless this crate
documents a stronger non-aborting guarantee for that path.

Confirmed fuzz findings that touch unsafe invariants must leave durable
evidence before the fix is complete:

- Update `docs/unsafe-audit.md` when the proof obligation or boundary
  explanation changes.
- Keep the minimized corpus input or add a focused regression test.
- Run the relevant sanitizer, Miri, Loom, parity, or stress check and record
  environmental limitations in the change description.
- Update the owning boundary or type documentation when its proof obligation
  changes; add a local comment only for a non-obvious operation-specific
  invariant.

Private security reproducers stay out of the public corpus until coordinated
disclosure allows publication. `SECURITY.md` and
`docs/security-advisories.md` define the private reporting and disclosure
process.

## Miri

Miri is limited to pure Rust models and small library tests. The crate keeps
core-type unit tests for layout math, scalar domains, page owner/flag packing,
and thread-free word packing so those invariants can run without exercising the
full OS allocator:

```sh
scripts/check-miri.sh
```

Full OS allocator behavior is not expected to run under Miri until the relevant
subsystems have model-only tests.
