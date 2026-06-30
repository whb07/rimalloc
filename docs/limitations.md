# Known Limitations

- The crate is pre-release at version `0.1.0`; SemVer commitments are scoped by
  `docs/compatibility.md`.
- The safe Rust API intentionally covers only the crate-root wrappers listed in
  `docs/api-inventory.md`.
- The `raw` facade exposes C-shaped unsafe APIs and should not be treated as a
  safe Rust abstraction.
- Linux x86_64 GNU is the only maintained target tier today.
- Non-Linux platform code is source-present but not yet release-supported.
- Benchmark results are hardware and workload dependent; noisy scenarios must
  be repeated according to `docs/performance.md`.
- `secure` and `guarded` are hardening aids, not complete protection against
  arbitrary memory corruption or malicious unsafe callers.
- When `track-valgrind` and `track-asan` are both enabled, Valgrind mode takes
  precedence.
- Internal strict types are complete for the current migration scope, but
  `types` remains crate-private and may change without SemVer notice.
- The crate does not currently publish an allocator-api trait integration layer.
