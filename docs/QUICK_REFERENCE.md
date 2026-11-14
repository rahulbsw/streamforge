# StreamForge Quick Reference

## Run

```bash
cargo build --release
CONFIG_FILE=config.yaml ./target/release/streamforge
```

Validate before running:

```bash
cargo run --quiet --bin streamforge-validate -- config.yaml
```

## Minimal YAML

```yaml
appid: streamforge
bootstrap: source-kafka:9092
target_broker: target-kafka:9092
input: source-topic
output: destination-topic
threads: 4
```

## V2 DSL Filters

StreamForge documentation uses V2 DSL only. Field access uses `$field` for simple paths and `field('/path')` for explicit JSON paths.

| Need | V2 DSL |
| --- | --- |
| Equality | `$status == 'active'` |
| Numeric comparison | `$amount >= 100` |
| Nested field | `$customer.tier == 'premium'` |
| Boolean AND | `and($status == 'active', $amount >= 100)` |
| Boolean OR | `or($priority == 'high', $priority == 'urgent')` |
| Boolean NOT | `not($test == true)` |
| Regex | `regex(field('/email'), '^[^@]+@[^@]+\\.[^@]+$')` |
| Exists | `exists('/customer/id')` |
| Not exists | `not_exists('/deleted_at')` |
| Null check | `$deleted_at == null` |

## V2 DSL Transforms

| Need | V2 DSL |
| --- | --- |
| Extract a value | `field('/customer/id')` |
| Extract shorthand | `$customer.id` |
| Build an object | `construct(id=$customer.id, amount=$amount, region=$region)` |
| Hash a field | `hash('SHA256', $customer.email)` |
| Hash into named output field | `hash('SHA256', $customer.email, 'email_hash')` |
| Build a key from a field | `key_transform: "$order_id"` |
| Build a hashed key | `key_transform: "hash('SHA256', $customer.email)"` |

## Common Patterns

### Content Routing

```yaml
routing:
  routing_type: "filter"
  destinations:
    - output: "user-events"
      filter: "regex(field('/type'), '^user')"
      transform: "field('/user')"

    - output: "order-events"
      filter: "regex(field('/type'), '^order')"
      transform: "construct(order_id=$order.id, amount=$order.total)"
```

### PII-Safe Output

```yaml
routing:
  routing_type: "filter"
  destinations:
    - output: "analytics-users"
      filter: "and(exists('/user/id'), $consent.analytics == true)"
      transform: "construct(user_id=$user.id, event=$event_type, region=$region)"
      key_transform: "hash('SHA256', $user.id)"
```

### CDC Routing

```yaml
routing:
  routing_type: "filter"
  destinations:
    - output: "datalake-orders"
      filter: "or($payload.op == 'c', $payload.op == 'u')"
      transform: "field('/payload/after')"
      key_transform: "$payload.after.id"

    - output: "datalake-orders-deleted"
      filter: "$payload.op == 'd'"
      transform: "construct(id=$payload.before.id, deleted_at=$payload.ts_ms)"
      key_transform: "$payload.before.id"
```

## Performance Tuning

```yaml
threads: 8
compression:
  compression_type: raw
  compression_algo: zstd
performance:
  fetch_min_bytes: 1048576
  fetch_max_wait_ms: 100
  batch_size: 1000
  linger_ms: 10
```

## Monitoring

```bash
curl --fail http://localhost:9090/health
curl --fail http://localhost:9090/ready
curl --fail http://localhost:9090/metrics
```

Useful metrics:

- `streamforge_messages_consumed_total`
- `streamforge_messages_produced_total`
- `streamforge_messages_delivered_total`
- `streamforge_messages_filtered_total`
- `streamforge_consumer_lag`
- `streamforge_processing_duration_seconds`

## Troubleshooting

```bash
RUST_LOG=debug CONFIG_FILE=config.yaml ./target/release/streamforge
cargo run --quiet --bin streamforge-validate -- config.yaml
```

For filter debugging, add a temporary catch-all output:

```yaml
routing:
  routing_type: "filter"
  destinations:
    - output: "debug-all-events"
      transform: "field('/')"
```
