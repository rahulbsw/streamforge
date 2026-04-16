"""
Demo event producer — generates 100 synthetic e-commerce events.
Run via docker-compose or directly: python produce_events.py
"""

import json
import os
import random
import time
from kafka import KafkaProducer

BOOTSTRAP = os.getenv("KAFKA_BOOTSTRAP", "localhost:9092")
TOPIC = "raw-events"
COUNT = 100

TIERS = ["free", "standard", "premium", "enterprise"]
STATUSES = ["active", "active", "active", "suspended"]  # weighted toward active
EVENT_TYPES = ["page.view", "item.added", "checkout.started", "purchase.completed", "login"]

def make_event(i):
    tier = random.choice(TIERS)
    has_user = random.random() > 0.05  # 5% missing userId (will go to DLQ)
    return {
        "userId":     f"usr-{i:04d}" if has_user else None,
        "email":      f"User{i}@Example.COM" if has_user else None,
        "phone":      f"+1-555-{i:04d}" if has_user else None,
        "tier":       tier,
        "status":     random.choice(STATUSES),
        "eventType":  random.choice(EVENT_TYPES),
        "totalSpend": round(random.uniform(0, 5000), 2),
        "timestamp":  int(time.time() * 1000),
        "sessionId":  f"sess-{random.randint(1000, 9999)}",
    }

def main():
    print(f"Connecting to Kafka at {BOOTSTRAP}...")
    producer = KafkaProducer(
        bootstrap_servers=BOOTSTRAP,
        value_serializer=lambda v: json.dumps(v).encode("utf-8"),
        key_serializer=lambda k: k.encode("utf-8") if k else None,
        retries=5,
    )

    print(f"Producing {COUNT} events to {TOPIC}...")
    for i in range(COUNT):
        event = make_event(i)
        key = event.get("userId")
        producer.send(TOPIC, key=key, value=event)
        if i % 10 == 0:
            print(f"  Sent {i}/{COUNT} events")
        time.sleep(0.05)

    producer.flush()
    print(f"\nDone. {COUNT} events sent to {TOPIC}.")
    print("\nWatch output:")
    print(f"  docker exec -it demo-kafka kafka-console-consumer \\")
    print(f"    --bootstrap-server localhost:9092 --topic processed-events --from-beginning")
    print(f"\n  docker exec -it demo-kafka kafka-console-consumer \\")
    print(f"    --bootstrap-server localhost:9092 --topic pii-safe --from-beginning")

if __name__ == "__main__":
    main()
