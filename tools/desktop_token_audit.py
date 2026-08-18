"""Read-only audit for Desktop Direct token accounting.

This script intentionally reads only the monitor database. It never creates,
updates, deletes, vacuums, or emits rollout conversation content.
"""

from __future__ import annotations

import argparse
import json
import os
import sqlite3
from datetime import datetime
from pathlib import Path
from typing import Any, Iterable


TOKEN_FIELDS = (
    "total_tokens",
    "input_tokens",
    "cached_input_tokens",
    "cache_write_input_tokens",
    "output_tokens",
    "reasoning_output_tokens",
)

PRICING = {
    "gpt-5.6-sol": (5.00, 0.50, 6.25, 30.00, True),
    "gpt-5.6": (5.00, 0.50, 6.25, 30.00, True),
    "gpt-5.6-terra": (2.50, 0.25, 3.125, 15.00, True),
    "gpt-5.6-luna": (1.00, 0.10, 1.25, 6.00, True),
    "gpt-5.5": (5.00, 0.50, None, 30.00, False),
    "gpt-5.4": (2.50, 0.25, None, 15.00, False),
    "gpt-5.4-mini": (0.75, 0.075, None, 4.50, False),
    "gpt-5.3-codex": (1.75, 0.175, None, 14.00, False),
    "gpt-5.2": (1.75, 0.175, None, 14.00, False),
}


def connect_read_only(path: Path) -> sqlite3.Connection:
    uri = f"file:{path}?mode=ro"
    connection = sqlite3.connect(uri, uri=True)
    connection.row_factory = sqlite3.Row
    return connection


def value(row: sqlite3.Row, key: str) -> int:
    return int(row[key] or 0)


def percent(numerator: int, denominator: int) -> float:
    return numerator / denominator * 100 if denominator else 0.0


