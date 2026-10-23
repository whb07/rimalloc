# Mechanical mimalloc performance worklog

This is the durable decision log for the experiments in `TODO.md`. Raw build,
benchmark, counter, and assembly artifacts live under
`target/perf-experiments/<experiment-id>/`; this file records their provenance
and the conclusion drawn from them.

## Rules used for every experiment

- Start from the latest accepted implementation and change one mechanism only.
- Preserve raw results, including failed and noisy runs.
- Use balanced order (`baseline -> candidate`, then `candidate -> baseline`).
- Treat focused runs as rejection screens, never as sufficient evidence to keep
  a change.
- Keep a production change only after mechanism evidence, two balanced timing
  comparisons, held-out workloads, resource checks, and the complete correctness
  gate agree.
- If a production change is kept, establish and record a fresh baseline before
  starting the next experiment.
- `NO CHANGE`, `REJECT`, and `INCONCLUSIVE` are valid outcomes and are not
  rewritten as wins.

## Accepted-baseline ledger

| Baseline | Source commit | Reason established | Artifact directory | Status |
|---|---|---|---|---|
| B00 | `72fe0f4db7e2a3a6c8caafdb56944102554c7144` | Fresh current-HEAD C 3.3.2 parity baseline before E01 | `target/perf-experiments/E00-current-head-baseline/` | Accepted baseline |
| B01 | `f6989ea2740e2aacb4026f08f5a290ac67eae010` plus the E02R working-tree diff | Behavioral-parity repairs plus accepted C-equivalent O(1) local-list transfer | `target/perf-experiments/E02R-corrected-local-o1-transfer/` | Accepted locally; commit pending |
| B02 | `f018285` | Accepted C-equivalent trusted detached-remote scan | `target/perf-experiments/X02-trusted-remote-batch-scan/` | Accepted and committed |
| B03 | `4b2531a` (allocator source unchanged since `59c9b02`) | Post-X11 baseline against C mimalloc, glibc, jemalloc, tcmalloc | `benchmarks/results/2026-10-13-epyc-4245p-rivals/` | Accepted; scoreboard in BENCHMARK_RESULTS.md |

## Experiment ledger

