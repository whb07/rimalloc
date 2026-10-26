# TechEmpower axum webserver benchmark

The TechEmpower FrameworkBenchmarks axum test, run under each allocator. It is not
part of the default suites; run it explicitly:

```sh
# Rust rimalloc versus C mimalloc 3.3.2 (the default allocators).
./benchmarks/bench.py webserver

# Every allocator, all three tests, 30 s each.
./benchmarks/bench.py webserver --allocator rust --allocator mimalloc --allocator glibc \
    --allocator jemalloc --allocator tcmalloc

# One test, three repeats (results are medians).
./benchmarks/bench.py webserver --scenario fortunes --repeats 3
```

For each allocator and test the runner starts the server pinned to the server CPUs
with the allocator in `LD_PRELOAD` (none for glibc), checks that the library is
mapped in the server and that no other allocator library is, checks the response
body, warms up (`--warmup-secs`, default 5), then measures with wrk for
`--duration-secs` (default 30) and checks the response again. The server and wrk
run on disjoint halves of the physical cores unless `--server-cpus` and
`--load-cpus` are given; the server starts one thread per server CPU. Results are
written to `summary.md` and `results.csv`: requests/s, the ratio to Rust, HTTP p99,
peak server PSS, and server CPU per request.

| Test | Binary | Request |
|---|---|---|
| `json` | `axum` | `GET /json`, serialize `{"message":"Hello, World!"}` |
| `plaintext` | `axum` | `GET /plaintext`, pipelined 16 deep as in TechEmpower |
| `fortunes` | `axum-sqlite` | `GET /fortunes`: read the 12 fortunes, add one, sort, render HTML |

wrk's latency percentiles are unreliable with pipelining, so plaintext reports
throughput only.

## Provenance and changes

`axum/` is FrameworkBenchmarks `frameworks/Rust/axum` at revision
`57d92fbec6f8fd7431bc77326dd0484e60c96e20` (BSD-3-Clause, `axum/LICENSE`), with:

- The `mimalloc` crate and its `#[global_allocator]` removed, so the server calls
  libc `malloc` and the allocator under test is selected by `LD_PRELOAD`.
- Fortunes reads SQLite (`rusqlite`, SQLite compiled in, one read-only connection
  per server thread) instead of Postgres: `src/main_sqlite.rs` is the `main_pg.rs`
  handler with the query moved. SQLite's own allocations also go through the
  allocator under test.
- Only the json, plaintext, and fortunes tests are kept (no Postgres, MongoDB,
  sqlx, or simd-json); the port comes from `PORT`, defaulting to 8000.
- `fortune.sql` is the fortune table from
  `toolset/databases/postgres/create-postgres.sql` at the same revision, without
  the Postgres grants. The runner loads it into a fresh SQLite file per run.

The binaries are built with `cargo build --release --locked` and
`RUSTFLAGS=-C target-cpu=native`, as in the TechEmpower axum image.
