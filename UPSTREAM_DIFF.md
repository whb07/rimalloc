# Differences From Upstream mimalloc 3.3.2

This crate preserves the allocator behavior and C-facing layouts of vendored
mimalloc 3.3.2 while translating the implementation to Rust. The vendored C
allocator remains the differential and benchmark oracle; it is not linked into
the Rust allocator artifact.

Current intentional differences are:

- a safe Rust facade (`MiMalloc` and owned allocation wrappers) alongside the
  explicitly unsafe raw C-shaped facade;
- crate-private semantic types in `src/types/`, internally aliased as
  `core_types`, around the translated C-compatible representation;
- opt-in C ABI/interposition build modes instead of unconditional symbol
  override in normal Rust-library builds;
- benchmark-only relaxed diagnostic counters and evidence probes, which do not
  participate in ordinary allocator behavior;
- repository release gates for Rust/C differential traces, layout and symbol
  parity, strict-type source shape, artifact-specific performance, and resource
  evidence.

Allocator policy changes are out of scope until the staged `CompatPolicy` work
has an exact-equivalence reference model. Any later intentional divergence must
be added here with its compatibility and benchmark evidence.
