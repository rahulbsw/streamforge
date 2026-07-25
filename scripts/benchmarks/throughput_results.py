#!/usr/bin/env python3
"""Config, validation, and schema-v3 results for sustained benchmarks."""

from __future__ import annotations

import hashlib
import json
import statistics
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


def write_json(path: str, value: object) -> None:
    with Path(path).open("w", encoding="utf-8") as output:
        json.dump(value, output, indent=2, sort_keys=True)
        output.write("\n")


def sha256(path: str) -> None:
    digest = hashlib.sha256()
    with Path(path).open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    print(digest.hexdigest())


def write_config(args: list[str]) -> None:
    (
        config_path,
        group_id,
        input_topic,
        output_topic,
        threads,
        metrics_port,
        processing_mode,
        delivery_mode,
        max_in_flight,
    ) = args
    write_json(
        config_path,
        {
            "appid": group_id,
            "bootstrap": "127.0.0.1:9092",
            "target_broker": "127.0.0.1:9092",
            "input": input_topic,
            "output": output_topic,
            "offset": "earliest",
            "threads": int(threads),
            "performance": {
                "consumer_batch_size": 1000,
                "consumer_batch_timeout_ms": 10,
                "parallelism_factor": 4,
                "processing_mode": processing_mode,
                "worker_queue_capacity": 1024,
                "producer_delivery_mode": delivery_mode,
                "producer_max_in_flight": int(max_in_flight),
                "fetch_min_bytes": 65536,
                "fetch_max_wait_ms": 50,
                "linger_ms": 20,
                "batch_size": 10000,
            },
            "retry": {"max_attempts": 1},
            "dlq": {"enabled": False},
            "observability": {
                "metrics_enabled": True,
                "metrics_port": int(metrics_port),
                "metrics_bind_address": "127.0.0.1",
                "lag_monitoring_enabled": False,
            },
        },
    )


def summary(values: list[float]) -> dict[str, float]:
    median = statistics.median(values)
    mean = statistics.mean(values)
    return {
        "median": median,
        "min": min(values),
        "max": max(values),
        "mean": mean,
        "mad": statistics.median(abs(value - median) for value in values),
        "coefficient_of_variation": (
            statistics.pstdev(values) / mean if mean else 0.0
        ),
    }


def relative_artifact(result_path: str, artifact_path: str) -> str:
    return str(
        Path(artifact_path).resolve().relative_to(Path(result_path).resolve().parent)
    )


def load_passed_job(path: str, name: str) -> dict[str, Any]:
    with Path(path).open(encoding="utf-8") as source:
        result = json.load(source)
    if result.get("status") != "passed":
        raise ValueError(f"{name} job did not pass")
    return result