| ID | Hypothesis | Candidate | Decision | Evidence |
|---|---|---|---|---|
| E00 | Establish authoritative current-HEAD C/Rust parity | Measurement only | Complete | `target/perf-experiments/E00-current-head-baseline/` |
| E01 | Diagnostics can identify collection and abandonment costs without changing production artifacts | 66 feature-gated counters plus reproducible JSONL runners/summarizer | Keep diagnostics; no production change | `target/perf-experiments/E01-observability/` |
| E02 | Avoid a redundant local-list scan when the available list is empty | Direct typed head transfer; production diff reverted | **REJECT**: strong speedup, but candidate hit the forced-collection invariant after cross-thread stress | `target/perf-experiments/E02-local-o1-transfer/` |
| E02R | Reopen E02 after reproducing and fixing its configuration-dependent forced-splice defect | C-equivalent direct head transfer with an unconditional forced nonempty splice | **KEEP**: correctness gates pass; mechanism is 7.6x–9.0x faster; Larson improves 2.5%–4.3%; no stable candidate regression | `target/perf-experiments/E02R-corrected-local-o1-transfer/` |
| E03 | Remove demonstrated surplus work from the abandoned-page decision spine | Assembly/CFG and outcome-frequency investigation only | **NO CHANGE**: no single safe redundant source operation demonstrated | `target/perf-experiments/E03-abandoned-decision-spine/` |
| E04 | Reject impossible reclaim work earlier for uninitialized freeing threads | Initialization gate before reclaim policy read; reverted | **REJECT**: mechanism worked but all targeted timings regressed | `target/perf-experiments/E04-uninitialized-reclaim-gate/` |
| E05 | Reuse validated detached-batch metadata instead of traversing twice | Full/partial local and abandoned path trace | **NO CHANGE**: every detached remote batch is already scanned once | `target/perf-experiments/E01-observability/diagnostic-runs/` |
| E06 | Remove demonstrated redundant work from ownership-release CAS retries | Instruction/counter/ordering comparison | **NO CHANGE**: expected word stays in-register; no repeated construction survives | `target/perf-experiments/E06-ownership-release-cas/` |
| E07 | Reuse one frequently reached reclaim/reabandon derivation | Frequency-gated source/assembly trace | **NO CHANGE**: late paths are frequent, but inputs are already derived once | `target/perf-experiments/E03-abandoned-decision-spine/investigation/` |
| E08 | Remove one measured residual page-queue search difference | Outline fresh-page calculation behind existing boundary; reverted | **REJECT**: mixed wins, stable Larson regressions | `target/perf-experiments/E08-page-queue-search/` |
| E09 | Match C++ fast-boundary code generation for every supported overload | 20-symbol ABI/assembly/semantic inventory | **NO CHANGE**: no Rust overload has a demonstrated extra successful-path detour | `target/perf-experiments/E09-cxx-boundaries/` |
| E10 | Remove refill/extension surplus work only if normalized evidence exists | Added missing ordinary diagnostic count; normalized assembly gate | **NO CHANGE**: Rust and C both emit five loop instructions per block | `target/perf-experiments/E10-page-refill-evidence/` |
| E11 | Attribute remaining gaps to source shape versus compiler behavior | GCC/Clang/Rust Latin-order comparison | Complete: remaining Larson gap is translation/source-shape, not GCC-vs-LLVM | `target/perf-experiments/E11-compiler-effects/` |
| E12 | Complete final parity and release qualification | Final B00-equivalent production artifact plus retained diagnostics | Complete investigation; baseline parity blocker repaired afterward and awaiting a fresh accepted performance baseline | `target/perf-experiments/E12-final-qualification/` |
| X01 | Root-cause the remaining xmalloc gap without changing production source | Thread/size sweeps, normalized counters, profiles, diagnostics, and C source/assembly comparison | Complete: per-block detached-chain validation dominates the low-contention excess | `target/perf-experiments/X01-xmalloc-investigation/` |
| X02 | Make the ordinary detached-remote scan match C while retaining hardened validation | Trusted bounded release scan; validating debug/secure/diagnostic scan | **KEEP**: +7.15% focused throughput, -6.16% instructions/free, full safety and held-out gates pass | `target/perf-experiments/X02-trusted-remote-batch-scan/` |
| X03 | Establish the authoritative post-X02 full Rust/C scoreboard | Fresh committed B02 artifact; complete `allt` suite in both orders plus 20-sample outlier reruns | Complete: broad parity, clear xmalloc/glibc-simple wins, stable ~1% glibc-thread gap, rptest/sh8 remain noisy | `target/perf-experiments/X03-b02-full-c-comparison/` |
| X04 | Locate the post-B02 xmalloc regression on the 12-CPU host | Bisect `f018285..5772656` with an interleaved harness control; release reabandon MemId re-check made debug-only | **KEEP**: first bad commit `1294a69`; removing the redundant check recovers 6.4% on `xmalloc-testN` | `benchmarks/results/2026-10-12-epyc-4245p/` |
| X05 | Remove call overhead from the option read on the remote-free collection path | Allow `_mi_option_get_fast` (a four-instruction table load) to inline | **KEEP**: a further -2.9% on `xmalloc-testN`; held-out geomean -0.84%, no scenario regresses beyond +1% | `benchmarks/results/2026-10-12-epyc-4245p/heldout-x04-x05.csv` |
| X06 | Inline reabandon so its frequent early rejection avoids a call | `#[inline]` on `_mi_arenas_page_try_reabandon_to_mapped_arena`; reverted | **REJECT**: +5.1% slower on `xmalloc-testN` with or without X05 | Measured in the X04/X05 session; not retained |
| X07 | Drop release page-map lookups that only feed debug assertions in abandon/unabandon | `_mi_ptr_page` (an Acquire page-map load) moved inside `mi_assert_internal!` in `src/allocator/arena/abandoned.rs` | **KEEP**: `xmalloc-testN` -2.1% (1.822 vs 1.861 ms, ranges disjoint), now ~1% faster than C | `benchmarks/results/2026-10-12-epyc-4245p/heldout-x07-x09.csv` |
| X08 | Inline `_mi_arenas_page_unabandon_internal` to drop the witness stack spill | `#[inline]` hint; reverted | **NO EFFECT**: hint not taken, binary identical to X07 | Measured in the X07 session; not retained |
| X09 | Make the hand-written fast-path layout independent of unrelated code size | `.p2align 6` anchor plus `.fill 0x20` before the `free`/`malloc` asm block in `src/raw/macros.rs` | **KEEP**: X07 alone shifted `malloc`/`free` cache-line offsets and cost `sh6benchN` +2.6%; anchored layout matches the validated one and restores parity | `benchmarks/results/2026-10-12-epyc-4245p/heldout-x07-x09.csv` |
| X11 | Match C's full-page test after a generic large-block allocation | `_mi_malloc_generic` uses `mi_page_is_full` (`used == reserved`, C `page.c:1035`) instead of "no free list and fully extended" | **KEEP**: Nginx `proxy-262144-on` +2.2%, `proxy-65536-on` +1.6% throughput (disjoint ranges); canary screen neutral | `target/bench-runs/x11-nginx-*` (local) |
| X13 | Reach the aligned allocator from the libc aligned entry points without GOT/PLT hops | Non-exported `posix_memalign`/`memalign`/`valloc`/`pvalloc` bodies called by the libc wrappers; reverted | **REJECT**: hops removed (0 GOT-indirect), but `memalign`+free 6.10 -> 6.95 ns (+13.9%, disjoint over 7 runs; +22 instructions/op); `posix_memalign` -1.0% | Recorded below |
| X14 | Turn internal GOT-indirect calls into direct calls with `-Wl,-Bsymbolic-functions` | Linker flag on the Linux cdylib; reverted | **NO CHANGE**: internal dynamic lookups 24 -> 0, but calls stay `jmp *GOT` because stable rustc does not emit relaxable GOT relocations | Recorded below |
| X12 | Collapse the second `mi_page_is_full` (`page/flags.rs`, "no free list and fully extended") onto the C-equivalent | Not started | **DEFERRED**: release callers already use the C-equivalent after X11; the other definition is re-exported through the public `page` module, so changing its meaning needs an API decision | — |
| X15 | Let hot `__mi_stat_*` counters inline (X05 precedent) | Not started | **NOT JUSTIFIED**: stat helpers are 0.05-0.13% of four profiles (`xmalloc-testN`, `sh6benchN`), below the 0.5% bar | Profiles from the X04-X11 sessions |
| X16 | Stop re-scanning exhausted small pages during page-queue search | `page_full_retain` default 2 -> 0 (documented divergence from C) | **KEEP**: `larsonN` -5.4%, `larsonN-sized` -5.0%; held-out -0.64%, `rptestN` CI [-3.7%, +3.9%], Nginx neutral, RSS unchanged | Recorded below and in UPSTREAM_DIFF.md |
| X17 | Compute a page's bin once when finding its queue | `mi_theap_page_queue_of` indexes with the bin from `mi_page_bin_typed` instead of recomputing `mi_bin` for ordinary pages | **KEEP**: 8 KiB malloc/free 246.6 -> 213.6 instructions/op (C 193.3); held-out neutral | Recorded below |
| X18 | Make the medium-size generic path one call, like C | Inline the generic body into `_mi_malloc_generic` (the C entry point) instead of wrapping `_mi_malloc_generic_typed` | **KEEP**: 213.6 -> 205.6 instructions/op; with X17, medium sizes 7.5-10.1% faster; `larsonN-sized` +0.24% to +1.02% recorded | Recorded in the X17/X18 section |
| X19 | C-equivalent fast path for the generic (medium-size) malloc hit case | `malloc_generic_fast`: count, quick-collect the size class's first page, pop; every other case continues with C's remaining steps out of line | **KEEP**: 8 KiB pair 205.6 -> 169.7 instructions (C 193.3); 8-16 KiB requests 3.5-7% faster, at or below C; held-out and Nginx neutral | Recorded below |
| X20 | Send C++ sized delete straight to the `free` fast path | `_ZdlPvm`/`_ZdaPvm` become aliases of `free` instead of a stub jumping to `mi_free` through the PLT | **KEEP**: sized new[]/delete[] pair 56.34 -> 53.34 instructions (unsized 52.34, C 63.38); `larsonN-sized` -0.82% (4 of 5 rounds) | Recorded below |
| X21 | Stop abandoning full pages while the owner is alive | `page_full_retain` default 0 -> -1 (`allow_page_abandon` false: full pages go to the owner's full queue); reverted | **REJECT**: `larsonN` -2.0%, `larsonN-sized` -2.4%, but `xmalloc-testN` +318% (4x slower) | Recorded below |
| X22 | Stop loading libstdc++ into every process | The C++ `operator new` shim looks up `std::get_new_handler` and `std::__throw_bad_alloc` with `dlsym` on allocation failure instead of linking `-lstdc++` | **KEEP**: axum json server PSS under load 19.06 -> 17.87 MiB (C mimalloc 17.82); C++ `new` behaviour unchanged | Recorded below |
| X23 | Reuse a block freed by its owner next (LIFO), as jemalloc does, keeping C's maintenance cadence with a per-free countdown | Owner frees push onto `page.free`; a 64-free theap countdown parks the page's free list on `local_free` when maintenance is due; reverted, never on master | **REJECT**: LIFO alone cut plaintext L1 misses 72 -> 47 per request and won the native screen (geomean -1.33%), but the countdown costs about 2 cycles per free: `glibc-thread` +8%, `glibc-simple` +6%, `larsonN` +1.6% versus HEAD | Recorded below |
| X24 | Make the page abandon/reclaim round trip cheaper without changing C behavior | Skip `mi_page_set_in_full`'s atomic RMW when the owner-only flag already matches; read the arena purge guard before its CAS; reverted, never on master | **REJECT**: no gain outside noise. No-op skip vs HEAD: `rptestN` -1.2%, `larsonN-sized` -0.6%, `larsonN` +0.1%; guard read on top: `rptestN` +0.5%, `larsonN` +0.9% | Recorded below |
| X25 | Batch remote frees per destination page (snmalloc-style) so one CAS publishes many blocks | Per-thread 16-slot table keyed by page; a batch publishes at 64 blocks or 32 KiB, on eviction, at the maintenance tick, in `mi_collect` and at thread exit; prototype only, never on master | **REJECT**: quick screen vs HEAD `larsonN` +4%, `larsonN-sized` +5%, `xmalloc-testN` +4%, `sh8benchN` about -1%, `mstressN` flat. The CAS that costs is the first one to a page, which batching keeps | Recorded below |
| X26 | Reuse a block freed by its owner next (LIFO), dropping C's maintenance cadence for threads that churn without a page filling or emptying | Owner frees push onto `page.free` (asm and Rust paths) and clear `free_is_zero`; no counter on the free path; owner-approved divergence in `UPSTREAM_DIFF.md` | **ADOPT**: final screen geomean -1.08% (`glibc-thread` -8.5%, `larsonN` -5.1%, `larsonN-sized` -4.1%, `rptestN` -3.0%; `sh8benchN` +1.4%); held-out -0.79%; plaintext +1.1%; Nginx +0.6% | Recorded below |
| X27 | Profile-guided optimization (PGO) of the Rust library, and BOLT post-link layout | PGO: instrumented cdylib trained on 11 of 21 native scenarios, rebuilt with `-Cprofile-use`. BOLT 18: opt-in `MIMALLOC_BENCH_BOLT=1` in `bench.py`, `--emit-relocs` link, checked-in LBR profile of the same 11 scenarios (`benchmarks/bolt.py`, `bolt-profile.fdata`) | PGO **REJECT**: screen geomean +0.15%. BOLT **ADOPT (opt-in)**: 21-scenario held-out geomean -0.87% (trained -0.94%, held out -0.79%); `malloc-large` -10.1%, `xmalloc-testN` -3.2%; worst `cfrac` +0.44% | Recorded below |
| X28 | Per-thread cache of freed singleton pages (blocks above 512 KiB) to skip the arena's slice bookkeeping | Freed main-heap singleton pages kept registered and committed in the freeing theap (8 slots, 256 MiB, purge-delay expiry); reuse within 25% slack; never on master | **REJECT**: steady-state `malloc-large` -29%, but the screen's short runs +145% (first-touch THP faults: cached pages push the arena onto untouched slices), `xmalloc-testN` +6.6%, `sh6benchN` +4.0%; releasing the cache on a miss removes the faults and the gain | Recorded below |
| X29 | Request transparent huge pages only once the heap is large | Arenas reserved without `MADV_HUGEPAGE`; armed once at 32 MiB of arena pages, then all arenas are advised and their resident memory collapsed (`allocator/arena/thp.rs`) | **ADOPT** (owner decision): axum PSS -67/-67/-63% (json/plaintext/fortunes), Nginx worker PSS -22 to -31%, throughput equal or better; native screen +0.71% geomean, all from `malloc-large-old` +12% in short runs (steady state unchanged); other 20 within +/-1.3% | Recorded below |
| X30 | Run the main thread heap's one-time initialization (upstream bug) | `theap_main` starts with an empty memid so `theap_main_init` runs (C and the translation started it static, skipping options and cookie) | **ADOPT** (owner decision): native screen +0.27% (noise), Nginx +0.5 to +1.0% at equal PSS; regression test added | Recorded below |
| X31 | Keep full pages with their owner while the process has one allocating thread | `MI_PROCESS_MULTI_THREADED` set by the first non-main TLD; until then `mi_page_to_full` uses the full queue instead of abandoning; maintenance abandons kept pages once set | **ADOPT** (owner decision): Redis SET +4.6% interleaved (1.747M vs tcmalloc 1.754M); native screen +0.15%, Nginx +0.2 to +1.4% | Recorded below |
| X32 | Track full pages without the full-queue links (prototype) | Measurement-only builds: full pages counted but not linked; unfull at front or end of the normal queue; separately, a plain store for the in-full flag while single-threaded | **REJECT**: Redis SET -8% (front), +0.8% (end), +0.8% (plain flag); the unfull cost is misses on cold page metadata, not queue links or the atomic | Recorded below |
| X33 | Identify the event releasing post-burst resident memory | Four live-owner/collection/exit arms with external OS sampling; measurement only | **NO POLICY CHANGE**: shared-arena purging releases about 64 MiB in both Rust and C; idle time alone does not. A smaller THP-related residual is separately observed | `target/perf-experiments/X33/`; recorded below |
| X34 | Measure the residency and resume cost of collecting at a burst boundary | Immediate owner collection versus retention, with 10 ms through 10 s pauses; frozen X33 libraries | **NO POLICY CHANGE**: collection saves about 64 MiB; short-pause Rust refill costs about 2 ms more. After the purge deadline, passive refill executes pending discard calls; early collection moves them before the pause | `target/perf-experiments/X34/`; recorded below |
| X35 | Test X31 with a main-thread producer and free-only consumer | Three initialization histories, external sampling, and four repeated handoffs | **RETENTION REGRESSION OBSERVED**: Rust free-only RSS grows about 64 MiB per handoff; owner collection recovers it. Consumer initialization before the burst avoids the growth | `target/perf-experiments/X35/`; recorded below |
| X36 | Test X26 maintenance progress during active small-object reuse | Same cold burst followed by idle, anchored hot-loop reuse, or generic-path activity | **RETENTION COST OBSERVED**: Rust hot-loop RSS remains 72 MiB with no heartbeat progress; C and Rust generic activity release the cold burst after about 1.1 s | `target/perf-experiments/X36/`; recorded below |
| X37 | Separate survivor placement from THP burst history | Page-aware packed/scattered survivors, empty and never-large controls, THP on/off | **OBSERVED**: scattered survivors add about 32/28 MiB for 64 B/8 KiB objects in both libraries; Rust's post-burst small-heap floor also depends strongly on THP | `target/perf-experiments/X37/`; recorded below |
| X38 | Compare grace periods under irregular repeated bursts | Passive, immediate, 100 ms and 1 s owner collection, including arrivals near timer deadlines | **NO POLICY CHANGE**: 100 ms grace reduces mean Rust RSS from 102 to 33 MiB at similar measured CPU cost in this mix; near-deadline arrivals expose collection stalls | `target/perf-experiments/X38/`; recorded below |
| X39 | Check secure-release decommit protection against C | Owned primitive mapping, isolated access probe, recommit and allocation reuse | **FIXED**: secure level 4 now protects decommitted memory and requests recommit, matching C | `target/perf-experiments/X39/`; recorded below |
| X40 | Resolve the historical thread-lifecycle TSan report | Reproducing release C-ABI stress, minimized lifecycle test, direct pthread-key regression | **FIXED inherited defect**: explicit teardown now clears the destructor association before metadata release; ten clean runs of each repaired Rust probe | `target/perf-experiments/X40/`; recorded below |
| X41 | Establish a fresh baseline for the authorized follow-ups | Six balanced full-suite Rust/C rounds before the Rust-only scope change | Complete: frozen inputs and remaining gaps recorded below | `target/perf-experiments/X41/` |
| X42 | Recognize consumers that free without allocating | Mark participation on owned remote publication and owner collection | **KEEP**: twelve-burst RSS 784→108 MiB; final combined gate in X48 | `target/perf-experiments/X42/` |
| X43 | Measure idle grace with real producer arrivals | Queue-driven owner, external sampling, overlap timestamps | **KEEP application recipe**: 100 ms grace reduces mean RSS 102→33 MiB in this workload; overlapping arrivals remain a latency cost | `target/perf-experiments/X43/` |
| X44 | Avoid sparse survivor pinning by separating lifetimes | Same live cache and temporary payload in one versus two private heaps | **KEEP application guidance**: RSS 50→18 MiB, occupied cache pages 512→1 | `target/perf-experiments/X44/` |
| X45 | Provide maintenance during uninterrupted hot reuse | Non-forced owner collection between batches | **KEEP application recipe**: 1 s checkpoint reduces final RSS 72→8 MiB at −0.18% measured loop rate | `target/perf-experiments/X45/` |
| X46 | Localize THP advice to dense backing | Every-allocation and first-use prototypes | **REJECT both**: repeated collapse costs up to 22×; first-use full-suite time +5.12%, sh6 +88.20%, rptest +52.21% | `target/perf-experiments/X46/` |
| X47 | Skip reclaim/reabandon without a main-heap association | Exact null-destination gate after collection and final-free handling | **KEEP**: futile calls removed; final xmalloc time −14.83% in X48 | `target/perf-experiments/X47/` |
| X48 | Qualify the final retained changes together | Six paired full-suite Rust old/new rounds, application controls, retention repeats, local verification | **KEEP retained scope**: full-suite time −1.45%, local checks pass; Redis inconclusive; cross-target limitations recorded below | `target/perf-experiments/X48/` |
| X49 | Repeat Redis five consecutive times on request | Five balanced old/new Rust pairs, unchanged frozen artifacts | SET +1.77% in this batch (5/5 positive), GET −0.56% (2/5 positive); no repeatable overall application claim | `target/perf-experiments/X49/` |
| X50 | Find Redis allocator hotspots shared with other workloads | Separate SET/GET instruction profiles, bounded cache simulation, native profiles, source/assembly checks | **INVESTIGATION ONLY**: prioritize queue-cache range calculation and redundant full-flag transitions; metadata locality is a broader research candidate | `target/perf-experiments/X50/` |
| X10 | Measure rptestN like upstream rptest (worker CPU time per op) instead of wall time that includes ~20 s of main-thread sleep polling | Harness: `rptest::run` returns summed worker ticks per 1M ops; metric renamed `per-1M-rptest-cpu-ops` | **KEEP (measurement change)**: missed the planned <2% CV bar (Rust 4.6%, C 3.8%; both bimodal from thread scheduling); adopted with a rule of at least 5 runs per allocator | Recorded below |
| H01 | Fix native harness publication and timing defects found in external review | mstressN AcqRel handoff; sh8benchN atomic foreign slots; rptestN metric renamed `per-1M-rptest-worker-ops` and final foreign drain timed; Larson start gate; xmalloc counts only dequeued batches | **KEEP (measurement change)**: A/A screen of the six affected scenarios runs clean; their old and new numbers are not comparable | Recorded below |

## E02 — O(1) empty-available local-list transfer

Started and decided: 2026-08-09

### Candidate and invariant

The candidate matched C `_mi_page_free_collect`: when `free` was empty it
stored the existing `LocalFreeList` head as `AvailableFreeList`, cleared
`local_free`, and cleared `free_is_zero`, without reading a block link. The
typed owner already exclusively controlled both page fields; both list domains
use the page's existing encoded links, and neither `used` nor `capacity` changes
during this ownership transfer. Remote collection and the forced nonempty-list
splice were unchanged.

Focused tests covered empty, transfer, non-forced, forced splice, local and
claimed-abandoned ownership, and secure encoded links. The existing bounded
scanner corruption test was retained after an attempted end-to-end corrupted
collector test correctly aborted under the default corruption policy instead
of returning to the test. The complete Rust gate (`fmt`, workspace/all-target/
all-feature `clippy`, and workspace/all-target/all-feature `test`) passed.

### Mechanism and measurements

- Plain page-batch forced collection of 128 blocks fell from about 177 ns to
  23 ns (about 7.7x); secure mode was neutral at about 23.4 ns.
- Empty-destination diagnostics fell from 128,000 scanned nodes in 1,000
  collections to zero. The nonempty forced case remained 128,000/1,000.
- Native local nodes per collection fell from 1.83 to approximately zero in
  both Larson variants and from 3.00 to zero in glibc-thread. Remote detach and
  collection counts did not materially change.
- Assembly showed direct metadata stores in the empty-available branch, with no
  new call, owner dispatch, or locked instruction.
- The balanced focused screen improved Larson unsized 2.55% and sized 2.42%.
  Balanced `perf stat` reduced instructions 1.02%/2.95%, branches 5.75%/7.30%,
  and branch misses 12.32%/14.13% for unsized/sized respectively.
- The balanced full screen (five repetitions in both orders) improved Larson
  unsized 2.92%. Other stable workloads were essentially neutral; apparent
  Redis, rptest, and sh6 movements had high variance or conflicting order
  splits. Malloc-large time and commit/RSS accounting were unchanged. The
  fragmentation order split showed run drift and no stable regression.

### Correctness failure and decision

`scripts/check-behavioral-parity.py` exposed a candidate debug abort after its
cross-thread stress wave at the final forced-collection invariant
`!force || page.local_free.is_empty()`. A debugger captured a zero-used page
with both `free` and `local_free` nonempty. The unchanged B00 harness also has
pre-existing nonzero parity cases (and the wrapper therefore exits 1), but two
unchanged direct runs did not reproduce this assertion abort, whereas the
candidate did. Changing remote collection, validation, or corruption behavior
to pursue the failure would violate E02's one-mechanism scope.

Decision: **REJECT**. The production transfer was reverted; E01 diagnostics and
the generally useful invariant/page-batch tests remain. No accepted allocator
change exists, so B00 remains the baseline and no baseline rerun is required.
Raw timing, counters, resources, assembly, debugger captures, failed attempts,
and gate logs are preserved in the E02 artifact directory.

## E02R — Corrected O(1) empty-available local-list transfer

Reopened: 2026-08-09. Decided: 2026-08-10.

### Why the original rejection was not evidence against the algorithm

The original E02 failure was fully reproduced and traced to the experiment's
source shape, not to the O(1) transfer. Its diagnostics attribute placement
made the forced nonempty-destination splice configuration-dependent: the splice
was present with `bench-diagnostics`, but absent from the default build. The
cross-thread parity probe eventually reached forced collection with both
`free` and `local_free` nonempty, so the final invariant correctly aborted.
That explains all of the previous observations: diagnostics and mechanism
measurements looked correct, while the production/default artifact could leave
the local list behind.

E02R keeps the two semantic decisions in ordinary, unconditional control flow:
`free.is_empty()` performs the direct transfer; otherwise `force` performs the
existing validated detach-and-splice. Diagnostics annotate the result inside
those branches and cannot decide whether either branch exists. A default-build
regression test now executes all four C cases, including the formerly missing
`free` nonempty plus `force == true` case. The old E02 rejection remains above
as historical evidence; it is superseded by this corrected experiment.

### Candidate and provenance

The production change is confined to `collect_page_free_lists`. When the
available list is empty, the owner writes the existing typed local head to
`free`, clears `local_free`, and clears `free_is_zero`; it does not read a block
link. The nonempty forced case retains the existing validated tail scan and
splice. Remote collection, accounting, encoding, corruption policy, atomic
ordering, and ownership dispatch are unchanged.

- Frozen pre-change release SHA-256:
  `7de82434047e409872347e828e15b61cfd7a9589b0a5fee8a408e98ddd5f2778`.
- Candidate release SHA-256:
  `3ac9ab5a5b916fe627c56faf3bfa0d809a8599a338a620dffc5887b16e51bb7b`.
- Both artifacts were built from the same `f6989ea` source base in separate
  target directories. Allocator-order files reference those immutable paths.
- The retained disassemblies show a direct-store empty branch. Function size
  changed from 925 bytes to 958 bytes because the direct and validated splice
  paths are now distinct; no locked instruction was added.

### Correctness, safety, and concurrency

- The configuration-independent local-list test covers empty local, exact-head
  transfer with unchanged links and counts, non-forced nonempty destination,
  forced nonempty splice, local and claimed-abandoned owners, and secure encoded
  links. Focused default and all-feature runs passed.
- The complete behavioral-parity driver passed, followed by five direct parity
  executable runs. The release C-ABI thread stress, six Loom concurrency models,
  Valgrind (zero errors and leaks), and manually linked ASan probe passed.
- Current-nightly TSan reports a race between `_mi_theap_create` and
  `_mi_thread_done`. The exact frozen pre-change artifact reproduces the same
  report at the same boundary under the same sustained probe. Neither stack
  contains the changed collector, so this is a pre-existing thread-lifecycle
  issue and is tracked separately rather than attributed to E02R.
- `cargo fmt --all`, workspace/all-target/all-feature Clippy, and workspace/
  all-target/all-feature tests passed from isolated target directories. The
  first focused test invocation accidentally reused stale shared-target test
  artifacts and hit duplicate `panic_impl`; the isolated rerun passed and the
  complete clean gate independently confirmed the result.

### Mechanism and candidate-versus-baseline performance

The direct page-batch mechanism benchmark used the same rebuilt source and the
same 128-node empty-destination fixture in balanced alternating order:

- plain: about 176 ns before versus 23.2 ns after, approximately 7.6x faster;
- secure: about 207 ns before versus 23.1 ns after, approximately 9.0x faster.

The earlier statement that secure mode was neutral came from a contaminated
measurement and is superseded by these freshly rebuilt artifacts. Preserving
encoded links requires no decoding because only the already encoded head is
moved.

In the balanced five-repetition focused screen, candidate/baseline median
ratios were 0.9752 for Larson, 0.9688 for Larson-sized, 1.0127 for xmalloc, and
0.9986 for glibc-thread. Both order halves agreed on the Larson improvements.
The short xmalloc warning did not reproduce in the broader screen.

The balanced three-repetition complete `allt` screen confirmed Larson-sized at
0.9570, alloc-test1 at 0.9773, xmalloc at 0.9869, and glibc-thread at 0.9857.
Stable controls were neutral or better. mstress and rptest had high variation
or opposite order splits; sh6 was also noisy. No held-out workload showed a
repeatable candidate regression. In balanced `perf stat`, Larson branches fell
4.09%/4.41% and branch misses fell 12.26%/12.58% for unsized/sized; task clock
was flat and cycles fell 0.17%/0.54%. That agrees with the removed traversal.

### Fresh accepted Rust-versus-C baseline

Because E02R is kept, B01 was measured against the retained GCC release build
of C mimalloc 3.3.2 in both allocator orders. The focused ten-sample-per-
allocator results are:

| Workload | Rust/C median | C CV | Rust CV | Interpretation |
|---|---:|---:|---:|---|
| Larson | 0.9959 | 0.79% | 1.27% | parity |
| Larson-sized | 0.9875 | 0.92% | 0.95% | Rust 1.25% faster |
| xmalloc | 1.0112 | 1.87% | 1.49% | small remaining warning |
| glibc-thread | 1.0012 | 0.65% | 0.54% | parity |

The broader balanced `allt` reset used three repetitions in each order. Stable
longer workloads were at parity or better, including Larson-sized 0.9861,
alloc-test1 0.9952, alloc-testN 0.9859, glibc-thread 1.0012, and RocksDB 0.9978.
The sh8 row was 1.0629, but E02R candidate/baseline was exactly 1.0000 there and
the prior 20-sample C comparison had 4%–7% CV and conflicting order halves;
therefore it is an existing/noisy C comparison, not a candidate regression.
Redis, rptest, sh6, and the 60 ms cache-scratch row also had material variation
or order effects and are not used as acceptance claims.

### Decision

`KEEP` — E02R exactly restores C's O(1) ownership transfer, removes a measured
general-purpose traversal, passes the complete correctness gates, improves the
workloads that exercise it, and has no repeatable held-out regression. B01 is
the new local accepted baseline. The TSan thread-lifecycle report remains a
separate pre-existing correctness investigation; it is not hidden or counted
as an E02R pass.

## E03 — Abandoned-page decision spine

Started and decided: 2026-08-09

The B00 Rust `collect_after_remote_free` symbol is 892 bytes/245 instructions,
with 11 calls, 40 conditional branches, 12 unconditional jumps, and 17 stack
references. C `mi_free_try_collect_mt` is 390 bytes/104 instructions, with 8
calls, 18 conditional branches, 6 jumps, and no `%rsp` references. Rust's
separate reclaim helper is 500 bytes/136 instructions versus C's 382 bytes/101
instructions. Annotated disassembly and static counts are archived under the
E03 investigation directory.

The difference is not a single repeated page/block-size extraction: LLVM keeps
the page in `%rbx` and loads block size once into `%r14`. Ownership release also
already receives the constructed expected thread-free word and compiles to an
owner-bit clear feeding the same acquire/release CAS loop as C. The main extra
text is attributable to:

- bounded page-membership, capacity, encoded-link, and corruption validation in
  partial remote collection (required Rust safety invariant);
- typed `Result` ownership returns and cold error formatting (invariant/ABI);
- reabandon eligibility checks duplicated in the initial and post-concurrent-
  collection paths (required because collection changes page state);
- a few constant discriminant tests that survive optimization (compiler/layout
  effect, not a demonstrated removable source operation).

E01 outcome frequencies show why a broad rewrite would be risky. Larson reclaim
succeeds on about 97.56% of attempts, while xmalloc reclaim succeeds effectively
never: roughly 99.35% fail because the freeing thread is uninitialized. Larson
reabandon is attempted only after reclaim failure and succeeds about 0.33%; in
xmalloc it is almost always attempted but does not succeed. Ownership-release
CAS retries are only about 0.1% of attempts in Larson and 0.01% in xmalloc.
Existing loom coverage already races ownership release with remote publication
and verifies that heads are preserved; atomic orderings match C (`AcqRel` CAS,
`Acquire` failure, relaxed reload).

Decision: **NO CHANGE**. Removing the validation work would weaken corruption
handling, while changing the typed return ABI or all reabandon helpers would
violate the one-mechanism constraint. No candidate was built or timed and B00
remains the accepted baseline.

## E04 — Early uninitialized-thread reclaim rejection

Started and decided: 2026-08-09

The candidate preserved collect and all-free destruction ordering, then tested
`_mi_thread_is_initialized()` before reading `page_reclaim_on_free`. An
uninitialized thread skipped reclaim only and continued through the unchanged
reabandon and ownership-release paths. The reclaim helper retained its own
defensive initialization check, so no atomic, ownership, TLS side effect, or
initialized-path policy was changed.

Mechanism evidence was strong: normalized xmalloc reclaim-policy reads/attempts
fell by about 99.35% (roughly 916 million to 5.9 million per million frees).
Reabandon and ownership-release frequencies remained consistent, all diagnostic
relationships passed, and Larson continued to exercise successful initialized
reclaim. Focused concurrency tests and production/diagnostic builds passed.

The production codegen moved the option call behind the initialization branch
but grew `collect_after_remote_free` from 892 to 923 bytes (+31); the separately
retained reclaim helper was unchanged. A balanced three-repeat, both-order
screen produced candidate/baseline mean ratios of 1.0129 for Larson, 1.0190 for
Larson-sized, 1.0298 for xmalloc, and 1.0019 for glibc-thread. Both order splits
were slower for both Larson variants and xmalloc; thus the primary workloads
all moved in the wrong direction despite the counter reduction.

Decision: **REJECT** at the focused screen. The source diff was reverted and no
full-suite time was spent on a broadly slower candidate. B00 remains accepted.
Two command/setup failures are retained in the artifacts: the first benchmark
path was formed relative to `mimalloc-bench`, the second invocation used the
repository rather than `out/bench` working directory, and the first summary
attempt used obsolete CLI arguments. All were corrected without discarding raw
measurements.

## E05 — Detached remote-batch traversal count

Decided: 2026-08-09

Source and counter tracing found no second traversal. Full local and full owned-
abandoned collection detach the atomic head once, call
`RemoteFreeBatch::from_detached` once, and retain its head, tail, page, and length
for splice and used-count subtraction. Partial abandoned collection keeps the
published head, uniquely detaches only its old tail, scans that detached tail
once, and reuses the batch metadata. Its possible final collection processes
the separately retained published head after the used count reaches one; it is
not a rescan of the detached tail.

E01 fixture relationships show exactly one `remote_lists_detached` and one
length-accounted scan for batches of 1, 8, 32, and 128 blocks, with zero
validation failures. Native Larson and xmalloc records likewise report remote
blocks per detach near one because nearly all real batches have length one;
there is no second counter increment or second link loop in assembly. Existing
property/fuzz coverage exercises capacity bounds, cycles, foreign/invalid
links, secure encoding, and `len > used` at the one scanner boundary.

Decision: **NO CHANGE**. The prerequisite evidence for a candidate is absent,
so no source, assembly, or timing candidate was produced. B00 remains accepted.

## E06 — Ownership-release CAS loop

Decided: 2026-08-09

The instruction-by-instruction Rust/C comparison found the same essential
protocol: expected owned word in a register, one owner-bit transformation,
AcqRel weak CAS with Acquire failure observation, nested collection only after
a newly nonempty observed head, and relaxed reload after collection. Rust does
not reconstruct `(head, owned)` or reload the atomic before its first CAS.

Rust retains one defensive `expected.is_owned()` test inside the shared typed
atomic method. Removing it would weaken the method contract (tests deliberately
exercise an unowned expected word), while adding a second unchecked API/proof
type for this rare cold path is not a demonstrated net simplification. E01
shows first-attempt success above 99.9%: retry/attempt is about 0.0010 Larson,
0.0009 Larson-sized, and 0.0001 xmalloc. Existing loom release-vs-push coverage
preserves remote heads and the required orderings.

Decision: **NO CHANGE**. No candidate was produced; B00 remains accepted.

## E07 — Reclaim/reabandon input derivation

Decided: 2026-08-09

The frequency gate is met for Larson: about 97.56% of reclaim attempts succeed.
The source and B00 assembly nevertheless show no repeated derivation suitable
for a one-mechanism candidate. `try_reclaim_abandoned` extracts page, block size,
associated thread heap, thread-pool state, and arena suitability once on each
mutually exclusive path; LLVM retains the hot values in registers. The
arena-specific reabandon helper already receives the validated `ArenaMemId`.
Its ownership/mapping checks protect a distinct arena mutation boundary and
cannot be deleted merely because the caller made an earlier policy check.

Decision: **NO CHANGE**. Carrying a context object or weakening the arena entry
would widen common failure returns or remove boundary validation without
eliminating a measured repeated computation. B00 remains accepted.

## E08 — Residual page-queue search difference

Started and decided: 2026-08-09

B00 Rust `mi_page_queue_find_free_ex_typed` is 1,568 bytes/396 instructions
versus C's 1,464 bytes/360 instructions. Annotated source/assembly showed one
specific mismatch: LLVM hoisted `mi_slice_count_of_size(block_size)` from the
fresh-page path to the Rust function entry, so existing-page searches paid the
calculation. C keeps it behind `mi_page_fresh`.

The candidate made the existing Rust `mi_page_fresh` boundary non-inlined and
called it only after search/extension failed. Assembly removed the hoisted
slice calculation from the entry, added the expected cold-path call, preserved
the queue loop/order, and reduced the search symbol by 16 bytes (1,568 to 1,552).
Focused queue tests and the production build passed.

The first balanced 3+3 screen was neutral: Larson 1.0004, sized 1.0006,
xmalloc 0.9977, glibc-thread 0.9977 candidate/baseline. The independent 5+5
screen added alloc-test and resolved the tradeoff: Larson 1.0183 and sized
1.0109 (both slower in both order splits), xmalloc 0.9935, glibc-thread 1.0019,
alloc-test1 0.9968, and alloc-testN 0.9919. Thus outlining helped some
fresh/refill-heavy cases but stably harmed the important Larson family.

Decision: **REJECT**. This is a real codegen improvement in isolation but not a
general allocator improvement. The source diff was reverted; B00 remains the
baseline. Raw logs, both summaries, assembly, builds, and the initial obsolete-
path invocation failures are retained.

## E09 — C++ allocation/deallocation boundaries

Decided: 2026-08-09

Both exact B00 Rust and C release artifacts export the same 20 supported
Itanium ABI symbols: scalar/array new; throwing/nothrow and aligned variants;
and unsized, sized, aligned, nothrow, and array delete variants. `nm -D` and
`readelf -Ws` confirm ownership by each allocator artifact. The full per-symbol
assembly table is archived.

No Rust overload has an extra successful-path semantic detour. Ordinary scalar
and array new share the repaired allocator fast boundary; nothrow variants are
tail aliases; aligned throwing paths retain only required alignment/new-handler
work. Delete aliases tail-route to the allocator free implementation, and sized
forms introduce no usable-size recomputation. Where direct instruction counts
are comparable, Rust is equal or smaller (for example array throwing new 19
instructions versus C 38, and the primary array delete entry 44 versus 48).
Zero-length alias symbols in `objdump --disassemble` share another exported
entry address and are not missing code.

`check-cxx-operator-new-parity.py` passed C++11/14/17/20 throwing, nothrow,
zero-size, OOM, alignment, sized-delete, and array semantics.
`check-release-cxx-artifacts.py` passed the dynamic/static/standalone artifact
ownership checks, and `check-release-ld-preload.py` passed focused standalone
interposition. E00/E08 already cover alloc-test and Larson probes plus full
guards for this unchanged B00 code.

Decision: **NO CHANGE**. No overload warrants modification; B00 remains
accepted.

## E10 — Page refill/extension evidence gate

Decided: 2026-08-09

The E01 extension counters had only been called by secure refill, causing
ordinary diagnostic workloads to report zero. The same feature-gated call was
added to ordinary `mi_page_free_list_extend_typed`; this is retained as an E01
visibility correction and compiles out of production builds.

Fresh diagnostics show average blocks/call of 9.60 Larson, 9.61 Larson-sized,
63.92 xmalloc, 45.84 glibc-thread, and 64.09 glibc-simple. B00 ordinary refill
assembly is 90 bytes versus C's 74, but the actual link loop is five
instructions per block in both. Rust stores a separately incremented next
pointer and advances two independent registers; C copies current, advances one
dependent register, then stores it. Rust's extra text is one-time address/range
setup and does not establish a normalized per-block instruction gap.

Decision: **NO CHANGE**. The evidence gate forbids another loop-form candidate;
prior unrolling/non-null/alternate forms were already neutral or slower. B00
remains accepted. Diagnostic JSONL, counts, and both assembly listings are
archived.

## E11 — Compiler effects versus translation effects

Completed: 2026-08-09

Vendored C 3.3.2 was rebuilt with Clang 20.1.8 using the official GCC 13.3
Release configuration (`-O3 -DNDEBUG`). A three-order Latin rotation gave nine
samples per allocator/workload, with every allocator in every position.

Mean time ratios:

| Workload | C-Clang/C-GCC | Rust/C-GCC | Rust/C-Clang | Classification |
|---|---:|---:|---:|---|
| Larson | 1.0018 | 1.0320 | 1.0302 | Translation/source-shape gap |
| Larson sized | 1.0041 | 1.0291 | 1.0249 | Translation/source-shape gap |
| xmalloc | 0.9853 | 0.9644 | 0.9789 | Rust broadly faster |
| glibc-thread | 1.0244 | 1.0036 | 0.9798 | Rust at official GCC parity |
| alloc-test1 | 1.0079 | 1.0179 | 1.0099 | Mixed; mostly translation gap |
| alloc-testN | 1.0045 | 0.9970 | 0.9925 | Rust at/better than parity |

All CVs were at most 1.47%. Assembly supports the classification. Ordinary C
refill is 74 bytes with GCC but 90 with Clang, exactly Rust's 90, so that size
difference is backend-specific. In contrast, page-queue search is 1,464 bytes
GCC, 1,109 Clang, and 1,568 Rust; abandoned collection is 390 GCC, 266 Clang,
and 892 Rust. Those remaining Rust gaps are dominated by typed validation,
ownership-result layout, and translation source shape—not LLVM's ability to
optimize equivalent C.

No PGO attempt was made: the prior unresolved interposition profiling-runtime
problem remains, and no deployed-equivalent mixed training artifact was
available. The official parity baseline remains GCC C.

## E00 — Fresh current-HEAD C parity baseline

Started: 2026-08-09

### Hypothesis and purpose

The existing ten-run full-suite comparison predates retained C++ and page-queue
improvements. Fresh C mimalloc 3.3.2 and Rust interposition artifacts built from
the current accepted source are required before attributing any later change.

### Source state

- Rust commit: `72fe0f4db7e2a3a6c8caafdb56944102554c7144`
- Branch: `master`
- C reference: repository copy of mimalloc 3.3.2
- Pre-existing untracked files were left untouched.

### Planned evidence

- Artifact hashes, dependencies, symbols, compiler versions, CPU/host state.
- Balanced five-plus-five `allt` comparison in both allocator orders.
- Separate balanced Larson sized/unsized and alloc-test1/N comparisons.
- `perf stat` for the focused workloads listed in E00.
- Resource probes and release assembly.

### Observations

- Fresh release artifacts built and passed `ldd -r` and `LD_PRELOAD=/bin/true`
  smoke checks. Both artifacts export malloc/free and the expected Itanium C++
  allocation/deallocation symbols.
- Rust SHA-256: `435643a94708df2ef1aa115d2177d708c721b17597f89a0d2e0cc3926fd7ca1a`.
- C SHA-256: `3f21eb1b182896f980ac9711897cd588c395aa8c4b4677bfd031a48491e140e4`.
- The balanced `allt` screen completed five C-first and five Rust-first
  repetitions. Raw logs and a ten-sample median/mean/min/max/CV table are under
  `target/perf-experiments/E00-current-head-baseline/full-results/`.
- Dedicated five-plus-five reversed-order probes produced these medians:

  | Workload | C | Rust | Rust versus C |
  |---|---:|---:|---:|
  | Larson unsized | 2.0540 s | 2.1235 s | 3.38% slower |
  | Larson sized | 2.0555 s | 2.1065 s | 2.48% slower |
  | alloc-test1 | 2.1000 s | 2.1350 s | 1.67% slower |
  | alloc-testN | 2.2200 s | 2.2100 s | 0.45% faster |

- The separate probe variation was controlled: Larson CV was 0.86% C versus
  1.01% Rust unsized and 0.91% C versus 0.92% Rust sized. The alloc-test1 gap is
  substantially smaller than the earlier pre-fix 6% to 7% loss, while the
  multithreaded form is at parity.
- Five-repeat `perf stat` on unsized Larson reported C/Rust instruction counts
  of 477,922,083,570 / 489,991,165,865 (+2.53%) and branch counts of
  56,701,427,001 / 61,925,155,418 (+9.21%). Branch misses were
  698,200,219 / 810,120,991 (+16.03%).
- Five-repeat `perf stat` on sized Larson reported C/Rust instruction counts of
  484,506,460,727 / 496,361,792,112 (+2.45%) and branch counts of
  58,755,109,956 / 64,204,261,895 (+9.27%). Branch misses were
  694,651,391 / 807,595,267 (+16.27%).
- These counters support investigating collection and abandoned-page control
  flow. They do not support cache-layout or memory-policy changes: task-clock,
  cycles, migrations, and page-fault totals were close in the paired Larson
  counter runs.
- The xmalloc cross-thread probe was faster in elapsed time for Rust but retired
  substantially more work: 639,977,496,904 versus 496,317,278,666 instructions
  (+28.94%) and 149,019,891,680 versus 122,483,057,029 branches (+21.67%). Rust
  had fewer branch misses and roughly half as many context switches. This is a
  mechanism signal, not a claim that reducing instructions alone will improve
  xmalloc wall time.
- alloc-test1 was close in instruction count (+0.18%) but Rust retired 4.58%
  more branches and 2.69% more cycles. At 16 threads Rust retired 2.21% fewer
  instructions, essentially the same branches, and 0.33% fewer cycles.
- glibc-simple was effectively equal in instructions and branches; Rust used
  0.59% more cycles in this counter run. glibc-thread retired 1.46% more
  instructions, 1.25% more branches, and 1.07% more cycles in Rust.
- malloc-large favored Rust on emitted work: 3.93% fewer instructions, 4.66%
  fewer branches, and 0.10% fewer cycles. This argues against touching the
  large-allocation policy for the current residual problem.
- Release assembly for the documented public hot symbols was captured under
  the E00 assembly directory.
- The repository resource probe was run three times for local alloc/free and
  remote-free against both exact hashed libraries. All 12 records passed
  `check-bench-resources.py`. Local median wall time was 106.28 ms C versus
  106.01 ms Rust; peak commit and end-minus-start commit were identical.
  Remote median wall time was 264.27 ms C versus 273.44 ms Rust; both reported
  3,997,696-byte peak commit, at most 393,216-byte commit growth, and zero page
  faults during the measured phase. Rust remote-free CV was 7.90%, so these
  resource runs establish memory behavior but are not timing evidence.
- The first external resource-harness attempt failed before executing a probe
  because the external-library path did not create its `release/` output
  directory. The measurement-only tool was corrected to create the directory;
  the failed attempt produced no result record. Its original build-own-Rust
  path was rerun successfully as a regression check.
- An initial ad hoc combined-table command incorrectly reported 15 samples per
  allocator because parser state carried across input files and counted the
  second transcript's live rows plus its final table. That table was discarded.
  `scripts/summarize-mimalloc-bench-logs.py` now parses only each transcript's
  final result table, enforces five samples per allocator per order, and emits
  the required statistics. A deliberate six-sample expectation correctly
  failed on these five-sample logs.
- Host evidence records all 32 CPUs using the `performance` governor and
  `performance` energy preference, full CPU affinity, toolchains, kernel, and
  load after measurement. Temperature capture was attempted, but the host has
  no `sensors` command; that environmental limitation is preserved in
  `host/sensors-after.txt`.
- The full screen exposed high variation in short `sh6bench`, `sh8bench`, and
  multithreaded `rptest` rows. Those rows will not be used alone to accept a
  candidate.

### Decision

`COMPLETE` — B00 is the accepted source and measurement baseline for E01. Every
result is traceable to the hashes above; no allocator implementation changed.

## E01 — Non-production observability of collection work

Started and completed: 2026-08-09

### Hypothesis

Feature-gated counters can identify whether the remaining collection overhead
comes from call frequency, list traversal, reclaim/reabandon decisions, or queue
searches, while compiling entirely out of the production allocator.

### Candidate and provenance

- Added or verified 66 `bench-diagnostics` counters covering every E01 outcome,
  validation reason, histogram bucket, scan, queue result, and ownership CAS.
- Added `scripts/run-page-batch-diagnostics.py` to exercise all 18 fixture
  combinations and `scripts/summarize-bench-diagnostics.py` to normalize work
  by its corresponding allocator-internal operation and enforce conservation
  relationships for every JSONL record.
- Final default-release SHA-256:
  `435643a94708df2ef1aa115d2177d708c721b17597f89a0d2e0cc3926fd7ca1a`.
  This is byte-for-byte identical to B00.
- Final diagnostic SHA-256:
  `d3a8b71b546f8ddb2c165423662a06c11e35891387a9b2f714ea42c3794fe1c8`.
- The final rebuild reproduced both hashes from the retained source. Therefore
  the earlier diagnostic workload records remain traceable to the exact final
  diagnostic bytes after the instrumentation-placement correction.

### Measurements and observations

- Larson unsized and sized each produced 11 independently reset snapshots.
  Local moves occurred on 34.71%/34.72% of full collections and scanned an
  average 5.2783/5.2797 blocks solely to find the local tail. Only 0.0013% of
  collections required the nonempty-destination force splice. This is direct
  evidence for E02: almost all of that local-tail traversal is avoidable when
  transferring the head to an empty available list.
- glibc-thread moved a local batch on 30.38% of collections and averaged 9.8728
  blocks per tail scan. It performed no force splices. This independently
  confirms that E02 is general collection work, not a Larson-specific trick.
- xmalloc moved a local batch on only 0.1543% of collections and averaged 3.5591
  blocks per tail scan. It is therefore a held-out regression workload for E02,
  not the reason to make the change.
- Detached remote batches averaged 1.0189/1.0206 blocks in Larson and 1.0306 in
  xmalloc. The length-one bucket contained 99.92%/99.92%/99.98% of detached
  lists. E05 must not be justified as a broad scan win from current evidence.
- Larson queue searches scanned 2.6747/2.6746 candidates per search and
  succeeded 99.97% of the time; almost every selected page was at position 2–4.
  xmalloc scanned 2.9888 candidates and succeeded 93.78%; its successful choice
  was essentially always first. No measured workload recorded a queue retry or
  failed extension. E08 remains conditional rather than automatically useful.
- Larson reclaim attempts succeeded about 97.56%; xmalloc reclaim attempts
  almost always failed because the freeing thread was uninitialized. This gives
  E04 a concrete mechanism signal, but E04 must still pass its own timing and
  correctness experiment before any production change is retained.
- Ownership-release CAS retry/attempt ratios were about 0.10% in Larson and
  0.01% in xmalloc. That frequency is too small to justify E06 without stronger
  per-retry evidence.
- All observed remote-list validation-failure counters were zero. All per-record
  relationships passed: collection force/owner splits, detached-list histogram,
  reclaim outcomes, reabandon outcomes, selected queue positions, bitmap
  outcomes, and tracked option reads exactly conserved their totals.
- The page-batch fixture recorded exactly the requested 1, 8, 32, or 128 scanned
  blocks per transition across empty and nonempty destinations, proving the
  scan counters reflect list length rather than Criterion iteration structure.
- Counts are normalized by their corresponding allocator operation (for
  example, blocks per tail scan, blocks per remote detach, candidates per queue
  search, and successes per attempt). Diagnostic elapsed times are deliberately
  excluded from performance conclusions.

### Compile-out and correctness validation

- Production `.text` remained 197,436 bytes and contained no `mi_bench_*`
  symbols. The diagnostic artifact had 430,465 bytes of `.text` and eight
  benchmark-only exports, as expected for a non-production feature.
- B00 and final E01 production `mi_malloc` and `mi_free` disassemblies compare
  byte-for-byte after removing the object filename header. Since the complete
  shared-library hashes also match, diagnostics introduced no production
  storage, symbols, branches, atomics, or instructions.
- Reset, snapshot, JSON sizing/termination, histogram relationships, validation
  failure distinctions, and all page-batch fixtures have focused tests.
- `cargo fmt --all`: passed.
- `cargo clippy --workspace --all-targets --all-features`: passed. The first
  attempt found one redundant test-only `u64` cast; it was removed before the
  successful full rerun.
- `cargo test --workspace --all-targets --all-features`: passed. The first
  attempt found that changing a CAS `while` to `loop` violated a source-shape
  release audit. Instrumentation was repositioned around the original `while`
  without changing counter meaning, then the entire gate was rerun successfully.

### Decision

`KEEP DIAGNOSTICS / NO PRODUCTION CHANGE` — E01 directly prioritizes E02 and
provides rejection evidence against speculative remote-list and CAS work. B00
remains the accepted performance baseline because the default production
artifact is identical; no baseline rerun is required before E02.

## E12 — Final parity and release qualification

Completed: 2026-08-09

### Provenance and controls

- Source commit: `72fe0f4db7e2a3a6c8caafdb56944102554c7144` on `master`.
  The worktree contained the retained E01 diagnostics and investigation
  documentation, so it was recorded as dirty rather than represented as clean.
- C mimalloc 3.3.2 SHA-256:
  `aaa9798f271f176ce37d75f9a7d8a2af2c5200699883ccb97a8903781df43750`.
- Final Rust interpose SHA-256:
  `435643a94708df2ef1aa115d2177d708c721b17597f89a0d2e0cc3926fd7ca1a`.
  It is byte-for-byte identical to B00. The complete normalized disassembly is
  also identical; only `objdump`'s input-filename header differed.
- Host: AMD Ryzen 9 5950X, 32 logical CPUs, Linux 6.17; Rust 1.92.0,
  GCC 13.3.0, and CMake 4.2.1. Exact environment and hashes are in
  `target/perf-experiments/E12-final-qualification/environment-and-hashes.txt`.

No production candidate survived E02-E10. B00 therefore remains the accepted
baseline and no post-acceptance baseline reset was necessary.

### Balanced wall-time results

The complete `allt` suite ran ten C-first and ten Rust-first repetitions: 20
samples per allocator and workload. Stable broad-suite ratios were at parity or
better except Larson sized (Rust/C 1.0395) and alloc-test1 (1.0189). Dedicated
20-sample reversed-order reruns confirmed Larson unsized at 1.0281, Larson
sized at 1.0312, and alloc-test1 at 1.0219. Both order halves agreed. The same
focused rerun put alloc-testN at 0.9926 and Lua at 1.0074. sh8's 1.0082 ratio had
4.08%/6.96% CV and conflicting order halves, so it is noise rather than a
retained regression.

The remaining full-suite workloads were broadly neutral: cfrac 0.9964,
espresso 1.0017, barnes 0.9991, Redis 1.0003, mstress 1.0034, xmalloc 0.9965,
glibc-simple 0.9982, glibc-thread 1.0000, and RocksDB 0.9977. rptest and sh6
had high CVs and contradictory order splits. Raw transcripts and both summary
tables are in the E12 artifact directory.

### CPU and memory attribution

Four fresh balanced `perf stat` repetitions per allocator showed:

| Workload | Task-clock | Cycles | Instructions | Branches | Branch misses | Cache refs | Cache misses |
|---|---:|---:|---:|---:|---:|---:|---:|
| Larson | -0.13% | +0.34% | +0.88% | +7.47% | +14.09% | +0.26% | -0.52% |
| Larson sized | +0.04% | +0.11% | +1.40% | +8.14% | +14.83% | +1.10% | +0.40% |
| alloc-test1 | +2.96% | +2.45% | +0.18% | +4.60% | +0.36% | +3.43% | -0.68% |
| glibc-thread | +0.00% | +0.42% | +0.73% | +0.53% | +0.33% | +3.31% | +29.04% |

The Larson deficit is a control-flow/translation-shape problem, not a cache
layout signal. glibc-thread's cache-miss delta did not move elapsed time and is
not an optimization justification by itself. E11 independently showed that
Clang-built C retains Larson parity with GCC C, so the gap is not explained by
GCC versus LLVM alone.

Five resource repetitions per allocator and scenario reported identical commit
behavior: 3,801,088-byte local and 3,997,696-byte remote peaks, 262,144-byte
local and 393,216-byte remote growth, and zero measured page faults. Rust peak
RSS was 14,352,384 bytes versus C's 14,614,528 in both scenarios. Local wall
time was 105.34 ms Rust versus 104.73 ms C. Remote timing was 288.12 ms versus
265.72 ms, but its 9.8%/16.6% CV is too high for a timing conclusion.

External preload probes cannot read allocator-private committed, abandoned, or
purge counters. E01's direct diagnostics therefore provide the disposition
evidence: Larson reclaim succeeded about 97.56%, reabandon succeeded about
0.33%, and xmalloc reclaim/reabandon almost always exited early. Purge and arena
rollback policy were not changed. Since final production bytes equal B00, their
effect relative to the accepted baseline is exactly zero; no unsupported
absolute C/Rust purge-count claim is made.

### Five-mode benchmark and correctness gates

`scripts/check-bench-full.sh` exercised default Rust, feature-linked interpose,
LD_PRELOAD interpose, plain page-batch, and secure page-batch modes. Default
threshold status was `pass`; interpose was correctly `not-applicable`. All
page-batch rows passed hard limits, while 10 plain and 13 secure diagnostic rows
crossed the investigation threshold and are marked `review-required`. Those
rows include `bench-diagnostics` counter overhead and do not exist in the
byte-identical production artifact.

The official archive command refused the truthful `dirty: true` manifests.
All five validated directories and assembly were instead preserved without
manifest alteration in
`archive/five-mode-dirty-evidence.tar.zst` (SHA-256
`54fc6a57cace1b1ddce32d2e79c16bc34bfbca78d262a04ab2c453b113db3404`).

`cargo fmt --all -- --check`, workspace/all-target/all-feature Clippy, workspace/
all-target/all-feature tests, safety stress, concurrency models, layout, public
ABI, atomic-ordering audit, C++ operator parity, release C++ artifacts,
LD_PRELOAD, and release integrity all passed. Behavioral parity remains a
release blocker: C accepts `mi_realloc_aligned(NULL, 24, 3)` and
`mi_realloc_aligned_at(NULL, 24, 3, 0)`, while Rust rejects both non-power-of-two
alignment requests. Those two failed checks make the unchanged B00 Rust test
executable exit 1 before the Python driver can compare complete output. The
exact traceback and captured Rust fields are in `validation/`; this
investigation did not suppress or reclassify the mismatch.

### Decision

`COMPLETE INVESTIGATION / NO PRODUCTION CHANGE / RELEASE BLOCKED` — Rust is
broadly at C parity and wins several held-out cases, but a reproducible 2.8% to
3.1% Larson gap and 2.2% alloc-test1 gap remain. Every attempted production
change was either unnecessary or rejected on correctness/performance evidence.
The retained diagnostics compile entirely out of release. Future work should
target a provably equivalent reduction in Larson's collection/queue branch
shape and must separately repair the baseline behavioral-parity subprocess
failure before release qualification can pass.

### Behavioral-parity remediation addendum

Completed locally: 2026-08-09

The E12 failure was repaired as a correctness change, not as a benchmark
candidate. The first mismatch was C mimalloc's regular-realloc shortcut for
nonzero alignments no larger than a machine word: C selects that route before
requiring a power-of-two aligned-allocation value. Rust had performed the
power-of-two conversion first. Regression coverage now exercises realloc,
rezalloc, and recalloc, including heap and `_at` variants, preservation,
zero-extension, and nonzero-offset rejection. The implementation retains the C
ordering while routing through Rust's checked `LayoutRequest` boundary.

Completing the full parity driver exposed two failures that the original early
exit had masked. Abandoned and huge pages reported areas but no live blocks
because the typed migration had unconditionally skipped abandoned pages. The
visitor now atomically claims an abandoned page, collects its free lists,
visits the live blocks, drains concurrent remote publications, and returns the
page to the abandoned state. A separate arena-exhaustion case crashed because
the read-only `_mi_theap_empty_wrong` failure sentinel was treated as a local
thread heap; the borrowed-heap allocation boundary now rejects that sentinel
and returns the C-compatible null result.

Validation passed after the final implementation:

- `cargo fmt --all`
- `cargo clippy --workspace --all-targets --all-features`
- `cargo test --workspace --all-targets --all-features`
- `python3 scripts/check-behavioral-parity.py`
- `python3 scripts/check-strict-types.py`
- `bash scripts/check-layout.sh`
- `python3 scripts/check-ffi-boundary.py`
- `bash scripts/check-raw-facade.sh`
- `python3 scripts/check-release-integrity.py`

The parity repair changes the production source even though it does not target
the benchmark hot paths. If it is accepted into the branch, establish a fresh
B01 C/Rust performance baseline before resuming optimization experiments; do
not compare later candidates directly against B00. E02R's accepted-baseline
reset above satisfies this requirement.

## X01/X02 — Root-cause and repair xmalloc detached-chain scan overhead

Investigated and decided: 2026-08-10.

### Baseline and hypothesis

X01 used the accepted E02R Rust production artifact as its immutable baseline:

- source `987d463`, equal to `origin/master` at capture;
- Rust SHA-256
  `3ac9ab5a5b916fe627c56faf3bfa0d809a8599a338a620dffc5887b16e51bb7b`;
- C mimalloc 3.3.2 SHA-256
  `aaa9798f271f176ce37d75f9a7d8a2af2c5200699883ccb97a8903781df43750`.

The standard 32-producer/32-releaser xmalloc run was already at timing parity,
but work-normalized counters showed 20.2% more Rust instructions per free. A
thread-count sweep exposed the actual allocator deficit: Rust/C throughput was
0.8244 at 1+1, 0.8861 at 4+4, 0.9829 at 8+8, 1.0222 at 16+16, and 1.0079 at
32+32. Queue contention and scheduling therefore hid excess allocator work in
the standard saturated result.

At 1+1 with 64-byte objects, the remote-free collection group retired about
98.4M Rust versus 72.4M C instructions and 25.6M versus 19.8M cycles per one
million frees. Allocation work itself was effectively equal. Diagnostics found
zero validation failures and 99.98% length-one detached chains.

### Root cause

Rust's `scan_detached_chain` classified every detached block before decoding
its link. Each iteration loaded page geometry, checked subtraction and
multiplication overflow, checked the committed range, and performed dynamic
block-size alignment modulo. The alignment check compiled to division in the
loop. Ordinary release C `_mi_page_free_collect_partly` instead follows the
already detached page-local links and enforces capacity/used bounds; its deep
corruption checks belong to hardened configurations.

The checks were redundant on the production ingress. `AtomicThreadFree`
publishes page-local remote frees, its compare-exchange detachment returns a
linear `DetachedRemoteList`, and conversion requires the matching `LocalPage`
or claimed `OwnedAbandonedPage` witness. Those unsafe contracts already require
every uniquely owned node to belong to that page and use its encoded-link
domain. Rechecking the same fact on every collection did not add a recoverable
safety guarantee in ordinary release, because violating that unsafe contract
is already outside the valid state space.

### Candidate and preserved invariants

X02 factors the common traversal into one const-specialized implementation.
Debug, `secure`, and `bench-diagnostics` builds select the validating scanner.
Ordinary release selects a trusted scanner whose safety contract explicitly
requires the detached list's page identity, encoded-link domain, readability,
and unique ownership.

Both scanners retain the capacity bound before following more than one page of
links. `RemoteFreeBatch` retains the independent `len <= used` bound. Local-list
scans remain validating in every build. No atomic ordering, publication,
detachment, splice, allocation, reclamation, purge, or memory-retention policy
changed. A focused differential test proves that trusted and validating scans
produce identical page/head/tail/length metadata for valid chain lengths 1, 2,
4, 8, and 16.

The immutable X02 candidate artifact SHA-256 is
`a75d08896625d8028b675c29efa710f00692f9a6bb16ef4f16c002a88feb0553`.
In `collect_after_remote_free`, static instructions fell from 458 to 353,
dynamic-division sites from eight to zero, and symbol size from `0x6e8` to
`0x590`. The complete artifact `.text` shrank 848 bytes. Call and locked-
instruction counts did not increase.

### Direct baseline/candidate measurement

Eight interleaved 1+1, 64-byte, five-second samples used the order
`base,candidate,candidate,base` twice and normalized counters by exact completed
frees:

| Metric | B01 baseline | X02 candidate | Change |
|---|---:|---:|---:|
| Throughput | 88.640M frees/s | 94.978M frees/s | **+7.15%** |
| Cycles/free | 83.932 | 80.188 | **-4.46%** |
| Instructions/free | 282.813 | 265.384 | **-6.16%** |
| Branches/free | 64.996 | 60.096 | **-7.54%** |
| Branch misses/M frees | 104,024 | 103,865 | -0.15% |
| Cache misses/M frees | 450,581 | 414,677 | -7.97% |

The direction agrees across wall time, retired work, code shape, and the exact
source mechanism.

### C comparison across contention and sizes

Two balanced samples per allocator and point compared the accepted candidate
directly with C. At 64 bytes, candidate/C throughput was 0.890 at 1+1, 0.908 at
4+4, 0.976 at 8+8, 1.009 at 16+16, and 1.044 at 32+32. This materially closes
the old 0.824/0.886 low-contention ratios without sacrificing saturated
performance.

At 1+1, candidate/C throughput ratios for 8, 64, 1024, 2048, and mixed
8..2048-byte allocations were 0.988, 0.901, 0.862, 0.893, and 0.980.
Candidate/C instruction ratios were 1.001, 1.114, 1.138, 1.132, and 1.050,
improving from the X01 baseline ratios 1.151, 1.182, 1.194, 1.178, and 1.126.
The remaining 64..2048-byte gap is real and is not claimed solved by X02.

### Held-out and resource validation

Six balanced observations per allocator covered Larson-sized, mstress,
glibc-thread, alloc-test single/multithread, and sh6 in both orders. Candidate/
baseline elapsed ratios were 1.006, 1.003, 1.008, 1.001, 1.004, and 0.883;
the sh6 samples are only 0.10--0.16 seconds and not a standalone claim. RSS was
neutral or lower except a noise-level 0.4% glibc-thread movement.

The complete 17-workload `allt` suite then ran three repetitions in each
allocator order. Stable ratios included xmalloc 0.998, rptest 0.963, mstress
0.997, glibc-simple 0.964, Larson-sized 1.006, RocksDB 1.004, alloc-test1 1.003,
and alloc-testN 1.008. Redis initially appeared 2.1% slower and the 0.25-second
sh8 test 4.7% slower, so both were rerun for five repetitions in each order.
Redis resolved to 1.0007 over ten samples. sh8 reversed from 1.104 in baseline-
first order to 0.954 in candidate-first order, demonstrating order/noise rather
than a repeatable regression. No allocation policy changed, full-suite RSS did
not show a coherent increase, and Valgrind reported zero bytes live at exit.

### Correctness and safety gates

All of the following passed on the final source:

- validating/trusted differential tests in debug and ordinary release;
- all production and secure page-batch tests;
- six Loom concurrency models;
- strict-type audit (335 sites);
- behavioral parity against C;
- release C-ABI cross-thread stress;
- ASan allocation/API suite and TSan stress/fuzz/thread-stat suite;
- Valgrind full leak/error checking of the release stress probe;
- `cargo fmt --all`;
- `cargo clippy --workspace --all-targets --all-features`;
- `cargo test --workspace --all-targets --all-features`.

The first ASan invocation hit the current nightly's pre-existing
`unused_unsafe` lint on `__cpuid_count`. It was rerun successfully with only
`-Aunused-unsafe`, the established repository sanitizer workaround; allocator
source was not changed for the toolchain lint.

### Decision

**KEEP.** Committed as `f018285`. X02 removes redundant dynamic work proven by the typed/unsafe
ownership boundary, follows ordinary C mimalloc's algorithm, retains hardened
validation and universal traversal bounds, produces a repeatable general
remote-free improvement, and has no repeatable correctness, memory, or held-out
regression. Promote the committed X02 artifact to B02 before the next
performance experiment; do not compare future candidates directly to B01.

## X03 — Authoritative B02 versus C full-suite comparison

Completed: 2026-08-10.

### Provenance and protocol

A fresh interposition library was built directly from committed B02
`f0182851f357e8fd00e8cd4cfefc6fd5f29443c8`. Its SHA-256 is
`a75d08896625d8028b675c29efa710f00692f9a6bb16ef4f16c002a88feb0553`,
byte-for-byte identical to the accepted X02 candidate. C mimalloc 3.3.2 used
SHA-256
`aaa9798f271f176ce37d75f9a7d8a2af2c5200699883ccb97a8903781df43750`.
Both libraries passed `ldd -r`, `LD_PRELOAD=/bin/true`, and export smoke checks.
The host was quiet and used the `performance` governor before measurement.

The complete `allt` suite ran three repetitions with C first and three with
Rust first, producing six samples per allocator for all 19 reported workload
rows. The outlying rptest, glibc-thread, Lua, and sh8 rows then ran ten
repetitions in each allocator order, producing 20 samples per allocator.
Benchmark arguments, operation counts, and allocator policies were unchanged.

### Complete-suite results

Ratios below are Rust/C elapsed time, so values below 1.0 favor Rust.

| Workload | Rust/C median | Mean ratio/order evidence | Classification |
|---|---:|---|---|
| xmalloc-testN | **0.9725** | 0.9771 C-first, 0.9755 Rust-first | Stable Rust win, about 2.5%--2.8% |
| glibc-simple | **0.9670** | 0.9658, 0.9634 | Stable Rust win, about 3.3%--3.6% |
| RocksDB | **0.9946** | 0.9938, 0.9893 | Small consistent Rust win |
| alloc-testN | **0.9955** | 1.0000, 0.9924 | Parity/slight Rust win |
| Larson-sized | **0.9956** | 1.0016, 0.9943 | Parity |
| espresso | 0.9983 | 0.9989, 1.0011 | Parity |
| barnes | 0.9976 | 0.9984, 1.0016 | Parity |
| mstressN | 1.0000 | 1.0021, 1.0021 | Parity |
| alloc-test1 | 1.0024 | 1.0016, 1.0000 | Parity |
| Redis | 1.0072 | 1.0025, 1.0023 by means | Parity; median granularity exaggerates gap |
| cfrac | 1.0055 | 1.0123, 0.9964 | Parity; order halves disagree |
| cache-scratch1 | 1.0060 | 1.0000, 1.0080 | Parity |

The descriptive sum of per-workload mean times was C 27.7693 seconds and Rust
27.8253 seconds, Rust/C 1.0020. This is not treated as an aggregate score
because workloads have different semantics and noisy short tests can distort
it; it only confirms there is no broad suite-wide shift.

### Qualified outliers

The 20-sample follow-up found:

- **glibc-thread is a small real remaining gap.** Rust/C median is 1.0106 and
  mean is 1.0104. Both order halves agree at 1.0123 and 1.0087, with 0.83% Rust
  CV. Rust is about 1.0%--1.1% slower than C here.
- **Lua is near parity with a small directional gap.** Rust/C median is 1.0062;
  order halves are 1.0037 and 1.0062. The approximately 0.5% mean difference is
  smaller than each allocator's 1.0%--1.3% CV.
- **rptest is inconclusive, not 9% slower.** The six-sample full-suite median
  suggested Rust/C 1.0919, but 20 samples produce median 0.9777 and mean 1.0195.
  CV remains 7.81% C and 6.07% Rust, and order halves conflict at 0.9668 and
  1.0759. No allocator ranking is justified.
- **sh8 remains noisy but directionally favors C.** Rust/C median is 1.0417 and
  mean is 1.0242; order halves are 1.0327 and 1.0159. Both allocators have about
  6.5% CV on a 0.24--0.25 second test, so this is recorded as an unresolved
  possible gap rather than a stable 4% regression.
- sh6 and cache-scratchN are likewise too short/noisy for percentage claims.

### Memory observations

No leak, fault, or failure appeared. RSS is workload-dependent rather than a
uniform Rust increase: Rust is about 21.4 MiB lower on RocksDB and 5.7 MiB lower
on mstress, equal on Larson-sized and alloc-test1, but commonly 1--3 MiB higher
for small processes. It is about 8.5 MiB higher on saturated xmalloc and about
5.8 MiB higher in the dedicated sh8 run. These are process-RSS observations,
not allocator-private committed-memory counts; the Rust artifact also carries
different runtime/link dependencies. X03 changed no memory policy and makes no
causal memory-regression claim from these cross-implementation RSS values.

### Overall standing

B02 is at broad workload parity with C mimalloc 3.3.2 and now clearly wins the
standard saturated xmalloc and glibc-simple tests. It remains ahead or at parity
on Larson-sized, RocksDB, mstress, and alloc-test. The clearest stable broad-
suite deficit is glibc-thread at about 1%. The separate X02 low-contention
xmalloc sweep remains the more important optimization target: B02 is 0.890x C
throughput at 1+1 for 64-byte uniform remote frees and 0.862x at 1024 bytes,
despite reaching or beating C under high contention. Future performance work
should target that remaining low-contention instruction burden without giving
back the saturated xmalloc win or the general-suite parity established here.

## X04/X05/X06 — Recover the post-B02 xmalloc regression

Investigated and decided: 2026-10-12 to 2026-10-13, AMD EPYC 4245P (12 logical
CPUs, so `xmalloc-testN` runs 12+12 threads). Every measurement used one
harness executable from `bench.py`, a fresh process per run, Criterion with 10
samples, 3 s measurement, and 1 s warmup, and interleaved rotating order.
Rust/C references come from the October 12 full run (C 1.842 ms).

### Bisect

The October full run showed `xmalloc-testN` at +10.5% against C, versus Rust
ahead of C at B02 on the 32-CPU host. Rebuilding B02 (`f018285`) on this host
measured +7.1%, so the gap combined the known low-contention deficit with a
regression. `git bisect run` used the Aug 10 library as a drift reference on
every step (1.968-1.988 ms throughout). The history was not monotonic:
`984e1fc` measured 1.845 ms (equal to C), `d1f39f2` 1.869 ms, and its child
`1294a69` ("Consume arena page release capabilities") 2.044 ms.

### X04 mechanism and change

`1294a69` made `_mi_arenas_page_try_reabandon_to_mapped_arena` decode the
page's MemId and compare arena, slice start, and slice count with the
`arena_memid` argument before its early exits. Its only caller,
`try_reabandon_mapped`, decodes that argument from the same claimed page
immediately before the call, and the function's safety contract already
requires that identity. C has no equivalent check. With 99.98% length-one
detached chains in xmalloc (X01), the repeated decode runs about once per
remote free. X04 keeps the comparison as `mi_assert_internal!`, so debug
builds still check the contract, and removes it from release.

### X05 mechanism and change

The flat profile after X04 showed `_mi_option_get_fast` at 2.9% of
`xmalloc-testN` time (C's equivalent was 4.1%), called from the collection
path. It is a four-instruction table load that was `#[inline(never)]` since
the source-layout commit, with no recorded reason. Allowing it to inline
removes all three call sites. C cannot do the same: its option getters are
exported and reached through the PLT.

### Results

| Library | `xmalloc-testN` median | vs C 3.3.2 |
|---|---:|---:|
| Before (`9f7d3c4`) | 2.036 ms | +10.5% |
| X04 | 1.902-1.909 ms | +3.3% to +3.6% |
| X04 + X05 | 1.853-1.864 ms | +0.6% to +1.2% |
| X04 + X06 | 2.006 ms | rejected |

The committed release library is byte-identical to the measured X04+X05
candidate (SHA-256 prefix `228c84000ee66278`).

Held-out, all 21 native scenarios, new versus old Rust library, two
interleaved rounds each (four for the borderline `sh6benchN`, `mstressN`,
`mleak100`, and `mleak10`): geomean **-0.84%**, no scenario slower than +1%,
and improvements beyond 1% on `xmalloc-testN` (-8.77%), `sh6benchN` (-3.53%),
`glibc-thread` (-1.74%), `larsonN` (-1.08%), and `larsonN-sized` (-1.03%).
`glibc-thread` was the previously recorded stable ~1% deficit.

### Verification

`cargo fmt`, `cargo clippy --workspace --all-targets --all-features` (no
warnings), and `cargo test --workspace --all-targets --all-features` (404
passed) pass. `scripts/check-safety-stress.sh` and
`scripts/check-release-thread-stress.py` pass. `scripts/check-sanitizers.sh`
passes for AddressSanitizer and ThreadSanitizer with
`RUSTFLAGS=--cap-lints=warn`; without it the current nightly fails to build
because `__cpuid_count` became safe and `src/allocator/init/cpu.rs:22` now
trips `unused_unsafe`. That pre-existing toolchain drift is unrelated to this
change.

### Remaining

At `984e1fc` Rust matched C on this host (1.845 ms); X04+X05 leaves about
+0.6% to +1.2%. The rest of the `1294a69` reshuffle and the later
`5f5910c..41d0e6e` creep (2.000 to 2.038 ms before these fixes) are the next
candidates. Miri was not rerun for this change.

## X07/X08/X09 — Remaining abandon-path loads and fast-path layout

Decided 2026-10-13 on the same host and protocol as X04/X05.

Rebuilding historical points with X05 applied showed the parent of `1294a69`
(`d1f39f2`) at 1.817 ms versus 1.861 ms for `c8dd865`, so work remained.
Aligning every function to 64 bytes (`-C llvm-args=-align-all-functions=6`)
moved nothing, ruling out general code layout. `_mi_arenas_page_abandon_internal`
and `_mi_arenas_page_unabandon_internal` performed `_mi_ptr_page` in release
only to feed `mi_assert_internal!`; the page-map read is an Acquire atomic load
that the compiler cannot discard. X07 moves both lookups inside the assertions:
`xmalloc-testN` 1.861 -> 1.822 ms (C 1.842 ms). X08 (`#[inline]` on unabandon)
was not taken by the compiler and is recorded only.

X07 also shifted the `global_asm!` fast paths: `malloc` moved from cache-line
offset 0 to 32 and `free` from 32 to 0, because the block start depended on
preceding code size. `sh6benchN`, which is bound by `malloc`/`delete[]`, ran
+2.6% slower over 8 interleaved rounds although X07's code never executes there
(profile: no abandon/collect symbols). X09 anchors the block with `.p2align 6`
and a 0x20 pad so `free` sits at offset 32 and `malloc` at 0 regardless of
other code: `sh6benchN` returned to +0.03% while `xmalloc-testN` kept -1.71%.
Layout gate for later work: release `malloc` offset 0, `free` offset 32.

Held-out, 21 native scenarios, X07+X09 versus `c8dd865` (two rounds, four for
`barnes`): geomean -0.22%, worst +0.67% (`cfrac`, noisy), improvements on
`xmalloc-testN` -1.67%, `mleak100` -1.46%, `sh8benchN` -1.07%. Gates: fmt,
clippy (no warnings), 405 tests, `check-safety-stress.sh`,
`check-release-thread-stress.py`, ASan and TSan (with `--cap-lints=warn` for
the nightly `unused_unsafe` drift in `src/allocator/init/cpu.rs`).

## X11 — Full-page test after generic large-block allocation

Decided 2026-10-13. After allocating a block larger than
`MI_SMALL_MAX_OBJ_SIZE`, C moves the page to the full queue only when
`used == reserved`. The translation instead tested "free list empty and page
fully extended", which is also true while freed blocks wait in `local_free`,
so such pages were moved to the full queue (and back on the next free) early.
The C-equivalent predicate already existed as `crate::internal::mi_page_is_full`;
X11 uses it at the end of `_mi_malloc_generic` in `src/allocator/page.rs`.

The new `bench.py screen` canary run was neutral (geomean -0.02%, no verdict
beyond its ~1.5% noise floor), as expected: the canaries rarely allocate
blocks above the small-object limit. The target was Nginx large bodies.
Rust-only Nginx A/B (production profile, proxy and transfer workloads, three
repeats each, X11 then HEAD): `proxy-262144-on` 61.6k-62.7k vs 59.0k-61.4k
requests/s (+2.2%), `proxy-65536-on` 152.4k-153.0k vs 149.9k-151.5k (+1.6%);
`upload-1048576` read -6.2% on the median but its ranges overlapped completely
(9.2k-10.2k vs 9.0k-10.1k), so it is noise. Gates: clippy (no warnings), tests,
`check-safety-stress.sh`, `check-release-thread-stress.py`,
`check-behavioral-parity.py`, ASan and TSan (`--cap-lints=warn`), and
`check-miri.sh`. A deterministic regression test was not added: in a single
thread the generic path drains `local_free` before allocating, so reaching the
divergent state needs a queue-search interleaving that unit fixtures do not
model.

## X10 — rptestN measures worker time

Decided 2026-10-13 (measurement change, applies identically to every
allocator). `rptest::run` divided wall time by operations, but wall time
included the main thread's 1-second sleep polls (about 20 s per run), so the
result mostly measured sleep quantization; the workers' measured `ticks` were
computed and discarded. Upstream rptest reports memory operations per CPU
second from those ticks. X10 reports summed worker ticks per million
operations and renames the metric `per-1M-rptest-cpu-ops`, so old and new
results cannot be compared by accident.

Five fresh-process runs each: Rust median 145.4 ms per 1M ops (runs 145.4,
146.5, 133.9, 145.9, 134.1; CV 4.6%), C mimalloc 148.4 ms (136.2, 148.9,
143.8, 148.8, 148.4; CV 3.8%), Rust/C -2.0% on medians. Both allocators are
bimodal (about 134 vs 147 ms), consistent with scheduling of 12 workers on six
SMT cores rather than allocator behaviour. The planned <2% CV bar was not met;
the user chose to adopt the metric and require at least five runs per
allocator for rptestN in scoreboards and held-out comparisons.

## X13/X14 — Indirect calls between the library's own exported functions

Investigated 2026-10-13. Calls written as `crate::mimalloc::f` resolve to the
`extern "C"` declarations re-exported from `src/ffi/abi.rs`, so they compile to
GOT-indirect calls that cannot inline (106 such call sites in
`src/raw/macros.rs` alone). Rust also routes calls to any exported
(`no_mangle`) function through the GOT unless the call inlines, and ignores
`#[inline]` on exported functions (a hard error under `-D warnings`).

X14 linked the cdylib with `-Wl,-Bsymbolic-functions`. Internal symbols
needing dynamic lookup dropped from 24 to 0, but the instructions stayed
`jmp *GOT(...)`; turning them into direct calls needs relaxable GOT relocations,
which stable rustc does not emit for x86_64-unknown-linux-gnu. No runtime work
removed; reverted.

X13 gave the libc aligned entry points non-exported bodies (the C
internal-alias pattern), which removed every GOT hop on `posix_memalign`,
`memalign`, and `aligned_alloc`. `rptestN` could not resolve the change
(bimodal, ranges overlapping over five runs), so a C microbenchmark dlopened
each library and timed 10 million aligned allocate/free pairs, interleaved over
seven runs on one pinned CPU: `posix_memalign` 8.81 -> 8.72 ns (-1.0%) but
`memalign` 6.10 -> 6.95 ns (+13.9%, disjoint), with 22 more instructions per
operation overall. Inlining the Rust aligned path into the wrapper costs more
than the hops it removes; the existing route through the hand-written
`mi_malloc_aligned` stub reaches a leaner compiled body. Reverted.

## X16 — page_full_retain default 0 (tcmalloc Larson gap)

Decided 2026-10-13. The rival baseline (B03) showed tcmalloc ahead on
`larsonN-sized` (1.183) and `larsonN` (1.099). Profiles normalized per
operation showed Rust's fast paths cheaper than tcmalloc's whole allocator;
the gap was Rust's slow path (~11% of cycles, absent in tcmalloc). A
`bench-diagnostics` build (quick mode now writes counters) on `larsonN-sized`
counted 36.0M page-queue searches for about 683M allocations (1 in 19),
102.5M free-list collections of which 66.4M found nothing, and 99.5% of
searches selecting the 2nd-4th candidate. C's search keeps up to
`page_full_retain` (default 2) exhausted small pages in the queue, so each
search re-examines them. The translation matches C, which is why Rust is at
parity with C here.

`MIMALLOC_PAGE_FULL_RETAIN` sweeps (3 runs each): 0 gave `larsonN-sized`
2.535 -> 2.431 ms and `larsonN` 2.543 -> 2.406 ms; 8 was 17-19% slower;
`page_max_candidates=1` was neutral. The default lives in two tables in
`src/allocator/options/storage.rs` (descriptor and `OPTION_VALUES`); changing
only the descriptor had no effect, which the screen caught (Larson neutral).

Evidence for the default change (Rust-only, interleaved): canary screen
`larsonN` -4.1%, `larsonN-sized` -3.5%, others neutral; 21-scenario held-out
`larsonN` -5.37%, `larsonN-sized` -4.99%, geomean over 20 scenarios -0.64%
after four-round rechecks (`malloc-large` -1.63%, `mleak100` 0.00%, `mleak10`
+0.94% overlapping); `rptestN` +3.5% in two Criterion runs but -9.8% over seven
quick runs and +1.2% median over 15 (95% bootstrap CI [-3.7%, +3.9%], means
140.2 vs 140.8 ms); Nginx production 18 cases +0.52% throughput; architecture
probes max RSS difference 0.25 MiB, identical occupied pages; differential
traces match. `check-behavioral-parity.py` initially failed on exactly the
changed default and on default-policy search counts (C 255 page visits vs Rust
102 for the same 103 searches); it now compares under equal option values and
requires exact C behavior when Rust is configured with C's default.

## X17/X18 — Medium-size generic path versus C

Decided 2026-10-13. An Nginx-shaped microbenchmark (dlopen each library, time
4M operations pinned to one CPU) showed tcmalloc about 4x faster than both
mimallocs on 8-16 KiB requests (a structural difference: mimalloc sends sizes
above 1 KiB through the generic path) and Rust 12-15% slower than C mimalloc
(16 KiB `posix_memalign` 12.95 vs 11.30 ns; 8 KiB `malloc` 9.08 vs 7.97 ns).
Callgrind on an 8 KiB malloc/free loop counted 251.5 Rust vs 194.8 C
instructions per pair (`perf stat`: 246.6 vs 193.3). Each `free` empties the
page and retires it, so `mi_theap_page_queue_of` runs every time: it cost 60
instructions because Rust computed the page bin with `mi_page_bin_typed` and
then, for ordinary pages, recomputed it from the block size; C's
`mi_theap_page_queue_of` computes `mi_page_bin` once (`mi_bin` 28).

X17 indexes the queue table with the bin already computed: 246.6 -> 213.6
instructions per pair. Collapsing the redundant normal-queue revalidation in
`mi_find_page_typed` (`NormalPageQueue::new_unchecked` plus `unwrap_unchecked`)
changed nothing (246.6), because the compiler had already removed it.

X18 inlines the generic body into the C entry point `_mi_malloc_generic`, which
had been a 16-instruction wrapper around `_mi_malloc_generic_typed`:
213.6 -> 205.6 instructions per pair, library +1.4 KB. Time, seven interleaved
pinned runs, X17+X18 versus HEAD: 8 KiB malloc/free 9.08 -> 8.16 ns (-10.1%),
16 KiB `posix_memalign` pool 12.94 -> 11.68 ns (-9.7%), pool plus 1 KiB and
4 KiB 26.11 -> 24.15 ns (-7.5%); the remaining gap to C is +2.3% to +6.8%.
Fast-path layout unchanged (`malloc` offset 0, `free` 32).

Held-out (X17+X18 versus X16, 20 scenarios; `rptestN` measured separately):
geomean -0.21%, `mleak10` -3.72%, `malloc-large` -1.42%. `larsonN-sized` read
+1.57% (two rounds), +1.02% over four rounds with disjoint ranges, and +0.24%
in nine quick 1-second rounds (X17 alone -0.08%); `larsonN` +0.17% and -0.64%.
`rptestN`, seven quick runs: median -3.0%, mean +0.28%. The canary screen
flagged `sh6benchN` +2.09% (one +11.4% round) and read -1.95% over ten rounds.
Gates: fmt, clippy, tests, safety stress, release thread stress, behavioral
parity, differential trace, ASan/TSan, Miri.

## H01 — Native harness publication and timing fixes

Decided 2026-10-13, from an external source review (`NOTES_ALLOCATOR.md`,
G1-G4), each finding checked against `benches/mimalloc_bench.rs`:

- `mstressN`: the transfer-slot `swap` was `Acquire`, whose store half is
  Relaxed, so the producer's item writes were not released to the thread that
  received and verified them. Now `AcqRel` (same `xchg` on x86-64).
- `sh8benchN`: the main thread published foreign blocks through plain
  volatile slot writes while workers read them and the owner word inside the
  block: a data race under the Rust memory model. Slots are now accessed
  through `AtomicPtr::from_ptr` (Release store, Acquire load; plain `mov` on
  x86-64).
- `rptestN`: the summed worker spans are wall-clock, not CPU time, so the
  metric is renamed `per-1M-rptest-worker-ops`. Per-thread CPU time was not
  adopted: it would hide time lock-based rivals spend blocked. The drain after
  each iteration counted its frees without timing them; it is now timed.
- `larsonN`/`larsonN-sized`: workers started counting as soon as they were
  created, before the coordinator read its start time. They now wait on a
  start flag set right after it.
- `xmalloc-testN`: a releaser counted a batch of 4,096 frees when shutdown
  made `dequeue_batch` return none; it now counts only dequeued batches.

An A/A screen (same library both sides, 3 rounds) of the six scenarios ran
clean and shows their noise floor: `rptestN` +1.75%, `larsonN` +1.34%,
`larsonN-sized` +1.10%, `sh8benchN` +0.94%, `xmalloc-testN` +0.26%,
`mstressN` -0.24%. Results for these scenarios from before H01 must not be
mixed with later ones in a scoreboard; re-measure every allocator for them.
`BENCHMARK_RESULTS.md` also no longer claims that page-map registration
covers every 64 KiB slice of a `malloc-large` buffer: the span is capped at
the 4 MiB large-page window.

## X19 — Medium-size generic malloc fast path

Decided 2026-10-13. Sizes above 1 KiB skip the hand-written `malloc` fast path
and run C's whole generic routine (`_mi_malloc_generic`, about 100
instructions on an 8 KiB malloc/free loop). When maintenance is not due and
the size class's first page has a free block, C's work reduces to: increment
`generic_count`, find the queue, quick-collect the first page, reset its
retire countdown, pop, and the X11 full-page test. `malloc_generic_fast` does
exactly that, for non-zeroed requests up to `MI_LARGE_MAX_OBJ_SIZE`, and both
entries (`_mi_malloc_generic`, `_mi_malloc_generic_typed`) try it first. A miss
after counting continues in `malloc_generic_outlined::<true>`, which skips the
count and maintenance check; anything else takes the unchanged path. No
behavioral divergence from C. A regression test drives 2,500 mixed generic
calls (both entries, zeroed, size 0, page-filling misses) and checks
`generic_count` and the heartbeat after every call.

Codegen: the hit path saves one register and keeps no frame; misses tail-jump
out of line; `malloc`/`free` stay at cache-line offsets 0/32; library +760
bytes. The first version inlined the fast path and two copies of the body
into the typed entry; in debug builds that frame grew from 2,824 to about
4,700 bytes and overflowed loom's 32 KiB model stacks (the crate is the test
global allocator). Both misses now go out of line, and the fast path is
`inline(never)` under `debug_assertions` only (debug miss path +232 bytes
over HEAD).

