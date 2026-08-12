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
        "workload_id": "passthrough-1k",
        "publication_eligible": True,
        "repetitions": 3,
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
        )
        self.assertTrue(all(check.passed for check in checks))

    def test_regression_and_missing_latency_fail(self):
        candidate = evidence(throughput_median_messages_per_second=94000)
        del candidate["metrics"]["p99_latency_milliseconds"]
        failed = {
            check.name for check in MODULE.compare(evidence(), candidate) if not check.passed
        }
        self.assertIn("throughput_median_messages_per_second", failed)
        self.assertIn("p99_latency_milliseconds", failed)

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
