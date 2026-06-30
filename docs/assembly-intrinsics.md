# Assembly And Platform Intrinsics Policy

This port uses small amounts of inline assembly, global assembly, naked
functions, TLS accessors, and platform syscalls to preserve mimalloc's C ABI
and Linux x86_64 hot paths. These sites are part of the allocator contract and
must be audited like unsafe code.

## Maintained Assembly Target

The maintained assembly target is Linux x86_64 GNU. Other translated platform
paths may be source-present, but they are not maintained until the platform
support document lists parity, ABI/layout, CI or local verification, and
benchmark evidence.

## TLS Accessors

The Linux x86_64 fast path accesses C TLS cells directly with initial-exec TLS
model symbols:

- `__mi_theap_default`
- `__mi_theap_cached`
- `__mi_theap_main`
- `mi_thread_locals`

The C shim in `src/tls_shim.c` owns the hidden TLS storage. Rust accessors in
`src/platform/mod.rs`, `src/allocator/init.rs`, and
`src/allocator/threadlocal.rs` use `@GOTTPOFF` loads and `fs:` addressing to
read or write those cells. Scratch registers for TLS offsets are declared as
assembly outputs; the call sites must not rely on their prior values.

## Malloc And Free Fast Paths

On Linux x86_64 with the fast malloc configuration, selected allocation entry
points are implemented with `#[unsafe(naked)]`, `naked_asm!`, or `global_asm!`
to avoid Rust prologue/epilogue code before the allocator reaches the thread
heap fast path. The exported ABI names include the C malloc surface and selected
C++ `operator new/delete` mangled symbols.

These entry points must preserve the platform ABI, branch to the normal Rust
fallback on slow paths, and avoid stack assumptions that Rust cannot verify.
Any change to registers, clobbers, symbol names, or fallback labels requires a
source audit and fast-path benchmark check.

## Hidden Symbols

Internal allocator globals used by assembly are hidden from the dynamic ABI.
`global_asm!` declarations and the Linux version script work together so that
implementation details such as heap sentinels and CPU thresholds do not become
public ABI. Public symbol changes must be reflected in the ABI policy and
symbol audit.

## Platform Primitives

The Linux maintained path depends on libc/pthread and kernel primitives such as
`mmap`, `munmap`, `mprotect`, `madvise`, `getrandom` or `/dev/urandom`, errno,
page-size queries, and TLS behavior. Platform-specific intrinsics and syscalls
belong behind the existing OS/primitive modules; callers should not duplicate
direct syscall or inline assembly logic outside those boundaries.

Windows, macOS, WASI, Emscripten, and other Unix-like translations are
source-present but not maintained targets unless `docs/platforms.md` says
otherwise.

## Change Requirements

Assembly or intrinsic changes must document:

- the exported or hidden symbol name,
- the ABI and target cfg,
- register inputs, outputs, and scratch values,
- stack assumptions,
- fallback path,
- feature gating,
- expected codegen or source-shape invariant.

For hot-path assembly, run a benchmark smoke check first and escalate to a fuller
run when instruction shape, branches, TLS access, or allocator-wide behavior can
move.

## Verification

Relevant local checks are:

```sh
scripts/check-abi-symbols.sh
scripts/check-layout.sh
cargo test --quiet --test release_fast_path_audit --features "std unstable-raw-internals"
```

The release fast-path audit checks the expected TLS symbol usage, hidden-symbol
declarations, naked/global assembly presence, exported override names, and
selected source-shape invariants.
