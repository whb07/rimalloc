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

### Transparent huge pages are requested only for large heaps (C: for every arena)

Adopted 2026-10-15 by owner decision (`PERFORMANCE_WORKLOG.md` X29). C
asks for transparent huge pages (`MADV_HUGEPAGE`) on every arena it reserves
when `allow_thp` is enabled (the default). On a host in `madvise` THP mode every
2 MiB region the process touches then becomes resident, and because arenas
start each thread's search in a different chunk, a small multi-threaded
process holds several mostly empty huge pages. Rust reserves arenas without the
request and makes it, once, the first time the arenas have handed out 32 MiB
of pages: then every arena and every later reservation asks for huge pages, and
what the arenas already hold is collapsed into them (`MADV_COLLAPSE`, Linux 6.1
and later; best effort, `errno` unchanged). `allow_thp=0` still disables huge
pages entirely.

Evidence on the EPYC 4245P host (THP `madvise`): the TechEmpower axum server
drops from 17.9 to 5.9 MiB PSS (json), 17.8 to 5.8 (plaintext) and 20.7 to 7.7
(fortunes), below jemalloc (10.9 to 11.1 MiB), with throughput +0.1%, +3.5% and
+0.1%; Nginx worker PSS drops 20.2 to 14.9 MiB (`download-4096`), 24.4 to 16.8
(`proxy-16384-on`) and 32.4 to 25.1 (`headers`) at equal throughput. Large
heaps keep C's huge-page coverage (`larsonN` 76 of 82 MiB) and speed; a process
whose heap stays below 32 MiB uses base pages, as glibc, jemalloc and tcmalloc
do by default, which costs `malloc-large-old` (a single 2 MiB buffer written in
full 5000 times) 11 to 13% in short runs while its steady state is unchanged.

Compatibility: an arena reserved by another thread at the moment the request is
armed may miss it and run without huge pages.

### The main thread's heap is initialized (C 3.3.2: never)

Fixed 2026-10-15 by owner decision (`PERFORMANCE_WORKLOG.md` X30). C declares
the main thread's statically allocated theap with `MI_MEMID_STATIC`, and
`mi_theap_main_init` runs its one-time initialization only while the memid is
not static, so it never runs. The main thread then ignores the options read by
`_mi_theap_options_init` (it keeps `page_full_retain` 2 and abandonment
enabled whatever `MIMALLOC_PAGE_FULL_RETAIN` says, and therefore never received
this crate's default of 0) and its theap cookie stays 0; its random state is
still reseeded by process initialization. Rust starts the static with an empty
memid, so the initialization runs once and marks it static. This changes the
main thread's behavior only where options or the cookie matter: Nginx workers,
Redis and other single-threaded programs now use the configured
`page_full_retain`. Evidence: native screen +0.27% geomean (all within
+/-1.4%), Nginx throughput +0.5% to +1.0% at equal PSS.

### Full pages stay with their owner while only one thread uses the allocator (C: always abandoned)

Adopted 2026-10-15 by owner decision (`PERFORMANCE_WORKLOG.md` X31). C
abandons a page as soon as it moves to the full queue, so that other threads
freeing into it can reclaim it; when the owner frees into it instead, the owner
reclaims it, which costs a thread-free CAS, an owner CAS, list collection and a
queue push for every page that fills and empties again. Until a thread other
than the main thread initializes its allocator state, no other thread exists to
reclaim anything, so Rust moves such pages to the owner's full queue without
abandoning them (as C does for heaps that cannot abandon), and the owner's next
free returns the page with a queue move. The first non-main thread sets a
process-wide flag for good; afterwards pages are abandoned as in C, and the
generic path's periodic maintenance abandons the pages kept before (C asserts
that only non-abandoning heaps have full-queue pages; that assertion is dropped).

Evidence: Redis SET (single-threaded server) +4.6% interleaved against master,
1.747M against tcmalloc's 1.754M requests/s in the application suite (Redis
-2.6% and RocksDB -4.7% time against tcmalloc, -4.9% and -0.4% against C);
native screen +0.15% (no scenario outside noise), Nginx +0.2% to +1.4% at equal
memory. Multi-threaded processes behave as before (X21's `xmalloc-testN`
regression came from never abandoning in a multi-threaded process).

