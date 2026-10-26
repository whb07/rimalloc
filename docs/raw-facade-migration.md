# Raw Facade Migration

The public raw facade exposes the C-header-shaped API at
`rimalloc::raw`, with grouped aliases under `raw::abi` and
`raw::mimalloc`. Translated implementation modules are private and are not
supported integration paths.

## Replacing old paths

- Replace `raw::alloc::*`, `raw::alloc_aligned::*`, and `raw::free::*` with the
  corresponding root `raw::mi_*` functions.
- Replace `raw::heap::*`, `raw::stats::*`, and `raw::options::*` with root
  `raw::mi_*` functions or their `raw::mimalloc::*` aliases.
- Replace public C layouts and aliases formerly reached through `raw::types::*`
  with `raw::abi::*` or the root raw re-exports.
- There is no public replacement for translated implementation modules such as
  `raw::arena`, `raw::bitmap`, `raw::internal`, `raw::memory`, `raw::page`, or
  `raw::threadlocal`. Applications should use the safe crate-root API or the
  public raw C API.

The raw Rust module-path surface is unstable until explicitly versioned. The C
ABI and layout compatibility policies are documented in [abi.md](abi.md) and
[compatibility.md](compatibility.md).
