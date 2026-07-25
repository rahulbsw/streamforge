#!/usr/bin/env python3
"""Unit tests for sustained benchmark parsing and result validation."""

from __future__ import annotations

import io
import json
import tempfile
import unittest
from pathlib import Path

import benchmark_job_common
import benchmark_ingress_job
import benchmark_output_job
import throughput_results


class MetricParsingTests(unittest.TestCase):
    def test_execution_command_supports_direct_and_container_modes(self) -> None:
        command = ["kafka-topics", "--list"]
        self.assertEqual(
            benchmark_job_common.execution_command(
                "podman", "benchmark-kafka", command, direct=True
            ),
            command,
        )
        self.assertEqual(
            benchmark_job_common.execution_command(
                "podman",
                "benchmark-kafka",
                command,
                direct=False,
                interactive=True,
            ),
            [
                "podman",
                "exec",
                "-i",
                "benchmark-kafka",
                "kafka-topics",
                "--list",
            ],
        )

    def test_exact_destination_and_vector_sum(self) -> None:
        samples = benchmark_job_common.parse_metrics(
            "\n".join(
                [
                    "streamforge_messages_consumed_total 12",
                    'streamforge_messages_produced_total{destination="out"} 10',
                    'streamforge_messages_produced_total{destination="other"} 2',
                    'streamforge_processing_errors_total{error_type="parse"} 1',
                    'streamforge_processing_errors_total{error_type="send"} 2',
                ]
            )
        )
        self.assertEqual(
            benchmark_job_common.metric_value(
                samples,
                "streamforge_messages_produced_total",
                {"destination": "out"},
            ),
            10,
        )
        self.assertEqual(
            benchmark_job_common.metric_value(
                samples, "streamforge_processing_errors_total"
            ),
            3,
        )

    def test_required_metric_is_not_silently_zero(self) -> None:
        with self.assertRaisesRegex(ValueError, "required metric is missing"):
            benchmark_job_common.metric_value([], "missing")

    def test_cpu_time_parser(self) -> None:
        self.assertEqual(benchmark_job_common.parse_cpu_time("01:02"), 62)
        self.assertEqual(benchmark_job_common.parse_cpu_time("1:01:02.5"), 3662.5)
        self.assertEqual(
            benchmark_job_common.parse_cpu_time("2-01:00:00"), 176400
        )


class SustainedResultTests(unittest.TestCase):
    def test_parallel_ingress_distributes_exact_record_count(self) -> None:
        streams = [io.BytesIO(), io.BytesIO()]
        written = benchmark_ingress_job.write_lines(
            streams,
            [b'{"id":1}\n', b'{"id":2}\n'],
            count=5,
            flush_every=2,
        )
        self.assertEqual(written, 5)
        self.assertEqual(
            sum(stream.getvalue().count(b"\n") for stream in streams),
            5,
        )
        self.assertEqual(
            [stream.getvalue().count(b"\n") for stream in streams],
            [3, 2],
        )

    def test_consumer_performance_count_parser(self) -> None:
        output = "\n".join(
            [
                "start.time, end.time, data.consumed.in.MB, MB.sec, "
                "data.consumed.in.nMsg, nMsg.sec",
                "start, end, 1.0, 2.0, 12345, 678.0",
            ]
        )
        self.assertEqual(benchmark_output_job.parse_final_count(output), 12345)

    def write_fixture(self, root: Path) -> list[str]:
        config = root / "config.json"
        ingress = root / "jobs" / "ingress.json"
        observer = root / "jobs" / "observer.json"
        metrics = root / "jobs" / "metrics.json"
        samples = root / "jobs" / "metrics.csv"
        result = root / "result.json"
        ingress.parent.mkdir()
        config.write_text("{}\n", encoding="utf-8")
        samples.write_text("monotonic_ns\n", encoding="utf-8")
        ingress.write_text(
            json.dumps(
                {
                    "status": "passed",
                    "records_sent": 100,
                    "records_per_second": 50,
                }
            ),
            encoding="utf-8",
        )
        observer.write_text(
            json.dumps(
                {
                    "status": "passed",
                    "active_during_measurement": False,
                    "observer_started_monotonic_ns": 3_100_000_000,
                    "completion_monotonic_ns": 4_000_000_000,
                    "measurement_records_final": 100,
                }
            ),
            encoding="utf-8",
        )
        metrics.write_text(
            json.dumps(
                {
                    "status": "passed",
                    "start_monotonic_ns": 1_000_000_000,
                    "window_sample_monotonic_ns": 3_000_000_000,
                    "final_sample_monotonic_ns": 4_000_000_000,
                    "window_deltas": {
                        "consumed": 85,
                        "produced": 82,
                        "delivered": 80,
                        "errors": 0,
                    },
                    "final_deltas": {
                        "consumed": 100,
                        "produced": 100,
                        "delivered": 100,
                        "errors": 0,
                    },
                    "resources": {
                        "mean_cores": 1.5,
                        "rss_bytes_max": 1024,
                    },
                }
            ),
            encoding="utf-8",
        )
        return [
            str(result),
            "1",
            "input",
            "output",
            "group",
            "10",
            "2",
            "110",
            "110",
            str(config),
            str(ingress),
            str(observer),
            str(metrics),
            str(samples),
        ]

    def test_write_run_requires_exact_final_accounting(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            args = self.write_fixture(Path(directory))
            throughput_results.write_run(args)
            result = json.loads(Path(args[0]).read_text(encoding="utf-8"))
            self.assertEqual(result["schema_version"], 3)
            self.assertEqual(result["counts"]["delivered"], 100)
            self.assertEqual(
                result["phases"]["steady_state"][
                    "output_delivery_rate_messages_per_second"
                ],
                40,
            )
            self.assertEqual(
                result["phases"]["steady_state"]["classification"],
                "engine_saturated",
            )

    def test_write_run_rejects_offset_mismatch(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            args = self.write_fixture(Path(directory))
            args[7] = "109"
            with self.assertRaisesRegex(ValueError, "exact accounting"):
                throughput_results.write_run(args)


if __name__ == "__main__":
    unittest.main()
