#!/usr/bin/env python3
"""Post-window independent output validation for the sustained benchmark."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path
from typing import Any

from benchmark_job_common import (
    atomic_json,
    monotonic_ns,
    read_json,
    wait_for_barrier,
    wait_for_path,
)


def parse_final_count(output: str) -> int:
    """Parse Kafka ConsumerPerformance's final cumulative message count."""
    for line in reversed(output.splitlines()):
        fields = [field.strip() for field in line.split(",")]
        if len(fields) >= 5 and fields[4].isdigit():
            return int(fields[4])
    raise ValueError("consumer performance output has no final message count")


def run(args: argparse.Namespace) -> None:
    result_path = Path(args.result)
    error_log: Any = None
    try:
        atomic_json(
            args.ready,
            {
                "status": "ready",
                "active_during_measurement": False,
                "ready_at_ns": monotonic_ns(),
            },
        )
        barrier = wait_for_barrier(args.start_signal, args.control_timeout)
        start_ns = int(barrier["start_monotonic_ns"])
        deadline_ns = (
            start_ns + int(barrier["duration_seconds"]) * 1_000_000_000
        )
        ingress = read_json(
            wait_for_path(args.ingress_result, args.control_timeout)
        )
        if ingress.get("status") != "passed":
            raise RuntimeError("ingress job failed")
        expected = args.warmup_messages + int(ingress["records_sent"])
        observer_started_ns = monotonic_ns()
        if observer_started_ns < deadline_ns:
            raise RuntimeError("output validator started before measurement ended")

        error_log = Path(args.log).open("wb")
        command = [
            args.runtime,
            "exec",
            args.container,
            "kafka-consumer-perf-test",
            "--broker-list",
            args.bootstrap,
            "--topic",
            args.topic,
            "--group",
            args.group,
            "--messages",
            str(expected),
            "--timeout",
            str(round(args.control_timeout * 1000)),
        ]
        process = subprocess.run(
            command,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=error_log,
            check=False,
            timeout=args.control_timeout,
        )
        completed_ns = monotonic_ns()
        error_log.close()
        error_log = None
        if process.returncode != 0:
            raise RuntimeError(
                f"output validator exited with status {process.returncode}"
            )
        final_count = parse_final_count(process.stdout.decode())
        if final_count != expected:
            raise RuntimeError(
                f"observed {final_count} records, expected {expected}"
            )
        atomic_json(
            result_path,
            {
                "status": "passed",
                "job": "output_validator",
                "active_during_measurement": False,
                "start_monotonic_ns": start_ns,
                "measurement_deadline_monotonic_ns": deadline_ns,
                "observer_started_monotonic_ns": observer_started_ns,
                "completion_monotonic_ns": completed_ns,
                "warmup_records": args.warmup_messages,
                "final_count": final_count,
                "measurement_records_final": final_count
                - args.warmup_messages,
            },
        )
    except Exception as error:
        if error_log is not None:
            error_log.close()
        atomic_json(
            result_path,
            {"status": "failed", "job": "output_validator", "error": str(error)},
        )
        raise


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    result.add_argument("--runtime", required=True)
    result.add_argument("--container", required=True)
    result.add_argument("--bootstrap", required=True)
    result.add_argument("--topic", required=True)
    result.add_argument("--group", required=True)
    result.add_argument("--warmup-messages", type=int, required=True)
    result.add_argument("--ready", required=True)
    result.add_argument("--start-signal", required=True)
    result.add_argument("--ingress-result", required=True)
    result.add_argument("--result", required=True)
    result.add_argument("--log", required=True)
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
