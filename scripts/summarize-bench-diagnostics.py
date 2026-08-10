#!/usr/bin/env python3
"""Summarize benchmark diagnostic JSONL and enforce counter invariants."""

from __future__ import annotations

import argparse
import collections
import json
from pathlib import Path


HISTOGRAM = (
    "remote_list_len_1",
    "remote_list_len_2_to_4",
    "remote_list_len_5_to_8",
    "remote_list_len_9_to_16",
    "remote_list_len_17_to_32",
    "remote_list_len_33_to_64",
    "remote_list_len_65_plus",
)

RELATIONSHIPS = (
    ("collection force split", "page_free_collect_calls", (
        "page_free_collect_force", "page_free_collect_no_force")),
    ("collection owner/force split", "page_free_collect_calls", (
        "page_free_collect_local_force", "page_free_collect_local_no_force",
        "page_free_collect_abandoned_force", "page_free_collect_abandoned_no_force")),
    ("remote length histogram", "remote_lists_detached", HISTOGRAM),
    ("reclaim outcomes", "abandoned_reclaim_attempts", (
        "abandoned_reclaim_success", "abandoned_reclaim_fail_thread_uninit",
        "abandoned_reclaim_fail_no_theap", "abandoned_reclaim_fail_reclaim_disabled",
        "abandoned_reclaim_fail_queue_limit")),
    ("reabandon outcomes", "reabandon_attempts", (
        "reabandon_success", "reabandon_fail_mostly_used", "reabandon_fail_non_arena",
        "reabandon_fail_already_mapped", "reabandon_fail_arena_reject")),
    ("queue selected positions", "page_queue_success", (
        "page_queue_selected_first", "page_queue_selected_2_to_4",
        "page_queue_selected_5_to_8", "page_queue_selected_9_plus")),
    ("bitmap outcomes", "bitmap_find_clear_attempts", (
        "bitmap_find_clear_success", "bitmap_find_clear_fail")),
    ("tracked option reads", "option_reads_total", (
        "option_reads_page_reclaim_on_free", "option_reads_page_max_reclaim",
        "option_reads_page_cross_thread_max_reclaim", "option_reads_page_max_candidates")),
)


def ratio(numerator: int, denominator: int) -> str:
    return "—" if denominator == 0 else f"{numerator / denominator:.4f}"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("jsonl", type=Path)
    args = parser.parse_args()

    groups: dict[str, collections.Counter[str]] = collections.defaultdict(collections.Counter)
    records: collections.Counter[str] = collections.Counter()
    violations: list[str] = []

    with args.jsonl.open(encoding="utf-8") as source:
        for line_number, line in enumerate(source, 1):
            record = json.loads(line)
            suffix = ""
            if "length" in record:
                suffix = f"/len={record['length']}/destination={record['destination_len']}"
            group = record["scenario"] + suffix
            counters = record["counters"]
            groups[group].update(counters)
            records[group] += 1
            for name, total, parts in RELATIONSHIPS:
                if counters[total] != sum(counters[part] for part in parts):
                    violations.append(f"line {line_number}: {name}")

    print("| Scenario | Records | Collections | Force / collect | Moves / collect | Local blocks / tail scan | Remote blocks / detach | Queue candidates / search | Queue success / search | Fresh pages | Retries |")
    print("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
    for group in sorted(groups):
        c = groups[group]
        print(
            f"| {group} | {records[group]} | {c['page_free_collect_calls']} | "
            f"{ratio(c['page_free_collect_force'], c['page_free_collect_calls'])} | "
            f"{ratio(c['page_free_collect_moved_to_free'], c['page_free_collect_calls'])} | "
            f"{ratio(c['local_tail_blocks_scanned'], c['local_tail_scan_calls'])} | "
            f"{ratio(c['remote_list_blocks_scanned'], c['remote_lists_detached'])} | "
            f"{ratio(c['page_queue_candidates_scanned'], c['page_queue_searches'])} | "
            f"{ratio(c['page_queue_success'], c['page_queue_searches'])} | "
            f"{c['page_queue_fresh_allocations']} | {c['page_queue_retries']} |"
        )

    print("\n| Scenario | Partial calls | Final / partial | Remote len=1 / detach | Validation failures | Reclaim attempts | Reclaim success / attempt | Reabandon attempts | Reabandon success / attempt | Ownership CAS retries / attempt | Selected first / success |")
    print("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
    for group in sorted(groups):
        c = groups[group]
        validation_failures = sum(c[name] for name in (
            "remote_validation_too_long", "remote_validation_exceeds_used",
            "remote_validation_different_page", "remote_validation_invalid_link"))
        print(
            f"| {group} | {c['page_free_collect_partly_calls']} | "
            f"{ratio(c['page_free_collect_partly_final_collect'], c['page_free_collect_partly_calls'])} | "
            f"{ratio(c['remote_list_len_1'], c['remote_lists_detached'])} | "
            f"{validation_failures} | {c['abandoned_reclaim_attempts']} | "
            f"{ratio(c['abandoned_reclaim_success'], c['abandoned_reclaim_attempts'])} | "
            f"{c['reabandon_attempts']} | "
            f"{ratio(c['reabandon_success'], c['reabandon_attempts'])} | "
            f"{ratio(c['ownership_release_cas_retries'], c['ownership_release_cas_attempts'])} | "
            f"{ratio(c['page_queue_selected_first'], c['page_queue_success'])} |"
        )

    if violations:
        print("\nInvariant violations:")
        for violation in violations:
            print(f"- {violation}")
        return 1
    print("\nAll per-record counter relationships passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
