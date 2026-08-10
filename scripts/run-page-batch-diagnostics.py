#!/usr/bin/env python3
"""Exercise every page-batch fixture and write diagnostic counter JSONL."""

from __future__ import annotations

import argparse
import ctypes
import json
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--library", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--iterations", type=int, default=1_000)
    args = parser.parse_args()
    if args.iterations <= 0:
        parser.error("--iterations must be positive")

    library = ctypes.CDLL(str(args.library.resolve()))
    library.mi_bench_page_batch_fixture_new.argtypes = [
        ctypes.c_ubyte,
        ctypes.c_size_t,
        ctypes.c_size_t,
    ]
    library.mi_bench_page_batch_fixture_new.restype = ctypes.c_void_p
    library.mi_bench_page_batch_fixture_prepare.argtypes = [ctypes.c_void_p]
    library.mi_bench_page_batch_fixture_prepare.restype = ctypes.c_bool
    library.mi_bench_page_batch_fixture_run.argtypes = [ctypes.c_void_p]
    library.mi_bench_page_batch_fixture_run.restype = ctypes.c_bool
    library.mi_bench_page_batch_fixture_destroy.argtypes = [ctypes.c_void_p]
    library.mi_bench_diagnostics_reset.argtypes = []
    library.mi_bench_diagnostics_json.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
    library.mi_bench_diagnostics_json.restype = ctypes.c_size_t

    def snapshot() -> dict[str, int]:
        needed = library.mi_bench_diagnostics_json(None, 0)
        buffer = ctypes.create_string_buffer(needed + 1)
        actual = library.mi_bench_diagnostics_json(buffer, len(buffer))
        if actual != needed:
            raise RuntimeError("diagnostic JSON size changed while reading it")
        return json.loads(buffer.value)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w", encoding="utf-8") as output:
        cases = (
            (0, "forced_page_free_collect", (128,)),
            (1, "local_to_available", (1, 8, 32, 128)),
            (2, "detached_remote", (1, 8, 32, 128)),
        )
        for kind, scenario, lengths in cases:
            for length in lengths:
                for destination_len in (0, 3):
                    fixture = library.mi_bench_page_batch_fixture_new(
                        kind, length, destination_len
                    )
                    if not fixture:
                        raise RuntimeError(f"fixture creation failed for {scenario}/{length}")
                    try:
                        library.mi_bench_diagnostics_reset()
                        for _ in range(args.iterations):
                            if not library.mi_bench_page_batch_fixture_prepare(fixture):
                                raise RuntimeError("fixture preparation failed")
                            if not library.mi_bench_page_batch_fixture_run(fixture):
                                raise RuntimeError("fixture transition failed")
                        record = {
                            "scenario": scenario,
                            "length": length,
                            "destination_len": destination_len,
                            "iterations": args.iterations,
                            "counters": snapshot(),
                        }
                        print(json.dumps(record, separators=(",", ":")), file=output)
                    finally:
                        library.mi_bench_page_batch_fixture_destroy(fixture)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
