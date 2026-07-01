# Rust Style Guide

This guide defines the Rust-facing style for this crate. It complements the
release, ABI, unsafe-audit, and performance policies; it does not replace the
upstream parity rules.

## Boundary Rule

Do not mechanically rustify C ABI or raw compatibility code. C-shaped names,
null/error conventions, and raw pointer APIs belong only in:

- `raw`, where Rust callers explicitly opt into the unsafe C-shaped facade;
- `ffi`, where exported C ABI declarations, layouts, callbacks, and symbols are
  implemented;
- `platform`, where platform primitives intentionally mirror upstream or OS
  naming;
- crate-private mechanical translation internals where matching upstream shape
  preserves parity and reviewability.

The safe Rust facade should read like ordinary Rust ownership code. New safe
APIs should use Rust names and typed results instead of `mi_*`, `_mi_*`,
malloc-family, or C++ operator names.

## Naming

Use these conventions for Rust-facing APIs:

- `new` for infallible construction or conventional constructors that return a
  crate-specific result for resource acquisition already established by the
  type.
- `try_new` when construction validates input and callers need a typed error.
- `from_raw` or `from_non_null` for unsafe conversions from raw handles.
- `as_ptr` or `as_non_null` for borrowed raw pointer views.
- `into_raw` when ownership is deliberately transferred across an unsafe
  boundary.
- `len` and `is_empty` for byte or collection-like views.
- `layout` for allocation layout accessors.
- `with_*` for scoped callback or temporary-state helpers.

Unchecked invariant constructors must be named:

```rust
unsafe fn new_unchecked(...)
```

Avoid ambiguous constructor names such as `make`, `create`, `init`, or `get`
unless they are established domain vocabulary for the translated allocator
operation being modeled.

## Errors

Safe fallible APIs should return typed errors callers can match or inspect.
Use these names consistently:

- `ThingError` for the primary error type of a fallible operation or domain.
- `TryThingError` for conversion or construction failures where that naming
  reads naturally.
- `ThingErrorKind` when a stable wrapper carries both preserved ownership and a
  classified reason.

Expected absence should use `Option<T>` or `Option<&T>`. Do not expose C-style
integer status codes, null returns, or string errors from the Tier S safe
facade unless they are wrapped in a typed Rust API.

## Accessors And Borrowing

Borrow by default:

- read text as `&str`;
- read bytes as `&[u8]`;
- read paths as `&Path`;
- read sequences as `&[T]` or iterators;
- return internal sequences as `&[T]`, not `&Vec<T>`;
- return optional borrowed data as `Option<&T>`.

Take ownership only when the API stores, consumes, or transfers ownership.
Store owned text, paths, and bytes as `String`, `PathBuf`, `Vec<T>`, or
`Box<[T]>` according to growth and allocation needs.

Raw pointers should appear only at unsafe, raw, FFI, or platform boundaries.
Safe wrappers should expose borrowed slices, typed accessors, or ownership
types instead.

## Copy And Clone

Derive or implement `Copy` only for small scalar, pointer, handle, or witness
newtypes where duplication is cheap and semantically invisible. Do not make a
type `Copy` only because the compiler permits it.

Non-trivial `Clone` use should be intentional. Prefer borrowing or moving.
When a clone preserves a snapshot, test fixture, or ownership handoff, make the
reason visible through the variable name or a short comment.

## Public Representation

Public Rust-facing structs should keep fields private unless literal
construction is intentionally part of the stable contract. Preserve invariants
with constructors and accessors. Public fields are acceptable for C ABI
`repr(C)` layouts and explicitly documented DTO-like types, not as accidental
representation leaks.

Public enums should have an explicit exhaustiveness decision before release.
Use `#[non_exhaustive]` for growable safe or unsafe Rust enums. Keep exhaustive
enums only when downstream exhaustive matching is intentional.

## Unsafe Style

Keep unsafe blocks as small as practical and wrap the exact unsafe operation.
Avoid broad unsafe regions that mix pointer operations with ordinary control
flow.

Every unsafe block, unsafe impl, unsafe extern boundary, or unsafe attribute
needs a nearby safety comment with this shape:

```rust
// SAFETY[AUDIT:<operation-id>]: <why the operation is valid here>.
```

The comment should state the relevant pointer validity, aliasing, lifetime,
initialization, threading, ownership, and unwind invariants. Public unsafe
functions must also document caller obligations in a `# Safety` rustdoc section.

Every `SAFETY[AUDIT:*]` operation ID must be mapped to tests, tooling evidence,
or manual proof in `docs/unsafe-audit-evidence.tsv`.

## Hot Paths

Rusty types are useful only when they preserve allocator behavior. New
hot-path wrappers must not introduce hidden allocation, formatting, panics,
locks, syscalls, branches, stack spills, bounds-check slow paths, or dynamic
dispatch without measured justification.

Use transparent scalar or pointer wrappers for hot invariants, keep accessors
inline where the existing codebase does so, and attach the benchmark or
assembly evidence required by `docs/performance.md` and
`docs/strict-types.md` for allocator-core changes.

## Macros And Traits

Public macros are exceptional. Keep `#[macro_export]` for documented Tier R
C-header compatibility only. Prefer functions, inherent methods, or
crate-private macros for Rust-facing behavior.

Traits are public contracts. Prefer inherent methods for types owned by this
crate. Add traits only for real capability boundaries, keep them small, and
seal them when downstream implementations would block future representation or
optimization changes.
