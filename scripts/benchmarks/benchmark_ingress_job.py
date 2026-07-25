#!/usr/bin/env python3
"""Persistent, startup-free ingress job for the sustained benchmark."""

from __future__ import annotations

import argparse
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

from benchmark_job_common import (
    atomic_json,
    monotonic_ns,
    terminate_process,
    wait_for_barrier,
    wait_for_path,
)


def load_payload_lines(path: str) -> list[bytes]:
    with Path(path).open("rb") as source:
        lines = source.readlines()
    if not lines or any(not line.endswith(b"\n") for line in lines):
        raise ValueError("payload file must contain newline-terminated records")
    return lines


def write_lines(
    stream: Any,
    payload_lines: list[bytes],
    count: int,
    flush_every: int,
) -> int:
    written = 0
    for index in range(count):
        stream.write(payload_lines[index % len(payload_lines)])
        written += 1
        if written % flush_every == 0:
            stream.flush()
    stream.flush()
    return written


def run(args: argparse.Namespace) -> None:
    result_path = Path(args.result)
    process: subprocess.Popen[bytes] | None = None
    try:
        if args.target_rate < 0:
            raise ValueError("target rate must be non-negative")
        if args.flush_every <= 0:
            raise ValueError("flush interval must be positive")
        payload_lines = load_payload_lines(args.payload)
        with Path(args.log).open("wb") as log:
            command = [
                args.runtime,
                "exec",
                "-i",
                args.container,
                "kafka-console-producer",
                "--bootstrap-server",
                args.bootstrap,
                "--topic",
                args.topic,
                "--batch-size",
                str(args.batch_size),
                "--producer-property",
                "acks=all",
                "--producer-property",
                f"linger.ms={args.linger_ms}",
            ]
            process = subprocess.Popen(
                command,
                stdin=subprocess.PIPE,
                stdout=log,
                stderr=subprocess.STDOUT,
                bufsize=1024 * 1024,
            )
            time.sleep(args.settle_seconds)
            if process.poll() is not None:
                raise RuntimeError(
                    f"producer exited during startup with status {process.returncode}"
                )
            if process.stdin is None:
                raise RuntimeError("producer stdin is unavailable")

            atomic_json(
                args.ready,
                {"status": "ready", "pid": process.pid, "ready_at_ns": monotonic_ns()},
            )
            wait_for_path(args.warmup_signal, args.control_timeout)
            warmup_started_ns = monotonic_ns()
            warmup_sent = write_lines(
                process.stdin,
                payload_lines,
                args.warmup_messages,
                args.flush_every,
            )
            atomic_json(
                args.warmup_result,
                {
                    "status": "sent",
                    "records_sent": warmup_sent,
                    "started_at_ns": warmup_started_ns,
                    "submitted_at_ns": monotonic_ns(),
                },
            )

            barrier = wait_for_barrier(args.start_signal, args.control_timeout)
            start_ns = int(barrier["start_monotonic_ns"])
            deadline_ns = (
                start_ns + int(barrier["duration_seconds"]) * 1_000_000_000
            )
            while monotonic_ns() < start_ns:
                time.sleep(0.001)

            sent = 0
            payload_index = 0
            while monotonic_ns() < deadline_ns:
                for _ in range(args.flush_every):
                    process.stdin.write(payload_lines[payload_index])
                    sent += 1
                    payload_index = (payload_index + 1) % len(payload_lines)
                process.stdin.flush()
                if args.target_rate > 0:
                    target_ns = start_ns + round(
                        sent / args.target_rate * 1_000_000_000
                    )
                    remaining_ns = target_ns - monotonic_ns()
                    if remaining_ns > 0:
                        time.sleep(remaining_ns / 1_000_000_000)
            process.stdin.flush()
            measurement_stop_ns = monotonic_ns()
            process.stdin.close()
            try:
                return_code = process.wait(timeout=args.flush_timeout)
            except subprocess.TimeoutExpired as error:
                terminate_process(process)
                raise TimeoutError("producer did not flush before timeout") from error
            completed_ns = monotonic_ns()
            if return_code != 0:
                raise RuntimeError(f"producer exited with status {return_code}")

        duration_seconds = (measurement_stop_ns - start_ns) / 1_000_000_000
        atomic_json(
            result_path,
            {
                "status": "passed",
                "job": "ingress",
                "start_monotonic_ns": start_ns,
                "deadline_monotonic_ns": deadline_ns,
                "measurement_stop_monotonic_ns": measurement_stop_ns,
                "producer_completed_monotonic_ns": completed_ns,
                "records_sent": sent,
                "measurement_duration_seconds": duration_seconds,
                "producer_flush_seconds": (
                    completed_ns - measurement_stop_ns
                )
                / 1_000_000_000,
                "records_per_second": sent / duration_seconds,
                "target_rate_messages_per_second": args.target_rate,
            },
        )
    except Exception as error:
        if process is not None:
            terminate_process(process)
        atomic_json(
            result_path,
            {"status": "failed", "job": "ingress", "error": str(error)},
        )
        raise


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    result.add_argument("--runtime", required=True)
    result.add_argument("--container", required=True)
    result.add_argument("--bootstrap", required=True)
    result.add_argument("--topic", required=True)
    result.add_argument("--payload", required=True)
    result.add_argument("--warmup-messages", type=int, required=True)
    result.add_argument("--ready", required=True)
    result.add_argument("--warmup-signal", required=True)
    result.add_argument("--warmup-result", required=True)
    result.add_argument("--start-signal", required=True)
    result.add_argument("--result", required=True)
    result.add_argument("--log", required=True)
    result.add_argument("--settle-seconds", type=float, default=2.0)
    result.add_argument("--control-timeout", type=float, required=True)
    result.add_argument("--flush-timeout", type=float, required=True)
    result.add_argument("--flush-every", type=int, default=1000)
    result.add_argument("--batch-size", type=int, default=10000)
    result.add_argument("--linger-ms", type=int, default=20)
    result.add_argument("--target-rate", type=int, default=0)
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
