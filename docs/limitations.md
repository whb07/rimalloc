# Known Limitations

- The crate is pre-release and `publish = false`.
- The safe Rust API is intentionally limited to `MiMalloc`, `Allocation`, and
  allocation error/status types.
- Safe wrappers for heaps, options, stats, and allocator-owned handles are not
  stabilized.
- The `raw` facade exposes C-shaped unsafe APIs and should not be treated as a
  safe Rust abstraction.
- Linux x86_64 GNU is the only maintained target tier today.
- Non-Linux platform code is source-present but not yet release-supported.
- Benchmark results are hardware and workload dependent; noisy scenarios must
  be repeated according to `docs/performance.md`.
- `secure` and `guarded` are hardening aids, not complete protection against
  arbitrary memory corruption or malicious unsafe callers.
- `track-valgrind` and `track-asan` are mutually exclusive.
- Internal strict-type migration is ongoing; `core_types` remains crate-private
  and may change without SemVer notice.
- The crate does not currently publish an allocator-api trait integration layer.
