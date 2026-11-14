import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("performance_release_gate.py")
SPEC = importlib.util.spec_from_file_location("performance_release_gate", SCRIPT)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def evidence(**metrics):
    return {
        "schema_version": 1,
        "workload": {
            "profile_id": "passthrough",
            "payload_size_bytes": 1024,
            "processing_mode": "partition_ordered",
            "delivery_mode": "acknowledged",
            "destinations": 1,
        },
        "publication_eligible": True,
        "repetitions": 3,
        "environment": {"dedicated_hardware": True, "host_id": "benchmark-host-01"},
        "teardown": {"completed": True, "evidence": "artifact://run/teardown"},
        "cost": {"amount": 1.25, "currency": "USD", "evidence": "artifact://run/cost"},
        "accounting": {"exact": True, "errors": 0},
        "metrics": {
            "throughput_median_messages_per_second": 100000,
            "throughput_cv": 0.01,
            "p99_latency_milliseconds": 20,
            "peak_rss_bytes": 100000000,
            **metrics,
        },
    }


class PerformanceReleaseGateTests(unittest.TestCase):
    def test_candidate_within_thresholds_passes(self):
        checks = MODULE.compare(
            evidence(),
            evidence(
                throughput_median_messages_per_second=95000,
                p99_latency_milliseconds=22,
                peak_rss_bytes=110000000,
            ),
            MODULE.load_json(MODULE.CATALOG_PATH),
        )
        self.assertTrue(all(check.passed for check in checks))

    def test_regression_and_missing_latency_fail(self):
        candidate = evidence(throughput_median_messages_per_second=94000)
        del candidate["metrics"]["p99_latency_milliseconds"]
        catalog = MODULE.load_json(MODULE.CATALOG_PATH)
        failed = {
            check.name
            for check in MODULE.compare(evidence(), candidate, catalog)
            if not check.passed
        }
        self.assertIn("throughput_median_messages_per_second", failed)
        self.assertIn("p99_latency_milliseconds", failed)

    def test_publication_claim_does_not_replace_catalog_evidence(self):
        candidate = evidence()
        candidate["publication_eligible"] = True
        del candidate["teardown"]
        del candidate["cost"]
        candidate["environment"] = {"dedicated_hardware": True}
        failed = {
            check.name
            for check in MODULE.compare(
                evidence(), candidate, MODULE.load_json(MODULE.CATALOG_PATH)
            )
            if not check.passed
        }
        self.assertIn("candidate.teardown_evidence", failed)
        self.assertIn("candidate.cost_evidence", failed)
        self.assertIn("candidate.dedicated_hardware", failed)

    def test_catalog_rejects_unlisted_payload_and_invalid_mode_pair(self):
        candidate = evidence()
        candidate["workload"]["payload_size_bytes"] = 2048
        candidate["workload"]["processing_mode"] = "legacy_batch"
        candidate["workload"]["delivery_mode"] = "queued"
        failed = {
            check.name
            for check in MODULE.compare(
                evidence(), candidate, MODULE.load_json(MODULE.CATALOG_PATH)
            )
            if not check.passed
        }
        self.assertIn("candidate.payload_size", failed)
        self.assertIn("candidate.mode_pair", failed)

    def test_error_counts_must_be_nonnegative_integers(self):
        catalog = MODULE.load_json(MODULE.CATALOG_PATH)
        for error_count in (-1, 0.5, True):
            with self.subTest(error_count=error_count):
                candidate = evidence()
                candidate["accounting"]["errors"] = error_count
                failed = {
                    check.name
                    for check in MODULE.compare(evidence(), candidate, catalog)
                    if not check.passed
                }
                self.assertIn("candidate.zero_errors", failed)

    def test_repetition_count_must_be_an_integer(self):
        candidate = evidence()
        candidate["repetitions"] = 3.5
        failed = {
            check.name
            for check in MODULE.compare(
                evidence(), candidate, MODULE.load_json(MODULE.CATALOG_PATH)
            )
            if not check.passed
        }
        self.assertIn("candidate.repetitions", failed)

    def test_core_metrics_must_be_finite_and_positive(self):
        candidate = evidence(
            throughput_median_messages_per_second=float("inf"),
            p99_latency_milliseconds=-1,
            peak_rss_bytes=0,
        )
        failed = {
            check.name
            for check in MODULE.compare(
                evidence(), candidate, MODULE.load_json(MODULE.CATALOG_PATH)
            )
            if not check.passed
        }
        self.assertIn("throughput_median_messages_per_second", failed)
        self.assertIn("p99_latency_milliseconds", failed)
        self.assertIn("peak_rss_bytes", failed)

    def test_exception_requires_owner_and_milestone(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "exceptions.json"
            path.write_text(
                json.dumps(
                    {
                        "exceptions": [
                            {
                                "check": "p99_latency_milliseconds",
                                "owner": "performance",
                                "removal_milestone": "v1.4.1",
                                "reason": "baseline capture scheduled",
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            loaded = MODULE.load_exceptions(path)
            self.assertEqual(loaded["p99_latency_milliseconds"]["owner"], "performance")


if __name__ == "__main__":
    unittest.main()
