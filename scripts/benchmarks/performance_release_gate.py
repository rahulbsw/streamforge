#!/usr/bin/env python3
"""Evaluate publication-grade StreamForge benchmark evidence.

The gate consumes normalized baseline and candidate evidence. It deliberately
fails when a required measurement is missing; a release exception must name an
owner, a removal milestone, and the specific check it waives.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


CATALOG_PATH = Path(__file__).with_name("workload_catalog.json")


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


def validate_evidence(
    value: dict[str, Any], label: str, catalog: dict[str, Any]
) -> list[Check]:
    checks: list[Check] = []
    checks.append(
        Check(
            f"{label}.schema",
            value.get("schema_version") == 1,
            f"schema_version={value.get('schema_version')!r}",
        )
    )
    requirements = catalog.get("publication_requirements", {})
    repetitions_value = value.get("repetitions", 0)
    repetitions = (
        repetitions_value
        if isinstance(repetitions_value, int) and not isinstance(repetitions_value, bool)
        else 0
    )
    minimum_repetitions = int(requirements.get("minimum_repetitions", 3))
    checks.append(
        Check(
            f"{label}.repetitions",
            repetitions >= minimum_repetitions,
            f"repetitions={repetitions}, required>={minimum_repetitions}",
        )
    )

    workload = value.get("workload")
    if not isinstance(workload, dict):
        workload = {}
    profiles = catalog.get("profiles", [])
    profile_id = workload.get("profile_id")
    profile = next(
        (entry for entry in profiles if isinstance(entry, dict) and entry.get("id") == profile_id),
        None,
    )
    checks.append(
        Check(
            f"{label}.workload_profile",
            profile is not None,
            f"profile_id={profile_id!r}, required catalog profile",
        )
    )
    allowed_payloads = profile.get("payload_sizes_bytes", []) if profile else []
    payload_size = workload.get("payload_size_bytes")
    checks.append(
        Check(
            f"{label}.payload_size",
            isinstance(payload_size, int)
            and not isinstance(payload_size, bool)
            and payload_size in allowed_payloads,
            f"payload_size_bytes={payload_size!r}, allowed={allowed_payloads!r}",
        )
    )
    processing_mode = workload.get("processing_mode")
    delivery_mode = workload.get("delivery_mode")
    modes_valid = (
        processing_mode in catalog.get("processing_modes", [])
        and delivery_mode in catalog.get("delivery_modes", [])
    )
    invalid_pair = any(
        isinstance(pair, dict)
        and pair.get("processing_mode") == processing_mode
        and pair.get("delivery_mode") == delivery_mode
        for pair in catalog.get("invalid_mode_pairs", [])
    )
    checks.append(
        Check(
            f"{label}.mode_pair",
            modes_valid and not invalid_pair,
            f"processing={processing_mode!r}, delivery={delivery_mode!r}",
        )
    )
    destinations = workload.get("destinations")
    required_destinations = profile.get("required_destinations") if profile else None
    checks.append(
        Check(
            f"{label}.destinations",
            isinstance(destinations, int)
            and not isinstance(destinations, bool)
            and destinations == required_destinations,
            f"destinations={destinations!r}, required={required_destinations!r}",
        )
    )

    environment = value.get("environment")
    if not isinstance(environment, dict):
        environment = {}
    dedicated_required = requirements.get("dedicated_hardware") is True
    checks.append(
        Check(
            f"{label}.dedicated_hardware",
            not dedicated_required
            or (
                environment.get("dedicated_hardware") is True
                and isinstance(environment.get("host_id"), str)
                and bool(environment.get("host_id", "").strip())
            ),
            "dedicated host attestation and host_id required"
            if dedicated_required
            else "dedicated hardware not required by catalog",
        )
    )
    teardown = value.get("teardown")
    if not isinstance(teardown, dict):
        teardown = {}
    teardown_required = requirements.get("teardown_evidence") is True
    checks.append(
        Check(
            f"{label}.teardown_evidence",
            not teardown_required
            or (
                teardown.get("completed") is True
                and isinstance(teardown.get("evidence"), str)
                and bool(teardown.get("evidence", "").strip())
            ),
            "completed teardown and evidence reference required"
            if teardown_required
            else "teardown evidence not required by catalog",
        )
    )
    cost = value.get("cost")
    if not isinstance(cost, dict):
        cost = {}
    cost_amount = cost.get("amount")
    cost_valid = (
        isinstance(cost_amount, (int, float))
        and not isinstance(cost_amount, bool)
        and math.isfinite(cost_amount)
        and cost_amount >= 0
        and isinstance(cost.get("currency"), str)
        and bool(cost.get("currency", "").strip())
        and isinstance(cost.get("evidence"), str)
        and bool(cost.get("evidence", "").strip())
    )
    cost_required = requirements.get("cost_evidence") is True
    checks.append(
        Check(
            f"{label}.cost_evidence",
            not cost_required or cost_valid,
            "amount, currency, and evidence reference required"
            if cost_required
            else "cost evidence not required by catalog",
        )
    )

    counts = value.get("accounting", {})
    exact = isinstance(counts, dict) and counts.get("exact") is True
    errors = counts.get("errors") if isinstance(counts, dict) else None
    exact_required = requirements.get("exact_accounting") is True
    checks.append(
        Check(
            f"{label}.exact_accounting",
            not exact_required or exact,
            f"exact={exact}, required={exact_required}",
        )
    )
    maximum_errors = requirements.get("maximum_errors", 0)
    valid_error_count = (
        isinstance(errors, int)
        and not isinstance(errors, bool)
        and errors >= 0
        and errors <= maximum_errors
    )
    checks.append(
        Check(
            f"{label}.zero_errors",
            valid_error_count,
            f"errors={errors!r}, required<={maximum_errors}",
        )
    )
    try:
        coefficient = metric(value, "throughput_cv")
        maximum_cv = float(requirements.get("maximum_throughput_cv", 0.03))
        checks.append(
            Check(
                f"{label}.throughput_cv",
                math.isfinite(coefficient) and 0 <= coefficient <= maximum_cv,
                f"cv={coefficient:.6f}, limit={maximum_cv:.6f}",
            )
        )
    except (KeyError, TypeError, ValueError) as error:
        checks.append(Check(f"{label}.throughput_cv", False, f"missing: {error}"))
    return checks


def compare(
    baseline: dict[str, Any], candidate: dict[str, Any], catalog: dict[str, Any]
) -> list[Check]:
    checks = validate_evidence(baseline, "baseline", catalog)
    checks.extend(validate_evidence(candidate, "candidate", catalog))
    base_workload = baseline.get("workload")
    candidate_workload = candidate.get("workload")
    checks.append(
        Check(
            "workload_match",
            isinstance(base_workload, dict) and base_workload == candidate_workload,
            f"baseline={base_workload!r}, candidate={candidate_workload!r}",
        )
    )

    requirements = catalog.get("publication_requirements", {})
    comparisons = (
        (
            "throughput_median_messages_per_second",
            False,
            float(requirements.get("maximum_throughput_regression", 0.05)),
        ),
        (
            "p99_latency_milliseconds",
            True,
            float(requirements.get("maximum_p99_latency_regression", 0.10)),
        ),
        (
            "peak_rss_bytes",
            True,
            float(requirements.get("maximum_peak_rss_regression", 0.10)),
        ),
    )
    for name, lower_is_better, limit in comparisons:
        try:
            base = metric(baseline, name)
            current = metric(candidate, name)
            if not all(math.isfinite(value) and value > 0 for value in (base, current)):
                checks.append(
                    Check(
                        name,
                        False,
                        (
                            f"baseline={base!r}, candidate={current!r}; "
                            "both metrics must be finite and greater than zero"
                        ),
                    )
                )
                continue
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
    parser.add_argument("--catalog", type=Path, default=CATALOG_PATH)
    parser.add_argument("--exceptions", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    baseline = load_json(args.baseline)
    candidate = load_json(args.candidate)
    catalog = load_json(args.catalog)
    exceptions = load_exceptions(args.exceptions)
    checks = compare(baseline, candidate, catalog)

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
