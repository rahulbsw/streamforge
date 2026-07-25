#!/usr/bin/env python3
"""Prometheus correctness and StreamForge resource validator job."""

from __future__ import annotations

import argparse
import csv
import os
import sys
import time
from pathlib import Path
from typing import Any

from benchmark_job_common import (
    atomic_json,
    monotonic_ns,
    percentile,
    process_resources,
    read_json,
    scrape_metrics,
)


def run(args: argparse.Namespace) -> None:
    result_path = Path(args.result)
    try:
        initial = scrape_metrics(
            args.url, args.destination, require_destination=False
        )
        initial_resources = process_resources(args.pid)
        atomic_json(
            args.ready,
            {
                "status": "ready",
                "metrics": initial,
                "resources": initial_resources,
                "ready_at_ns": monotonic_ns(),
            },
        )
        barrier: dict[str, Any] | None = None
        start_ns = 0
        deadline_ns = 0
        baseline = {
            "consumed": args.warmup_messages,
            "produced": args.warmup_messages,
            "delivered": args.warmup_messages,
            "errors": 0,
        }
        cpu_start = 0.0
        final_snapshot: dict[str, int] | None = None
        final_resources: dict[str, float] | None = None
        window_snapshot: dict[str, int] | None = None
        window_resources: dict[str, float] | None = None
        window_sample_ns: int | None = None
        rows: list[dict[str, float | int]] = []
        last_snapshot = initial
        overall_deadline = time.monotonic() + args.control_timeout

        while time.monotonic() <= overall_deadline:
            sampled_ns = monotonic_ns()
            snapshot = scrape_metrics(
                args.url, args.destination, require_destination=False
            )
            resources = process_resources(args.pid)
            for name, value in snapshot.items():
                if value < last_snapshot[name]:
                    raise RuntimeError(f"Prometheus counter decreased: {name}")
            last_snapshot = snapshot
            if snapshot["errors"] > 0:
                raise RuntimeError("processing error counter increased")

            if barrier is None and Path(args.start_signal).exists():
                barrier = read_json(args.start_signal)
                start_ns = int(barrier["start_monotonic_ns"])
                deadline_ns = (
                    start_ns + int(barrier["duration_seconds"]) * 1_000_000_000
                )
                cpu_start = resources["cpu_seconds"]

            row: dict[str, float | int] = {
                "monotonic_ns": sampled_ns,
                **snapshot,
                **resources,
            }
            rows.append(row)
            atomic_json(
                args.progress,
                {"status": "running", "sampled_at_ns": sampled_ns, **snapshot},
            )

            if (
                barrier is not None
                and window_snapshot is None
                and sampled_ns >= deadline_ns
            ):
                window_snapshot = snapshot.copy()
                window_resources = resources.copy()
                window_sample_ns = sampled_ns

            if barrier is not None and Path(args.ingress_result).exists():
                ingress = read_json(args.ingress_result)
                if ingress.get("status") == "failed":
                    raise RuntimeError("ingress job failed")
                expected = int(ingress["records_sent"])
                deltas = {
                    name: snapshot[name] - baseline[name]
                    for name in ("consumed", "produced", "delivered", "errors")
                }
                measured = ("consumed", "produced", "delivered")
                if any(deltas[name] > expected for name in measured):
                    raise RuntimeError(
                        f"metric count exceeded ingress count: {deltas}, "
                        f"expected {expected}"
                    )
                if (
                    all(deltas[name] == expected for name in measured)
                    and deltas["errors"] == 0
                ):
                    final_snapshot = snapshot
                    final_resources = resources
                    break
            time.sleep(args.sample_interval)
        else:
            raise TimeoutError("metrics validator did not reach exact final counts")

        if (
            window_snapshot is None
            or window_resources is None
            or window_sample_ns is None
        ):
            window_snapshot = final_snapshot
            window_resources = final_resources
            window_sample_ns = monotonic_ns()
        if final_snapshot is None or final_resources is None:
            raise RuntimeError("final metrics snapshot is unavailable")

        samples_path = Path(args.samples)
        samples_path.parent.mkdir(parents=True, exist_ok=True)
        with samples_path.open("w", encoding="utf-8", newline="") as output:
            writer = csv.DictWriter(output, fieldnames=list(rows[0]))
            writer.writeheader()
            writer.writerows(rows)

        measured_rows = [
            row
            for row in rows
            if start_ns <= int(row["monotonic_ns"]) <= int(window_sample_ns)
        ]
        if not measured_rows:
            raise RuntimeError("no resource samples were captured in the measured window")
        cpu_percent_values = [float(row["cpu_percent"]) for row in measured_rows]
        rss_values = [float(row["rss_bytes"]) for row in measured_rows]
        final_sample_ns = int(rows[-1]["monotonic_ns"])
        final_deltas = {
            name: final_snapshot[name] - baseline[name]
            for name in ("consumed", "produced", "delivered", "errors")
        }
        window_deltas = {
            name: window_snapshot[name] - baseline[name]
            for name in ("consumed", "produced", "delivered", "errors")
        }
        window_seconds = (window_sample_ns - start_ns) / 1_000_000_000
        window_cpu_seconds = window_resources["cpu_seconds"] - cpu_start
        atomic_json(
            result_path,
            {
                "status": "passed",
                "job": "metrics_validator",
                "start_monotonic_ns": start_ns,
                "window_sample_monotonic_ns": window_sample_ns,
                "final_sample_monotonic_ns": final_sample_ns,
                "baseline": baseline,
                "window_deltas": window_deltas,
                "final_deltas": final_deltas,
                "resource_samples": len(measured_rows),
                "resources": {
                    "cpu_seconds": window_cpu_seconds,
                    "mean_cores": window_cpu_seconds / window_seconds,
                    "cpu_percent_p50": percentile(cpu_percent_values, 0.50),
                    "cpu_percent_p95": percentile(cpu_percent_values, 0.95),
                    "cpu_percent_max": max(cpu_percent_values),
                    "rss_bytes_p50": percentile(rss_values, 0.50),
                    "rss_bytes_p95": percentile(rss_values, 0.95),
                    "rss_bytes_max": max(rss_values),
                },
            },
        )
    except Exception as error:
        atomic_json(
            result_path,
            {"status": "failed", "job": "metrics_validator", "error": str(error)},
        )
        raise


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    result.add_argument("--url", required=True)
    result.add_argument("--destination", required=True)
    result.add_argument("--pid", type=int, required=True)
    result.add_argument("--warmup-messages", type=int, required=True)
    result.add_argument("--ready", required=True)
    result.add_argument("--progress", required=True)
    result.add_argument("--start-signal", required=True)
    result.add_argument("--ingress-result", required=True)
    result.add_argument("--result", required=True)
    result.add_argument("--samples", required=True)
    result.add_argument("--sample-interval", type=float, default=0.5)
    result.add_argument("--control-timeout", type=float, required=True)
    return result


def main() -> None:
    args = parser().parse_args()
    try:
        run(args)
    except Exception as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1) from error


if __name__ == "__main__":
    main()