def write_run(args: list[str]) -> None:
    (
        result_path,
        repetition,
        input_topic,
        output_topic,
        group_id,
        warmup_messages,
        duration_seconds,
        input_end_offset,
        output_end_offset,
        config_path,
        ingress_path,
        observer_path,
        metrics_path,
        metric_samples_path,
    ) = args
    ingress = load_passed_job(ingress_path, "ingress")
    observer = load_passed_job(observer_path, "output observer")
    metrics = load_passed_job(metrics_path, "metrics validator")
    with Path(config_path).open(encoding="utf-8") as source:
        config = json.load(source)

    warmup = int(warmup_messages)
    target = int(ingress["records_sent"])
    metric_counts = metrics["final_deltas"]
    counts = {
        "ingress_submitted": target,
        "input_records": int(input_end_offset) - warmup,
        "consumed": int(metric_counts["consumed"]),
        "produced": int(metric_counts["produced"]),
        "delivered": int(metric_counts["delivered"]),
        "output_records": int(output_end_offset) - warmup,
        "observed_output_records": int(observer["measurement_records_final"]),
        "errors": int(metric_counts["errors"]),
    }
    counted_names = (
        "ingress_submitted",
        "input_records",
        "consumed",
        "produced",
        "delivered",
        "output_records",
        "observed_output_records",
    )
    mismatches = {
        name: {"expected": target, "actual": counts[name]}
        for name in counted_names
        if counts[name] != target
    }
    if counts["errors"] != 0:
        mismatches["errors"] = {"expected": 0, "actual": counts["errors"]}
    if mismatches:
        raise ValueError(f"exact accounting validation failed: {mismatches}")

    if observer.get("active_during_measurement") is not False:
        raise ValueError("output validator must not run during measurement")
    start_ns = int(metrics["start_monotonic_ns"])
    measurement_deadline_ns = start_ns + int(duration_seconds) * 1_000_000_000
    if int(observer["observer_started_monotonic_ns"]) < measurement_deadline_ns:
        raise ValueError("output validator overlapped the measurement window")
    window_sample_ns = int(metrics["window_sample_monotonic_ns"])
    completion_ns = int(metrics["final_sample_monotonic_ns"])
    window_seconds = (window_sample_ns - start_ns) / 1_000_000_000
    completion_seconds = (completion_ns - start_ns) / 1_000_000_000
    configured_seconds = float(duration_seconds)
    delivered_in_window = int(metrics["window_deltas"]["delivered"])
    backlog = target - delivered_in_window
    classification = "engine_saturated" if backlog > 0 else "ingress_limited"

    write_json(
        result_path,
        {
            "schema_version": 3,
            "status": "passed",
            "benchmark": "kafka_sustained_passthrough",
            "completed_at": datetime.now(timezone.utc).isoformat(),
            "repetition": int(repetition),
            "topics": {"input": input_topic, "output": output_topic},
            "group_id": group_id,
            "phases": {
                "warmup": {"records": warmup, "timed": False},
                "steady_state": {
                    "configured_duration_seconds": configured_seconds,
                    "actual_measurement_window_seconds": window_seconds,
                    "ingress_records": target,
                    "output_delivery_records": delivered_in_window,
                    "ingress_rate_messages_per_second": float(
                        ingress["records_per_second"]
                    ),
                    "output_delivery_rate_messages_per_second": (
                        delivered_in_window / window_seconds
                    ),
                    "backlog_records_at_window_end": backlog,
                    "classification": classification,
                },
                "drain": {
                    "duration_seconds": max(
                        0.0, completion_seconds - window_seconds
                    ),
                    "completion_duration_seconds": completion_seconds,
                    "completion_rate_messages_per_second": (
                        target / completion_seconds
                    ),
                },
            },
            "counts": counts,
            "resources": metrics["resources"],
            "config": config,
            "validation": {
                "passed": True,
                "rules": [
                    "all_jobs_passed",
                    "exact_input_output_observer_accounting",
                    "exact_destination_metrics",
                    "zero_processing_errors",
                    "startup_excluded_by_shared_barrier",
                    "output_validator_excluded_from_measurement",
                ],
                "errors": [],
            },
            "artifacts": {
                "config": relative_artifact(result_path, config_path),
                "ingress": relative_artifact(result_path, ingress_path),
                "output_observer": relative_artifact(result_path, observer_path),
                "metrics_validator": relative_artifact(result_path, metrics_path),
                "metric_samples": relative_artifact(
                    result_path, metric_samples_path
                ),
            },
        },
    )


def nested_value(value: dict[str, Any], path: tuple[str, ...]) -> float:
    current: Any = value
    for component in path:
        current = current[component]
    return float(current)