Results versus HEAD:

- 8 KiB malloc/free: 205.6 -> 169.7 instructions per pair (C 193.3).
- Nginx-shaped microbenchmark (ns/op, pinned): 16 KiB `posix_memalign`
  11.7 -> 11.3 (C 11.3), 8 KiB malloc/free 8.2 -> 7.6 (C 8.0), pool plus
  1 KiB and 4 KiB 24.1 -> about 23.0 (C about 22.9). tcmalloc 2.95, 2.22,
  7.25: per-thread caches remain about 3x cheaper.
- Canary screen geomean -0.34%. 21-scenario held-out geomean -0.09%;
  `malloc-large` +1.86% and `mleak10` +1.71% rechecked over four rounds at
  +0.67% (overlapping ranges) and -1.67%. `rptestN` -0.39% in two rounds; the
  seven-run recheck was stopped before completing.
- Nginx, 3 repeats, versus the HEAD rows of the same-settings run 90 minutes
  earlier (`nginx-3rep`), excluding the bimodal `headers` and
  `upload-1048576`: production -0.12% (4 of 16 cases faster), debug-palloc
  +0.26%; peak worker PSS +0.5% / -0.1%. Medium-size allocation is too small
  a share of Nginx's per-request CPU (3-5 allocations per request) for the
  saving to show; tcmalloc's ~1% Nginx lead is larger than our whole
  medium-size allocation cost difference and needs a different explanation.

