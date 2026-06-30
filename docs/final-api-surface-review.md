# Final API Surface Review

Date: 2026-06-18

This review closes the safe Rust API and raw/FFI split workstream. It records
the final public surface, rustdoc review, and dynamic symbol review evidence.

## Scope

Reviewed workstream history:

- `195dc9f Add API and support policy docs`: introduced the public API
  inventory and release-policy documentation.
- `258c8cb Add checked public API inventory`: added the inventory drift check.
- `afdd562 Move C ABI declarations into abi module`: moved public C ABI
  declarations into the dedicated ABI owner.
- `5094e9d Gate raw internals behind unstable feature`: stopped default raw
  from exposing implementation modules.
- Later release-readiness work removed that temporary feature entirely after
  repository tests stopped importing raw implementation modules.
- `fc790aa Record final signoff checks`: recorded the full local signoff pass
  before this final review artifact.

The current final surface is the authority for release signoff. The repository
does not contain a separate public-API diff generator; the reviewed diff is the
checked inventory and policy delta across the workstream commits above.

## Public API Diff Review

Current crate root public items are:

- `MiMalloc`
- `Allocation`
- `InitializedAllocation`
- `AllocError`
- `Heap`
- `HeapAllocation`
- `HeapError`
- `HeapRef`
- `InitializedHeapAllocation`
- `LayoutError`
- `MainHeap`
- `GlobalOptions`
- `OptionError`
- `OptionKey`
- `ProcessInfo`
- `ResizeError`
- `ResizeErrorKind`
- `StatCount`
- `StatTotal`
- `StatsError`
- `StatsSnapshot`
- `TryAllocError`
- `raw`
- `unsafe_callbacks`

`scripts/check-api-inventory.py` validates this list against
`docs/api-inventory.md` and the source. Implementation modules such as `alloc`,
`arena`, `free`, `heap`, `internal`, `os`, `page`, `page_map`, `prim`,
`threadlocal`, and `types` are not public crate-root API.

Intentional API changes reviewed:

- Removed crate-root implementation modules from the public facade. This is
  intentional Tier I cleanup, not a safe API removal.
- Added and documented the safe Tier S facade: `MiMalloc`, allocation wrappers,
  heap wrappers, options, stats, process info, and the safe error/status types.
- Classified `mechanical_mimalloc::unsafe_callbacks` as unsafe-adjacent Rust
  API outside Tier S because borrowed callback guards require process-global
  synchronization and lifetime obligations.
- Classified `mechanical_mimalloc::raw` as Tier R: unsafe, C-shaped, and
  unstable as a Rust path surface unless explicitly versioned later.
- Kept default `raw` narrow: root `mi_*` C-header mirror plus `raw::abi`,
  `raw::mimalloc`, and `raw::headers`.
- Removed translated implementation modules from the raw facade after migrating
  repository coverage to public raw paths, source inspection, or crate-private
  unit tests. This is raw facade cleanup, not C ABI removal.
- Kept C ABI declarations and raw layouts owned by `src/ffi/abi.rs`, exposed
  through `raw::abi` and `raw::mimalloc`.

Review result: the final API shape matches `docs/api-inventory.md`,
`docs/compatibility.md`, `docs/safe-api.md`, and `docs/abi.md`.

## Rustdoc Review

Command:

```sh
cargo doc --no-deps --features std
```

Review result:

- The crate landing page presents the safe Rust facade first.
- `MiMalloc`, `Allocation`, and the safe error/status types have item-level
  rustdoc with examples, failure behavior, panic behavior, and feature notes.
- `raw` is documented as a low-level unsafe C-shaped facade.
- Unsafe raw examples use explicit `unsafe` blocks.
- Implementation modules are absent from the crate-root rustdoc surface.
- Broken intra-doc links are denied by crate lint policy and the docs command
  passes.

Additional evidence:

```sh
cargo test --doc --features std
scripts/check-docs.sh
```

## Dynamic Symbol Review

Commands:

```sh
scripts/check-abi-symbols.sh
scripts/check-layout.sh
scripts/check-c-link-public-api.py
scripts/check-ffi-boundary.py
```

Review result:

- No mimalloc-compatible public-header symbol is missing.
- Public ABI layouts, offsets, constants, and feature-sensitive layout checks
  pass.
- C smoke examples compile and link against the produced artifact.
- Export attributes, naked shims, and assembly exports are confined to
  classified boundary files.
- Extra relevant public symbols are classified in
  `symbols/public-symbol-classification.tsv`.

The latest symbol audit reported:

- 190 public header symbols exported.
- 228 public ABI symbols present.
- 27 relevant extra exports classified.

## Compatibility Conclusion

The final split is:

- Tier S: crate-root safe Rust API, Rust SemVer surface.
- Tier R: `raw` Rust facade, unsafe and unstable unless explicitly versioned.
- Tier C: exported C ABI symbols and raw layouts, validated independently from
  Rust SemVer.
- Tier I: private allocator internals, not public API.

The workstream preserved the C ABI while narrowing the accidental Rust public
surface. Future public additions must update `docs/api-inventory.md` and pass
the API, raw facade, safe boundary, FFI boundary, docs, layout, and symbol
checks before release.