def write_aggregate(args: list[str]) -> None:
    (
        output_path,
        dataset_records,
        partitions,
        threads,
        repetitions,
        seed,
        dataset_path,
        dataset_sha,
        warmup_messages,
        duration_seconds,
        poll_interval_ms,
        ingress_target_rate,
        git_sha,
        os_description,
        cpu_description,
        rust_version,
        git_dirty,
        kafka_container,
        kafka_image,
        kafka_image_id,
        ingress_container,
        output_container,
        container_runtime_version,
        container_vm_cpus,
        container_vm_memory_mib,
        *run_paths,
    ) = args
    runs = []
    for run_path in run_paths:
        with Path(run_path).open(encoding="utf-8") as source:
            runs.append(json.load(source))
    if len(runs) != int(repetitions) or any(
        run.get("status") != "passed" for run in runs
    ):
        raise ValueError("aggregate requires every configured repetition to pass")

    def series(path: tuple[str, ...]) -> list[float]:
        return [nested_value(run, path) for run in runs]

    primary = series(
        ("phases", "steady_state", "output_delivery_rate_messages_per_second")
    )
    ingress = series(
        ("phases", "steady_state", "ingress_rate_messages_per_second")
    )
    completion = series(
        ("phases", "drain", "completion_rate_messages_per_second")
    )
    mean_cores = series(("resources", "mean_cores"))
    rss_max = series(("resources", "rss_bytes_max"))
    publication_reasons = []
    if int(repetitions) < 3:
        publication_reasons.append("fewer_than_three_repetitions")
    if int(duration_seconds) < 120:
        publication_reasons.append("steady_state_shorter_than_120_seconds")
    if git_dirty == "true":
        publication_reasons.append("dirty_worktree")
    if any(
        run["phases"]["steady_state"]["classification"] != "engine_saturated"
        for run in runs
    ):
        publication_reasons.append("one_or_more_runs_ingress_limited")

    write_json(
        output_path,
        {
            "schema_version": 3,
            "status": "passed",
            "benchmark": "kafka_sustained_passthrough",
            "completed_at": datetime.now(timezone.utc).isoformat(),
            "parameters": {
                "dataset_records": int(dataset_records),
                "partitions": int(partitions),
                "threads": int(threads),
                "repetitions": int(repetitions),
                "warmup_messages": int(warmup_messages),
                "steady_state_seconds": int(duration_seconds),
                "poll_interval_ms": int(poll_interval_ms),
                "ingress_target_rate_messages_per_second": int(
                    ingress_target_rate
                ),
            },
            "dataset": {
                "path": dataset_path,
                "seed": int(seed),
                "sha256": dataset_sha,
            },
            "environment": {
                "git_sha": git_sha,
                "git_dirty": git_dirty == "true",
                "os": os_description,
                "cpu": cpu_description,
                "rust": rust_version,
                "kafka": {
                    "container": kafka_container,
                    "image": kafka_image,
                    "image_id": kafka_image_id,
                },
                "jobs": {
                    "ingress_container": ingress_container,
                    "output_container": output_container,
                },
                "container_runtime": {
                    "version": container_runtime_version,
                    "vm_cpus": int(container_vm_cpus),
                    "vm_memory_mib": int(container_vm_memory_mib),
                },
            },
            "summary": {
                "primary_metric": "output_delivery_rate_messages_per_second",
                "output_delivery_rate_messages_per_second": summary(primary),
                "ingress_rate_messages_per_second": summary(ingress),
                "completion_rate_messages_per_second": summary(completion),
                "streamforge_mean_cores": summary(mean_cores),
                "streamforge_peak_rss_bytes": summary(rss_max),
            },
            "publication": {
                "eligible": not publication_reasons,
                "reasons": publication_reasons,
            },
            "runs": runs,
        },
    )


def print_summary(path: str) -> None:
    with Path(path).open(encoding="utf-8") as source:
        result = json.load(source)
    output_rate = result["summary"]["output_delivery_rate_messages_per_second"]
    ingress_rate = result["summary"]["ingress_rate_messages_per_second"]
    cores = result["summary"]["streamforge_mean_cores"]
    print(
        "sustained output delivery msg/s: "
        f"median={output_rate['median']:.2f}, "
        f"min={output_rate['min']:.2f}, max={output_rate['max']:.2f}"
    )
    print(
        "sustained ingress msg/s: "
        f"median={ingress_rate['median']:.2f}, "
        f"min={ingress_rate['min']:.2f}, max={ingress_rate['max']:.2f}"
    )
    print(
        "StreamForge mean cores: "
        f"median={cores['median']:.3f}, "
        f"min={cores['min']:.3f}, max={cores['max']:.3f}"
    )
    print(
        "publication eligible: "
        f"{str(result['publication']['eligible']).lower()}"
    )


def main() -> None:
    command, *args = sys.argv[1:]
    commands = {
        "sha256": lambda: sha256(*args),
        "write-config": lambda: write_config(args),
        "write-run": lambda: write_run(args),
        "write-aggregate": lambda: write_aggregate(args),
        "print-summary": lambda: print_summary(*args),
    }
    try:
        action = commands[command]
    except KeyError as error:
        raise SystemExit(f"unknown command: {command}") from error
    action()


if __name__ == "__main__":
    main()
