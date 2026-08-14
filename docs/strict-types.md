# Strict Internal Types

The crate-private types under `src/types/` encode allocator invariants without
changing the C ABI or hot-path representation. Raw C layouts remain in
`src/types/constants.rs`; the public C-shaped surface remains under `raw` and
`ffi`.

## Design rules

- Scalar wrappers distinguish sizes, alignments, counts, bins, indexes, and
  packed words when the distinction prevents invalid internal calls.
- Pointer wrappers are non-null identities with explicit accessors. Allocator
  metadata wrappers do not implement `Deref` or `DerefMut`.
- Linear capabilities represent validated pointers, page state, queue
  membership, detached lists, and ownership transitions. They implement
  neither `Copy` nor `Clone`.
- Fallible state transitions return the original capability on failure.
- Raw compatibility entry points validate or classify input once before
  entering typed internal operations.
- Wrappers used on allocation and free paths must compile away; layout and
  release assembly checks apply when their representation or hot behavior
  changes.

## Enforcement

Compiler assertions in `src/types/linearity_tests.rs` verify that ownership
capabilities are not `Copy` or `Clone`. Unit tests cover constructors, layout,
state transitions, and boundary values.

`scripts/check-strict-types.py` is intentionally narrow. It checks only
architectural rules Rust visibility cannot currently express:

- memid union interpretation stays behind `MemId` helpers;
- raw block-link access stays behind free-list helpers;
- packed thread-owner and thread-free bit operations stay behind typed owners;
- pointer-to-atomic casts stay in atomic owner modules;
- unchecked raw page-state construction is private to the allocator subtree;
- metadata pointer wrappers do not implement `Deref`;
- allocator internals do not introduce dynamic dispatch.

The checker has no debt allowlist, per-file caps, wildcard counts, or
documentation snapshot. Rust privacy, rather than the checker, confines raw
page-state construction.

Run:

```sh
scripts/check-strict-types.py
scripts/check-layout.sh
cargo test --lib --all-features
```