Gates: fmt, clippy, tests, safety stress, release thread stress, behavioral
parity, differential trace, ASan/TSan, Miri.

## X20 — C++ sized delete aliases `free`

Decided 2026-10-13, from the tcmalloc Larson gap. tcmalloc gets 4% faster
with sized delete (`larsonN` 2.227 -> `larsonN-sized` 2.136 ms per 1M pairs);
Rust got 0.9% slower (2.383 -> 2.404). Unsized `_ZdlPv`/`_ZdaPv` were already
aliases of the hand-written `free`, but sized `_ZdlPvm`/`_ZdaPvm` were a stub
doing `jmp mi_free`, which binds through the PLT because `mi_free` is
exported. C mimalloc has the same hop (`_ZdaPvm` -> `mi_free@plt`). Release
sized delete ignores the size in both, so the sized forms are now aliases of
`free`. GCC and Clang emit sized delete for ordinary `delete` from C++14, so
this applies to every C++ program, not only Larson.

The removed stub moved `malloc` to cache-line offset 48; the padding before
it grows by the stub's 16 bytes, restoring offset 0 (`free` 32) with
byte-identical `malloc` code.

A `new[](64)`/`delete[]` loop through the dlopened operators: sized pair
56.34 -> 53.34 instructions (unsized 52.34, C sized 63.38); time 2.24 -> 2.22
ns, since the indirect jump was well predicted. Screen, 5 rounds:
`larsonN-sized` -0.82% (4 of 5 rounds faster), `larsonN` -0.26% (does not use
sized delete). Most of tcmalloc's sized-delete advantage remains: it uses the
size to skip its page-map lookup, which mimalloc cannot, because size does not
establish a block's page or owner.

Gates: fmt, clippy, tests, release C++ artifacts, C++ operator-new parity,
public and ABI symbols, release LD_PRELOAD, release thread stress,
differential trace, behavioral parity.

## X21 — Keep full pages in the owner's full queue (rejected)

Decided 2026-10-13, from the tcmalloc Larson gap after X20. Per allocation on
`larsonN`, Rust executes fewer instructions than tcmalloc (196.7 vs 203.6;
C 213.3) but runs at lower IPC (1.30 vs 1.38): 3x the branch misses (0.132
vs 0.044) and about 20% more L1 data misses (6.01 vs 4.88). A branch-miss
profile put 17.5% of Rust's misses in `_mi_page_abandon_internal`; abandon,
reclaim, and unown together took about 6% of cycles. Like C 3.3.2,
`mi_page_to_full` abandons a page that fills up whenever
`page_full_retain >= 0` (C: "the usual case in order to allow for sharing of
memory between theaps"), and Larson's owner then frees into it, claims it
back, and reclaims it.

`MIMALLOC_PAGE_FULL_RETAIN=-1` disables that (the search behaves as with 0;
only abandonment changes): `larsonN` Rust 2.434 -> 2.396 ms, C 2.634 -> 2.381
(three runs each). As a built-in default, the canary screen gave `larsonN`
-1.98% and `larsonN-sized` -2.44%, but `xmalloc-testN` +318.55% (+311% to
+378% in every round): producers' full pages stay in their full queues while
consumers free into them remotely, instead of being reclaimed by the freeing
thread. C reserves the full-queue path for heaps that cannot abandon. Reverted.
The Larson cost of the abandon round trip remains; a fix would have to make
an owner's free into its own just-abandoned page cheaper, not stop
abandoning.

## X22 — No libstdc++ dependency

Decided 2026-10-13, from the TechEmpower axum webserver run, where the server
used 1.3 MiB more PSS under Rust than under C mimalloc. `smaps` showed equal
allocator arenas (16,544 vs 16,532 kB); the difference was `libstdc++.so`
(1,265 kB), loaded only because the library had `NEEDED libstdc++.so.6`.
`build.rs` linked `-lstdc++` for `src/cxx_new_shim.cpp`, whose `operator new`
slow path calls `std::get_new_handler()` and throws `std::bad_alloc` on
failure. C mimalloc built as C depends only on libc and aborts instead.

The shim now resolves `_ZSt15get_new_handlerv` and `_ZSt17__throw_bad_allocv`
with `dlsym(RTLD_DEFAULT, ...)` when an allocation fails (as `mi_new` already
resolves the handler), and the library no longer links libstdc++ or imports
any GLIBCXX/CXXABI symbol. A C++ program has libstdc++ loaded, so `new[]`,
aligned `new`, the new-handler loop (three calls, then `bad_alloc`), and
`nothrow` behave as before; in a process without a C++ runtime, failure
aborts (an uncaught `bad_alloc` already ended in `std::terminate`). Hot paths
are unchanged (8 KiB pair 169.59 instructions; `malloc`/`free` offsets 0/32).

Every non-C++ process saves about 1.2 MiB and one library load: the json
server under load 19.06 -> 17.87 MiB (C mimalloc 17.82).

Gates: fmt, clippy, tests, safety stress, release thread stress, behavioral
parity, differential trace, ASan/TSan, Miri, C++ bad_alloc divergence, C++
operator-new parity, release C++ artifacts, ABI and public symbols, release
link metadata, LD_PRELOAD, dlopen, artifact consumer, package.

## X23 — LIFO reuse of owner frees with a free-path heartbeat (rejected)

Decided 2026-10-14, from the TechEmpower axum plaintext run, where jemalloc
led Rust by about 2.6% (3 interleaved runs). Both allocators took about 5.3%
of server CPU; the difference was in application code touching new blocks:
72 L1 data misses per request for Rust against 47 for jemalloc. As in C, an
owner's `free` pushes onto `local_free`, which is reused only once the page's
`free` list runs dry, so a size class cycles through a whole extension chunk
(up to 4 KiB) before reusing a hot block.

Prototype: the asm `free` fast path pushes onto `page.free` (and clears
`free_is_zero`, without which `calloc` returned dirty memory). Plaintext L1
misses fell to 47 per request, and the 21-scenario canary screen gave
geomean -1.33%: `glibc-thread` -9.0%, `larsonN` -3.8%, `larsonN-sized`
-3.5%, `rptestN` -3.9%, `sh6benchN` -3.9%, `sh8benchN` +1.2%. Server PSS
was unchanged. But C keeps `local_free` separate so the free list runs dry and
allocation regularly reaches the generic path, which runs deferred-free
callbacks, retired-page collection and periodic collection. Over 20M
malloc/free pairs of one size, C and HEAD fired the deferred-free callback
1,047 times and the prototype 0 times.

Candidate (owner-approved divergence): the asm and Rust free paths count owner
frees down from 64 in a theap field placed in existing padding; at zero,
`_mi_page_free_heartbeat` counts one generic tick and, when maintenance is
due, parks the page's free list on `local_free`, so the next allocation from
the page runs the maintenance on the generic path as in C. Frees that empty a
page must count too: a size class that allocates and frees one object at a
time never reached the counter otherwise. That gave 343 callbacks over the
same 20M pairs, and the gates passed (behavioral parity needed one reuse-order
allowance).

The counter update costs about 2 cycles per free, more than its four
instructions suggest. On `glibc-simple`, cycles rose from 3,393M (LIFO alone)
to 3,589-3,679M (+2.1% instructions, +5 to 8% cycles). A thread-local counter
(no dependent `page->theap` load) recovered about 1%; aligning `free` to fit
in two cache lines changed nothing. Quick runs (ms) for HEAD, LIFO alone, and
the candidate: `glibc-thread` 1.42 / 1.29 / 1.53, `larsonN` 2.45 / 2.36 /
2.49, `glibc-simple` 625 / 624-633 / 662-681. The held-out run, stopped after
13 of 21 scenarios, had `glibc-simple` +5.7%, `malloc-large` +6.2%,
`sh6benchN` +15.3%, `sh8benchN` +6.6% versus HEAD. The webserver gain was also
layout-sensitive: two LIFO-only builds 16 bytes apart measured +2.4% and
-4.5% on plaintext with identical instruction counts and L1 misses. Reverted
without merging.

Counting only frees that empty a page would cost nothing on the fast path,
but a size class whose page also holds long-lived objects then never reaches
maintenance (0 callbacks with 64 live blocks of the same size), a larger
divergence from C than the one approved. Not pursued.

## X24 — Cheaper abandon/reclaim round trip, C-equivalent (rejected)

Decided 2026-10-14. At X16's `page_full_retain=0`, Rust and C run `rptestN`
within about 1% of each other, so the remaining cost of abandoning full pages
and reclaiming them on free is shared with C. The profile (`perf record`,
Criterion steady state, `MIMALLOC_PAGE_FULL_RETAIN=0`) put the round trip at
about 5 to 8% of cycles on `rptestN` and `larsonN`. Within it, nearly all
samples land on locked instructions on the page's first cache line:
`_mi_page_abandon_internal` 68% on the `lock btr` that clears the in-full flag
and 21% on the owner-clearing CAS; `_mi_theap_page_reclaim` 56-64% on the
owner-setting CAS and 19-43% on a `lock btr` that clears an in-full flag an
abandoned page never has; `_mi_arenas_collect` 99% on the global purge-guard
CAS. C executes the same atomics.

Two implementation-only changes were tried. (1) `mi_page_set_in_full` returns
early when the flag already has the requested value; only the owner writes it,
so a relaxed read is exact and the RMW was a no-op. (2) The atomic guard reads
the word before its CAS, which is the same as C's CAS failing at the load.
Interleaved Criterion runs (6 rounds, median):

| Scenario | (1) vs HEAD | (1)+(2) vs (1) |
|---|---|---|
| `rptestN` | -1.20% | +0.47% |
| `larsonN` | +0.12% | +0.89% |
| `larsonN-sized` | -0.58% | not run |

The (1) build ran `rptestN` at 115.9 ms in the second comparison, against
116.0 ms for HEAD in the first, so its gain is within run-to-run noise. A
locked instruction waits for earlier stores to drain, and those stores miss:
page headers that other threads free into, and the neighbouring pages' queue
links. Removing one lock moves the wait to the next. The cost is the cache
traffic of handing pages between threads, which only `page_full_retain` (X16)
or a different design (tcmalloc's batched central free lists) avoids. Both
changes reverted without merging; merging abandon's two atomics into one CAS
was not tried for the same reason.

## X25 — Batched remote frees per page (rejected)

Decided 2026-10-14. X24 traced the abandon/reclaim cost to cache-line traffic
on page headers, and on `rptestN` 74% of `delete`'s cycles (3.7% of the run)
sit on the remote-free CAS onto the page's `xthread_free`. The idea, from
snmalloc: a freeing thread collects blocks bound for another thread's page in
a local chain and publishes the chain with one CAS.

Prototype: a 16-slot direct-mapped table in the TLD, keyed by page address.
A remote free links the block into its page's slot; the slot publishes (tail
linked to the current list, one `try_push`, then the usual abandoned-page
claim and collect) at 64 blocks or 32 KiB, when another page evicts it, at the
generic maintenance tick, in theap collection and at thread exit. Blocks
larger than 8 KiB and uninitialized threads publish directly. The asm `free`
sent remote frees to the Rust generic path, which now batches.

Batch hit rates (quick runs, counted in the library):

| Scenario | Hits | Blocks per publish |
|---|---|---|
| `xmalloc-testN` | 98.4% | 63.0 |
| `sh8benchN` | 98.4% | 61.2 |
| `larsonN` | 70.9% | 3.44 |
| `mstressN` | 38.3% | 1.62 |
| `rptestN` | 15.4% | 1.18 |

Quick screen (3 interleaved runs each) vs HEAD: `larsonN` +4%,
`larsonN-sized` +5%, `xmalloc-testN` +4%, `sh8benchN` about -1%, `mstressN`
flat, `rptestN` too noisy to call. On `larsonN` the publish CAS still took
about 2% of cycles (63% of `mi_free_block_mt`), the same as the per-block CAS
on HEAD, despite 3.4 times fewer CASes. The expensive CAS is the first one to
a page, which must fetch the header line from the owner; consecutive remote
frees to the same page already find that line in the local cache, so the
hardware was batching them. Batching removes only the cheap CASes and adds a
TLS lookup and slot update to every remote free. `xmalloc-testN`, at 63 blocks
per publish, still ran 4% slower with 5% more instructions.

What remains is the read of the page header that every `free` makes to find
the owner (about 30% of `delete` on `larsonN`, with the page-map load another
30%). jemalloc and tcmalloc avoid it by freeing into a thread cache found
from the page map, without touching slab metadata. Reverted; the prototype
diff is kept outside the repository.

## X26 — LIFO reuse of owner frees without the maintenance counter (adopted)

Decided 2026-10-14 by the owner, who allowed departing from C where it pays.
X23 showed that reusing a block freed by its owner next (pushing onto
`page.free` instead of C's `local_free`) wins on the native suite and matches
jemalloc's L1 misses on TechEmpower plaintext, and was rejected only for the
per-free countdown that kept C's maintenance cadence. X26 drops the countdown
and accepts the consequence: a thread whose size classes churn without a page
filling or emptying no longer reaches the generic path, so it runs no
deferred-free callbacks, keeps at most one retired page per size class longer,
leaves pending purges to the next page free, abandon or collection by any
thread, and collects remote frees into its pages when a page runs dry
(`UPSTREAM_DIFF.md`).

A first version kept C's cadence while a deferred-free callback was
registered: owner frees went back to `local_free`, with the `free` fast path
testing the callback slot (moved to its own cache line, since it shared one
with the purge guard). It gave exactly C's 1,047 callbacks over 20M
malloc/free pairs of one size. But those two instructions cost about 2.5% on
`sh8benchN`, a tight malloc/free loop (296 ms against 287.5-288.1 ms without
the check, HEAD 285-286 ms; 3.5% more instructions than HEAD, 2.4% from the
check), and the owner chose to drop it.

Measurements of the version with the check (held-out: 3 interleaved
Criterion rounds per scenario against HEAD; the check only adds cost):

| Scenario | Held-out |
|---|---|
| `glibc-thread` | -8.94% |
| `larsonN-sized` | -3.12% |
| `sh6benchN` | -1.86% |
| `larsonN` | -1.48% |
| `rptestN` | -1.25% |
| `alloc-testN` | -1.19% |
| `mstressN` | -1.08% |
| `alloc-test1` | -0.91% |
| `malloc-large` | +2.93% |
| `sh8benchN` | +1.58% |
| other 11 | -0.53% to +0.48% |
| geomean (21) | -0.79% |

`malloc-large` (5-25 MiB blocks in single-block pages, which LIFO cannot
affect) is bimodal per process in both builds (HEAD 725 or 736-741 us,
candidate 733 or 750 us over 6 runs each); the residue is layout. TechEmpower
plaintext in 6 interleaved rounds with fresh servers: candidate won every
round, median 3.966M against 3.923M requests/s (+1.1%); the separate-session
`bench.py webserver` runs had shown -4.4%, json -0.7%, fortunes +0.6%. Nginx
production (3 repeats, separate sessions, HEAD built from a master worktree):
+0.6% throughput geomean over 18 cases, the 8 proxy cases +0.6% to +2.8%,
`headers` +8.3% with peak worker PSS 78.5 against 74.4 MiB,
`upload-1048576` -3.2%; other PSS within 1 MiB. The first HEAD Nginx run
stopped when its PSS sampler read `/proc` for an exited worker (no crash in
the kernel log or Nginx error logs) and was rerun.

Final build (no check), 21-scenario screen, 5 rounds against HEAD: geomean
-1.08%; `glibc-thread` -8.47%, `larsonN` -5.12%, `larsonN-sized` -4.05%,
`rptestN` -2.95%, `sh6benchN` -1.74%, `mstressN` -0.94%, `alloc-test1`
-0.84%, `sh8benchN` +1.41%, `malloc-large` +0.39%, the rest within +/-0.7%.
`sh8benchN` keeps a small cost from the extra stores on the free path
(`free_is_zero`). Gates passed: fmt, clippy, all-features tests,
safety and release thread stress, behavioral parity (one allowance,
`theap_wrong_thread_malloc_aligned_status`), differential traces, ASan, TSan
and Miri.

## X27 — PGO and BOLT (rejected)

Decided 2026-10-14. Code-layout effects of +/-2 to 5% had shown up repeatedly
(X23's plaintext builds 16 bytes apart, the `.fill` alignment of the asm fast
paths), so profile-guided compilation and post-link layout looked like a cheap
way to make layout deliberate.

PGO: `llvm-profdata` from the toolchain's `llvm-tools` (LLVM 21.1.3, matching
rustc 1.92). The instrumented cdylib (`-Cprofile-generate`) also needs
`-Clink-arg=-lc`, because the profiler runtime calls `atexit`, which glibc
provides in `libc_nonshared.a`. To separate real gains from overfitting it was
trained on 11 scenarios (`alloc-test1`, `barnes`, `cache-scratchN`,
`cache-thrash1`, `cfrac`, `glibc-thread`, `larsonN`, `malloc-large`,
`mleak10`, `mstressN`, `sh6benchN`, one quick run each) and the other 10 held
out. The `-Cprofile-use` build grew `.text` by 34 KiB (profile-guided
inlining) and kept `malloc` at cache-line offset 0 and `free` at 32. The
21-scenario screen against master (X26), 5 rounds, gave geomean +0.15%: no
scenario faster than -0.63% (`glibc-simple`), and training scenarios among the
slower ones (`sh6benchN` +1.73%, `cache-scratchN` +1.08%). The hot code is the
hand-written asm fast path, which PGO does not touch, and the Rust slow paths
are a small share of cycles.

BOLT reorders code to cut instruction-cache and iTLB misses. On master:
`glibc-simple` 0.002 L1 i-cache misses per 1000 instructions, `larsonN` 0.12,
`sh8benchN` 0.060, `xmalloc-testN` 0.069, and iTLB misses below 0.001 per 1000
instructions. `rptestN` has 3.5 per 1000, but 59% of them are in libc
(`pthread_enable_asynccancel`, `memset`) and 23% in the allocator, after its
threads sleep and wake. That predicted little, so BOLT was tried anyway at the
owner's request.

BOLT setup: `bolt-18` (Ubuntu, LLVM 18.1.3); the cdylib relinked with
`-Clink-arg=-Wl,--emit-relocs`; `perf record -j any,u` (AMD LBRv2) on the same
11 training scenarios, converted with `perf2bolt` and merged; `llvm-bolt
-reorder-blocks=ext-tsp -reorder-functions=cdsort -split-functions
-split-all-cold`. It reordered 87 functions (72.5% of those with profile),
split 25 KiB of hot code from 27 KiB cold, and cut taken branches in profiled
code by 83%. It also moved `free` from cache-line offset 32 to 0; a second
build with `-skip-funcs=free,_ZdlPvmSt11align_val_t,malloc,calloc` kept the asm
fast paths at their original addresses.

Screen against master, 5 rounds: geomean -0.52%; `sh6benchN` -3.57%,
`xmalloc-testN` -3.26%, `malloc-large` -2.06% (noisy), `mleak100` -1.54%,
`rptestN` -1.52% (noisy), `glibc-thread` +0.67%, `cache-scratchN` +0.83%,
others within +/-0.85%. The 10 held-out scenarios averaged -0.66% and the 11
training scenarios -0.38%, so the gain is not overfitting. Interleaved
Criterion runs (3 rounds) of the 6 scenarios that moved, against master:

| Scenario | BOLT | BOLT, asm fast paths kept |
|---|---|---|
| `xmalloc-testN` | -2.96% | -1.98% |
| `rptestN` | -2.02% | +0.33% |
| `mleak100` | -0.87% | -0.79% |
| `cache-scratchN` | -0.60% | -0.58% |
| `glibc-thread` | -0.27% | -0.08% |
| `sh6benchN` | +0.07% | -0.17% |

The screen's `sh6benchN` and `glibc-thread` moves were noise; the rest hold,
and these 6 were chosen because they moved, so the suite-wide gain looked
smaller, roughly 0.3 to 0.5% (the full held-out below measured -0.87%). Most of it comes from reordering the Rust slow paths;
`rptestN` gains only when `free` also moves to offset 0. Adopting BOLT would
need a relocation-preserving link, a stored profile regenerated as the code
changes, and `llvm-bolt` in the benchmark and release builds.

Since `rptestN` gained only when BOLT also moved `free`, master was rebuilt
with `free` at offset 0 and `malloc` at 0 (the 0x20 pad moved after `free`),
without BOLT. Screen, 5 rounds: geomean +0.19%; `sh6benchN` -2.05%,
`sh8benchN` -1.97%, `larsonN` +1.31%, `malloc-large` +1.23%, `alloc-testN`
+0.96%, `rptestN` +3.32% (noisy). Interleaved, 4 rounds: `rptestN` +1.10%,
`xmalloc-testN` -0.51%, `sh6benchN` -0.75%. That is layout reshuffling, not
`rptestN`'s BOLT gain; reverted, and the X09 layout (`malloc` 0, `free` 32)
stays.

Adopted by owner decision as an opt-in benchmark build. `bench.build()` reads
`MIMALLOC_BENCH_BOLT=1`: it links the Rust library with
`-Clink-arg=-Wl,--emit-relocs`, runs `llvm-bolt` with the checked-in profile
`benchmarks/bolt-profile.fdata` (122 functions profiled, 0 stale at
recording), records the BOLT version, profile hash and options in the build
cache key, and reports profiled and stale function counts in
`artifacts.json`. `python3 benchmarks/bolt.py profile` re-records the profile
from the 11 training scenarios. The default build is unchanged (same cache key).
The first integrated build left `malloc` at cache-line offset 24 and `free` at
28: BOLT pads a function to its 64-byte alignment only when that costs at most
32 bytes. `-align-functions-max-bytes=63` now always pads, which put `malloc`,
`free` and `calloc` at offset 0, the layout measured above. The C library and
the release artifacts are not BOLTed.

Held-out of that build against master (3 interleaved Criterion rounds per
scenario, all 21; training scenarios marked T):

| Scenario | BOLT vs master |
|---|---|
| `malloc-large` T | -10.09% (ranges disjoint: 682-695 us vs 759-772 us) |
| `xmalloc-testN` | -3.23% |
| `mleak100` | -1.36% |
| `rptestN` | -1.16% |
| `malloc-large-old` | -1.06% |
| `sh8benchN` | -0.96% |
| `mleak10` T | -0.69% |
| `larsonN-sized` | -0.33% |
| `cfrac` T | +0.44% |
| `mstressN` T | +0.31% |
| `glibc-thread` T, `larsonN` T | +0.28% |
| other 9 | -0.14% to +0.24% |
| geomean (21) | -0.87% (training -0.94%, held out -0.79%) |

`malloc-large` spends about 45% of its time in Rust arena bitmap searches,
which BOLT lays out from the profile; without it the other 20 average about
-0.4%. No scenario is worse than +0.44%, and the held-out scenarios gain as
much as the training ones, so the layout is not fitted to the training set.

## X28 — Cache of freed singleton pages (rejected)

Decided 2026-10-15. On the rivals scoreboard `malloc-large` is the widest gap,
3.07 times tcmalloc's time. Blocks above `MI_LARGE_MAX_OBJ_SIZE` (512 KiB) get
a singleton page carved from 64 KiB arena slices, and every allocate/free pair
of a 5 to 25 MiB block updates 80 to 400 slices: bitmap search and clear, the
committed and free bitmaps, purge scheduling with a clock read, page-map
registration, and (as in C, page.c:1036) abandoning the page as soon as it is
full and reclaiming it on free. The profile on master: bitmap operations about
63% of cycles, purge handling about 12%. The Nginx 1 MiB cases, which looked
related on the scoreboard, were not: measured on the server CPUs they ran at
jemalloc's throughput with 1 to 2 page faults per request, bound by kernel
networking.

Prototype: a freed main-heap singleton page reaching `try_free_abandoned` was
kept, registered and committed, in the freeing thread's theap (new
`CachedSingletonPage` capability and `AbandonedDisposition::Cached`), with
statistics as for a free; a singleton allocation took the smallest cached page
with at most 1/4 more slices than needed, reset its metadata as
`mi_arenas_page_alloc_fresh` does (including separate page metadata, which all
singleton pages use) and initialized it normally. Entries expired after the
arena purge delay (1000 ms by default), and theap collection, `mi_collect` and
thread exit released the cache. All gates passed.

Steady-state Criterion `malloc-large` (master 729 us), by slack, slots and
byte cap:

| Slack | Slots | Cap | Time |
|---|---|---|---|
| 1/4 | 4 | 64 MiB | 652 us |
| 1/2 | 4 | 64 MiB | 563 us |
| 1/1 | 4 | 64 MiB | 461 us |
| 1/2 | 8 | 128 MiB | 445 us |
| 1/4 | 8 | 128 MiB | 543 us |
| 1/4 | 8 | 256 MiB | 528 us |
| 1/8 | 8 | 256 MiB | 648 us |

With 1/4, 8 slots and 256 MiB, 46% of allocations hit (4 slots). The
21-scenario screen (short runs, first run discarded) then gave `malloc-large`
+144.7% in every round, `xmalloc-testN` +6.6% (every round), `sh6benchN`
+4.0%, others within +/-2.5%, geomean +4.90%. In one process, master's
`malloc-large` runs took 12.6 ms cold then 0.75 to 0.81 ms; the cache's took
11.0 ms then 3.2, 1.7, 1.0, 1.6, 0.9, 1.0 and 0.7 ms, with 66 more page faults
and 48% more kernel cycles over 8 runs. Holding up to 8 freed pages makes the
arena satisfy misses from slices nobody has touched, and the first write to a
block (the free-list link written by `free`) faults in a 2 MiB transparent
huge page that the kernel zeroes. In a real program the same holding shows up
as resident memory. Releasing the whole cache on a miss, so the arena reuses
those resident slices, removed most of the extra faults (29 remained) but also
the gain: steady state 763 us against 743 us. Nginx production (one sample per
case before the harness's PSS sampler lost an exited worker again; no crash in
the kernel or Nginx logs) moved within +/-1% except `upload-1048576` +18.7%,
a case that varied +/-13% between sessions before.

The arena's bitmap already reuses the most recently freed, still-resident
slices, so a cache beside it competes with that reuse; closing the gap would
take cheaper arena operations rather than a cache in front of them. Reverted;
the prototype diff is kept outside the repository.

## X29 — Transparent huge pages only for large heaps (adopted)

Decided 2026-10-15; the owner approved the divergence. With the speed
scoreboard won after X26 (native geomean 0.987 of C mimalloc, 0.586 of
tcmalloc; Nginx at parity once session drift is removed, the `headers` lead of
tcmalloc measuring 124,606 against 124,356 requests/s interleaved), memory is
where Rust and C mimalloc lose. On the axum server under load, Rust holds 19.9
MiB resident of which 16.0 MiB are anonymous huge pages, C the same, jemalloc
13.1 MiB and glibc about 7.5 MiB with none; with `MIMALLOC_ALLOW_THP=0` Rust
holds 7.9 MiB at -0.9% throughput. The host runs THP in `madvise` mode, C and
Rust advise every arena, and arenas start each thread's search in a different
32 MiB chunk, so a small multi-threaded server touches eight huge pages for
about 4 MiB of live data.

Turning huge pages off costs big heaps (screen against master: geomean +1.28%;
`malloc-large-old` +11.9%, `larsonN` +4.8%, `larsonN-sized` +4.6%, `barnes`
+4.2%, `mstressN` +4.1%, `sh6benchN` -4.3%). Peak resident memory of the
scenarios, including about 10 MiB of harness: `mstressN` 384 MiB, `sh6benchN`
218, `larsonN` 83, `rptestN` 80, `barnes` 44, `malloc-large-old` 12,
`glibc-simple` 11.

Policy: arenas are reserved without `MADV_HUGEPAGE`. A global count of arena
page bytes (added when a fresh arena page is allocated, removed when an arena
page is freed, no longer kept once armed) arms the request at 32 MiB: every
registered arena is advised, later reservations are advised by the OS layer,
and `MADV_COLLAPSE` turns what the arenas already hold into huge pages. Both
calls restore `errno`: the collapse fails with `EINVAL` on memory the
application gave the allocator, which the behavioral-parity gate caught as
`errno` 22 where C leaves 0.

Steps (screens against master, 5 rounds):

| Variant | Result |
|---|---|
| THP off | +1.28% (21) |
| 32 MiB threshold | +0.94% (21); `malloc-large-old` +12.3%, `larsonN` +2.1% |
| + arm at a page of 2 MiB or more, collapse | +0.38% (21); `sh8benchN` +3.5%, `malloc-large` +2.0% while the count was kept after arming |
| count stopped once armed, no collapse | +0.17% (9); held-out (21, 3 interleaved rounds) +0.17%, `sh6benchN` +3.68% |
| same, with collapse | +0.01% (9); held-out (9) within +/-0.8%, `malloc-large` +1.69% |
| page trigger removed (final) | +0.71% (21); `malloc-large-old` +12.03%, others within +/-1.3% |

Without the collapse, memory touched before arming stays on base pages:
`larsonN` held 42 MiB of huge pages against 76 MiB on master and `sh6benchN`
174 against 210; with it, 76 and 182 MiB.

The page trigger (arming at a page of 2 MiB or more) was dropped for Nginx. As
first written it also fired on the 4 MiB pages of the large size classes
(84 to 512 KiB blocks); restricted to blocks of 2 MiB or more it still armed
Nginx, which allocates a 3.5 MiB block when its heap holds about 6 MiB of
pages, and the collapse then filled sparse 2 MiB regions: worker PSS rose from
20.2 to 23.4 MiB (`download-4096`) and from 24.4 to 27.4 MiB
(`proxy-16384-on`), with 12 to 14 MiB of huge pages against master's 8 to 10.
Without the page trigger, `malloc-large-old` (one 2 MiB buffer written in full
5000 times; heap about 2 MiB) never arms: +11 to 13% in the screen's short
runs, but steady-state Criterion is unchanged (75.4 ms against 73.3 to 75.7 ms
on master), so the loss is faulting fresh buffers in 4 KiB pages. 16, 32 and
64 MiB thresholds gave the same Nginx and axum memory (neither reaches 16 MiB)
and screened alike on the 9 THP-sensitive scenarios (16 MiB +1.39%, 32 MiB
+1.19%, both from `malloc-large-old`); 32 MiB leaves more room for mid-size
processes.

Final policy, axum server (3 interleaved rounds, PSS under load; measured with
the policy before the collapse and page-trigger changes, which do not affect a
process that never arms; the final build measured 5.8, 6.1 and 7.8 MiB PSS
with no huge pages):

| Scenario | Requests/s | PSS master | PSS policy |
|---|---|---|---|
| json | +0.1% | 17.9 MiB | 5.9 MiB |
| plaintext | +3.5% (every round) | 17.8 MiB | 5.8 MiB |
| fortunes | +0.1% | 20.7 MiB | 7.7 MiB |

Nginx (single 5-second runs, master then 16/32/64 MiB in sequence): worker PSS
20.2 -> 14.9 MiB (`download-4096`), 24.4 -> 16.8 MiB (`proxy-16384-on`),
32.3-32.4 -> 25.0-25.3 MiB (`headers`, two runs), requests/s within 1.5% except
one 146k `headers` run (16 MiB) against about 124k for every other run.

Final screen (21 scenarios): geomean +0.71%; `malloc-large-old` +12.03%,
`rptestN` +1.23%, `sh6benchN` +0.97%, `alloc-test1` +0.83%, `larsonN-sized`
-1.23%, `alloc-testN` -0.76%, the rest within +/-0.65%. Gates passed.