def percentile_linear(values: list[int], percentile_value: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    position = (len(ordered) - 1) * percentile_value / 100
    lower = int(position)
    upper = min(lower + 1, len(ordered) - 1)
    return ordered[lower] + (ordered[upper] - ordered[lower]) * (position - lower)


def pricing_profile(model: str | None) -> tuple[float, float, float | None, float, bool] | None:
    if model in PRICING:
        return PRICING[model]
    if model and len(model) > 11 and model[-11] == "-" and all(char.isdigit() for char in model[-10:]):
        return PRICING.get(model[:-11])
    return None


def exact_match(rows: Iterable[sqlite3.Row], delta_field: str, last_field: str) -> int:
    return sum(value(row, delta_field) == value(row, last_field) for row in rows)


def local_timestamp(timestamp: int | None) -> str | None:
    if timestamp is None:
        return None
    return datetime.fromtimestamp(timestamp).isoformat(timespec="seconds")


def main() -> None:
    parser = argparse.ArgumentParser(description="Audit Desktop Direct token accounting (read-only)")
    default_path = (
        Path(os.environ.get("APPDATA", ""))
        / "com.codexusagemonitor.app"
        / "codex-usage-monitor.db"
    )
    parser.add_argument("--db", type=Path, default=default_path)
    parser.add_argument("--top", type=int, default=20)
    args = parser.parse_args()

    connection = connect_read_only(args.db)
    rows = connection.execute(
        """
        SELECT id, thread_id, turn_id, observed_at, total_tokens, input_tokens,
               cached_input_tokens, cache_write_input_tokens, output_tokens,
               reasoning_output_tokens, last_total_tokens, last_input_tokens,
               last_cached_input_tokens, last_cache_write_input_tokens,
               last_output_tokens, last_reasoning_output_tokens, model_context_window,
               model_id, baseline_only, reset_detected, source, cache_write_telemetry_present,
               delta_total_tokens, delta_input_tokens, delta_cached_input_tokens,
               delta_cache_write_input_tokens, delta_output_tokens,
               delta_reasoning_output_tokens
        FROM thread_token_snapshots
        WHERE source='desktop_rollout' AND delta_total_tokens IS NOT NULL
        ORDER BY observed_at ASC, id ASC
        """
    ).fetchall()

    totals = {field: sum(value(row, f"delta_{field}") for row in rows) for field in TOKEN_FIELDS}
    input_tokens = totals["input_tokens"]
    cached_tokens = totals["cached_input_tokens"]
    cache_write_tokens = totals["cache_write_input_tokens"]
    uncached_tokens = max(0, input_tokens - cached_tokens - cache_write_tokens)
    api_cost = 0.0
    priced_events = 0
    for row in rows:
        profile = pricing_profile(row["model_id"])
        if not row["cache_write_telemetry_present"] or profile is None:
            continue
        uncached = max(
            0,
            value(row, "delta_input_tokens")
            - value(row, "delta_cached_input_tokens")
            - value(row, "delta_cache_write_input_tokens"),
        )
        uncached_rate, cached_rate, cache_write_rate, output_rate, long_context = profile
        cache_write = value(row, "delta_cache_write_input_tokens")
        if cache_write and cache_write_rate is None:
            continue
        input_total = uncached + value(row, "delta_cached_input_tokens") + cache_write
        multiplier_input = 2.0 if long_context and input_total > 272_000 else 1.0
        multiplier_output = 1.5 if long_context and input_total > 272_000 else 1.0
        api_cost += (
            (
                uncached * uncached_rate
                + value(row, "delta_cached_input_tokens") * cached_rate
                + cache_write * (cache_write_rate or 0.0)
            )
            / 1_000_000.0
            * multiplier_input
            + value(row, "delta_output_tokens")
            / 1_000_000.0
            * output_rate
            * multiplier_output
        )
        priced_events += 1

    exact_matches = {
        field: exact_match(rows, f"delta_{field}", f"last_{field}")
        for field in (
            "total_tokens",
            "input_tokens",
            "cached_input_tokens",
            "cache_write_input_tokens",
            "output_tokens",
            "reasoning_output_tokens",
        )
    }
    last_sums = {
        field: sum(value(row, f"last_{field}") for row in rows)
        for field in ("total_tokens", "input_tokens", "cached_input_tokens", "output_tokens", "reasoning_output_tokens")
    }

    arithmetic = {"eligible": 0, "correct": 0, "incorrect": 0, "examples": []}
    previous_by_thread: dict[str, sqlite3.Row] = {}
    ordered_all = connection.execute(
        """
        SELECT id, thread_id, turn_id, observed_at, total_tokens, input_tokens,
               cached_input_tokens, cache_write_input_tokens, output_tokens,
               reasoning_output_tokens, delta_total_tokens, delta_input_tokens,
               delta_cached_input_tokens, delta_cache_write_input_tokens,
               delta_output_tokens, delta_reasoning_output_tokens, baseline_only,
               reset_detected
        FROM thread_token_snapshots
        WHERE source='desktop_rollout'
        ORDER BY thread_id, observed_at ASC, id ASC
        """
    ).fetchall()
    for row in ordered_all:
        previous = previous_by_thread.get(row["thread_id"])
        previous_by_thread[row["thread_id"]] = row
        if (
            previous is None
            or row["delta_total_tokens"] is None
            or row["baseline_only"]
            or row["reset_detected"]
        ):
            continue
        arithmetic["eligible"] += 1
        checks = [
            value(row, field) - value(previous, field) == value(row, f"delta_{field}")
            for field in TOKEN_FIELDS
        ]
        if all(checks):
            arithmetic["correct"] += 1
        else:
            arithmetic["incorrect"] += 1
            if len(arithmetic["examples"]) < 20:
                arithmetic["examples"].append(
                    {
                        "thread_id": row["thread_id"],
                        "turn_id": row["turn_id"],
                        "timestamp": local_timestamp(row["observed_at"]),
                        "previous": {field: value(previous, field) for field in TOKEN_FIELDS},
                        "current": {field: value(row, field) for field in TOKEN_FIELDS},
                        "stored_delta": {
                            field: value(row, f"delta_{field}") for field in TOKEN_FIELDS
                        },
                    }
                )

    first_rows = connection.execute(
        """
        SELECT * FROM (
            SELECT thread_id, total_tokens, last_total_tokens, delta_total_tokens,
                   baseline_only,
                   ROW_NUMBER() OVER (PARTITION BY thread_id ORDER BY observed_at, id) AS row_number
            FROM thread_token_snapshots
            WHERE source='desktop_rollout'
        ) WHERE row_number=1
        """
    ).fetchall()
    first_event = {"A_total_equals_last_and_delta": 0, "B_baseline_only": 0, "C_other": 0}
    for row in first_rows:
        if row["baseline_only"] and value(row, "total_tokens") != value(row, "last_total_tokens"):
            first_event["B_baseline_only"] += 1
        elif value(row, "total_tokens") == value(row, "last_total_tokens") and row["delta_total_tokens"] is not None:
            first_event["A_total_equals_last_and_delta"] += 1
        else:
            first_event["C_other"] += 1

    today_row = connection.execute(
        """
        SELECT COALESCE(SUM(delta_total_tokens), 0) AS total,
               COALESCE(SUM(delta_input_tokens), 0) AS input,
               COALESCE(SUM(delta_cached_input_tokens), 0) AS cached,
               COALESCE(SUM(delta_output_tokens), 0) AS output,
               COALESCE(SUM(delta_reasoning_output_tokens), 0) AS reasoning
        FROM thread_token_snapshots
        WHERE source='desktop_rollout' AND delta_total_tokens IS NOT NULL
          AND date(observed_at, 'unixepoch', 'localtime') = date('now', 'localtime')
        """
    ).fetchone()
    dates = connection.execute(
        """
        SELECT date(observed_at, 'unixepoch', 'localtime') AS date,
               COALESCE(SUM(delta_total_tokens), 0) AS tokens,
               COUNT(*) AS events
        FROM thread_token_snapshots
        WHERE source='desktop_rollout' AND delta_total_tokens IS NOT NULL
          AND date(observed_at, 'unixepoch', 'localtime') >= date('now', 'localtime', '-29 days')
        GROUP BY date ORDER BY date
        """
    ).fetchall()

    distribution = [value(row, "delta_total_tokens") for row in rows]
    context_rows = [row for row in rows if row["model_context_window"] is not None]
    above_context = sum(value(row, "delta_input_tokens") > int(row["model_context_window"]) for row in context_rows)
    above_double = sum(value(row, "delta_input_tokens") > int(row["model_context_window"]) * 2 for row in context_rows)
    top_events = [
        {
            "thread_id": row["thread_id"],
            "turn_id": row["turn_id"],
            "timestamp": local_timestamp(row["observed_at"]),
            "model": row["model_id"],
            "delta_total": value(row, "delta_total_tokens"),
            "delta_input": value(row, "delta_input_tokens"),
            "delta_cached": value(row, "delta_cached_input_tokens"),
            "delta_output": value(row, "delta_output_tokens"),
            "model_context_window": row["model_context_window"],
        }
        for row in sorted(rows, key=lambda item: value(item, "delta_total_tokens"), reverse=True)[: args.top]
    ]

    thread_totals = connection.execute(
        """
        SELECT thread_id, SUM(delta_total_tokens) AS summed_delta, COUNT(*) AS events
        FROM thread_token_snapshots
        WHERE source='desktop_rollout' AND delta_total_tokens IS NOT NULL
        GROUP BY thread_id ORDER BY summed_delta DESC LIMIT ?
        """,
        (args.top,),
    ).fetchall()
    thread_ids = [row["thread_id"] for row in thread_totals]
    latest_by_thread: dict[str, sqlite3.Row] = {}
    if thread_ids:
        placeholders = ",".join("?" for _ in thread_ids)
        latest_rows = connection.execute(
            f"""
            SELECT thread_id, total_tokens, observed_at FROM (
                SELECT thread_id, total_tokens, observed_at,
                       ROW_NUMBER() OVER (PARTITION BY thread_id ORDER BY observed_at DESC, id DESC) AS row_number
                FROM thread_token_snapshots
                WHERE source='desktop_rollout' AND thread_id IN ({placeholders})
            ) WHERE row_number=1
            """,
            thread_ids,
        ).fetchall()
        latest_by_thread = {row["thread_id"]: row for row in latest_rows}
    top_threads = [
        {
            "thread_id": row["thread_id"],
            "summed_delta": row["summed_delta"],
            "events": row["events"],
            "latest_total_tokens": latest_by_thread.get(row["thread_id"], {}).get("total_tokens")
            if isinstance(latest_by_thread.get(row["thread_id"]), dict)
            else (latest_by_thread.get(row["thread_id"])["total_tokens"] if row["thread_id"] in latest_by_thread else None),
        }
        for row in thread_totals
    ]

    result: dict[str, Any] = {
        "database": str(args.db),
        "event_count": len(rows),
        "current_totals": {
            **totals,
            "uncached_input_tokens": uncached_tokens,
            "cached_input_ratio_percent": percent(cached_tokens, input_tokens),
            "api_equivalent_cost_usd": api_cost if priced_events else None,
            "priced_events": priced_events,
        },
        "delta_vs_last": {
            "exact_matches": exact_matches,
            "match_percent": {key: percent(count, len(rows)) for key, count in exact_matches.items()},
            "sum_delta": {field: totals[field] for field in last_sums},
            "sum_last": last_sums,
            "difference": {field: totals[field] - last_sums[field] for field in last_sums},
            "difference_percent": {
                field: percent(abs(totals[field] - last_sums[field]), last_sums[field])
                for field in last_sums
            },
        },
        "cumulative_arithmetic": arithmetic,
        "first_event": first_event,
        "today_independent": dict(today_row),
        "date_distribution_last_30_days": [dict(row) for row in dates],
        "distribution": {
            "max_event": max(distribution, default=0),
            "p50": percentile_linear(distribution, 50),
            "p90": percentile_linear(distribution, 90),
            "p99": percentile_linear(distribution, 99),
            "p99_9": percentile_linear(distribution, 99.9),
            "events_above_context_window": above_context,
            "events_above_2x_context_window": above_double,
            "top_events": top_events,
        },
        "top_threads": top_threads,
    }
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
