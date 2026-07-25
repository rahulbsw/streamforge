#!/usr/bin/env python3
"""Shared primitives for independent sustained-benchmark jobs."""

from __future__ import annotations

import json
import math
import os
import re
import subprocess
import time
import urllib.request
from pathlib import Path
from typing import Any


METRIC_LINE = re.compile(
    r"^(?P<name>[a-zA-Z_:][a-zA-Z0-9_:]*)"
    r"(?:\{(?P<labels>[^}]*)\})?\s+(?P<value>[^\s]+)"
)
METRIC_LABEL = re.compile(r'([a-zA-Z_][a-zA-Z0-9_]*)="((?:\\.|[^"])*)"')


def monotonic_ns() -> int:
    return time.monotonic_ns()


def atomic_json(path: str | Path, value: dict[str, Any]) -> None:
    destination = Path(path)
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.with_name(f".{destination.name}.{os.getpid()}.tmp")
    with temporary.open("w", encoding="utf-8") as output:
        json.dump(value, output, indent=2, sort_keys=True)
        output.write("\n")
    os.replace(temporary, destination)


def read_json(path: str | Path) -> dict[str, Any]:
    with Path(path).open(encoding="utf-8") as source:
        return json.load(source)


def wait_for_path(path: str | Path, timeout_seconds: float) -> Path:
    target = Path(path)
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() <= deadline:
        if target.exists():
            return target
        time.sleep(0.05)
    raise TimeoutError(f"timed out waiting for {target}")


def wait_for_barrier(path: str | Path, timeout_seconds: float) -> dict[str, Any]:
    return read_json(wait_for_path(path, timeout_seconds))


def terminate_process(process: subprocess.Popen[bytes]) -> None:
    if process.poll() is not None:
        return
    process.terminate()
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=10)


def parse_metrics(text: str) -> list[tuple[str, dict[str, str], float]]:
    parsed = []
    for line in text.splitlines():
        if not line or line.startswith("#"):
            continue
        match = METRIC_LINE.match(line)
        if match is None:
            continue
        value = float(match.group("value"))
        if not math.isfinite(value):
            raise ValueError(f"non-finite Prometheus sample: {line}")
        labels = {
            key: bytes(raw_value, "utf-8").decode("unicode_escape")
            for key, raw_value in METRIC_LABEL.findall(match.group("labels") or "")
        }
        parsed.append((match.group("name"), labels, value))
    return parsed


def metric_value(
    samples: list[tuple[str, dict[str, str], float]],
    name: str,
    labels: dict[str, str] | None = None,
    required: bool = True,
) -> float:
    values = [
        value
        for sample_name, sample_labels, value in samples
        if sample_name == name and (labels is None or sample_labels == labels)
    ]
    if not values:
        if required:
            raise ValueError(f"required metric is missing: {name} {labels or {}}")
        return 0.0
    return sum(values)


def scrape_metrics(
    url: str, destination: str, require_destination: bool = True
) -> dict[str, int]:
    with urllib.request.urlopen(url, timeout=2) as response:
        samples = parse_metrics(response.read().decode("utf-8"))
    destination_label = {"destination": destination}
    return {
        "consumed": round(
            metric_value(samples, "streamforge_messages_consumed_total")
        ),
        "produced": round(
            metric_value(
                samples,
                "streamforge_messages_produced_total",
                destination_label,
                required=require_destination,
            )
        ),
        "delivered": round(
            metric_value(
                samples,
                "streamforge_messages_delivered_total",
                destination_label,
                required=require_destination,
            )
        ),
        "errors": round(
            metric_value(
                samples,
                "streamforge_processing_errors_total",
                required=False,
            )
        ),
    }


def parse_cpu_time(value: str) -> float:
    day_parts = value.strip().split("-", 1)
    days = 0
    clock = day_parts[0]
    if len(day_parts) == 2:
        days = int(day_parts[0])
        clock = day_parts[1]
    parts = [float(part) for part in clock.split(":")]
    seconds = 0.0
    for part in parts:
        seconds = seconds * 60 + part
    return days * 86400 + seconds


def process_resources(pid: int) -> dict[str, float]:
    proc_stat = Path(f"/proc/{pid}/stat")
    if proc_stat.exists():
        fields = proc_stat.read_text(encoding="utf-8").split()
        ticks = os.sysconf("SC_CLK_TCK")
        cpu_seconds = (int(fields[13]) + int(fields[14])) / ticks
        rss_kib = 0
        for line in Path(f"/proc/{pid}/status").read_text(
            encoding="utf-8"
        ).splitlines():
            if line.startswith("VmRSS:"):
                rss_kib = int(line.split()[1])
                break
    else:
        result = subprocess.run(
            ["ps", "-o", "time=", "-o", "rss=", "-p", str(pid)],
            check=True,
            capture_output=True,
            text=True,
        )
        fields = result.stdout.split()
        if len(fields) != 2:
            raise RuntimeError(f"unexpected ps output for PID {pid}")
        cpu_seconds = parse_cpu_time(fields[0])
        rss_kib = int(fields[1])
    cpu_percent_result = subprocess.run(
        ["ps", "-o", "%cpu=", "-p", str(pid)],
        check=True,
        capture_output=True,
        text=True,
    )
    return {
        "cpu_seconds": cpu_seconds,
        "cpu_percent": float(cpu_percent_result.stdout.strip()),
        "rss_bytes": rss_kib * 1024,
    }


def percentile(values: list[float], quantile: float) -> float:
    ordered = sorted(values)
    index = round((len(ordered) - 1) * quantile)
    return ordered[index]
