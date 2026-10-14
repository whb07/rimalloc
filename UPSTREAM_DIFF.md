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

### Owner frees are reused first (C: after the page's free list runs dry)

Adopted 2026-10-14 by owner decision (`PERFORMANCE_WORKLOG.md` X26). C pushes
a block freed by its page's owner onto `local_free`, which is reused only once
the page's `free` list is empty; Rust pushes it onto `free`, so the next
allocation of that size reuses the still-cached block, as jemalloc and
tcmalloc do. `free_is_zero` is cleared on that push so `calloc` keeps zeroing.

C relies on `free` running dry to send allocation through the generic path,
which runs the periodic maintenance. With LIFO reuse, a thread whose size
classes churn without a page filling or emptying never reaches that path. For
such a thread: deferred-free callbacks (`mi_register_deferred_free`) are not
called (C: 1,047 calls over 20M malloc/free pairs of one size with 64 live
blocks; Rust: none), at most one retired page per size class waits to be freed,
pending arena purges wait for the next page free, abandon or collection by any
thread, and remote frees into its pages are collected when a page runs dry.
Callbacks still run whenever allocation reaches the generic path. Keeping C's
cadence while a callback is registered was measured and declined: the check on
every `free` cost about 2.5% on `sh8benchN`.

Evidence on the EPYC 4245P host: 21-scenario native held-out geomean -0.79%
(`glibc-thread` -8.9%, `larsonN-sized` -3.1%, `sh6benchN` -1.9%, `larsonN`
-1.5%, `rptestN` -1.3%), measured with the callback check, which only adds
cost. TechEmpower plaintext +1.1% over 6 interleaved rounds; Nginx (18 cases)
+0.6% throughput geomean.

Compatibility: allocation order for a size class differs from C after owner
frees. `scripts/check-behavioral-parity.py` allows one case,
`theap_wrong_thread_malloc_aligned_status`: a debug-build wrong-thread
allocation is caught only on the generic path, which C reaches and Rust, with a
free block available, does not.
