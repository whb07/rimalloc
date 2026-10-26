# FFI Linking

The production-supported C ABI artifact for the current release policy is the
explicit cdylib build. Default Cargo library builds produce an `rlib` for Rust
integration and do not produce a linkable C library.

The crates.io package is the Rust library distribution. Production FFI
libraries and C headers are distributed as GitHub release assets built from the
tagged repository checkout. Do not use the crates.io package as the source for
FFI artifact production: repo-only release scripts and vendored C headers are
intentionally not part of that package.

## Release Artifact Layout

Build the release payload with:

```sh
scripts/build-release-artifacts.py
```

The script builds each artifact mode with an isolated `CARGO_TARGET_DIR`, copies
the outputs under `target/release-artifacts/artifacts/`, stages public headers
under `target/release-artifacts/include/`, generates link metadata under
`target/release-artifacts/lib/`, and writes
`target/release-artifacts/release-artifacts.json` with artifact paths, header
paths, link metadata paths, feature sets, target triple, profile, rustc
version, and SHA-256 checksums.

The staged binary artifacts are:

- `rust-rlib`: Rust integration artifact for ordinary Cargo users.
- `c-abi-cdylib`: production-supported dynamic C ABI artifact.
- `interpose-cdylib`: production-supported dynamic artifact for malloc-family
  interposition and C++ operator overrides.
- `c-abi-static-preview`: preview-only static C ABI archive.
- `interpose-static-preview`: preview-only static interposition/C++ archive.

## Dynamic C ABI Linking

Use the release payload's `c-abi-cdylib` artifact when a C or C-compatible
consumer needs explicit mimalloc `mi_*` symbols without replacing the process
allocator. The underlying build command is:

```sh
cargo rustc --release --features c-abi --lib --crate-type cdylib
```

Consumers should include headers from `target/release-artifacts/include/` and
link against the staged `c-abi-cdylib` path recorded in
`target/release-artifacts/release-artifacts.json`.

The release metadata also exposes this default mode through pkg-config and
CMake. The pkg-config package links the production `c-abi` cdylib by default,
and the CMake config exposes `rimalloc::c_abi`.

The focused consumer checks for this mode are:

```sh
scripts/check-release-artifact-consumer.py
scripts/check-release-link-metadata.py
scripts/check-release-dlopen.py
```

`check-release-dlopen.py` verifies that `dlopen` of the clean C ABI cdylib
resolves representative `mi_*` symbols and does not redirect plain libc
`malloc` by loading the library alone.

## Static C ABI Linking

Static archives are preview-only. They are used by repository parity tests,
source-build experiments, and static archive audits, but they are not a
production-supported FFI distribution artifact until the static symbol
contract, collision tests, and checked static manifests are complete.

The release artifact script builds the preview static archives as native-object
archives so focused C consumer smoke tests can link them with the platform C
linker. The underlying C ABI preview build command is:

```sh
RUSTFLAGS="-C lto=off -C linker-plugin-lto=no -C embed-bitcode=no" \
  cargo rustc --release --features c-abi --lib -- --crate-type staticlib
```

Static preview consumers must link the archive directly and provide the
platform libraries recorded in the release metadata: `pthread`, `dl`, and `m`.
If a release candidate includes static archives for evaluation, the artifact
name must include a preview marker such as
`rimalloc-static-preview`, and the release notes must state that the
archive is not a stable static ABI.

The focused static consumer coverage is:

```sh
scripts/check-release-artifact-consumer.py
```

That script builds and runs C consumers against both `c-abi-static-preview` and
`interpose-static-preview` from a copied release payload outside the repository.

## macOS DYLD Interposition

Build a release dylib from the repository, then inject it into a native program:

```sh
cargo rustc --features interpose --lib --release --crate-type cdylib
DYLD_INSERT_LIBRARIES="$PWD/target/release/librimalloc.dylib" ./program
```

Pass `--crate-type` to Cargo, before any `--` separator, so Cargo selects the
correct LTO mode. `DYLD_FORCE_FLAT_NAMESPACE` is not needed. macOS may block or
strip loader injection for protected programs and hardened applications without
the relevant entitlements; use a program that permits dynamic-library injection.

The `interpose` build registers an SDK-defined malloc zone, promotes it to
libmalloc's default zone, and interposes malloc-family functions. The default
zone serves `malloc()`, typed allocation, and default-zone calls from every
image, so programs that link the crate statically are overridden as well; dyld
applies `__interpose` tuples only to dylibs loaded at launch, where they skip
libmalloc's dispatch. Load the dylib at launch (`DYLD_INSERT_LIBRARIES` or a
link-time dependency), not with `dlopen`, because promotion reorders the zone
list. Ownership checks route system and custom-zone pointers back to libmalloc.
libc++ supplies `operator new`/`delete`, exceptions, and `new_handler` behavior
through the interposed C allocation functions.

