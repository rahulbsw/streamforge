#!/usr/bin/env python3
"""
High-performance JSON message producer for Kafka benchmarking
"""

import json
import time
import sys
from kafka import KafkaProducer
from kafka.errors import KafkaError

def create_message(msg_id, size_bytes=1024):
    """Create a JSON message of approximately size_bytes"""
    base_msg = {
        "id": msg_id,
        "message": {
            "confId": msg_id,
            "siteId": (msg_id % 10000) + 10000,
            "status": "active" if msg_id % 2 == 0 else "inactive",
            "timestamp": int(time.time()),
            "priority": "high" if msg_id % 3 == 0 else "normal"
        },
        "metadata": {
            "producer": "json-producer.py",
            "version": "1.0"
        }
    }

    # Pad to reach desired size
    base_size = len(json.dumps(base_msg))
    if base_size < size_bytes:
        padding_size = size_bytes - base_size - 50  # Leave room for padding field
        base_msg["padding"] = "x" * max(0, padding_size)

    return base_msg

def produce_messages(bootstrap_servers, topic, num_messages, target_rate, message_size):
    """Produce messages to Kafka at target rate"""

    producer = KafkaProducer(
        bootstrap_servers=bootstrap_servers,
        value_serializer=lambda v: json.dumps(v).encode('utf-8'),
        key_serializer=lambda k: str(k).encode('utf-8') if k else None,
        acks=1,
        compression_type=None,
        batch_size=65536,
        linger_ms=10,
        buffer_memory=33554432
    )

    print(f"Starting production: {num_messages} messages at {target_rate} msg/s")
    print(f"Message size: ~{message_size} bytes")
    print("")

    start_time = time.time()
    sent_count = 0
    error_count = 0

    # Calculate sleep time for rate limiting
    sleep_time = 1.0 / target_rate if target_rate > 0 else 0

    try:
        for i in range(num_messages):
            msg = create_message(i, message_size)
            key = str(i)

            try:
                future = producer.send(topic, key=key, value=msg)
                # Don't wait for each send, just track it
                sent_count += 1

                # Report progress
                if (i + 1) % 10000 == 0:
                    elapsed = time.time() - start_time
                    rate = sent_count / elapsed if elapsed > 0 else 0
                    print(f"Sent {sent_count} messages ({rate:.1f} msg/s)")

                # Rate limiting
                if sleep_time > 0:
                    time.sleep(sleep_time)

            except KafkaError as e:
                error_count += 1
                if error_count < 10:
                    print(f"Error sending message {i}: {e}")

        # Flush remaining messages
        print("Flushing...")
        producer.flush(timeout=30)

    except KeyboardInterrupt:
        print("\nInterrupted by user")

    finally:
        producer.close()

    end_time = time.time()
    duration = end_time - start_time
    actual_rate = sent_count / duration if duration > 0 else 0

    print("")
    print("=" * 50)
    print("PRODUCTION COMPLETE")
    print("=" * 50)
    print(f"Messages sent: {sent_count}")
    print(f"Errors: {error_count}")
    print(f"Duration: {duration:.2f}s")
    print(f"Actual rate: {actual_rate:.1f} msg/s")
    print("")

    return sent_count, error_count, duration

if __name__ == "__main__":
    if len(sys.argv) < 5:
        print("Usage: python3 json-producer.py <bootstrap_servers> <topic> <num_messages> <target_rate_per_sec> [message_size]")
        print("Example: python3 json-producer.py localhost:9092 test-input 100000 10000 1024")
        sys.exit(1)

    bootstrap_servers = sys.argv[1]
    topic = sys.argv[2]
    num_messages = int(sys.argv[3])
    target_rate = int(sys.argv[4])
    message_size = int(sys.argv[5]) if len(sys.argv) > 5 else 1024

    produce_messages(bootstrap_servers, topic, num_messages, target_rate, message_size)
