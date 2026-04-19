#!/bin/bash
#
# Generate JSON test data for high-throughput testing
#
# Usage:
#   ./generate_json_test_data.sh [num_messages] [output_file]
#
# Examples:
#   ./generate_json_test_data.sh 100000 test_data.jsonl
#   ./generate_json_test_data.sh 1000000 large_test.jsonl
#

set -e

NUM_MESSAGES=${1:-100000}
OUTPUT_FILE=${2:-test_messages.jsonl}

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}  JSON Test Data Generator${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo -e "${GREEN}Messages:${NC} $NUM_MESSAGES"
echo -e "${GREEN}Output:${NC} $OUTPUT_FILE"
echo ""

# Generate test data using Python for speed
python3 - "$NUM_MESSAGES" "$OUTPUT_FILE" << 'EOF'
import json
import sys
import time
from datetime import datetime

num_messages = int(sys.argv[1])
output_file = sys.argv[2]

# Sample data pools
first_names = ["Alice", "Bob", "Charlie", "Diana", "Eve", "Frank", "Grace", "Henry", "Ivy", "Jack",
               "Kate", "Liam", "Mia", "Noah", "Olivia", "Peter", "Quinn", "Rachel", "Sam", "Tina"]
last_names = ["Smith", "Johnson", "Brown", "Davis", "Wilson", "Moore", "Taylor", "Anderson", "Thomas", "Jackson",
              "White", "Harris", "Martin", "Garcia", "Martinez", "Robinson", "Clark", "Rodriguez", "Lewis", "Lee"]
actions = ["login", "logout", "purchase", "view", "click", "search", "add_cart", "checkout", "review", "share"]

print(f"🔄 Generating {num_messages:,} JSON messages...")
start = time.time()

with open(output_file, 'w') as f:
    for i in range(num_messages):
        user_id = 1000 + (i % 10000)
        first = first_names[i % len(first_names)]
        last = last_names[i % len(last_names)]
        action = actions[i % len(actions)]

        message = {
            "userId": user_id,
            "user": {
                "id": user_id,
                "name": f"{first} {last}",
                "email": f"{first.lower()}.{last.lower()}@example.com"
            },
            "action": action,
            "timestamp": datetime.utcnow().isoformat() + "Z",
            "metadata": {
                "source": "perf_test",
                "sequence": i,
                "batch": i // 1000
            }
        }

        f.write(json.dumps(message) + '\n')

        # Progress indicator
        if (i + 1) % 10000 == 0:
            elapsed = time.time() - start
            rate = (i + 1) / elapsed
            print(f"  Progress: {i+1:,} messages ({rate:,.0f} msg/s)", flush=True)

elapsed = time.time() - start
rate = num_messages / elapsed
print(f"\n✅ Generated {num_messages:,} messages in {elapsed:.2f}s ({rate:,.0f} msg/s)")

# File size
import os
size_mb = os.path.getsize(output_file) / (1024 * 1024)
print(f"📦 File size: {size_mb:.2f} MB")
EOF

echo ""
echo -e "${GREEN}✅ Test data ready!${NC}"
echo ""
echo -e "${YELLOW}Sample messages:${NC}"
head -3 "$OUTPUT_FILE"
echo ""
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}  Usage Examples${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo "1. Stream to Kafka (console producer):"
echo "   cat $OUTPUT_FILE | kafka-console-producer \\"
echo "     --bootstrap-server localhost:9092 \\"
echo "     --topic test-8p-input"
echo ""
echo "2. Batch load with rate limit:"
echo "   cat $OUTPUT_FILE | kafka-console-producer \\"
echo "     --bootstrap-server localhost:9092 \\"
echo "     --topic test-8p-input \\"
echo "     --batch-size 1000"
echo ""
echo "3. Check message format:"
echo "   head -1 $OUTPUT_FILE | jq ."
echo ""