## X30 — The main thread's heap was never initialized (fixed)

Decided 2026-10-15. Profiling Redis SET (where tcmalloc leads by about 3% in
the application suite and 9% in a direct `redis-benchmark -t set -P 16` run:
1.51-1.53M against 1.38-1.40M requests/s) showed the allocator at 20.8% of
server cycles against tcmalloc's 4.1%, 12.6% in `_mi_theap_page_reclaim`.
Counters in the library gave 2.45M owner reclaims for 3.2M SETs, all of small
pages moved to the full queue and abandoned. `MIMALLOC_PAGE_FULL_RETAIN=-1`
(which should disable abandonment) changed nothing, and a diagnostic print
showed the Redis main thread's theap with `page_full_retain` 2 and abandonment
enabled under any setting: the values of the `theap_main` static. Both C 3.3.2
and the translation declare it with a static memid, while
`mi_theap_main_init` initializes it only while the memid is not static, so the
options, the cookie and the guarded settings are never applied to the main
thread (its random state is reseeded by `_mi_random_reinit_if_weak` at process
start). X16's `page_full_retain` default of 0 therefore never reached main
threads; its evidence came from multi-threaded scenarios.

The fix starts `theap_main` with an empty memid. A test checks that after
process initialization the main theap's memid is static, its cookie nonzero and
its `page_full_retain` the option's value; it fails without the fix. Gates
passed. Screen against master: geomean +0.27%, every scenario within +/-1.4%
(`sh6benchN` +1.41%, `rptestN` +1.08%, `glibc-thread` +0.91%, all within the
screen's noise). Nginx (3 interleaved rounds per case, master library vs fix):
`download-4096` +0.6%, `proxy-16384-on` +0.8%, `headers` +0.5%,
`upload-65536` +1.0%, PSS equal. Redis SET with the fix: 1.395-1.397M at the
default `page_full_retain` 0, 1.466-1.481M with abandonment disabled (-1),
which X31 follows up.

## X31 — Full pages stay with a lone thread (adopted)

Decided 2026-10-15; the owner approved the divergence. With X30 applied, Redis SET under
`redis-benchmark -t set -d 256 -r 1000000 -c 50 -P 16` still spent 12.6% of
server cycles in `_mi_theap_page_reclaim`: 2.45M small pages moved to the full
queue and were abandoned (C's `mi_page_to_full`), and 2.45M owner frees into
them reclaimed them, for 3.2M SETs. Disabling abandonment
(`MIMALLOC_PAGE_FULL_RETAIN=-1`, effective on the main thread only after X30)
raised SET from 1.395-1.397M to 1.466-1.481M requests/s; tcmalloc did
1.51-1.53M in the same setup. Disabling it outright is X21's rejected change
(`xmalloc-testN` +318%: consumers' frees into producers' full pages were never
collected).

A process has no thread that could reclaim an abandoned page until a second
thread uses the allocator. `MI_PROCESS_MULTI_THREADED` (in `allocator/page.rs`)
is set when a non-main TLD is allocated and never cleared; until then
`mi_page_to_full` moves pages to the full queue as for non-abandoning heaps,
and `mi_free_generic_local` returns them with `_mi_page_unfull_internal`. Once
set, `mi_theap_abandon_kept_full_pages` (run with the other maintenance on the
generic path) abandons the theap's full-queue pages, which in a multi-threaded
process abandoning theap otherwise stays empty. C's assertion in
`_mi_page_unfull_internal` that only non-abandoning theaps have full pages
became a comment; with it the debug tests and the behavioral-parity binary
aborted. A test run in an isolated child process (where the library
constructor initializes the process main thread and libtest's body thread
counts as a second thread, so the test resets the flag first) checks that a
full page stays owned with one thread and is abandoned after another thread
allocates; it fails without the change. Gates passed.

Measurements (our library only; C, glibc, jemalloc and tcmalloc results are
reused): interleaved Redis SET A/B against master 1.404-1.413M ->
1.461-1.481M (+4.6%, every round). Application suite (`bench.py run
--allocator rust`, 3 repeats) against the stored suite of the same day: Redis
SET 1.741-1.753M (tcmalloc 1.751-1.762M, C 1.642-1.653M, Rust before 1.688-1.705M),
GET 2.245-2.255M (tcmalloc 2.090-2.105M); Redis 508.3 ns per request against
515.5 (Rust before), 522.0 (tcmalloc), 534.5 (C); RocksDB 1.250 us per write
against 1.259, 1.312 and 1.255. Native screen against master: geomean +0.15%,
every scenario within the screen's noise (`sh6benchN` +1.48%, `rptestN`
-1.51%, `malloc-large` -1.27%). Nginx (3 interleaved rounds, master library vs
candidate): `download-4096` +0.4%, `proxy-16384-on` +0.2%, `headers` +0.2%,
`upload-65536` +1.4%, PSS within 0.5 MiB. The axum server (3 interleaved
rounds, X29 library against master with X30 and X31; its tokio workers switch
X31 off at start) was unchanged: json +0.3%, plaintext +1.5% (+2.9, +1.5,
+1.2), fortunes -0.1%, PSS 6.0/6.1/7.6 MiB against 6.0/5.9/7.6 with no huge
pages in either.

## X32 — Full pages without queue links (rejected)

Decided 2026-10-15. After X31, Redis SET (`redis-benchmark -t set -d 256 -r
1000000 -c 50 -P 16`) matched tcmalloc's throughput while the allocator took
16.2% of server cycles against tcmalloc's 4.8% and jemalloc's 7.2%; Redis's own
code took 56% of cycles under Rust against 76% under tcmalloc, so the
allocator's placement makes the application cheaper while the allocator itself
costs more. `_mi_page_unfull_internal` alone took 10.6%: with about one
full/unfull transition per SET, each unfull relinks the page out of the full
queue and onto the end of its normal queue, touching neighbouring page headers.
Raising `page_full_retain` (keeping full pages in the normal queue) was worse:
1.47-1.48M requests/s at 0, 1.44-1.46M at 2, 1.43M at 8, 1.32-1.34M at 32.

Prototype (never on master): full pages counted in the full queue but not
linked, so moving to and from it touches no neighbours; whole-heap traversals
would have used the arenas' per-heap page bitmaps. Redis SET interleaved
against master (3 rounds): unfulled page pushed to the front of its queue
-8% (1.34-1.36M against 1.47-1.49M; the one free block is refilled at once and
the page cycles every SET), pushed to the end as in C +0.8% (1.461-1.483M
against 1.443-1.475M). A second variant kept the links but cleared the in-full
flag with a relaxed load and store instead of `lock btr` while the process is
single-threaded: +0.8% (rounds +1.8, -0.6, +0.6). IBS load sampling (`perf mem`)
showed the cost: the unfull path's loads of the page's `capacity` and
`block_size` hit only in L3. Redis frees old values spread over thousands of
pages whose metadata (in the arena's separate page-metadata array) does not
stay in L1/L2 next to the data, so each free into an old page misses on its
metadata, and the unfull accounting is where that miss lands. Neither the queue
links nor the atomic is the cost, so the full-page structure was left as it
is. Avoiding the metadata miss means not touching the page on free, as
tcmalloc's thread caches do.

IBS cycle sampling also put 9.4% of Redis's cycles in `strcasecmp` under Rust
(0.4% under tcmalloc). That is attribution, not a miss. Sampling by executed op
(`ibs_op/cnt_ctl=1/`) gives `strcasecmp` 1.11% of ops under Rust and 1.14%
under tcmalloc, and its loads have no measurable latency under either. In cycle
mode IBS tags the next op dispatched after its counter expires, so cycles spent
stalled behind an older miss land on whatever dispatches once the stall clears:
under tcmalloc the same window lands on `dictSdsKeyCaseCompare` (7.9% of
cycles), the command-table compare that calls `strcasecmp`, and on `siphash`
(17.4% of cycles for 6.3% of ops). Load latency summed over op samples is equal
(1,150 cycles per 1,000 ops under Rust, 1,144 under tcmalloc), as is the
allocator's share of executed ops (15.5% against 15.0%); the allocator's own
loads take 190 of those cycles against 32 (3.5 L3 hits per 1,000 ops against
0.5, about 3 per SET), and Redis's own loads 561 against 664. There is no
separate `strcasecmp` lever: its samples are stall cycles from older misses,
Redis's own (the window gets 7.9% under tcmalloc too) plus the allocator's
page-metadata misses above.

## X33 — Burst/idle retention observation

Completed 2026-10-22. **NO CHANGE to allocator policy.** This is the first
four-arm observation proposed in `REPORT.md`, using `08cf8a0d` plus the verified
hinted-mapping ownership fix. Frozen artifacts, source snapshots, the working
tree patch, commands, host state, phase events, and every OS sample are under
`target/perf-experiments/X33/`. SHA-256 prefixes: Rust `93b4d72fdb42`, C
`be00068c7a1c`, probe `617f18c1f7d7`; full hashes are in each manifest.

The host was an Azure Linux x86_64 VM with an EPYC 7763, 16 logical CPUs and
62 GiB RAM, no swap, and no observed memory pressure. Worker CPU 2 and sampler
CPU 4 are on different physical cores. This qualifies the observed residency
contrast, not small throughput differences or native M5 behavior. The native
release interpose libraries were loaded through explicit `mi_*` calls; Rust's
assembly allocation/free entries were verified. Harness storage used the
system allocator. Both owner and helper initialized before every baseline.
The defaults remained purge delay 1,000 ms, arena multiplier 1, THP 1, and
`page_full_retain` Rust 0 / C 2; the allocating owner was a worker thread.

Each fresh process allocated and wrote every byte of 64 MiB in 64-byte blocks,
verified and freed all blocks, then stayed idle for 10 seconds. At that point
it did nothing, collected on the owner, collected on the initialized helper,
or exited the owner while keeping the process alive. Another 10 seconds of
observation preceded an equal reburst/free and a final 10-second idle window.
The exit arm used a replacement owner. External `smaps_rollup` sampling targeted
100 ms (observed median 101.3 ms, maximum 120.9 ms). Five repetitions per
allocator/arm alternated allocator and action order: 40 primary processes.

Absolute process RSS at 20 seconds, median of five processes:

| Action at 10 seconds | Rust RSS MiB | C RSS MiB |
|---|---:|---:|
| Idle owner | 80.195 | 80.125 |
| Owner `mi_collect(true)` | 16.074 | 15.996 |
| Helper `mi_collect(true)` | 16.125 | 16.062 |
| Owner exit | 16.199 | 16.133 |

Preburst RSS was about 12.3 MiB for Rust and 16.0 MiB for C. Baseline-relative
idle excess was 67.891 / 64.078 MiB, respectively. Ten preceding Rust A/A
processes had exactly the same 20-second RSS excess and 98 reburst minor
faults each. The empty harness had zero anonymous-memory drift, 64 KiB RSS
drift, and 2 KiB PSS drift. All primary passive t90 endpoints were censored at
10 seconds; each collection/exit arm crossed the assisted t90 threshold by
roughly the first 100 ms sample, sustained for three periodic samples.

Separate syscall traces reproduced owner and helper collection for both
libraries. Each intervention made 17 successful `MADV_DONTNEED` calls:
64.125 MiB on the owner and 64.0625 MiB on the helper. The calling thread IDs
matched the intended owner/helper. The passive windows had no memory-management
syscalls, and every traced memory syscall parsed successfully. The 64 KiB
owner/helper difference is consistent with the sole retired page. The dominant
retention gate in this workload is scheduled shared-arena purging awaiting
execution; the recorded deadline does not schedule an independent worker.

Rust's lower preburst baseline had no anonymous huge pages, while C's already
had 4 MiB. Both had 68 MiB at the first burst peak. In four Rust owner-collect/
owner-exit runs, post-intervention idle RSS rose by 1.875 MiB together with
another 2 MiB anonymous huge page and no additional faults. This produced
Rust 20-second ranges of 16.039–17.902 MiB for owner collection and
16.168–18.078 MiB for exit. It is consistent with background THP promotion
after X29's one-way arming. A separately labelled, single-process-per-allocator
`MIMALLOC_ALLOW_THP=0` diagnostic left only 0.199 MiB above baseline after
owner collection in both libraries, with zero huge pages throughout.

That THP-off diagnostic recorded reburst times of 41.43 ms Rust / 40.55 ms C
and 16,417 minor faults each, versus default-policy owner-collection medians
14.82 / 14.07 ms and 114 / 113 faults. It is a mechanism control, not a policy
decision. A/A paired reburst timing differences ranged from -19.3% to +11.9%,
so these runs cannot establish the native-suite throughput budget.

Correctness checks: strict-warning C++ build; 71 benchmark Python tests completed
(63 passed, 8 optional integrations skipped), followed by all 9 real-library architectural
checks passing, including every retention action and the empty control. Full
payload verification and phase/ownership checks passed in every measured run.
No Rust production code changed in X33. `analysis.json`, `summary.csv`, both
absolute and baseline-relative retention figures, and reproducible analysis/
diagnostic scripts accompany the raw results. The next supported question is
the cost of existing purge policy or an explicit idle-boundary collection;
hot-loop owners, free-only consumers, sparse survivors, and other size classes
remain separate measurements.

## X34 — Collection at the idle boundary

Completed 2026-10-22. **NO CHANGE to allocator policy.** The question was whether
the existing owner `mi_collect(true)` call is useful immediately after a burst,
and how its cost changes with time until reuse. The experiment source, frozen
executables, commands, raw observations, analysis, and exported figures are in
`target/perf-experiments/X34/`. It reuses X33's exact Rust/C libraries (SHA-256
prefixes `93b4d72fdb42` / `be00068c7a1c`); the new probe is `44931d3884f2`.
No Rust source changed. The experiment files are local artifacts, and this
entry is the only tracked-file change made for X34.

The Linux EPYC VM and CPU placement match X33: worker CPU 2, observer CPU 4,
different physical cores, THP enabled, no observed memory pressure or major
faults. Default purge delay remained 1,000 ms and arena multiplier 1. Each
fresh process used one initialized allocating worker, with the main thread
waiting for it. It filled and verified every byte of 64 MiB in 64-byte blocks,
freed everything, optionally collected on that same worker, waited until a
fixed deadline measured from free completion, then allocated/touched the same
amount again and verified it. Collection consumes part of the quiet interval;
it does not move the scheduled resume deadline. Allocation timings include
payload writes, and the first 1 MiB is also timed separately. Collection time
is measured directly around the API call, without controller dispatch.

Seven fresh processes per allocator/policy/pause, with rotated policy/library
order and reversed pause order, produced 112 primary observations. The pauses
were 10, 100, 1,000, and 10,000 ms. Telemetry is buffered until all timed work
finishes. The probe records RSS/PSS/anonymous/huge-page checkpoints outside the
timed API calls; an external observer also samples every 20 ms. RSS averages
start after the collection/checkpoint, so they do not pretend to resolve the
sub-millisecond purge transition. Median checkpoint cost immediately before
resume was 0.354 ms, and median actual resume lateness was 0.452 ms. Both costs
are recorded separately. The additional timing control below removes them.

Median full refill-and-touch time, milliseconds (seven processes per cell):

| Pause | Rust retain | Rust collect | C retain | C collect |
|---|---:|---:|---:|---:|
| 10 ms | 9.070 | 11.075 | 9.076 | 11.491 |
| 100 ms | 9.050 | 10.855 | 9.155 | 11.044 |
| 1 s | 11.735 | 10.461 | 11.928 | 10.946 |
| 10 s | 12.225 | 10.030 | 12.401 | 9.902 |

Collection itself took a pooled median 0.344 ms Rust / 0.345 ms C. Including
that work, paired Rust collection-plus-refill **CPU-time** differences were
+2.43, +2.19, -0.49, and -1.32 ms for the four pauses, respectively. The C
differences were +2.39, +1.94, -0.34, and -2.05 ms. These are exploratory VM
measurements, not a native-suite throughput decision or a tail-latency bound.

Passive process RSS stayed around 80.1 MiB Rust / 80.0 MiB C. Immediate
collection reduced it to about 16.0 / 15.9 MiB. At 10 seconds it had risen to
17.9 / 17.8 MiB. The mean RSS saving after collection was about 64.1 MiB for
the three shorter pauses and 63 MiB for the ten-second pause. Preburst RSS
was 12.3 MiB Rust / 16.0 MiB C; these absolute comparisons avoid turning the
baseline difference into an apparent Rust retention regression.

The mechanism is visible in 12 separate syscall traces (both libraries,
both policies, pauses 10 ms / 1 s / 10 s). No `madvise` ran during the quiet
interval. Short passive reuse issued no discard calls during refill. Once
the one-second deadline had expired, passive Rust issued 17 `MADV_DONTNEED`
calls totaling 64.0625 MiB **during the first 1 MiB of refill**; C issued 49
calls totaling 63.9375 MiB in that same phase. With immediate collection,
both libraries instead issued 17 calls totaling 64.125 MiB before waiting
and none during refill. All those discard calls succeeded, with no unparsed
trace lines. The initial Rust burst's existing best-effort whole-arena
`MADV_COLLAPSE` returned `EINVAL` in all six Rust traces; those responses are
preserved separately from the successful discard operations. Both libraries
had 68 MiB of anonymous huge pages at the initial peak.

Short-pause Rust refill had a median 15 minor faults with retention versus
561 with collection; its first 1 MiB went from about 0.14–0.15 ms to
0.66–0.67 ms. At one second, passive Rust already incurred about 560 faults
and C 1,039; collection incurred 561 in both. At ten seconds, all 14 collected
processes gained exactly 1.875 MiB RSS and 2 MiB anonymous huge pages during
idle, with no discard calls there in the diagnostic traces. Refill then
incurred only 81 minor faults. Background huge-page promotion therefore
contributes to the long-pause result; the apparent timing crossover is not
a portable universal break-even threshold.

Controls and limits:

- Thirty Rust A/A processes (five pairs at each of 10 ms, 1 s, and 10 s)
  demonstrated timing noise: paired refill differences ranged from -10.2%
  to +38.0%. No outliers were removed. Medians and IQRs are exported alongside
  every raw process result; small allocator speed rankings are unsupported.
- Fifty-six additional processes removed all RSS checkpoints and external
  sampling (seven per library/policy at 10 ms and 1 s). Rust retain/collect
  refill medians were 9.10/11.64 ms at 10 ms and 11.36/10.55 ms at 1 s;
  C was 10.10/10.80 and 12.12/10.59 ms. Fault counts reproduced the primary
  pattern. This checks that observation overhead did not create that pattern.
- Eight separately labelled THP-off diagnostic processes used one observation
  per library/policy at 10 ms and 1 s. At 1 s, Rust retain/collect refills were
  44.35/35.98 ms and C 42.03/36.93 ms, with roughly 16,400 faults. Collection
  itself cost 7.84/5.96 ms. These single observations reinforce dependence on
  VM backing policy; they are not evidence for disabling THP.
- Only fully freed 64-byte objects, a 64 MiB burst, and one allocating owner
  were tested. Arrival times were fixed, with no memory pressure. Sparse
  survivors, concurrent owners, unpredictable arrivals, and helper/grace-period
  policies remain unmeasured.

The supported application choice is to preserve resident memory for brief
reuse and consider explicit owner collection at a known long idle boundary.
It returns memory sooner and moves due purge syscalls out of the next burst.
An unconditional call after every burst would impose these refault costs
even when reuse follows quickly. These results do not justify changing
the allocator's global delay or adding work to its allocation/free fast paths.

Validation: strict-warning C++ compilation; eight payload-verified smoke
processes; rejection checks for incomplete, reordered, invalid-counter, and
early-resume observations; 71 benchmark Python tests (63 passed, 8 optional
integrations skipped). All 206 primary/control processes and 12 traces
completed. Frozen input hashes matched, and replay from raw events/samples
exactly reproduced every saved measurement. Rust checks were not rerun because
no Rust code changed. `validation.json`, `analysis.json`, `summary.csv`, and
`idle-boundary.png`/`.svg` accompany the data and reproducible scripts.

## X35 — Free-only consumers and X31

Completed 2026-10-22; measurement only. Unlike X33/X34, the allocating owner
is the process main thread, and no allocating helper is introduced before
the designated initialization point. The consumer either only frees,
initializes after the burst but before freeing, or initializes before the
burst. It stays alive and inactive after freeing. System-allocator harness
storage and external OS sampling preserve these histories. The exact X33
Rust/C libraries and CPU placement were reused.

The primary matrix used 64 MiB in fully verified 64-byte blocks, three fresh
processes per history/library (18 processes), and a ten-second passive window.
Then the consumer initialized if necessary and forced collection, followed
by forced collection on the owner. Read-only owner queue/heap fields were
recorded at quiescent checkpoints; no heap visitor or collection ran during
the passive window. Median process RSS in MiB:

| Library / consumer history | Passive | Consumer collected | Owner collected |
|---|---:|---:|---:|
| Rust / free-only | 80.191 | 80.191 | 16.066 |
| Rust / init after burst | 80.160 | 80.160 | 16.035 |
| Rust / init before burst | 80.184 | 16.121 | 16.059 |
| C / free-only | 80.125 | 16.250 | 16.062 |
| C / init after burst | 80.125 | 16.188 | 16.000 |
| C / init before burst | 80.125 | 16.188 | 16.000 |

Rust's first two histories retained 1,025 full-queue pages after the burst
and after remote frees; initializing the consumer afterward did not flush
them. The before-burst history and every C history had zero pages in the
owner's full queue. Effective main-heap `page_full_retain` was Rust 0 / C 2.
A separate six-process control set Rust's option to 2 before library loading,
verified the effective value, and reproduced the retention contrast (1,023
full pages in the first two Rust histories). The C main-initialization
difference therefore does not explain this bulk effect.

A second matrix used four consecutive owner-allocate/consumer-free handoffs,
with one-second pauses and no intermediate collection, again three processes
per history/library. Every post-handoff logical live-byte count was zero.
Median post-free RSS for Rust's free-only history grew **80.195, 144.203,
208.211, 272.219 MiB**, with full-queue counts **1,025, 2,050, 3,075, 4,100**.
C's corresponding RSS remained 80.094–80.098 MiB. Final owner collection
reduced Rust to 15.906 MiB. Initializing the consumer after the first burst
allowed subsequent owner activity to flush the old full queue; its Rust RSS
sequence was 80.195, 108.195, 78.453, 80.195 MiB. Initialization before the
first burst stayed near 80.16 MiB throughout.

Four separate syscall traces corroborated ownership attribution. Rust's
free-only consumer collection issued no discard calls; owner collection then
issued 17 successful `MADV_DONTNEED` calls for 64.125 MiB. With consumer
initialization before the burst, consumer collection discarded 64.0625 MiB,
and owner collection discarded the remaining 64 KiB. C consumer collection
discarded the bulk in both traced histories. All discard calls succeeded and
all trace lines parsed.

This is recoverable owner-local retention caused by X31's progress condition:
a remote free does not itself initialize a non-main allocator TLD, while full
pages retained by the main owner require owner-side processing. Four cycles
demonstrate repeated growth, not an unlimited-duration bound. Explicit
collection is an effective workaround in this workload. Revising when X31
recognizes a second allocator participant merits a separate candidate and
native performance validation; no production policy was changed here.

Validation: strict-warning C++ builds; 12 small-payload smoke processes; full
payload verification and phase/live-byte checks in all 42 primary/control
processes; matched frozen hashes; four separate traces. Raw data, both probe
sources, commands, effective heap fields, and `analysis.json` are saved under
`target/perf-experiments/X35/`. These observations do not establish a throughput
regression or the frequency of genuinely free-only consumers in applications.

## X36 — X26 hot-loop maintenance progress

Completed 2026-10-22; no policy change. A worker initializes before the
baseline, excluding X31's single-main-thread state. It keeps a 64-byte anchor
live, warms that size class, then allocates, touches, verifies and frees a
separate 64 MiB cold burst in 8 KiB objects. The ten-second observation is
either idle, repeated allocation/free of one 64-byte object beside the
anchor, or allocation/verification/free of 2,048-object batches that reach
generic paths. All arms have the same preallocated system-allocator harness
storage. The hot page never becomes empty. The existing owner heartbeat and
generic counters are read at quiescent checkpoints; no callback, counter
instrumentation, or heap visitor was added to the allocator.

Three interleaved fresh processes per library/arm (18 primary runs) used the
unchanged X33 libraries and the same CPU placement. Median results:

| Library / activity | RSS after 10 s, MiB | Heartbeat advance | Allocation/free pairs | Sustained RSS t90 |
|---|---:|---:|---:|---:|
| Rust / idle | 72.270 | 0 | 0 | >10 s |
| Rust / hot loop | 72.203 | 0 | 1,746,987,008 | >10 s |
| Rust / generic batches | 9.910 | 2,426 | 265,691,136 | 1.071 s |
| C / idle | 72.137 | 0 | 0 | >10 s |
| C / hot loop | 9.855 | 25,640 | 1,468,465,152 | 1.091 s |
| C / generic batches | 9.855 | 2,582 | 267,063,296 | 1.089 s |

All three repeats in every idle/Rust-hot cell were censored at ten seconds;
all repeats in the other cells reached t90, sustained for three periodic
100 ms samples. Forced owner collection afterward reduced Rust hot-loop RSS
from 72.203 to 8.203 MiB. The roughly 1.9 MiB variability in already-drained
cells is consistent with the THP effects observed in X33/X34. RSS here includes
smaller pointer storage than those 64-byte-burst experiments, so absolute
totals should not be directly compared across probes.

Four separate two-second syscall traces confirmed that Rust's hot activity
issued no discard calls before the final forced collection, whereas C's hot
activity and both generic-batch arms issued successful discard calls during
activity. The existing code's heartbeat changes corroborate execution of
maintenance; Rust hot-loop reuse made no such progress despite substantial
allocator activity. This is the accepted X26 LIFO cadence tradeoff, distinct
from the shared completely-idle purge limitation. Lowering a positive purge
delay or changing a generic-path counter cannot cause progress in a path
that never enters generic allocation.

The mixed-work arms establish progress, not comparable throughput scores:
generic batches perform different work and have a transient 128 KiB live
set. No new heartbeat on the free fast path is proposed; X23's rejected cost
still applies. Validation included strict C++ compilation, six small smoke
processes, full cold/batch payload checks and anchor verification, phase/live
checks in all primary runs, frozen hashes, and separately parsed syscall
traces. Raw samples, counters, `summary.csv`, and reproducible analysis are
under `target/perf-experiments/X36/`.

## X37 — Sparse survivors and THP history

Completed 2026-10-22; no policy change. A worker uses a private logical heap
and the unchanged X33 libraries. A 64 MiB burst is fully touched and verified.
Both packed and scattered cases retain exactly 512 objects; the packed case
takes the first 512 blocks from visitor page order, and the scattered case
takes one block from each of 512 distinct allocation pages. Two visitor passes
before any frees establish real page membership and apply identical
preconditioning. After freeing the other objects, the owner forces heap
collection. The visitor is not called during passive observation; a final
visit afterward verifies live counts and occupied-page counts.

Controls free every object, or allocate only the same 512 survivors without
ever having a large heap. System-allocator pointer storage and the page-group
scratch space are reserved before the baseline in every case. The primary
matrix has three processes per library, size (64 B / 8 KiB), and case
(empty / packed / scattered / never-large): 48 processes, with two seconds
of observation after collection. A further 48 THP-off controls use the same
cases and three repetitions, with 100 ms observation; twelve additional
64-byte packed/never-large runs observe ten seconds. Each published cell
therefore has three independent process repetitions.

Median process RSS immediately after forced collection, MiB:

| Object size / library | Empty burst | Packed | Scattered | Never large |
|---|---:|---:|---:|---:|
| 64 B / Rust | 18.098 | 18.156 | 50.145 | 12.668 |
| 64 B / C | 18.012 | 18.039 | 50.012 | 16.125 |
| 8 KiB / Rust | 10.324 | 14.324 | 42.316 | 8.668 |
| 8 KiB / C | 10.164 | 14.195 | 42.195 | 14.184 |

For 64-byte objects the survivor payload is just 32 KiB: packed survivors
occupy one allocator page and scattered survivors occupy 512. Their paired
RSS difference is about 31.95 MiB in both libraries. For 8 KiB objects the
survivor payload is 4 MiB: packed survivors occupy 64 pages and scattered
survivors occupy 512; the paired RSS difference is about 28 MiB. With THP
disabled these differences remain about 31.94–31.98 and 27.98–28.02 MiB,
respectively. Thus the dominant scattered-survivor cost is occupied allocator
pages, not the purge deadline or THP. A more aggressive idle collection
cannot release these live pages under the current design.

Six separate 64-byte syscall traces verified that collection discarded
64.0625 MiB in the packed cases versus 32.125 MiB in the scattered cases,
for both libraries; the never-large cases had no discardable payload pages
at that checkpoint. All discard calls succeeded. Releasing the final
survivors subsequently removed the placement-dependent difference.

There is a separate history effect. Paired Rust packed-minus-never-large
RSS immediately after collection was 5.52 MiB at 64 B and 5.62 MiB at 8 KiB.
The ten-second 64-byte control showed a paired 7.09 MiB difference after
background huge-page promotion. With THP disabled, the corresponding
immediate differences were only 0.15 and 0.11 MiB. C also has history effects:
its 64-byte difference was about 1.94 MiB immediately and 3.57 MiB at ten
seconds, whereas its 8 KiB never-large case already had nearly the same RSS
as its previously-large packed case. This is consistent with X29 preserving
a low footprint for never-armed Rust heaps and losing that advantage after
arming; it is not evidence of an absolute Rust/C post-burst RSS regression.

Private-heap metadata contributes to the remaining baseline: deleting the
empty logical heap after the final cleanup releases roughly another 2 MiB
in the default-policy burst cases. The empty control and THP-off comparison
keep that contribution distinct from survivor placement. The observed
post-sleep RSS-only warmup is also small relative to the reported MiB-scale
contrasts. No de-arming mechanism or live-object relocation is proposed here.

Validation: strict-warning C++ build; 16 short full-payload smoke processes;
108 primary/control processes with payload, live-count, actual page-membership,
phase and idle-duration checks; frozen hash verification; six parsed syscall
traces. Raw results, `summary.csv`, paired contrasts, effective heap fields,
and probe/analysis sources are under `target/perf-experiments/X37/`.

## X38 — Irregular bursts and collection grace periods

Completed 2026-10-22; application-policy simulation only. The initialized
worker and frozen X33 libraries perform a warm-up burst followed by 32
measured 64 MiB bursts in 64-byte objects, verifying every byte before each
free. Each next arrival is scheduled relative to completion of the previous
free. A seeded shuffle uses twelve 10 ms gaps, six 100 ms, four 100.25 ms,
four 500 ms, three 1,000.25 ms, two 1,500 ms, and one 5,000 ms gap. Every
library/policy pair sees the same sequence within a repetition. Policies
retain passively, collect immediately, or collect after 100 ms / 1 s of
continued idleness. A timer that wakes after an arrival cancels collection;
collection already in progress is allowed to finish and its delay is charged
to that arrival.

The latency proxy is completion of the first synthetic 1 MiB allocation-and-
touch request, measured from its scheduled arrival. It includes wakeup delay,
any overlapping collection, and allocation/page-fault work. It is not an
application request/SLO measurement. All round telemetry is buffered until
the entire sequence finishes. External RSS sampling has a median interval
of 21.06 ms and median read cost 0.593 ms; average RSS is an integrated
process-wide estimate over both bursts and gaps, not a precise measurement
of each individual 10 ms pause. Allocator CPU includes collection, full
refill/touch, and freeing; correctness verification is excluded from that
CPU metric but included in the process residency interval.

Five independent processes per library/policy give 40 primary processes and
160 measured arrivals per cell. RSS and CPU columns are medians of process
metrics; latency percentiles pool the 160 arrivals and are descriptive.

| Library / policy | Mean RSS MiB | Allocator CPU ms/burst | First-request p50 ms | First-request p95 ms | Collections / 32 bursts |
|---|---:|---:|---:|---:|---:|
| Rust / passive | 101.93 | 14.462 | 0.263 | 2.172 | 0 |
| Rust / immediate | 21.35 | 16.129 | 0.725 | 0.973 | 32 |
| Rust / 100 ms grace | 33.27 | 14.497 | 0.286 | 1.205 | 14 |
| Rust / 1 s grace | 73.12 | 14.314 | 0.260 | 1.690 | 6 |
| C / passive | 101.85 | 14.581 | 0.272 | 2.010 | 0 |
| C / immediate | 20.93 | 15.960 | 0.717 | 0.830 | 32 |
| C / 100 ms grace | 32.97 | 14.593 | 0.275 | 1.138 | 14 |
| C / 1 s grace | 72.51 | 13.935 | 0.258 | 1.621 | 6 |

Immediate collection trades a higher median and CPU cost for low residency
and a lower observed tail in this mix. The 100 ms grace gives a substantial
memory reduction with similar measured allocator CPU, but aggregate
percentiles hide a boundary cost. For Rust arrivals at 100.25 ms, passive
median completion was 0.260 ms versus 1.180 ms under 100 ms grace; the
conditional empirical p95 was 0.294 versus 2.599 ms (20 arrivals per policy).
All twenty such grace-period arrivals overlapped collection. All fifteen
arrivals at 1,000.25 ms likewise overlapped the 1 s collection. Maximum
collection overruns past scheduled arrival were 1.156 ms / 1.492 ms for
Rust's 100 ms / 1 s policies, and 0.696 ms / 1.282 ms for C. These are
observations, not upper bounds.

Ten additional Rust processes disabled RSS sampling entirely. The initial
one-per-policy control reproduced all four near-100 ms and three near-1 s
collection overlaps. One 1 s-grace CPU result was high (17.53 ms/burst), so
three further interleaved passive/grace pairs were run. Their CPU results
were 14.10–14.38 ms/burst passive and 13.68–14.26 ms/burst with 1 s grace,
and all timer-overlap cases reproduced. The original high result remains
in the data. These controls support the mechanisms while reinforcing the
existing warning against small VM timing rankings or a native performance
KEEP decision.

The result supports choosing a grace period against an application's actual
arrival distribution and latency requirements. It does not establish 100 ms
as a universal default. The prior single-burst recommendation remains bounded:
known long idle intervals are useful places to collect, while a timer on the
request-owning thread can overlap an unexpected arrival. This experiment has
no survivors, no competing owners, and no memory pressure; it does not remove
the ownership and page-pinning limitations demonstrated in X35–X37.

Validation: strict C++ build; eight scaled smoke processes with 33 verified
bursts each; invalid phase/time/policy/gap records rejected; all 50 primary/
control processes completed (1,600 measured bursts, plus warm-ups). Frozen
hashes and identical per-repeat gap sequences matched; raw replay exactly
reproduced every saved metric. `analysis.json`, `summary.csv`, by-gap latency
results, `validation.json`, and `grace-periods.png`/`.svg` are under
`target/perf-experiments/X38/`. No allocator policy or fast path changed.

## X39 — Secure-release decommit protection

**Result: confirmed translation defect, repaired.** Rust's Unix decommit branch
tested only `MI_DEBUG == 0`; C also requires `MI_SECURE <= 2` before leaving
decommitted memory accessible. A release build with Rust's `secure` feature
(`MI_SECURE=4`) therefore discarded the contents but left the mapping writable
and returned `needs_recommit=false`.

The maintained `tests/unix/decommit.c` fixture allocates and touches an owned
four-page primitive mapping, decommits it, checks the complete mapping's Linux
permissions, and tests access in an isolated child. It then recommits, verifies
zero contents and writable protection, and frees the mapping. This is an owned
primitive protection test, not a use-after-free application probe. An initialized
worker also verifies three allocation/free/collection/reuse cycles of 128
32 KiB blocks, including all payload bytes.

| Release mode | Library/state | `needs_recommit` | Decommitted permissions | Child write |
|---|---|---:|---|---|
| default | C and Rust, before/after | false | `rw-p` | succeeds |
| secure 4 | C | true | `---p` | SIGSEGV |
| secure 4 | Rust before | false | `rw-p` | succeeds — regression fails |
| secure 4 | Rust after | true | `---p` | SIGSEGV — regression passes |

The production repair adds the missing secure-level condition. It adds no unsafe
site and does not alter the ordinary release branch. The existing Linux syscall
driver now runs this fixture against optimized default and secure-release C and
Rust builds. C-linked Rust release archives disable LTO to supply native objects;
the first link failure with fat-LTO objects is retained with the experiment.

Validation: the new fixture fails against the frozen old Rust secure archive
and passes against the repaired one; recommit, zeroing, and worker reuse pass
in all four final combinations. Existing hinted-mapping ownership checks and
all 11 normalized Unix syscall classes still pass. Secure/debug hardening parity,
strict C compilation (`-Wall -Wextra -Werror`), `cargo fmt --all`, workspace /
all-target / all-feature Clippy, and the corresponding complete Rust tests pass.
The syscall driver was rerun successfully after the subsequent X40 repair.

Pre-fix and post-fix executables, source snapshots, SHA-256 hashes, failing and
passing outputs, and Rust gate logs are under `target/perf-experiments/X39/`.

## X40 — Explicit teardown leaves a stale destructor pointer

**Result: reproduced, classified as inherited from C 3.3.2, and repaired.** The
original E02R binaries and exact invocation could not be recovered. The current
maintained sustained probe from `scripts/check-release-thread-stress.py`, built
with TSan, reproduces the named `_mi_theap_create` / `_mi_thread_done` boundary
in all three runs of both current Rust and vendored C. This closes the finding
with a defect and regression evidence, not as a stale non-reproduction.

### Mechanism and repair

`mi_thread_done` publishes the empty default heap and releases thread-heap
metadata. In both implementations the default setter updates the primitive
destructor association only for an initialized heap, so the pthread key still
contains the freed heap address. At actual thread exit, `mi_pthread_done` passes
that address to `_mi_thread_done`. Another thread can already be initializing
new metadata at the same address.

Rust reports the new owner's writes to `tld` and `heap` in
`_mi_theap_init_typed` against the exiting thread's `ThreadHeapPtr::state` reads
through `mi_pthread_done` and `done_typed`. C reports the initialization memcpy
against `_mi_thread_done`'s atomic heap read. List locks and metadata allocation
synchronization do not protect a destructor using a pointer after ownership
ended. An atomic heap read does not make this lifetime violation safe.

The repair clears the primitive destructor association at the start of
`theaps_done`, before releasing any metadata. Later initialization registers its
new live heap. No new lock, atomic ordering, unsafe block, or allocator policy
is introduced. A diagnostic copy of C with the same one-line association clear
also passes three sustained TSan runs; vendored C is unchanged.

The deterministic Unix unit regression checks the pthread key after each of
three explicit teardown/reinitialization cycles. It fails before the repair and
passes afterward, inspecting only pointer values. The maintained
`tests/thread_lifecycle.rs` runs eight workers through 256 init/allocate/touch/
free/done cycles each, mixing explicit teardown at exit with reinitialization
followed by automatic teardown. It also fails under release TSan before the
repair. Both sanitizer suites now include it.

The first minimized probe reinitialized **every** worker before exit and passed
ten debug and ten release runs. That variant overwrote the stale association
and missed the defect. Those passes are retained but are not the closure
evidence; the mixed-exit regression and sustained probe are.

### Reproduction configuration

Evidence is Linux x86_64 GNU, source `08cf8a0d` plus the recorded working-tree
repairs, rustc nightly `1.101.0-nightly (5d8fec066 2026-10-16)`, LLVM 23.1.4,
Clang 23.1.2, and the host/environment in `X40/manifest.json`. No TSan
suppression or `TSAN_OPTIONS` override was used. Current nightly makes the
repository's CPUID unsafe block unnecessary; `-Aunused-unsafe` allows that
unrelated lint in these sanitizer builds. The initial unmodified command's
compile failure is retained. Ordinary Rust verification uses pinned 1.92.0.

The optimized minimized test command is:

```sh
RUSTFLAGS='-Aunused-unsafe --cfg mi_sanitizer_runtime -Zsanitizer=thread -Zunstable-options -Cpanic=immediate-abort -C lto=off -C linker-plugin-lto=no -C embed-bitcode=no' \
  cargo +nightly test -Zbuild-std -Zpanic-abort-tests --release \
  --target-dir target/sanitizer-thread-release --target x86_64-unknown-linux-gnu \
  --features std --test thread_lifecycle
```

The sustained C-ABI Rust archive uses:

```sh
RUSTFLAGS='-Aunused-unsafe --cfg mi_sanitizer_runtime -Zsanitizer=thread -Zunstable-options -Cpanic=immediate-abort -C lto=off -C linker-plugin-lto=no -C embed-bitcode=no -C debuginfo=2' \
  cargo +nightly rustc -Zbuild-std=core,alloc --release --features c-abi --lib \
  --target-dir target/perf-experiments/X40/tsan-c-abi \
  --target x86_64-unknown-linux-gnu --crate-type staticlib
```

`X40/sustained.c` is the unchanged `PROBE` from the maintained release-thread
stress script. It is linked with that archive using
`clang -fsanitize=thread -O2 -g -std=c11 -DNDEBUG -DMI_DEBUG=0`, the vendored
include directory, and `-pthread -ldl -lm`. The C comparison links the same
probe with `mimalloc-3.3.2/src/static.c` and the same flags, adding its source
include directory. Do **not** define `MI_MALLOC_OVERRIDE=0`: C checks whether
the macro is defined, and that still overrides the sanitizer's allocator.
The first three C startup crashes from that configuration are retained and
excluded from the three completed reproductions. Rust's
`mi_sanitizer_runtime` cfg disables conflicting allocator interposition.

### Results and verification

| Probe | Before | After |
|---|---|---|
| Sustained Rust release C ABI | 3/3 fail, two race reports each | 10/10 clean |
| Sustained C 3.3.2 | 3/3 fail, one race report each | Diagnostic association-clear copy: 3/3 clean |
| Minimized mixed-exit Rust, release TSan | fails with race reports | 10/10 clean |
| Direct pthread-key regression | fails | passes |

After the repair, `cargo fmt --all`, workspace / all-target / all-feature
Clippy and tests pass. The latter includes all six Loom models. Release C-ABI
thread stress, complete behavioral parity, maintained debug TSan and ASan
suites, and the X39 Unix syscall/protection matrix pass. Valgrind thread stress
reports zero errors and zero leaks. The tracking-parity driver initially used
a shared-target archive whose tracking shims were inactive; a fresh isolated
archive passes all five C/Rust application classifications. The driver now
uses a dedicated target directory and itself passes. The raw Valgrind outputs
are retained; matching application classifications is distinct from the zero-
error result of the thread-stress run.

A fresh ordinary `interpose` artifact retains the same native malloc/free
symbol addresses and sizes (81/162 bytes) as the frozen X33 artifact, and
normalized disassembly matches for both. This is a cold lifecycle/protection
repair, not a measured throughput improvement. No platform beyond this Linux
host is claimed as runtime-validated.

`X40/manifest.json` records source/probe/binary hashes and toolchains. Failing
and passing probes, all raw sanitizer and gate logs, the diagnostic C source,
and the assembly comparison remain under `target/perf-experiments/X40/`.

## X41 — Fresh baseline before the retention follow-ups

Completed 2026-10-22. Baseline `a3d20edd` was committed and pushed before this
round. Release interpose Rust and bundled C 3.3.2 were rebuilt with the same
benchmark driver and frozen with the native executable. The six paired rounds
ran all 21 workloads in fresh processes, alternating allocator order and
shuffling scenario order. Affinity used the eight physical cores (logical CPUs
0,2,4,6,8,10,12,14) of this Linux x86_64 EPYC 7763 VM. Builds and tests were
paused during measurement. THP was in `madvise` mode.

Short workloads used one warmup and three samples per process. The longer
`cfrac`/`rptestN` workloads used one invocation per independent process; the
three timed workloads used two seconds. Ratios below are medians of the six
paired normalized-time ratios. Negative is less time for Rust. These are
exploratory workload results, not application-wide throughput guarantees.

| Workload | Rust / C time difference | Paired verdict |
|---|---:|---|
| `glibc-simple` | -2.16% | faster |
| `malloc-large` | -5.52% | faster |
| `malloc-large-old` | +0.05% | ~ |
| `cache-scratch1` | +0.01% | ~ |
| `cache-thrash1` | +0.02% | ~ |
| `cache-scratchN` | +0.05% | ~ |
| `cache-thrashN` | +0.02% | ~ |
| `mstressN` | -3.67% | faster |
| `sh6benchN` | -0.96% | likely faster |
| `sh8benchN` | +3.73% | SLOWER |
| `alloc-test1` | -0.55% | ~ |
| `alloc-testN` | -0.84% | likely faster |
| `barnes` | -0.05% | ~ |
| `cfrac` | +0.48% | ~ |
| `mleak10` | +1.52% | ~ |
| `mleak100` | +1.36% | ~ |
| `glibc-thread` | -2.24% | faster |
| `xmalloc-testN` | +3.16% | SLOWER |
| `larsonN` | -5.01% | ~ |
| `larsonN-sized` | -12.94% | faster |
| `rptestN` | +6.48% | SLOWER |

The unweighted geometric-mean time ratio was 0.99112
(-0.89%). `sh8benchN`, `xmalloc-testN`, and `rptestN` supply the clearest
remaining deficits; `larsonN` changes direction in the last two pairs, so its
median alone is not sufficient evidence of a win. Hardware performance events
are unavailable on this host (`perf_event_paranoid=4`); this round makes no
hardware-counter claims.

`target/perf-experiments/X41/rust-vs-c-v2/` preserves commands, library and
executable hashes, every raw timing, process CPU/fault/RSS/context-switch
resources, host controls, paired ratios, and bootstrap intervals. An earlier
incomplete run was retained separately after reducing redundant repetitions
of the long workloads; it is excluded from these results.

## X42 — Free-only consumers participate in retained-page maintenance

Kept on the follow-up branch, 2026-10-22. X31 treated non-main allocator TLD creation
as evidence of multiple participants. A consumer that only freed allocations
could leave that flag false indefinitely, so each producer burst added another
set of full pages to the producer's retained queue.

The fix marks participation in two existing slow paths: publication into an
owned page through `mi_free_block_mt`, and collection of a nonempty remote
list by its local owner. The second path covers normal native assembly frees,
including publication racing the owner's transition to a full page. Neither
path initializes the consumer's allocator state or mutates a foreign owner's
queues. The flag remains a relaxed, one-way policy signal. Local allocation
and free assembly is unchanged; both frozen libraries have `mi_free` at
`0x125e0` (162 bytes) and `mi_malloc` at `0x12bc0` (81 bytes).

A new isolated Rust regression failed at the missing participation assertion
on the old source and passes with the fix. A system-allocated pthread/dlopen
fixture independently failed against the frozen old Rust library in both
full-page and normal native remote-free cases; bundled C passed both. The
candidate passes both with the portable C ABI and native interpose artifact.
The normal case checks recognition before any full-page remote free can mask
an omitted native path. The release-thread stress driver now runs these cases,
and both Linux x86_64 and aarch64 CI jobs invoke it.

The memory comparison reused the X35 producer/consumer protocol with twelve
64 MiB, 64-byte-object bursts, 300 ms idle per burst, three fresh processes per
library/history, alternating library order, full payload verification, external
100 ms memory sampling, and quiescent owner-queue reads. All 27 processes
completed; requested live bytes were zero at every idle checkpoint. Median
whole-process RSS for the free-only history:

| Library | First idle, MiB | Twelfth idle, MiB | Full pages at twelfth idle | After owner collect, MiB |
|---|---:|---:|---:|---:|
| Old Rust | 80.195 | 784.281 | 12,300 | 15.469 |
| Candidate Rust | 80.160 | 108.164 | 0 | 14.227 |
| Bundled C | 80.133 | 108.137 | 0 | 14.199 |

Old Rust added 1,025 retained full pages per burst in every repeat. The
candidate's second burst drained the old queue in every repeat, and later
idle RSS stayed at or below 108.200 MiB in this experiment. The initialized
consumer controls remained approximately 108.1–108.2 MiB at the twelfth idle
for all libraries. The first idle still retains the initial burst: later
owner maintenance or explicit owner collection is required. This experiment
establishes the observed twelve-burst behavior, not a general memory cap.

Validation passed: `cargo fmt --all`,
`cargo clippy --workspace --all-targets --all-features`, the isolated regression,
and `scripts/check-release-thread-stress.py`. The all-target test command
passed with `MIMALLOC_BENCH_SCENARIOS=cache-scratch1`, running all unit,
integration, example, and other benchmark targets while selecting one native
benchmark smoke case. The earlier unrestricted invocation passed its unit and
integration tests and was interrupted in the expensive debug native benchmark;
the complete 21-workload performance comparison is run separately in release
mode. Initial test-fixture compilation errors were corrected before the
recorded pre-fix failure and post-fix pass.

Frozen libraries, native layout, pre-fix failures, passing checks, all phase
records, samples, and repeat summaries are under
`target/perf-experiments/X42/`.

The first placement was screened against frozen pre-fix Rust in six balanced
pairs across all 21 native workloads, with three-second timed workloads and
one invocation per process. Its geometric mean time ratio was +1.07%.
Warm focused repeats resolved the short, cold `malloc-large` difference
(+0.33% across eight pairs), but Larson and rptest had positive, noisy medians.
The owner-side marker was therefore moved before remote-list validation,
where a uniquely detached nonempty list already proves remote activity. This
reduced collection-routine growth from 78 to 48 bytes without changing the
native malloc/free entrypoints. Invalid lists still follow the existing error
path; the policy flag can conservatively become true before that error.

Six further balanced old-Rust/new-Rust pairs used the final placement,
five-second timed workloads, and one warmup plus three measured invocations
for the short cases. Median candidate time differences:

| Workload | Final placement versus pre-fix Rust |
|---|---:|
| Larson | −1.75% |
| rptest | −0.04% |
| xmalloc | +1.64% |
| glibc-simple | +0.13% |
| sh8 | −0.92% |

The five-case geometric mean was −0.20%. Threaded results remain variable:
Larson pairs ranged from −11.4% to +13.4%, and sh8 from −5.2% to +11.0%.
Five of six xmalloc pairs were slower, so the +1.64% median is retained as a
possible cost, not dismissed as a demonstrated zero-cost change. The clear
retention/progress benefit justifies keeping this small policy correction;
these measurements do not establish a general throughput improvement.

A held-out Redis 6.2.7 check of the first placement used six balanced pairs,
3.2 million SET/GET operations, 256-byte values, 50 connections, pipeline 16,
disjoint server/client physical cores, and an allocator-ownership check.
Median throughput ratios were +1.33% SET and +0.35% GET, with mixed pair
signs. This did not reveal a repeatable application regression and is not
claimed as an application speedup. The final placement also passed both
native free-only cases and a fresh twelve-burst old/new Rust confirmation.
Final aggregate Rust and sanitizer checks are recorded with the completed
follow-up results below.

## X43 — Owner idle collection with an actual producer queue

Kept as an opt-in application pattern, 2026-10-22. A system-allocated producer
thread enqueues work after the previous job completes; an allocator-owning
worker waits on a condition variable. This replaces X38's single-thread
arrival simulation with actual enqueue/wakeup and collection overlap. Four
fixed-seed orders contain 32 gaps each: twelve 10 ms, six 100 ms, four
100.25 ms, four 500 ms, three 1,000.25 ms, two 1.5 s, and one 5 s.
Every job allocates, touches, verifies, and frees 64 MiB of 64-byte objects.
The producer cannot tell the owner a future arrival time. A timeout collects
once, then waits for the next job, including jobs arriving during collection.

All sixteen fresh processes used only final-placement Rust from X42. Policy
order rotates by repeat; owner, producer, and external sampler use separate
physical cores 2, 4, and 6. Sampling is every 20 ms. Mean RSS is integrated
over the sequence and includes the system-allocated pointer array and harness.
CPU covers refill, free, and collection, excluding payload verification.
The latency measure ends after the first 1 MiB of each refill; it is not a
complete application-request latency or an HTTP benchmark.

| Policy | Mean RSS, MiB | Allocator CPU, ms/burst | First 1 MiB median, ms | First 1 MiB p95, ms | Collections per 32 jobs |
|---|---:|---:|---:|---:|---:|
| Passive | 101.866 | 14.909 | 0.227 | 2.077 | 0 |
| Immediate forced collection | 21.264 | 16.458 | 0.756 | 0.938 | 32 |
| 100 ms idle grace | 33.003 | 15.070 | 0.249 | 1.202 | 14.5 |
| 1 s idle grace | 72.565 | 14.382 | 0.224 | 1.520 | 6 |

RSS, CPU, and call counts are medians of the four process results; response
quantiles pool 128 bursts per policy. In this sequence the 100 ms grace reduced
mean RSS by 67.6%, with an observed 1.1% higher median CPU cost. Immediate
collection reduced residency further but raised median CPU by 10.4% and
increased the median first-refill response. These are workload-specific
tradeoffs, not universal timeout recommendations or statistically established
small CPU improvements. There were 18 collection/arrival overlaps with the
100 ms grace and 12 with the 1 s grace across all repeats. Maximum measured
collection overruns past actual enqueue were 0.759 ms and 0.944 ms.

`examples/worker_maintenance.rs` demonstrates owner-side idle collection
using the existing raw API, with a single collection per idle interval.
Applications should choose their grace from their own idle gaps, memory
budget, and response-time requirements. No real application arrival trace
was supplied, and the existing small-footprint web-server benchmark does not
justify imposing this policy on every request. Source, frozen library/hashes,
actual arrival timestamps, samples, payload checks, and summaries are under
`target/perf-experiments/X43/worker-primary/`.

A two-seed control repeated passive and 100 ms grace without the sampler.
Median CPU was 14.921 / 15.030 ms per burst, versus 14.997 / 15.047 ms for
the same two sampled seeds. Arrival overlaps remained present (ten across
the two grace runs). This supports the qualitative tradeoff without treating
the roughly one-percent CPU difference as a precise universal cost. Control
records are under `target/perf-experiments/X43/worker-no-sampling/`.

## X44 — Separate temporary allocations from sparse cache survivors

Kept as application guidance using existing heap ownership, 2026-10-22.
Six balanced pairs used only final X42 Rust. Both policies made exactly
1,048,576 allocations of 64 bytes: 512 cache entries, each interleaved with
2,047 temporary objects. The mixed policy allocated both classes in one
private heap; the separate policy used two private heaps through the same
allocation API. All payload bytes were verified. Temporary objects were then
freed, both relevant heaps force-collected, and the 32 KiB of cache payload
left live for three seconds. A visitor counted occupied pages only after the
observation window. Refill used the same temporary heap and traffic.

| Policy | Settled process RSS, MiB | Pages holding 512 cache objects | Initial fill, ms | Refill, ms | Refill minor faults |
|---|---:|---:|---:|---:|---:|
| Mixed lifetimes | 50.070 | 512 | 34.237 | 17.719 | 4,951 |
| Separate heaps | 18.135 | 1 | 34.283 | 13.012 | 552 |

Values are medians of six fresh processes per policy. The RSS reduction was
31.936 MiB despite the additional heap's metadata; initial fill time was
similar. The exact one-page versus 512-page occupancy and identical live
payload identify sparse lifetime mixing as the retained-capacity cause.
Refill also improved here, but that result depends on page layout, reuse, and
THP history and is not promised for every heap split. All cache bytes survived
refill verification; final live payload was zero after freeing objects and
deleting the heaps. Deleting a heap with live objects was not used as a
substitute for freeing those objects.

The README now demonstrates separate safe `Heap` lifetimes. This is an
allocation-classification decision for an application that knows which data
outlives a request, rather than a new allocator heuristic that tries to infer
object lifetime. Frozen artifacts, raw phases, samples, and strict-compiled
probe source are under `target/perf-experiments/X44/lifetimes-primary/`.

## X45 — Maintenance between hot allocation batches

Kept as an explicit application checkpoint, 2026-10-22. Eight balanced
four-policy rounds used final X42 Rust only. An initialized worker retains a
64-byte anchor, allocates/touches/verifies/frees a 64 MiB cold burst of 8 KiB
objects, then runs a three-second 64-byte allocate/write/free loop. Time is
checked between batches of 2,048 pairs. Policies are passive, non-forced
collection every 100 ms or 1 s, and forced collection every 100 ms. The
external sampler runs every 100 ms on a separate physical core. Payload
verification, anchor preservation, and zero final live bytes passed in all
32 processes.

| Policy | Final active RSS, MiB | Million pairs/s | Collections | Median RSS t90, s | Maximum single collection, ms |
|---|---:|---:|---:|---:|---:|
| Passive | 72.191 | 167.922 | 0 | Not reached in 3 s | 0 |
| Non-forced, 100 ms | 8.148 | 167.497 | 29 | 1.069 | 0.661 |
| Non-forced, 1 s | 8.254 | 167.621 | 2 | 1.069 | 1.360 |
| Forced, 100 ms | 8.174 | 171.248 | 29 | 0.161 | 0.673 |

RSS, rate, calls, and t90 are medians of eight processes. The pause column is
the maximum across all processes. t90 is relative to the pre-burst baseline
and requires three consecutive periodic samples; its precision is limited by
the sampling cadence. Both non-forced policies reached t90 in all repeats;
calling ten times as often did not bypass the pending arena-purge deadline.
The 1 s policy's median rate was 0.18% below passive, versus 0.25% for 100 ms.
The forced policy's higher measured rate is not treated as a general speedup.
Median excess RSS integrals over the three seconds were 203.45, 76.76, 76.78,
and 18.55 MiB-seconds respectively.

The worker example now includes non-forced maintenance between completed
batches as well as one forced collection after sustained idleness. These
hooks provide progress when the X26 hot free-list cycle never reaches generic
allocation. They do not add work to every allocator free. The previously
rejected X23 per-free countdown remains rejected; repeating its known fast
path cost would not improve this result. Normal collection of pending arena
purges is not a promise to drain every possible remote-owner retention state.
The source, frozen inputs, samples, resource counts, and summaries are under
`target/perf-experiments/X45/batches-primary/`.

## X46 — Localize THP advice without repeatedly collapsing reused backing

The first density prototype was rejected. It retained the 32 MiB activation
threshold, but advised/collapsed only fully allocated, aligned 2 MiB regions,
using the existing free-slice bitmap. Fresh arenas cleared the primitive's
broad huge-page advice before touching metadata. Three Rust-only paired memory
repeats saved about 1.6–1.8 MiB after empty/packed/large-to-small histories,
while a fresh small-process control stayed approximately 12.7 MiB. Dense
64 MiB bursts retained 66–68 MiB of huge-page coverage versus 68–70 MiB.

However, advising on every subsequent arena allocation repeated collapse on
recycled ranges. Two complete balanced native rounds found `malloc-large`
21.46–22.29 times slower and sh8 1.55–1.95 times slower. The four-round screen
was stopped after those two complete rounds; the interruption and partial
next round are retained, and only complete pairs are summarized. A separate
syscall trace of one `malloc-large` invocation found one `MADV_HUGEPAGE` and
one `MADV_COLLAPSE` in the baseline, versus 15,239 of each in this prototype.
That cost decisively outweighed the RSS reduction.

The revised candidate uses first-use information already produced by the
arena's dirty-slice bitmap. It captures that fact before recommit can report
zero-filled memory for reused backing. After activation, only wholly fresh
claimed ranges reconsider their overlapping dense regions. Dirty bits do not
reset, so recycled backing cannot repeatedly trigger these requests. The
initial activation still scans existing registered arenas. No persistent
per-region tracking structure, arena field, option, or public API was added.
The new density/fresh-range policy is limited to Linux/Android x86_64; other
architectures retain whole-arena advice because their native huge-page
configuration and performance were not validated here.

An experimental executable intercepts `madvise`, warms twenty rotating 16 MiB
slots, then repeats the same allocation/free traffic with purging disabled.
It verifies payload endpoints and requires no further huge-page requests once
backing is reused. Baseline: 1/1 warm advice/collapse calls and 0/0 reused calls.
Rejected prototype: 4,097/4,097 warm and 4,096/4,096 reused calls (test fails).
First-use candidate: 169/169 warm and 0/0 reused calls (test passes). The regression also passed against the combined candidate. Its prototype-only
source and release-driver hook were removed when the policy was rejected.

Four balanced native pairs screened the first-use candidate against X42 Rust,
with one warmup plus three short-workload measurements and three-second timed
workloads. Median time differences were Larson −1.82%, Larson-sized −1.70%,
mstress −4.34%, sh8 +0.93%, and `malloc-large` +0.47%; five-case geometric mean
−1.31%. These noisy focused results support a full-suite gate, not a standalone
KEEP. The first two cold `malloc-large` pairs were 15.404/15.402 ms and
15.557/15.780 ms (baseline/candidate), so the removed regression was not merely
hidden by discarding warmup. Process peak RSS in those pairs fell from about
254.37 to 238.40 MiB.

Two paired first-use memory repeats found empty RSS 18.207→16.631 MiB,
packed-survivor RSS 19.043→16.703 MiB, and post-burst four-small-worker RSS
18.154→17.264 MiB. The last case varied by roughly one huge page; its two-run
median is not a precise universal saving. Fresh-small RSS was 12.709/12.727 MiB.
The complete native gate then rejected this candidate. In three complete
fresh-process, single-invocation rounds across all 21 workloads, the combined
candidate had a +5.12% geometric mean time ratio, sh6 +88.20%, rptest +52.21%,
mstress +8.21%, and sh8 +6.10%. Every sh6 pair was 83.4–98.0% slower and every
rptest pair 47.6–58.5% slower. The remaining planned rounds were stopped;
partial fourth-round records are preserved separately. These large effects
exceed both the requested budget and the observed noise.

Removing THP localization while retaining the participation and remote-work
fixes brought two balanced recovery pairs to sh6 +2.56% and rptest +0.34%
versus the original Rust build. The small post-burst RSS saving does not
justify the broader workload regressions. Both THP implementations, the new
primitive, and the prototype-only regression plumbing were reverted. The
existing X29 policy is retained. X48 reruns the full gate on the remaining
changes; it does not treat the favorable five-case screen as acceptance.

All variants, patches, frozen inputs, failed/passing regression results, traces,
and raw measurements are under `target/perf-experiments/X46/`.

## X47 — Skip heap-local work when a main-heap association is absent

Fresh profiling used only Rust. Hardware performance counters were unavailable
on this host, so Callgrind 3.22.0 recorded instruction counts for the native
xmalloc and rptest workloads using the frozen X42 build. Instrumentation
changes timing and scheduling; these counts are not cycle costs or evidence
about native CAS contention. In xmalloc, `collect_after_remote_free` accounted
for 46.78% of recorded instructions and mapped reabandon for 8.71%. The latter
was called 1,459,499 times. In rptest, byte filling accounted for 86.33% of
recorded instructions, including required calloc zeroing and the benchmark's
explicit payload fills; those writes cannot be dropped as allocator overhead.

The candidate targets the exact null case of `_mi_page_associated_theap_peek`:
`page.heap` is null (main heap), and the executing thread has no main-heap TLS
association. After collecting the published remote batch and handling final
page destruction, that case goes directly to the existing ownership-release
routine. It cannot reclaim or map-reabandon into an absent local heap. Other
heap associations follow the existing path. In particular, a general
"uninitialized default heap" test was not substituted: the non-null main-heap
lookup path can initialize a main association and has different semantics.

The option read, remote-list validation, all-free destruction, atomic orders,
captured release-CAS expectation, and nested collect/retry/terminal transitions
are preserved. This is narrower than the previously rejected X06 general
inlining attempt and removes more futile work than E04's rejected
reclaim-only gate. The collector's generated body shrank from 1,990 to 1,483
bytes; ownership release is now separately outlined at 506 bytes. Their
combined 1,989 bytes are essentially unchanged from the original 1,990; the
benefit is skipped work, rather than a claimed reduction in total code size.

A new isolated Rust test exits an allocating producer, transfers 4,096 live
64-byte objects to a consumer with no allocator thread state, checks each
abandoned page and complete payload, frees every object once, and verifies
that the consumer still has no main-heap association. It passes, as do the
native full/normal free-only tests and the THP reuse regression.

Four balanced pairs against the first-use THP build gave median time changes:
xmalloc −15.56% (all pairs between −13.8% and −16.6%), Larson −1.14%,
Larson-sized +0.82%, rptest −9.63%, and glibc-thread +1.14%. The five-case
geometric mean was −5.11%. Larson and rptest remain variable, so the full-suite
and held-out application results in X48 decide acceptance. Native malloc/free
bodies remain 81/162 bytes with their 64-byte alignment offsets unchanged.
Source snapshots, profiles, annotations, regression output, and native pairs
are under `target/perf-experiments/X47/`. The first combined-candidate profile
in X48 reduced mapped-reabandon calls from 1,459,499 per 1,793,326 collector
entries to 13 per 4,062,429 entries. The work completed in each instrumented
timed run differs, so absolute instruction totals are not compared. X48
profiles and validates the retained build again after removing THP localization.
That retained-build profile recorded seven mapped-reabandon calls across
2,494,418 collector entries, confirming the same elimination of futile calls.

## X48 — Final retained-build gate

Completed measurements on 2026-10-22. The retained production changes are X42
with its owner marker before detached-list validation, and X47's exact
main-heap null-destination gate. X43–X45 provide an application example and
lifetime guidance; they do not change allocator defaults. Both X46 THP
prototypes and their dedicated fixture/driver additions were removed.

The original baseline is the X41 Rust build of `a3d20edd`. The retained
interpose library is `artifacts/rust-kept.so`, SHA-256
`e696b6f92b8573d64f53e70a448a8ff671e4860c2a80f2236217530a1aedc3f3`.
The earlier `artifacts/rust.so` contains the rejected THP candidate and is
not the retained result. Restoring source with preserved old timestamps first
produced a stale cached build; its unchanged hash exposed the error before
measurement. Source timestamps were refreshed, the library rebuilt, and its
new hash and absence of the experimental THP helper verified before any
retained-build run. Frozen inputs were checked again after measurement.

The final performance comparison used only old/new Rust, as requested. Six
balanced rounds ran all 21 workloads in fresh processes, shuffled scenario
order, one invocation per process, and three seconds for timed workloads,
on logical CPUs 0,2,4,6,8,10,12,14 of the same EPYC 7763 host. No builds,
tests, profiles, or other benchmarks ran concurrently. Each cell below is
the median of six paired candidate/baseline normalized-time differences;
negative means less time. These are not ratios of independently aggregated
medians.

| Workload | Final / baseline time difference |
|---|---:|
| `glibc-simple` | +0.15% |
| `malloc-large` | +0.00% |
| `malloc-large-old` | +0.45% |
| `cache-scratch1` | +0.02% |
| `cache-thrash1` | -0.02% |
| `cache-scratchN` | +0.01% |
| `cache-thrashN` | +0.07% |
| `mstressN` | -1.33% |
| `sh6benchN` | -0.18% |
| `sh8benchN` | -0.29% |
| `alloc-test1` | +0.23% |
| `alloc-testN` | +0.31% |
| `barnes` | +0.13% |
| `cfrac` | -1.13% |
| `mleak10` | +3.84% |
| `mleak100` | +1.78% |
| `glibc-thread` | -0.17% |
| `xmalloc-testN` | -14.83% |
| `larsonN` | -13.47% |
| `larsonN-sized` | -1.79% |
| `rptestN` | -2.09% |

The unweighted geometric-mean time ratio is **0.985501 (−1.45%)**, equivalent
to +1.47% in reciprocal normalized throughput. This meets the requested 2%
aggregate budget and the stricter 1% project budget. No workload's median
exceeds the requested 5% regression limit. Xmalloc improves in every pair
(12.7–23.4% less time). Larson's large median remains scheduling-sensitive:
one pair is 10.9% slower, so its point estimate is not presented as a precise
repeatable gain. Peak process RSS medians are retained alongside every timing;
`malloc-large` is 254.197/254.174 MiB and xmalloc 68.941/69.689 MiB.

The +3.84% `mleak10` point exceeds the stricter project's 3% per-case screen,
but its six pairs range from −14.0% to +15.3%. A bounded eight-pair A/A control
with one warmup plus three measured calls gives +0.27%; the matching eight-pair
old/new confirmation gives −0.52% (range −13.5% to +17.6%). There is no repeated
slowdown beyond this workload's noise. These follow-ups do not replace the
primary six-pair value or alter the full-suite geometric mean.

Redis was the held-out application, with fresh server processes and balanced
old/new order. Throughput differences are positive for more requests/second:

| Series | Pairs | SET | GET |
|---|---:|---:|---:|
| Primary old/new | 6 | +0.07% | −2.32% |
| Identical-old A/A control | 4 | +2.30% | +0.30% |
| Bounded old/new confirmation | 4 | −4.21% | +4.92% |

Primary GET is lower in five of six pairs; that possible cost is not erased
by the later sign reversal. Confirmation SET is lower in three of four pairs,
with individual differences −6.36%, +0.13%, −2.57%, and −5.85%. A/A GET alone
ranges from −5.13% to +8.44%. The available runs do not establish a stable
application benefit or a stable greater-than-5% regression. Redis remains
**inconclusive**, with both unfavorable series preserved. No further reruns
were used to seek a favorable result.

Three additional paired twelve-burst free-only retention repeats used the
exact retained artifact. Every phase and payload check passed:

| Library | First idle RSS, MiB | Twelfth idle RSS, MiB | Full pages at twelfth idle | After owner collect, MiB |
|---|---:|---:|---:|---:|
| Old Rust | 80.156 | 784.242 | 12,300 | 15.430 |
| Retained Rust | 80.160 | 108.164 | 0 | 14.227 |

These repeats confirm removal of cumulative owner-full-queue retention. The
first idle still needs owner progress or explicit collection; 108 MiB is an
observed endpoint, not a universal cap. The final profile and native free-only
fixtures also pass after THP removal. Dynamic defined symbol names are
identical to the baseline. Native `mi_free`/`mi_malloc` remain 162/81 bytes
with cache-line offsets 32/0; no public API, C layout, or caller safety
obligation changes.

Local stable verification passed `cargo fmt --all`,
`cargo clippy --workspace --all-targets --all-features`, and
`cargo test --workspace --all-targets --all-features`. The test command used
`CARGO_PROFILE_TEST_OPT_LEVEL=3` with debug assertions retained, selected only
`mechanical_mimalloc_raw` for the configurable native benchmark, and used the
frozen retained library. That target's default smoke selects `glibc-simple`;
the complete 21-workload comparison is the separate six-pair gate above.
Other benchmark targets run their normal test-mode smoke checks. The runnable
`worker_maintenance` example also completed.

The remaining local checks passed:

- `scripts/check-release-thread-stress.py`: release thread stress plus full-page
  and normal free-only regressions for both portable C ABI and interpose builds.
- Rust-only Unix mapping checks: hinted allocation retry success/failure both
  leave zero live mappings; ordinary and secure release decommit/recommit
  permissions, zeroing, and allocation reuse satisfy their expected contracts.
- `scripts/check-concurrency-models.sh`: all six models pass with
  `LOOM_MAX_PREEMPTIONS=3`.
- `SANITIZER=address` and `SANITIZER=thread` through
  `scripts/check-sanitizers.sh`, plus both consumer unit regressions under each
  sanitizer. Nightly uses `RUSTFLAGS=-Aunused-unsafe` for the existing intrinsic
  compatibility issue described in X40; stable formatting/lint/tests use no
  warning suppression.
- `git diff --check`.

The first ASan consumer-unit attempt failed in fixture setup: unlike interposed
std startup, sanitizer startup left no allocator thread state, and unconditional
teardown hit the existing empty-default debug assertion. Both fixtures now
check whether this thread is initialized before requesting teardown. They
still verify the uninitialized free path and do not skip their assertions.
ASan and TSan then pass both tests. Formatting, Clippy, and the complete
workspace/all-target/all-feature test command were rerun successfully after
that test-only correction. Production source did not change after the frozen
performance build. The failed initial manifest/log remains alongside the
successful follow-up and targeted ASan log.

Cross-compilation with `--features std --lib` passes for
`aarch64-apple-darwin`. Linux AArch64 fails with five pre-existing `c_char`
signedness errors; Windows MSVC fails with 79 pre-existing platform/type/lint
errors. Both failures were reproduced in a clean detached `a3d20edd` worktree,
with exactly the same error-message sequences as the candidate. They are not
claimed as passing platform validation. Native AArch64, macOS, and Windows
execution was unavailable here. GitHub Actions was not used after the user's
instruction to omit it.

Raw manifests, commands, environment settings, source snapshots, all primary
and confirmation results, profiles, and check logs are under
`target/perf-experiments/X48/`. These ignored local artifacts retain failures
as well as successes; this committed section is the durable result summary.

## X49 — Five consecutive Redis comparisons

Requested follow-up, 2026-10-22. Five old/new Rust pairs reused the exact X48
baseline and retained libraries, alternating execution order, with no other
benchmarks or builds running. Each fresh server handled 3.2 million SETs and
GETs, 256-byte values, 50 connections, pipeline 16, and separate pinned server
and client cores. Allocator ownership and artifact hashes passed verification.
Paired throughput differences, positive for faster:

| Pair | SET | GET |
|---|---:|---:|
| 1 | +0.50% | −1.25% |
| 2 | +1.77% | +1.80% |
| 3 | +4.23% | −0.56% |
| 4 | +3.53% | −3.27% |
| 5 | +0.07% | +4.15% |
| Median | +1.77% | −0.56% |

SET measured faster in all five pairs, but X48's opposite-direction series
and A/A variation still prevent attributing a repeatable application gain to
the two fixes. No implementation or policy changed. Raw results and summary
are under `target/perf-experiments/X49/redis-five/`.

## X50 — Redis profiles and shared optimization candidates

Investigation only, 2026-10-23; no allocator changes or new speedup claim.
Profiles use the retained `b1ce730a` source and the exact X48 interpose artifact
(SHA-256 `e696b6f92b8573d64f53e70a448a8ff671e4860c2a80f2236217530a1aedc3f3`).
Only our allocator was profiled. Hardware events remain unavailable
(`perf_event_paranoid=4`; an actual `perf stat` attempt was rejected), so
Callgrind 3.22.0 supplies instruction counts and simulated cache events,
not native cycles, hardware miss latency, or contention measurements.

Redis 6.2.7 used the X49 request settings. Startup, warmup, and FLUSHALL were
excluded; SET and GET each received their own 3.2-million-request profile.
Process-title rewriting was disabled because it invalidated Valgrind's control
FIFO identity. The final measured phases completed; the first wrapper then
mistook SHUTDOWN's expected connection close for an error. Its original
manifest is retained alongside a clarification and the clean server-exit log.
The later cache-profile wrapper handles that EOF correctly.

Fresh native instruction profiles cover Larson, sh6, mstress, and rptest with
the same retained library. The completed X48 xmalloc profile is reused. Native
profiles include startup, workload warmup, and teardown. In particular,
Larson's single-owner warmup exercises unfull transitions; their aggregate
share must not be presented as a steady threaded cost. Default Valgrind
scheduling starved Larson's timer thread; that attempt was stopped after a
bounded dump and excluded. The completed retry uses `--fair-sched=yes`.

All percentages below are **self instructions divided by whole-process user
instructions**. They are neither elapsed-time shares nor expected savings.

| Component | Redis SET | Redis GET | Larson | sh6 | mstress | rptest | xmalloc |
|---|---:|---:|---:|---:|---:|---:|---:|
| All allocator code | 16.165% | 8.205% | 36.780% | 60.201% | 4.913% | 8.022% | 81.059% |
| Direct-page-cache update | 0.944% | <0.001% | 2.015% | 0.110% | 0.043% | 0.030% | 0.102% |
| Page-queue search | 1.115% | <0.001% | 2.088% | 0.189% | 0.224% | 0.418% | 0.821% |
| Return full page to normal queue | 0.864% | <0.001% | 0.469% | <0.001% | <0.001% | <0.001% | <0.001% |
| Remote-free collector | 0 | 0 | 1.884% | 0.067% | 0.113% | 0.437% | 30.953% |

Redis SET executed 15.319 billion instructions, GET 8.856 billion. The usable
size entry (symbolized as `malloc_size`, also serving `malloc_usable_size`)
accounted for 4.312% of SET and 3.770% of GET: 12.143 and 6.136 queries per
request respectively. Its common path is already only 17 instructions.
`mi_realloc` used 1.504% of SET instructions across 3,200,153 calls, but made
only 71 calls to free old allocations; nearly all requests kept the block.
SDS space trimming explains the repeated resize calls. GET made no calls to
the remote-free collector, as did SET, confirming why X47 is not a direct
Redis throughput optimization.

A separate cache experiment first populated the same key range with 3.2
million SETs while instrumentation was off, warmed with 10,000 GETs, then
profiled 200,000 SETs and 200,000 GETs. This is a populated-dataset window,
not the initial-growth phase above. The accepted model explicitly uses
32 KiB, eight-way L1 instruction/data caches and a 32 MiB, sixteen-way last
level, with 64-byte lines. Callgrind does not model the host's intermediate
512 KiB L2, out-of-order latency, or coherence. An earlier automatic model
incorrectly selected a 256 MiB direct-mapped last level; it is preserved but
is not used for conclusions.

In the corrected SET window, allocator code caused 40.23% of simulated L1
read misses and 49.34% of write misses. The usable-size entry alone incurred
832,535 L1 read misses: 395,353 at the leaf page-map read and 437,146 at the
page flags/block-size reads. Thus the cold lookup/header chain is a concrete
site to investigate, not just a large function name. Two neighbouring-page
link stores in unfull caused 321,227 L1 write misses. GET's allocator accounted
for only 0.98% of L1 read misses, and usable-size queries had just 1,595 misses.
These results support a SET metadata-locality hypothesis, not a measured
hardware stall attribution or a guaranteed GET improvement.

Ranked follow-ups:

1. **Derive direct-cache spans without repeatedly classifying sizes.**
   `mi_theap_direct_cache_start` computes a bin from the current queue's size,
   then computes bins again while walking predecessor queues. Assembly confirms
   repeated bit scans/shifts. Derive the same span from the known queue index
   and immutable class boundaries, preserving alignment aliases and every
   published cache entry. This is the strongest small candidate with measured
   reach across Redis and the native suite. Verify all size classes and both
   pointer widths before a native performance gate; the entire helper's share
   is only a ceiling on instruction savings.
2. **Remove temporary full-flag changes used to locate a normal queue.**
   `_mi_page_unfull_internal` clears the flag, restores it, then the consuming
   queue move clears it again. All three locked operations and repeated
   `pages_full_size` accounting survive compilation. This behavior is inherited
   from bundled C, not introduced by the latest fixes. Compute the destination
   without changing page state, then perform the final transition once. SET
   made 1,482,299 such transitions in the full profile. Preserve queue order,
   accounting, remote publication, and typed ownership. This differs from
   X24's rejected no-op flag check and X32's rejected removal of queue links.
   Reach beyond Redis includes lone-owner/private-heap full-page returns and
   Larson warmup; a steady threaded benefit is not yet demonstrated.
3. **Investigate shared page-map/header locality.** The pointer-to-page chain
   is used by free, usable-size queries, and realloc. Free alone contributes
   32.67% of sh6 and 10.92% of Larson instructions, but those figures include
   work beyond lookup. Any improvement must reduce actual lookup/cache cost
   without adding a per-free tax, changing public layouts, or weakening pointer
   validation. The compact usable-size assembly leaves little obvious scalar
   work to remove. Obtain native cache/stall evidence before committing to a
   cache or metadata redesign; the prior X25 batching result warns that a
   per-free lookup table can cost more than it saves.
4. **Narrower candidate: a small in-place realloc path.** Isolate the existing
   same-heap, nonzero, retained-size case from the large allocation/copy fallback
   to avoid unnecessary register saves and repeated heap resolution. Preserve
   current zero-size, alignment, heap-transfer, failure, and usable-size rules.
   This could help Redis and the existing `realloc_churn` benchmark through
   `GlobalAlloc`, but it is not a broad 21-workload candidate: the observed
   mstress realloc calls were only 200 null-pointer allocations, and other
   profiled native workloads did not use this ordinary resize path.

Further remote-collector tuning would mainly help xmalloc, not this Redis
workload. Rptest's 87.75% `memset` share includes required calloc zeroing and
benchmark payload writes; it is not permission to remove initialization.
Mstress spends 94.93% of instructions in its workload body. Neither fact
supports promising uniform gains from an allocator change. THP localization,
full-queue removal, and generic per-free batching remain rejected as recorded
in X46, X32, and X25.

The recommended implementation order is the direct-cache span calculation,
then the temporary full-flag changes, each isolated and subjected to the
complete correctness, retention, Redis old/new, and full native gates. No
candidate has been implemented or accepted by this profiling task. Raw
profiles, parsed self/call-edge totals, assembly, source snapshots, failed
setup attempts, and `summary.json` live under `target/perf-experiments/X50/`.

## X51 — Ryzen hardware profiles of Redis and shared allocator paths

Investigation only, 2026-10-23; no allocator changes or speedup claim. At the
user's request, profiling ran over SSH on `wbright@100.103.18.44`, host
`amd-ubuntu`, in `~/projects/mechanical_mimalloc/target/perf-experiments/`
`X51-hardware-20261023`. The checkout there was clean at `c8c815f9` and was
left intact. The exact retained X48 allocator and X49 Redis binaries were
copied there and their SHA-256 hashes verified before execution. Allocator
source is `b1ce730a`; the library hash remains
`e696b6f92b8573d64f53e70a448a8ff671e4860c2a80f2236217530a1aedc3f3`.
Only our allocator was profiled; Redis's interposition check confirmed the
server PID. The native harness selected only `mechanical_mimalloc_raw`.

Hardware: Ryzen 9 5950X, 16 cores / 32 logical CPUs, 32 KiB L1D and 512 KiB L2
per core, two 32 MiB L3 caches; Linux 7.0.0-31-generic and perf 7.0.14.
Hardware counters and per-process AMD IBS worked with the existing permissions.
The host was already saturated by other jobs: load averages were about 61–64,
and the initial CPU sample showed all 32 logical CPUs busy. Those jobs were
not stopped or changed. These are hotspot profiles under contention, not
clean throughput measurements or an allocator comparison.

Redis settings match X49/X50: 6.2.7, 256-byte values, 50 clients, pipeline 16,
one client thread, random key range 3.2 million, and 3.2 million operations
per phase. Startup, 1,000-request SET/GET warmup, and FLUSHALL precede profiling;
SET grows an initially empty dataset and GET reads the resulting dataset.
The server and client were pinned to CPUs 2 and 4, controller to CPU 6. A fresh
server was used for each of two passes:

- `cycles:u` at 997 Hz with DWARF call stacks, separately enabled for SET/GET.
- `ibs_op/cnt_ctl=1/` with period 262,144, data addresses, memory data sources,
  and weights, again separately enabled for SET/GET.

Concurrent `perf stat` counted grouped user cycles, instructions, branches,
and branch misses, plus task-clock, context switches, and page faults. All
four hardware counters ran 100% of their enabled time: no multiplex scaling.
The profiles report zero lost samples. Control acknowledgments and the normal
SIGINT termination status required two wrapper corrections; the failed
attempts are preserved separately and excluded from the tables below. The
accepted Redis and native manifests both report completion, and no owned
benchmark/profiler processes remained after collection.

Native profiles cover Larson, sh6, mstress, rptest, and xmalloc, with one full
quick run per profiler, five-second timed workloads, and affinity
`0,2,4,6,8,10,12,14`. They include process startup, workload initialization,
and cleanup. The short rptest profile has substantial startup/kernel-address
attribution and is not used to rank small candidates. Native measurement is
separate from the Redis runs.

### Cycle evidence

Percentages are self sample-period weights divided by the whole recorded
`cycles:u` event weight. They are **not expected speedups**. Ordinary cycle
sampling has skid: despite user-only counting, kernel-address samples account
for 4.78% of SET and 6.18% of GET. They remain in the denominator rather than
being silently discarded. IBS is used below for precise instruction locations.
Perf names several aliases differently from Callgrind: `mi_malloc_usable_size`
is the X50 `malloc_size` entry, and `operator delete[](void*, unsigned long)`
shares the exact exported address with `free` and `mi_free` in this artifact.

| Component | Redis SET | Redis GET | Larson | sh6 | xmalloc |
|---|---:|---:|---:|---:|---:|
| All allocator code | 17.54% | 6.97% | 43.45% | 87.10% | 79.83% |
| malloc/new entry | 4.19% | 2.56% | 21.32% new[] | 39.13% | 10.67% |
| free entry | 0.68% | 1.12% | 12.94% | 42.48% | 22.65% |
| usable-size entry | 4.74% | 3.29% | — | — | — |
| realloc entry | 1.84% | <0.01% | — | — | — |
| Direct-page-cache update | 0.57% | no samples | 0.83% | 0.32% | 0.16% |
| Return full page to normal queue | 0.77% | no samples | 0.005% | no samples | no samples |
| Remote-free collector | no samples | no samples | 0.96% | 0.16% | 20.03% |
| Release abandoned ownership | no samples | no samples | no samples | no samples | 11.37% |

Redis SET has 5,981 cycle samples, GET 4,263; Larson 25,099, sh6 2,008, and
xmalloc 21,513. Tiny entries, especially sh6's six direct-cache-update samples,
are indications of reach, not precise estimates. SET counted 17.210 billion
user cycles and 15.628 billion instructions (0.908 instructions/cycle); GET
11.142 billion cycles and 9.166 billion instructions (0.823). Branch-miss
rates were 0.471% and 0.420%. Contending workloads, SMT, and profiling overhead
limit interpretation of these absolute values.

Redis's `dictFind` alone accounts for 11.50% of SET and 21.92% of GET cycle
weights. GET also spends 5.98% in `stringObjectLen`. Allocator-only changes
therefore address a limited fraction of GET's measured cost. In mstress,
87.36% is the benchmark's `stress` body and 11.37% allocator code. The previous
remote-free work remains primarily an xmalloc opportunity; X50's exact
instruction profiles establish zero collector calls in both Redis phases.

### Precise memory evidence

IBS yielded 73,002 total / 57,820 user samples for SET and 43,339 / 33,031 for
GET. User/kernel separation uses the sampled instruction address, because IBS
also samples kernel operations. L1-miss counts below include sampled loads
with an identified non-L1 source; they are sample counts, not total misses.

| Sampled loads | Redis SET | Redis GET |
|---|---:|---:|
| All user loads | 14,438 | 8,235 |
| All user loads missing L1 | 957 | 504 |
| Allocator loads | 2,740 | 947 |
| Allocator loads missing L1 | 261 | 42 |
| Allocator miss sources: L2 / other cache / RAM | 132 / 120 / 9 | 41 / 1 / 0 |

The usable-size leaf page-map load at offset `+0x22`, page flags at `+0x2b`,
and block size at `+0x2e` give a specific check of X50's locality hypothesis:
SET has 51 L1 misses among 412 sampled loads at those three instructions,
29 served by L2 and 22 reported as `core, same node Any cache hit`. Their
median recorded miss latency is 12 cycles, mean 26.3. GET has 14 misses among
218 such loads, all L2, median 10.5 cycles. No sampled load at those three
sites was served by RAM. This supports a SET-sensitive lookup dependency/cache
cost, but not a claim of pervasive DRAM stalls or a case for adding a new
lookup cache to every free.

SET's unfull function has 63 misses among 195 sampled loads, median miss
latency 43 cycles; 41 were other-cache hits and six RAM hits. Queue bookkeeping
and neighbouring-page state matter in addition to its redundant flag changes.
Direct-cache update has 35 sampled loads and zero L1 misses in this run;
its known repeated bin computations remain the concrete instruction target.

Across native profiles, Larson's allocator has 3,595 misses among 25,114
sampled loads; sh6 has only 59 among 4,482. In sh6, malloc/free together account
for 81.61% of cycle weights despite mostly hot loads. Reducing fast-path work
and dependent loads is therefore a broader hypothesis than reducing cache
capacity misses alone. IBS latency sums contain a few roughly 10,000–13,000
cycle outliers, including return/stack accesses, and are not interpreted as
exclusive stall-time shares. Counts, locations, source categories, and medians
above avoid treating those outliers as a large metadata speedup opportunity.

### Follow-up order

1. **Small shared change: direct-page-cache span computation.** Derive the
   span from the queue index/class boundaries instead of repeatedly applying
   the bin calculation in `page/queues.rs::mi_theap_direct_cache_start`.
   Hardware confirms reach in SET and Larson, with smaller sh6/xmalloc
   entries. Preserve alignment aliases and all cache entries. The measured
   enclosing update is only 0.57% of SET and 0.83% of Larson, so expect a
   modest opportunity, not a multi-percent promise.
2. **Larger shared research: malloc/free/usable-size fast paths.** These are
   substantial in Redis, Larson, and sh6. Examine avoidable dependent metadata
   loads and repeated work using the exact pointer-to-page/header locations.
   GET and sh6 are largely cache-resident, so a design must improve their hot
   path too, without adding a cache-probe tax or weakening validation. No
   specific redesign is justified or implemented by these profiles.
3. **Concrete SET-focused change: remove temporary full-flag transitions.**
   Compute the destination normal queue without clearing/restoring the full
   flag, then perform the final transition once. The current three locked
   modifications and repeated accounting remain visible in X50's assembly.
   Hardware sees 0.77% in this function for SET but only 0.005% in the aggregate
   Larson run; do not advertise a broad steady threaded benefit.
4. **Secondary resize candidate: a compact in-place realloc path.** The SET
   entry is 1.84%; X50 confirms almost all its 3.2 million calls retain the
   block. This could help Redis and dedicated realloc-churn workloads. It is
   not a demonstrated shared target across these native cases.

Raw perf data, scripts, hashes, manifests, command lines, load snapshots, and
reports remain in the remote X51 directory. A local copy of text reports and
manifests is under `target/perf-experiments/X51/remote`; `summary.json` checks
that symbol-period totals equal the reported event counts and that all
counter running percentages are 100. `analyze-ibs.py` retains the load-source
and instruction-offset aggregation. These scratch artifacts are untracked.
No Rust source changed, so this investigation adds no Rust verification run;
the change to this worklog is checked with `git diff --check`.

IBS event/precision interpretation follows the
[Linux perf AMD IBS documentation](https://linux.googlesource.com/linux/kernel/git/torvalds/linux/+/e34a79b96ab9d49ed8b605fee11099cf3efbb428/tools/perf/Documentation/perf-amd-ibs.txt).

## X52 — direct-cache span arithmetic: rejected (2026-10-23)

Following X50/X51, replaced the backward bin search with the preceding queue's
size boundary, including the alignment aliases on 32-bit and 64-bit targets.
A focused regression checked every small request byte against the original
bin classification; the optimized debug test passed. This was measured only
on the local EPYC VM, against the frozen retained `b1ce730a` library with
SHA-256 `e696b6f92b8573d64f53e70a448a8ff671e4860c2a80f2236217530a1aedc3f3`.
Each screen used six alternating fresh-process pairs, the existing six native
cases and pinned Redis SET/GET controls from X41/X42, selecting only our
allocator.

The first build let LLVM inline the enclosing queue update. Redis SET improved
1.39% and GET declined 0.14%, but native elapsed time rose 0.55% geometrically:
Larson +4.26%, sh8 +2.32%, each slower in five of six pairs. A second version
preserved the previous outline boundary and hot malloc/free/collector addresses;
the update shrank from 480 to 331 bytes. SET improved 2.33% and GET 0.71%, but
native time still rose 0.30% overall, with xmalloc +3.44% and slower in four
of six pairs. Other median native changes were Larson +0.90%, Larson-sized
−1.60%, sh6 −0.33%, sh8 −0.69%, and glibc-thread +0.18%.

Neither version demonstrated an all-around runtime improvement. Both source
changes and the candidate-only test were reverted. Patches, frozen libraries,
raw process outputs, resources, and summaries remain under untracked
`target/perf-experiments/X52/{native-screen,redis-screen,native-outlined,redis-outlined}`.

## X53 — select the unfull destination without temporary flag changes

The full-page transition now uses the existing size/huge classifier to select
its destination queue while the full flag remains set. The existing queue
transfer clears it once. This removes two locked flag updates and their
repeated full-size accounting per transition; it preserves the remaining
owner/interior-pointer flags and queue ordering. A focused regression covers
normal and huge destinations, existing destination pages, links, counts, and
full-size accounting. It passes with optimized debug assertions.

Against the original retained library, six local alternating Redis pairs give
SET +2.29% throughput (five of six positive), GET −0.04%. The six-workload
native screen gives elapsed-time changes of Larson −5.07%, Larson-sized
−1.73%, sh6 +1.00%, sh8 −1.54%, xmalloc −1.23%, and rptest −2.85%; geometric
mean −1.92%. Larson and rptest are highly variable; only sh8 meets the
screen's likely-faster criterion. The sh6 cost needs checking in the full
suite. The candidate is provisionally retained for the final combined gate,
not declared broadly faster from this screen alone. Raw paired evidence and
the isolated patch are under `target/perf-experiments/X53`.

## X54 — outline realloc's allocate/copy fallback: rejected

Kept validation, heap resolution, usable-size outputs, and the in-place
eligibility test intact, but outlined allocation/copy/free behind a private
helper. Boundary checks for half-size retention, below-half moves, private-heap
ownership, and aligned interior payload copying passed (eight integration
checks total). The exported realloc function became smaller, but materializing
helper arguments added stack writes to the common path.

The baseline for this independent experiment includes X53. Six local Redis
pairs gave SET +0.59% and GET +0.57% throughput, with substantial variation.
The existing public-allocator `realloc_churn` workload, however, took 6.59%
more time: all six paired changes were worse, ranging from +4.81% to +12.65%.
Both static benchmark binaries were frozen before measurement, ran on CPU 2,
and used fresh Criterion output directories, the existing deterministic
4,000-allocation workload, 100 samples, and ten seconds of measurement.
The ratio uses each process's Criterion mean, then the median across pairs.
No other allocator was selected. Redis and churn ran sequentially without
concurrent builds or tests.

Reverted the helper and candidate-specific tests. Their patch, frozen static
and shared artifacts, raw samples, commands, hashes, and summaries remain
under `target/perf-experiments/X54`. Smaller assembly alone did not justify
this runtime regression.

## X55 — schedule the free-path thread-ID load earlier: rejected

Moved the independent TLS thread-ID load before the page-map loads in the
release interpose free stub, using a caller-saved register. The instruction
count, 162-byte function size, and malloc/free/collector addresses stayed
unchanged. Release free-only checks passed for same-thread, cross-thread,
abandoned-owner, and shutdown cases.

Six local paired native runs against X53 showed xmalloc −4.18% time in every
pair, but sh6 +5.46% in every pair (range +3.0% to +7.2%). The aggregate of six
focused cases was essentially flat; the other medians were Larson +2.17%,
Larson-sized −2.90%, sh8 −0.33%, and glibc-thread +0.07%. Redis's six pairs
gave SET +0.13% throughput and GET −0.68%. This trades one hot workload's
performance for another, so the assembly change was reverted. Frozen binaries,
patch, release-check logs, raw pairs, and summaries are under
`target/perf-experiments/X55`.

## X56 — use the compact page-map leaf index in free: rejected

The handwritten free stub used a 64-bit shift and scaled index where the
compiler-generated usable-size path already used the equivalent low-32-bit
byte offset. Tested `(low32(pointer) >> 13) & 0xfff8` in the free stub, saving
two instruction bytes. Two bytes of unexecuted padding preserved the malloc
and subsequent function addresses. Release free-only probes passed.

Six native pairs against X53 gave +0.48% geometric-mean elapsed time, including
Larson-sized +4.77%, sh6 +0.55%, glibc-thread +0.77%, and xmalloc −1.66%.
The six Redis pairs gave SET −2.50% throughput (five of six lower) and GET
−0.77%. Reverted the candidate. This arithmetic simplification does not remove
a dependent metadata load; removing one would require a more substantial
page-map/layout change or a cache with a correct invalidation protocol. The
current evidence does not justify adding such a cache to each lookup.
Raw paired results, patches, hashes, and release checks are under
`target/perf-experiments/X56`.

## X57 — pass scalar realloc fallback inputs (rejected)

Revisited X54's argument-passing cost. The fallback now receives the already
validated old pointer/usable size, requested size, zeroing policy, local heap,
and optional output. It constructs the ordinary allocation layout inside the
fallback. Pointer validation, heap resolution, usable-size outputs, in-place
eligibility, zero-tail handling, and successful-copy-before-free ordering stay
in place. The exported realloc body is 377 bytes and saves five registers,
without X54's 64-byte stack packet and common-path layout stores. Boundary,
alignment, compatibility zero-extension, and seeded allocation-sequence checks
pass (22 integration tests).

Six local alternating pairs against X53 give Redis SET +1.67% throughput
(five of six higher), GET +1.43% with mixed signs. The existing frozen public
realloc-churn benchmark gives +0.44% elapsed time, with paired changes
−4.86%, +9.86%, +0.19%, +3.99%, −1.94%, and +0.69%. This removes X54's consistent
churn regression, but does not establish a churn speedup. The full X59 gate
then measured realloc churn **3.50% slower** than X53 alone, with five of six
pairs slower. The abstraction-only comparison is −0.13%, locating this cost
in the realloc experiment rather than the seven abstraction changes. Reverted
the prototype and its two added integration tests before the final retained
build. Raw samples, isolated patch, frozen
shared/static binaries, hashes, and commands are in `target/perf-experiments/X57`.

## X58 — profile another application before adding allocator work

Profiled the cached portable RocksDB 10.10.1 `fillrandom` workload locally with
the X53+X57 library: 50,000 writes, 256-byte values, one worker, seed 12345,
compression disabled, WAL enabled, sync disabled, and a fresh database.
Callgrind collected 645,771,121 instructions over the process lifetime. The
allocator library accounts for 1,940,979 self instructions, **0.301%**. Its
largest entries are free (0.102%) and the malloc/C++-new alias (0.079%).
The dominant application entries are the memtable comparator (11.66%), two
CRC routines (7.63% and 7.44%), and skip-list splice recomputation (6.15%).

These are instruction shares, not cycle or wall-time shares; startup and
cleanup are included. The profile does not justify another allocator change
for this workload. The ownership checker reports one distinct process ID,
and allocator-object call edges confirm the instrumented library is exercised.
Frozen binary hashes, commands, raw profile, checked instruction totals,
reports, and output are under `target/perf-experiments/X58/rocksdb-instructions`.
The remote VM was not needed for these experiments.

## X59 — Borrowed atomic fields and guards, locked-list capabilities, and bounded storage

Implemented the seven requested internal changes in order. The initial
pre-abstraction control contains X53 and the provisional X57 experiment:
SHA-256 `a5400eff3ade41de818071ce80f39a9784ea37c4c58bd9406acf04b297299209`.
The initial candidate is `4000f4e69483829040411fc389be13160a9c236bd3577b9378be1e3dd7e8c806`.
The held-out realloc result rejects X57; the final retained build and its
matching X53-only control are recorded below.

- Arena count and page-table cells hold borrowed atomic fields. Their five
  load/store/fetch methods are safe. Prefix projections accept the actual
  fixed atomic arrays; raw metadata is borrowed only at the call site and
  does not create a shared reference to unrelated mutable heap fields.
- Atomic, once, and mutex guard constructors borrow their backing objects.
  Guards retain their existing non-Send/non-Sync behavior. Linux pthread
  storage uses `UnsafeCell` with the same 40-byte size and word alignment;
  once operations no longer form exclusive references to borrowed locks.
  Dynamic subprocess heap visits hand off the registry lock inside a scoped
  callback, keeping the original lock order and callback reentrancy behavior
  without inventing a dynamic `'static` lifetime.
- Heap/TLD list capabilities identify the protected owner and replace both
  acquire booleans throughout thread-heap teardown. Each path acquires the
  other list lock once, preserves heap unlink → TLD unlink → TLD clear →
  terminal decref, and releases locks at the original points. Inlining the
  common free helper removes the capability packet and enum dispatch from
  the caller boundary; no allocator metadata or synchronization is added.
- `OwnedHeap` has neither `Copy` nor `Clone`. Intermediate drains borrow it;
  terminal release consumes it. Raw aliases remain an unsafe caller concern.
- A bounded `SmallRequestedSize` preserves the original request for tracking
  and derives `NormalizedSmallSize` for safe padding. Both types occupy one
  word. The existing small-path branch constructs the bounded proof; the two
  public unsafe heap-small boundaries use their existing size precondition.
  The old safe `RequestedSize::padded_unchecked` method is removed.
- Statistics buffering borrows `[c_char; 256]`. An eight-bit cursor admits
  only valid array indexes, and buffer writes use ordinary safe indexing.
  A formatting enum replaces the nullable C format pointer. Callback/context
  and input-string conversions retain their required unsafe boundaries.
- Bitmap bin diagnostics accept the fixed chunk array directly. The helper
  becomes safe and retains the same optimized instructions.

Compiler probes compile positive borrow controls and reject all five local
atomic/guard backing escapes with E0515. Probes built against the full crate
also reject both locked-list scope escapes and a second move of `OwnedHeap`
(E0382); their positive controls compile. Existing tests assert pointer-sized
handles/guards, unchanged pthread layout, and noncopyable ownership/guards.
New boundary tests cover zero, one, the small maximum, maximum + 1, and
`usize::MAX`, plus statistics output across capacity and newline boundaries.

Normalized x86-64 release code, excluding alignment padding and resolving GOT
relocations, gives the following initial comparison to the pre-abstraction control:

| Function | Before / after bytes | Normalized instruction bodies |
| --- | ---: | --- |
| `mi_malloc` | 81 / 81 | Identical |
| `mi_free` | 162 / 162 | Identical |
| `mi_theap_malloc` | 97 / 97 | Identical |
| `mi_realloc` | 377 / 377 | Identical |
| `mi_heap_free` | 610 / 610 | Identical |
| Once enter / release | 226 / 226; 40 / 40 | Identical |
| Bitmap debug bin lookup | 154 / 154 | Identical |
| `mi_heap_malloc_small` | 184 / 154 | Bounds-check path removed; 56 → 47 instructions |
| `mi_heap_zalloc_small` | 240 / 210 | Bounds-check path removed; 74 → 65 instructions |
| Statistics output callback | 183 / 178 | Safe indexing adds no bounds-check path |

Whole-library text grows by 336 bytes (0.13%), mostly from specializing the
teardown paths. No new heap allocation, metadata word, lock operation, or
atomic operation implements these abstractions. Code size and static
instruction counts alone are not treated as throughput measurements.

Validation: final `cargo fmt --all`,
`cargo clippy --workspace --all-targets --all-features`, and
`cargo test --workspace --all-targets --all-features` pass without warning
suppression. Portable C ABI/interpose release thread stress and free-only
regressions pass. ASan and TSan suites pass with the already-required nightly
`-Aunused-unsafe` compatibility flag for the existing CPUID intrinsic wrapper;
the initial unsuppressed nightly invocation fails on that unrelated warning.

The extra non-secure release unit run exposes an existing test problem:
`free_validation_covers_null_invalid_and_bad_alignment` passes arbitrary
invalid pointers to a release configuration requiring valid allocator
pointers. It SIGSEGVs in both this tree and clean `f56f6089`; the other 306
release unit tests pass with that case excluded. The production free path and
that unrelated test are unchanged. Apple ARM64 release checking passes.
Linux ARM64's five character-signedness errors and Windows MSVC's 79 existing
platform/type/lint errors reproduce at `f56f6089` with identical error-message
multisets. These cross-platform checks are not claimed as passes.

All source diffs, compile-probe logs, code-generation comparisons, frozen
shared/static artifacts, and verification logs are under
`target/perf-experiments/X59`. Performance comparisons below use only this
machine; no additional hardware-profile VM run was needed.

Initial six-round measurement used all six execution orders for original,
pre-abstraction, and candidate builds, with 378 successful fresh native
processes across 21 workloads. Geometric-mean normalized time was −0.03%
versus the original and −0.57% versus the pre-abstraction build. Against that
pre-abstraction control, glibc-simple was +1.80% in time (all six slower),
malloc-large +3.07% with mixed signs, and Larson +3.12% with mixed signs;
these points prevent a blanket claim that every workload is unchanged.
Redis's 18 successful server runs gave SET/GET throughput +0.37%/+1.77%
against the original, and +3.05%/+1.13% against the pre-abstraction build.

The 18-process realloc comparison was decisive for X57: candidate time was
+3.50% against the frozen X53-only build (five of six slower), but −0.13%
against X53+X57, with mixed signs. Thus the outlined fallback was removed;
the abstraction changes were retained for the final comparison. Six paired
cold-path checks gave statistics-rendering time +0.07% and private-heap
create/allocate/destroy time −0.44%, both mixed. The initial raw data remain
under `native-primary`, `redis-primary`, `realloc-screen`, and `cold-paths`;
they are not substituted for measurements of the final retained artifact.

After removing X57, the retained shared library is SHA-256
`ab2ef9b931ce917c036ddd47200dcf49682dd739ccb77291b4a06acc75680fbc`;
its matching pre-abstraction X53-only control is
`533d77733c55d15b9ed0a43580c93f852208f5e1c503f65b32374e6e61c2910e`.
The original pre-X53 control remains
`e696b6f92b8573d64f53e70a448a8ff671e4860c2a80f2236217530a1aedc3f3`.
The final code-generation comparison preserves every conclusion in the table
above; restored `mi_realloc` is 747 bytes/209 instructions in both builds,
with identical normalized bodies. All 240 dynamic export names match.
Text is 267,929 → 268,265 bytes (+336); data is unchanged at 44,008 bytes.

Re-ran formatting, full-workspace/all-target/all-feature Clippy and tests on
this exact source after the rollback: all pass, including 420 tests and the
native smoke harnesses. Release C ABI/interpose thread stress and free-only
regressions, ASan, and TSan pass again. The sanitizer commands retain the
previously documented nightly CPUID warning compatibility flag. Frozen final
artifacts and code-generation results are in `retained-artifacts.json` and
`codegen-retained.json` under the X59 artifact directory.

Final retained six-round comparison: all 378 native processes and all 18
Redis server runs complete successfully. Lower native normalized time is
better; higher Redis throughput is better. “Abstractions only” compares
X53 plus the seven changes against X53 alone.

| Native workload | Time vs original | Abstractions only |
| --- | ---: | ---: |
| glibc-simple | -0.07% | +0.21% |
| malloc-large | +1.61% | -2.23% |
| malloc-large-old | +0.42% | -0.14% |
| cache-scratch1 | -0.02% | +0.02% |
| cache-thrash1 | -0.03% | +0.03% |
| cache-scratchN | -0.07% | -0.24% |
| cache-thrashN | -0.18% | -0.18% |
| mstressN | +0.32% | +0.13% |
| sh6benchN | +1.08% | +1.51% |
| sh8benchN | +0.36% | +0.26% |
| alloc-test1 | -5.43% | -2.81% |
| alloc-testN | -0.05% | +0.35% |
| barnes | +0.51% | +0.75% |
| cfrac | +1.14% | +0.62% |
| mleak10 | +2.80% | -6.66% |
| mleak100 | +1.02% | -0.29% |
| glibc-thread | +0.94% | -0.18% |
| xmalloc-testN | +1.39% | +1.10% |
| larsonN | +2.42% | -1.99% |
| larsonN-sized | -0.24% | -2.15% |
| rptestN | -1.49% | +0.77% |
| **Geometric mean** | **+0.29%** | **-0.55%** |

| Redis operation | Throughput vs original | Abstractions only |
| --- | ---: | ---: |
| SET | +1.14% | +2.63% |
| GET | +3.35% | +1.82% |

SET improves in all six retained/original pairs. GET has mixed signs; its
point estimate is not treated as a reliable application-wide speedup. The
initial glibc-simple slowdown does not repeat on the final artifact: +0.21%
versus the abstraction control, with mixed signs. The final xmalloc point
is +1.10% with six positive differences and receives a bounded A/A and
old/new confirmation. Short malloc-large and mleak10 points versus the
original are also confirmed with identical-binary controls.

The retained realloc-churn comparison is **−1.61% time** versus the frozen
X53-only static build, with five of six pairs faster. This confirms that the
earlier +3.50% cost did not survive removing X57. Six paired statistics and
private-heap lifecycle checks give +0.49% and −1.23% respectively, with mixed
signs. There is no measured aggregate penalty from the seven abstractions;
these small point estimates are not universal zero-cycle guarantees.

Bounded confirmation used twelve balanced rounds per workload, with two
copies of the same frozen control and the retained candidate. The xmalloc
control is X53 alone; the malloc-large/mleak10 control is the original build.
All 108 processes pass. Median elapsed-time changes are:

| Workload | Identical-binary A/A | Candidate / control A | Candidate / control B |
| --- | ---: | ---: | ---: |
| xmalloc-testN | −0.09% | +1.45% | +1.86% |
| malloc-large | −0.42% | +0.87% | −0.33% |
| mleak10 | +7.52% | +2.77% | −1.84% |

The xmalloc confirmation has mixed signs (7/12 and 8/12 slower); descriptive
95% bootstrap intervals for its two candidate comparisons are
[−2.36%, +3.52%] and [−3.22%, +3.88%]. Its primary +1.10% point and positive
confirmation medians leave a small regression possible, even though the
confirmation cannot separate it from variation. No further reruns were used
to select a favorable result. The short malloc-large/mleak10 measurements do
not establish a source-level regression: the two identical controls themselves
diverge substantially, especially mleak10. Raw samples and the A/A calculations
are preserved in `confirm-abstractions`, `confirm-overall`, and
`confirmations-summary.json`.

Retain X53's removal of two redundant locked flag updates and all seven
abstraction changes. Reject the direct-cache, lookup-index, earlier-TLS-load,
and both realloc-outlining prototypes. The complete retained native suite is
essentially flat versus the original (+0.29%), while Redis SET improves
1.14% in all six pairs. The abstraction-only native aggregate is −0.55%.
The compiler-checked lifetime, lock identity, ownership, bounds, and array
extent guarantees require no additional allocator metadata, allocation,
lock, or atomic operation. Main-path instruction bodies remain identical;
the measurements support negligible overall cost, with the explicit xmalloc
uncertainty above rather than a claim of identical throughput everywhere.

## X60 — cross-thread measurement with more repetitions

Followed up the possible X59 xmalloc-testN slowdown with allocator code and
the benchmark executable held fixed. The candidate remains the verified
`6eda1797` source, artifact `ab2ef9b931ce917c036ddd47200dcf49682dd739ccb77291b4a06acc75680fbc`;
the X53-only control remains
`533d77733c55d15b9ed0a43580c93f852208f5e1c503f65b32374e6e61c2910e`.
Both control labels now use the exact same file and inode. The second phase
also reuses the first phase's artifact files without rebuilding or recopying.
No Rust source or benchmark workload code changes were made.

The workload retains its original eight allocator/eight freer threads on
CPUs 0,2,4,6,8,10,12,14 (one logical CPU per physical core). The controller
waits on CPU 15. Each block contains two control processes and one candidate
process. All six execution orders occur once per six-block cycle, shuffled
with a fixed seed. Three warmup processes per phase are excluded by plan;
all measured samples are retained. No other build, test, benchmark, or profile
runs alongside the measurements. Both phases completed their declared sample
counts without timing-based early stopping.

The primary statistic is the geometric mean of paired normalized-time
ratios: candidate / geometric-mean(two controls), with one observation per
complete block. Intervals use 100,000 fixed-seed bootstrap resamples of
complete blocks. Three-block moving and complete six-block-cycle resampling
check sensitivity to consecutive-run variation. Earlier X59 data are
recomputed with this same estimator; its original reported median estimates
are not compared directly to the new mean-ratio estimates.

| Run | Paired blocks | Seconds per process | Time change | 95% paired interval |
| --- | ---: | ---: | ---: | --- |
| Earlier X59 confirmation, same estimator | 12 | 3 | +0.73% | [−1.43%, +2.83%] |
| X60 longer samples | 36 | 15 | +0.81% | [−0.56%, +2.18%] |
| X60 more repetitions | 180 | 3 | **−0.03%** | **[−0.77%, +0.73%]** |

Longer samples alone did not reduce the observed per-block spread: log-ratio
standard deviation was 0.0424, compared with 0.0392 in the earlier short
confirmation. Therefore, before starting the second phase, its count was
fixed at 180 three-second blocks to target approximately ±0.6 percentage
points of uncertainty. Its actual interval is approximately ±0.75 points,
64.8% narrower than the earlier interval using the same estimator. The two
durations are analyzed separately, without pooling them to choose a result.

The larger run's checks agree with essentially flat performance:

| Check | Time change | 95% interval |
| --- | ---: | --- |
| Identical-file A/A controls | −0.22% | [−1.10%, +0.67%] |
| Candidate / control A | −0.14% | [−1.03%, +0.77%] |
| Candidate / control B | +0.08% | [−0.76%, +0.93%] |
| First 90 blocks | +0.19% | [−0.67%, +1.08%] |
| Last 90 blocks | −0.26% | [−1.44%, +0.98%] |
| Three-block moving bootstrap | −0.03% | [−0.79%, +0.73%] |
| Six-block-cycle bootstrap | −0.03% | [−0.80%, +0.77%] |

The median paired change is −0.42%; 82 of 180 blocks are slower. Candidate
position estimates are −0.23%, +0.14%, and −0.01%, each with an interval
spanning zero. Neither the two halves nor execution positions identify a
consistent penalty. Both phases report zero guest-observed steal ticks,
cgroup throttle events, and major faults. Median process CPU use is 7.78 and
7.74 CPU-seconds per wall second respectively. All 648 measured processes and
six warmups pass, and all frozen binary hashes match after measurement.

Code-generation inspection also finds that the cross-thread collector has
the same 1,483-byte, 367-instruction normalized body in both artifacts, but
its entry moves from `0x1e440` (cache-line offset 0) to `0x1df50` (offset 16).
That placement difference is a possible performance mechanism, not a proven
cause. No alignment workaround or allocator change is justified by these
measurements alone.

Conclusion: the larger run, at the original three-second benchmark duration,
does **not** support a repeatable 1–2% slowdown. Its conservative sensitivity
interval is approximately [−0.80%, +0.77%], so the measured effect is
essentially zero within roughly 0.8 percentage points on this machine.
This replaces X59's unresolved 1–2% concern with a substantially tighter
reading; it does not establish exact zero cost on every machine or workload.

Commands, schedule, hashes, raw per-process output, process resources, CPU
and cgroup snapshots, and analysis scripts/results are under
`target/perf-experiments/X60`: `long-run`, `many-run`, `analysis.json`,
`analysis-many.json`, and `collector-codegen.json`. No remote VM was used.

## X61 — six type-system fixes, checked against release assembly

Implemented the six reviewed fixes against the retained `1a79864d` baseline:

1. Bitmap claims pass the callback, arena, and result as typed arguments.
   This removes the shared-reference-to-mutable-reference cast that the new
   regression reproduces under Miri before the fix. The regression checks
   accepted, retried, and discarded bits and preserves the result on failure.
2. Three primitive and three specialized atomic views borrow actual atomic
   storage. Raw field projections remain at the caller's unsafe validity
   boundary; no lifetime is attached merely to a copied raw pointer.
   Primitive operations are safe during that borrow. Remote publication
   explicitly requires unique block ownership and a correctly prepared link.
   The deferred-callback globals now use actual `AtomicPtr` storage.
3. `SliceRange::new` validates arithmetic against a supplied slice count,
   without dereferencing an arena. Callers obtain the live extent in their
   existing unsafe context.
4. Direct bins derive from bounded small sizes. Unrestricted safe size and
   padded-size constructors are removed, and the bound reaches cache indexing.
5. `ArenaAllocation` is non-copyable and holds its arena/range identity once.
   Only initialization flags can change. Its consuming handoff constructs
   the matching MemId. Removing the duplicate identity shrinks this internal
   owner from 48 to 32 bytes on x86-64.
6. `OldAllocation` is non-copyable and derives usable size from one consumed
   validated lookup. Realloc borrows for inspection and moves the owner into
   terminal release. No Drop implementation changes failed-realloc ownership.
   Tests cover shrink boundaries, heap transfers, and interior payloads.

The changes add no allocator metadata fields, allocations, locks, atomic
operations, reference counts, or dynamic dispatch. Borrowed atomic views
retain their pointer size and alignment. The compiler rejects backing-storage
escapes and repeated moves; those restrictions require no runtime bookkeeping.

### Assembly evidence

Compared the actual release interpose libraries with Rust 1.92.0 / LLVM 21.1.3
on x86-64 Linux, using the same build command for both:
`cargo rustc --release --features interpose --lib --crate-type cdylib`.
Disassembly comparison resolves dynamic relocations and normalizes relocated
addresses; instruction counts exclude alignment padding. Full function bodies
and callers are inspected when inlining changes.

| Function | Baseline instructions | Retained instructions | Finding |
| --- | ---: | ---: | --- |
| `mi_malloc` | 20 | 20 | Identical normalized body |
| `mi_free` | 45 | 45 | Identical normalized body |
| `mi_theap_malloc` | 27 | 27 | Identical normalized body |
| `mi_realloc` | 209 | 209 | Identical normalized body |
| `mi_heap_malloc_small` | 47 | 47 | Identical normalized body |
| `mi_heap_zalloc_small` | 65 | 65 | Identical normalized body |
| `collect_after_remote_free` | 367 | 367 | Identical normalized body |
| `mi_bitmap_try_find_and_claim` | 268 | 268 | Identical normalized body |
| `mi_arena_try_alloc_at` | 212 | 207 | Smaller ownership packet |
| `mi_malloc_aligned_at` | 75 | 67 | Removes redundant bin arithmetic and bounds failure |
| `mi_calloc_aligned` | 111 | 111 | Same work; basic blocks reordered |
| `mi_calloc_aligned_at` | 128 | 115 | Removes redundant bin arithmetic and bounds failure |
| Aligned generic allocation | 241 | 242 | One extra mask on the large-alignment branch |
| Aligned realloc worker | 199 | 226 | Null-input allocation is now inlined; removes cache bounds checks |

Inspection led to three small implementation refinements: decrement the
bitmap traversal count at the loop latch, store arena identity once, and
inline the two typed aligned-request adapters. The last change prevents an
extra helper call and repeated alignment validation in aligned calloc.
The ordinary realloc core stays inlined, as in the baseline.

All 240 dynamic export names match. The ELF `.text` section decreases from
197,874 to 197,522 bytes; `.data` and `.bss` sizes are unchanged. Baseline SHA256:
`ab2ef9b931ce917c036ddd47200dcf49682dd739ccb77291b4a06acc75680fbc`.
Retained SHA256:
`dbc2a7a669e9761987ccd4eb76598e4b4ee1fec7b80359b1eb971bcd2e489bb0`.

The evidence establishes identical generated instructions for the listed
core paths and removes runtime bookkeeping from the ownership/lifetime
restrictions. It does **not** establish identical instructions everywhere:
LLVM inserts `and $0x1,%dl` before widening the zeroing flag on the cold
large-alignment path. That is one actual additional instruction, not hidden
by a timing average. Inlining and block placement also change elsewhere.
There is therefore no unconditional claim of zero added instructions on
every allocator path or identical elapsed time on every target.

### Correctness verification

The required `cargo fmt --all`,
`cargo clippy --workspace --all-targets --all-features`, and
`cargo test --workspace --all-targets --all-features` pass, including the full
Rust suite and native smoke harnesses. ASan, TSan, and release thread stress pass.
Thirty-three probes compile the actual modified crate: positive controls and
the owner-size assertion compile; backing-storage escapes, raw/null atomic
construction, safe remote publication, unbounded direct bins, identity
mutation, independent old-allocation parts, and repeated transfers fail with
the expected compiler errors. Committed assertions reject Copy/Clone for the
owners.

The supported Miri suite plus bitmap and arena-claim tests pass under Stacked
and Tree Borrows, each with and without strict provenance: 135 tests per
configuration, 540 passes in total, plus one existing ignored test per
configuration. The final bitmap/arena refinements were rerun in all four
configurations. An unrestricted `types::` selection reaches a pre-existing
heap fixture's unsupported `sysconf(_SC_PHYS_PAGES)` call; that broader run
does not pass. Nightly checks use the existing CPUID compatibility flag
`RUSTFLAGS=-Aunused-unsafe`; stable gates use no warning suppression.

### Earlier timing screen

Before switching the primary check to assembly, an initial six-fix artifact
completed 378 native workload processes and 18 Redis runs, with two control
labels using the exact same baseline file. Remaining realloc, cold-path,
aligned, and confirmation measurements were stopped or not started when
assembly became the requested focus. The completed screen used artifact
`85d3bf61171846c4a11c38b9378dc052ffbd57bbf1d916553ebb8115e8b69071`,
before the assembly-driven refinements above. Its results are retained as
historical observations, not presented as measurements of the final artifact
or proof of exact zero cost. No further timing runs were used to accept the
final code.

Raw logs, compiler probes, source snapshots, disassembly/diffs, and frozen
libraries are under `target/perf-experiments/X61`. The final evidence files
are `assembly-final-manifest.json`, `codegen-final.json`, `final.asm`, and
`*.final.diff`; earlier experimental artifacts are kept separately. No remote
VM, GitHub Actions run, or sub-agent was used.

## X62 — preserve validated alignment through generic allocation

Following the X61 assembly finding, the large-alignment path now passes its
existing `Alignment` proof into `_mi_theap_malloc_zero_ex` as `MaybeAlignment`.
The other three callers pass the existing zero-alignment value. A safe,
inlined conversion preserves the proof; no unsafe constructor or caller
obligation is added. This removes a redundant power-of-two validation and
its unreachable diagnostic path from the internal helper. Public raw-input
validation, heap/sentinel resolution, and allocation-failure behavior remain
in place.

Relative to the original pre-six-fix binary, the large-alignment routine
decreases from 1,024 bytes / 241 instructions to 920 bytes / 226 instructions.
Its stack reservation decreases from 152 to 136 bytes. For an initialized
heap, a 129-byte request, 128 KiB alignment, zero offset, and a successful
aligned return, the instruction trace decreases from **68 to 64**. This count
includes calls as single instructions; the called generic allocator retains
its identical 287-byte / 81-instruction body. The zeroing-flag mask remains,
but the path now executes fewer instructions overall: the redundant alignment
test and branch are gone. This resolves the added successful-path work found
in X61 without special assembly or a change to the allocation algorithm.

The X61 identical core bodies, bitmap claim body, and smaller arena owner
remain unchanged. Against the X61 final artifact, the aligned generic routine
is the only existing function whose instruction count changes; no function
symbols are added or removed. All 240 dynamic exports still match the
pre-six-fix baseline. ELF `.text` is 197,410 bytes versus 197,874 originally,
a 464-byte reduction, with unchanged `.data` and `.bss` sizes. Final SHA256:
`b83b8e826251bfc45a2d2c89bc7b2ee2ce225b55d427144927b7cfed061d9501`.

Code layout still differs. For example, the ordinary over-allocation failure
return has an additional unconditional jump after a failed allocation.
Instruction equality on every error path, identical cache placement, and
target-independent cycle counts are not claimed. The type restrictions add
no runtime ownership/lifetime bookkeeping, and the inspected successful
allocation paths are identical or perform less work.

The required formatting, full-workspace/all-target/all-feature Clippy, and
full test command pass again, including native smoke harnesses. A new
integration regression exercises 128 KiB alignment on a fresh thread for
zero, small, and above-small sizes, checking alignment, zeroing, writable
payloads, release, calloc, and rejection of unsupported offsets. ASan passes
all 25 selected tests; TSan passes its seven selected tests and this new
regression separately. The frozen release library also passes a C ABI smoke
test with a fresh pthread, as does the original baseline.

Three additional whole-crate compiler probes verify the typed call and reject
both raw integer arguments and unchecked safe integer conversions. The six
fixes retain X61's 33 compiler probes and 540 supported Miri passes; this
follow-up changes no pointer or borrowing operations. No further comparative
timing run was used, following the requested emphasis on assembly.

Evidence is under `target/perf-experiments/X62`: `manifest.json`,
`codegen-final.json`, `large-alignment-path.json`, `final.asm`, `*.final.diff`,
the compiler-probe logs, and the verification/C ABI logs. The ordinary-first
branch layout, shared-null-check, return-value, and no-inline experiments
were not retained.

## X63 — balanced verification of the final six-fix binary

Checked the frozen X62 final library against the original pre-six-fix library,
without rebuilding either artifact. The candidate remains
`b83b8e826251bfc45a2d2c89bc7b2ee2ce225b55d427144927b7cfed061d9501`;
both control labels use the exact same baseline file and inode, SHA256
`ab2ef9b931ce917c036ddd47200dcf49682dd739ccb77291b4a06acc75680fbc`.

Six workloads each run six balanced blocks, with all six orders of two
controls and one candidate represented once: 108 fresh processes in total.
Each process warms up with 128 operations, then runs for one second on CPU 0.
No build, test, or other benchmark runs concurrently. The fixed schedule,
all observations, and hashes are retained. No samples are discarded. There
are no observed steal ticks or cgroup throttle events during the run.

The statistic is the geometric mean of paired normalized-time ratios,
candidate / geometric-mean(two controls). Intervals use 20,000 fixed-seed
bootstrap resamples of the six complete blocks per workload. Negative changes
are faster. The payload, alignment, zeroing, expected allocation failure, and
release checks pass in all processes.

| Workload | Time change | 95% paired interval | Identical-file A/A change |
| --- | ---: | --- | ---: |
| malloc/free | −0.81% | [−1.24%, −0.43%] | +0.91% |
| realloc grow/shrink | −1.90% | [−3.37%, −0.93%] | −2.04% |
| Small aligned allocation | −4.26% | [−11.27%, +0.27%] | +7.76% |
| 128 KiB aligned allocation | +5.30% | [−5.39%, +13.54%] | −12.86% |
| Ordinary over-allocation failure | −1.89% | [−2.19%, −1.59%] | +0.10% |
| Private-heap lifecycle | −0.56% | [−1.71%, +0.53%] | −0.62% |

The extra jump in the failure block is not an observed runtime penalty in
this comparison. It does not justify requiring exact instruction equality on
every error branch as an additional acceptance rule. The requested type
constraints introduce no runtime ownership/lifetime bookkeeping; the core
instruction bodies match, and the reviewed arena and aligned paths perform
less work. Large-alignment elapsed time remains inconclusive in this short
gate, with substantial variation even between identical controls; its direct
68-to-64 instruction trace is the relevant evidence for the type changes.
These measurements do not establish exact zero elapsed-time difference on
every machine, and the small timing improvements are not attributed to the
type restrictions.

Two further source experiments expanded the aligned cold routine to 1,414
and 1,137 bytes by inlining allocation dispatch. Both were rejected; production
Rust source remains at `abd82301`, with all required verification from X62
still applicable. No extra unsafe code, branch-hint helper, or layout padding
was introduced to force a particular assembly layout.

The executable source, schedule, resource snapshots, raw results, analysis,
and artifact hashes are under `target/perf-experiments/X63` in `final-gate.c`,
`final-gate.py`, `final-gate-manifest.json`, `final-gate-results.json`, and
`final-gate-analysis.json`.
