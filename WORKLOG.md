# WORKLOG

## 2026-06-14

- Baseline after `b8ed598 Use direct free for Unix overrides`: single-thread/cache/general mimalloc-bench cases are close, but multi-threaded cases are still far from C mimalloc.
- Broad mimalloc-bench outliers:
  - `larsonN-sized`: Rust ~14.7s vs C ~2.1s, Rust RSS ~2.8GB vs C ~264MB, Rust sys time ~120s vs C ~0.95s.
  - `mstressN`: Rust ~5.34s vs C ~1.65s, Rust RSS ~13.9GB vs C ~1.0GB, Rust sys time ~105s vs C ~3.15s.
  - `xmalloc-testN`: Rust ~6.51s vs C ~0.213s, Rust sys time ~132s vs C ~3.63s.
- `perf` on `larson-sized` shows Rust has fewer instructions than C but much worse IPC, far more context switches/migrations, and much higher sys time.
- Hot Rust profile includes `std::sys::sync::mutex::futex::Mutex::lock_contended` under option reads. C mimalloc stores options in a static table and `_mi_option_get_fast` reads the value field directly.
- Next change: make Rust option fast reads match C by removing the global mutex from `_mi_option_get_fast`, while preserving existing option initialization and public mutation semantics.
- Implemented an atomic `OPTION_VALUES` mirror for option values:
  - `_mi_option_get_fast` now does a bounds check and relaxed atomic load, with no `Mutex` acquisition.
  - `mi_option_get` still performs lazy environment initialization through the existing descriptor table, then reads the same fast value.
  - `mi_option_set` and `mi_option_set_default` update both descriptor storage and the fast value.
- Added `tests/options_exports.rs` checks that public option mutations are visible through `_mi_option_get_fast`.
- Focused checks passed:
  - `cargo fmt --check`
  - `cargo test --quiet --test options_exports`
- Full validation before benchmark:
  - `cargo test --quiet`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test --release --quiet`
  - `cargo clippy --features secure --all-targets -- -D warnings`
  - `cargo clippy --features guarded --all-targets -- -D warnings`
  - `cargo test --features secure --quiet`
  - `cargo test --features guarded --quiet`
  - `cargo check --features track-valgrind --quiet`
  - `cargo check --features track-asan --quiet`
  - `git diff --check`
  - `scripts/check-behavioral-parity.py` failed once only on the known flaky `page_commit_on_demand_committed_alloc_delta` (`C 458752`, Rust `524288`), then passed on rerun.
- Rebuilt benchmark cdylib with `cargo rustc --release --lib -- --crate-type cdylib` and copied it to `target/bench-allocators/libmimalloc-rust.so`.
- Disassembly of `_mi_option_get_fast` now shows a bounds check plus indexed load from `OPTION_VALUES`; no mutex lock or call.
- Focused mimalloc-bench after option fast-path change:
  - `larsonN-sized`: C `2.064s`, Rust `2.643s`; Rust RSS still high at `14743320` KiB and sys time `34.12s`.
  - `mstressN`: C `1.62s`, Rust `2.47s`; Rust RSS still high at `14481724` KiB and sys time `23.05s`.
  - `xmalloc-testN`: C `0.209s`, Rust `0.211s`; time, RSS, user, and sys are now close.
- Remaining issue: multi-threaded RSS/sys-time overhead remains for `larsonN-sized` and `mstressN`, so the next target is not option-read locking but page/arena reclaim, commit, or purge behavior under threaded pressure.
