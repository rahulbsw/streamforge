#!/usr/bin/env bash
#
# Generate a deterministic JSONL workload for Streamforge benchmarks.
#
# Usage:
#   ./generate_json_test_data.sh [num_messages] [output_file] [seed]
#
# The same message count and seed always produce byte-for-byte identical data.

set -euo pipefail

NUM_MESSAGES=${1:-100000}
OUTPUT_FILE=${2:-test_messages.jsonl}
SEED=${3:-0}

if [[ ! "$NUM_MESSAGES" =~ ^[1-9][0-9]*$ ]]; then
    echo "error: num_messages must be a positive integer" >&2
    exit 2
fi

if [[ ! "$SEED" =~ ^[0-9]+$ ]]; then
    echo "error: seed must be a non-negative integer" >&2
    exit 2
fi

mkdir -p "$(dirname "$OUTPUT_FILE")"

python3 - "$NUM_MESSAGES" "$OUTPUT_FILE" "$SEED" <<'PYTHON'
import json
import os
import sys
from datetime import datetime, timedelta, timezone

num_messages = int(sys.argv[1])
output_file = sys.argv[2]
seed = int(sys.argv[3])

first_names = [
    "Alice", "Bob", "Charlie", "Diana", "Eve", "Frank", "Grace", "Henry",
    "Ivy", "Jack", "Kate", "Liam", "Mia", "Noah", "Olivia", "Peter",
    "Quinn", "Rachel", "Sam", "Tina",
]
last_names = [
    "Smith", "Johnson", "Brown", "Davis", "Wilson", "Moore", "Taylor",
    "Anderson", "Thomas", "Jackson", "White", "Harris", "Martin", "Garcia",
    "Martinez", "Robinson", "Clark", "Rodriguez", "Lewis", "Lee",
]
actions = [
    "login", "logout", "purchase", "view", "click", "search", "add_cart",
    "checkout", "review", "share",
]

# A fixed epoch plus a seed-derived offset keeps timestamps realistic while
# preserving byte-for-byte reproducibility.
base_time = datetime(2026, 1, 1, tzinfo=timezone.utc) + timedelta(seconds=seed)
name_offset = seed % len(first_names)
last_name_offset = (seed * 7) % len(last_names)
action_offset = (seed * 11) % len(actions)

with open(output_file, "w", encoding="utf-8", newline="\n") as output:
    for sequence in range(num_messages):
        user_id = 1000 + ((sequence + seed) % 10000)
        first = first_names[(sequence + name_offset) % len(first_names)]
        last = last_names[(sequence + last_name_offset) % len(last_names)]
        action = actions[(sequence + action_offset) % len(actions)]
        timestamp = (
            base_time + timedelta(milliseconds=sequence)
        ).isoformat(timespec="milliseconds").replace("+00:00", "Z")

        message = {
            "userId": user_id,
            "user": {
                "id": user_id,
                "name": f"{first} {last}",
                "email": f"{first.lower()}.{last.lower()}@example.com",
            },
            "action": action,
            "timestamp": timestamp,
            "metadata": {
                "source": "perf_test",
                "sequence": sequence,
                "batch": sequence // 1000,
                "seed": seed,
            },
        }
        output.write(
            json.dumps(message, sort_keys=True, separators=(",", ":")) + "\n"
        )

size_bytes = os.path.getsize(output_file)
print(
    f"generated {num_messages} deterministic messages "
    f"(seed={seed}, bytes={size_bytes}) at {output_file}"
)
PYTHON
