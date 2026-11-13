#!/usr/bin/env python3
"""Evaluate publication-grade StreamForge benchmark evidence.

The gate consumes normalized baseline and candidate evidence. It deliberately
fails when a required measurement is missing; a release exception must name an
owner, a removal milestone, and the specific check it waives.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class Check:
    name: str
    passed: bool
    detail: str


def load_json(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as source:
        value = json.load(source)
    if not isinstance(value, dict):
        raise ValueError(f"{path}: expected a JSON object")
    return value


def nested(value: dict[str, Any], *path: str) -> Any:
    current: Any = value
    for component in path:
        if not isinstance(current, dict) or component not in current:
            raise KeyError(".".join(path))
        current = current[component]
    return current


def metric(value: dict[str, Any], name: str) -> float:
    return float(nested(value, "metrics", name))


def percent_regression(baseline: float, candidate: float, lower_is_better: bool) -> float:
    if baseline <= 0:
        raise ValueError("baseline metrics must be greater than zero")
    change = (candidate - baseline) / baseline
    return change if lower_is_better else -change


def validate_evidence(value: dict[str, Any], label: str) -> list[Check]:
    checks: list[Check] = []
    checks.append(
        Check(
            f"{label}.schema",
            value.get("schema_version") == 1,
            f"schema_version={value.get('schema_version')!r}",
        )
    )
    checks.append(
        Check(
            f"{label}.publication_eligible",
            value.get("publication_eligible") is True,
            f"publication_eligible={value.get('publication_eligible')!r}",
        )
    )
    repetitions = int(value.get("repetitions", 0))
    checks.append(
        Check(
            f"{label}.repetitions",
            repetitions >= 3,
            f"repetitions={repetitions}, required>=3",
        )
    )
    counts = value.get("accounting", {})
    exact = isinstance(counts, dict) and counts.get("exact") is True
    errors = counts.get("errors") if isinstance(counts, dict) else None
    checks.append(Check(f"{label}.exact_accounting", exact, f"exact={exact}"))
    checks.append(
        Check(
            f"{label}.zero_errors",
            errors == 0,
            f"errors={errors!r}, required=0",
        )
    )
    try:
        coefficient = metric(value, "throughput_cv")
        checks.append(
            Check(
                f"{label}.throughput_cv",
                coefficient <= 0.03,
                f"cv={coefficient:.6f}, limit=0.030000",
            )
        )
    except (KeyError, TypeError, ValueError) as error:
        checks.append(Check(f"{label}.throughput_cv", False, f"missing: {error}"))
    return checks


def compare(baseline: dict[str, Any], candidate: dict[str, Any]) -> list[Check]:
    checks = validate_evidence(baseline, "baseline")
    checks.extend(validate_evidence(candidate, "candidate"))
    checks.append(
        Check(
            "workload_match",
            baseline.get("workload_id") == candidate.get("workload_id"),
            (
                f"baseline={baseline.get('workload_id')!r}, "
                f"candidate={candidate.get('workload_id')!r}"
            ),
        )
    )

    comparisons = (
        ("throughput_median_messages_per_second", False, 0.05),
        ("p99_latency_milliseconds", True, 0.10),
        ("peak_rss_bytes", True, 0.10),
    )
    for name, lower_is_better, limit in comparisons:
        try:
            base = metric(baseline, name)
            current = metric(candidate, name)
            regression = percent_regression(base, current, lower_is_better)
            checks.append(
                Check(
                    name,
                    regression <= limit,
                    (
                        f"baseline={base:.6f}, candidate={current:.6f}, "
                        f"regression={regression:.2%}, limit={limit:.2%}"
                    ),
                )
            )
        except (KeyError, TypeError, ValueError) as error:
            checks.append(Check(name, False, f"missing or invalid evidence: {error}"))
    return checks


def load_exceptions(path: Path | None) -> dict[str, dict[str, Any]]:
    if path is None:
        return {}
    value = load_json(path)
    entries = value.get("exceptions", [])
    if not isinstance(entries, list):
        raise ValueError("exceptions must be an array")
    result: dict[str, dict[str, Any]] = {}
    for entry in entries:
        if not isinstance(entry, dict):
            raise ValueError("each exception must be an object")
        check = entry.get("check")
        owner = entry.get("owner")
        milestone = entry.get("removal_milestone")
        reason = entry.get("reason")
        if not all(isinstance(item, str) and item.strip() for item in (check, owner, milestone, reason)):
            raise ValueError(
                "each exception requires non-empty check, owner, "
                "removal_milestone, and reason"
            )
        result[str(check)] = entry
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", required=True, type=Path)
    parser.add_argument("--candidate", required=True, type=Path)
    parser.add_argument("--exceptions", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    baseline = load_json(args.baseline)
    candidate = load_json(args.candidate)
    exceptions = load_exceptions(args.exceptions)
    checks = compare(baseline, candidate)

    results = []
    failed = []
    for check in checks:
        exception = exceptions.get(check.name) if not check.passed else None
        status = "passed" if check.passed else "excepted" if exception else "failed"
        item: dict[str, Any] = {
            "check": check.name,
            "status": status,
            "detail": check.detail,
        }
        if exception:
            item["exception"] = exception
        results.append(item)
        if status == "failed":
            failed.append(check.name)

    report = {
        "schema_version": 1,
        "status": "failed" if failed else "passed",
        "baseline": str(args.baseline),
        "candidate": str(args.candidate),
        "checks": results,
        "failed_checks": failed,
    }
    rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.write_text(rendered, encoding="utf-8")
    sys.stdout.write(rendered)
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
