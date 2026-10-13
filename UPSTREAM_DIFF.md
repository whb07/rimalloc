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

## Documented divergences

### `page_full_retain` defaults to 0 (C mimalloc: 2)

Adopted 2026-10-13 by explicit owner decision as an exception to the rule above
(`PERFORMANCE_WORKLOG.md` X16). With C's default, a page-queue search keeps up to
two small-block pages that have no free blocks in the queue instead of moving
them to the full queue, and later searches re-examine them. On `larsonN-sized`,
65% of the pages examined during searches yielded nothing. With 0, such pages
move to the full queue immediately (they return when a block is freed, as in C).

Evidence on the EPYC 4245P host: `larsonN` -5.4% and `larsonN-sized` -5.0%
(disjoint ranges), moving `larsonN` ahead of tcmalloc; 20-scenario native
held-out geomean -0.64% with no scenario worse than +1%; `rptestN` +1.2% median
over 15 interleaved runs, 95% CI [-3.7%, +3.9%]; Nginx (18 cases) +0.5%
throughput; architecture probes within 0.25 MiB RSS with identical occupied
pages. C and Rust differential allocation traces still match.

Compatibility: only the default changes. `MIMALLOC_PAGE_FULL_RETAIN=2` restores
C's behavior, and `scripts/check-behavioral-parity.py` checks Rust against C
under equal option values (`DOCUMENTED_OPTION_DEFAULTS` gives C Rust's default
for comparison and adds a `c_defaults` case running both with C's value).