Blocks are 16-byte aligned and sized by the plain size classes, as with
libmalloc. `malloc_size` and `malloc_zone_from_ptr` recognize block starts;
pages that hold over-aligned or guarded allocations also accept pointers inside
a block, and freed blocks are not detected. `aligned_alloc` is interposed when
the deployment target is macOS 10.15 or newer; otherwise libmalloc's
implementation reaches the zone through its `memalign` callback.

Allocator locks are quiesced around `fork` by `pthread_atfork` handlers that
process initialization registers on every Unix target: the prepare handler
waits until no thread holds an allocator lock and holds new acquisitions until
the fork completes, so a forked child can allocate even when other threads were
in allocator slow paths. The zone's `force_lock` and `force_unlock` hooks
therefore stay no-ops.

Native C and C++ consumers exercise constructor allocations, size/alignment,
size classes, zeroing, overflow and errno, zone callbacks and promotion, foreign
allocations, typed malloc, dlopen libraries, cross-thread frees, a
single-threaded fork, and statistics at exit:

```sh
scripts/check-macos-interpose.py
```

The script checks normal release and `secure,guarded` release builds without
linking the consumers against mimalloc, and compiles the shim with `-Werror` at
the arm64 and x86_64 deployment targets. Remote zone heap enumeration is not
implemented (`KERN_NOT_SUPPORTED`), and zone introspection statistics return
zeros; use the mimalloc stats APIs for allocator statistics. The release-asset
packaging pipeline is still scoped to Linux; this macOS path is built and
validated directly from source.

## LD_PRELOAD Interposition

Use the release payload's `interpose-cdylib` artifact when the process allocator
must be replaced through ELF interposition. The underlying build command is:

```sh
cargo rustc --features interpose --lib --release --crate-type cdylib
```

Run the target program with the staged interpose cdylib in `LD_PRELOAD`:

```sh
LD_PRELOAD=/path/to/librimalloc-interpose-cdylib.so ./program
```

The interpose feature enables the C ABI, malloc-family override exports, the C++
operator override shim, and the TLS shim required by the optimized Linux
interposition path.

The focused validation for this mode is:

```sh
scripts/check-release-ld-preload.py
```

That script compiles a C probe, runs it under `LD_PRELOAD`, verifies
malloc-family allocation ownership through `mi_check_owned`, checks
alignment-sensitive allocation APIs, and verifies shutdown stats when
`mimalloc_show_stats=1`.

## C++ Operator Override Modes

C++ operator new/delete overrides are shipped with the interpose artifacts, not
the clean `c-abi-cdylib`. Use `interpose-cdylib` for a dynamic C++ override
artifact and `interpose-static-preview` only for preview static evaluation.

The lower-level `cxx-overrides` feature depends on `c-abi`, but release
metadata exposes the supported C++ override path through the interpose artifact
names and the `rimalloc::interpose` CMake target.

The C++ header for this mode is `mimalloc-new-delete.h`. The focused validation
is:

```sh
scripts/check-release-cxx-artifacts.py
```

That script builds C++20 probes from a copied release payload and verifies
scalar, array, nothrow, sized delete, aligned allocation, aligned object, and
`std::new_handler` behavior for `interpose-cdylib` and
`interpose-static-preview`.

## Headers and Link Metadata

The staged headers are:

- `mimalloc.h` for the core C API.
- `mimalloc-stats.h` for stats APIs.
- `mimalloc-override.h` for C interposition users.
- `mimalloc-new-delete.h` for C++ operator new/delete users.

The staged link metadata is:

- `lib/pkgconfig/rimalloc.pc`, whose default `Libs` line links the
  production `c-abi` cdylib and records include path, artifact path, `pthread`,
  `dl`, `m`, and artifact-mode notes. Preview static archive paths are exposed
  as variables, not as the default production link mode.
- `lib/cmake/rimalloc/rimalloc-config.cmake`, which
  defines `rimalloc::c_abi`,
  `rimalloc::interpose`,
  `rimalloc::c_abi_static_preview`, and
  `rimalloc::interpose_static_preview` imported targets.

Validate package metadata consumers from a temporary directory outside the
repository:

```sh
scripts/check-release-link-metadata.py
```

## Supported Targets and Caveats

Linux x86_64 GNU is the maintained production target for release artifact
signoff. Other source-present platforms may build or pass smoke checks, but
they are not promoted to maintained release-artifact targets until their
platform-specific ABI, interposition, sanitizer, and consumer-test evidence is
recorded in the release process.

ELF-specific examples in this guide, including `LD_PRELOAD`, `dlopen`,
`readelf`, and the Linux version-script based export policy, apply to the
maintained Linux GNU target. Unsupported platforms must not infer a stable C
ABI, static archive contract, C++ operator override contract, or interposition
contract from source presence alone.

## Integrity Evidence

Release integrity evidence is generated under
`target/release-artifacts/integrity/`. The dry run writes `SHA256SUMS`, ABI
symbol manifests for each staged cdylib/staticlib artifact, an environment note
captured with `scripts/env-info.sh`, and benchmark evidence references aligned
with `docs/performance.md`.
