# Changelog

All notable changes to this repository should be recorded here.

## Unreleased

- Define the crate as a dual C ABI allocator artifact and experimental Rust
  library facade.
- Add the explicit `raw` compatibility facade and make implementation modules
  private at the crate root.
- Gate old broad `raw` implementation-module paths behind
  `unstable-raw-internals` as raw Rust path cleanup. This is not a C ABI symbol
  removal; mimalloc-compatible C ABI symbols and public C layouts remain
  validated separately.
- Add package metadata, project documentation, and initial policy documents.
