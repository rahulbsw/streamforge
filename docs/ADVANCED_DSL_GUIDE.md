---
title: DSL Reference
nav_order: 5
---

# V2 DSL Reference

StreamForge uses V2 DSL for documented filters, transforms, and keys. V2 reads like a small expression language instead of a delimiter-heavy configuration format.

## Field Access

Use `$field` for simple paths:

```yaml
filter: "$status == 'active'"
filter: "$customer.tier == 'premium'"
transform: "$customer.id"
```

Use `field('/path')` when the path should be explicit:

```yaml
filter: "field('/customer/email') != null"
transform: "field('/payload/after')"
```

Use `$('/path-with-special-chars')` when a path contains characters that do not work with dot shorthand.

```yaml
filter: "$('/event-type') == 'order_completed'"
```

## Filters

### Comparisons

```yaml
filter: "$amount >= 100"
filter: "$region == 'us'"
filter: "$deleted_at == null"
filter: "$customer.active == true"
```

Supported comparison operators:

```text
== != > >= < <=
```

### Boolean Logic

```yaml
filter: "and($region == 'us', $amount >= 100)"
filter: "or($priority == 'high', $priority == 'urgent')"
filter: "not($customer.tier == 'premium')"
```

For multiline YAML, use folded strings:

```yaml
filter: >
  and(
    $event_type == 'order_completed',
    $customer.tier == 'premium',
    $amount >= 100
  )
```

### Regex

```yaml
filter: "regex(field('/customer/email'), '^[^@]+@[^@]+\\.[^@]+$')"
filter: "regex(field('/event_type'), '^(created|updated|deleted)$')"
```

### Existence

```yaml
filter: "exists('/customer/id')"
filter: "not_exists('/deleted_at')"
filter: "and(exists('/customer/id'), $customer.active == true)"
```

## Transforms

### Extract

```yaml
transform: "$customer.id"
transform: "field('/payload/after')"
```

### Construct

```yaml
transform: "construct(order_id=$order.id, amount=$order.amount, region=$region)"
```

For larger payloads:

```yaml
transform: >
  construct(
    order_id=$order.id,
    customer_id=$customer.id,
    amount=$order.amount,
    currency=$order.currency,
    created_at=$event_time
  )
```

### Hash

Use hashes for keys or privacy-preserving fields.

```yaml
key_transform: "hash('SHA256', $customer.email)"
transform: "hash('SHA256', $customer.email, 'email_hash')"
```

Supported hash algorithms:

- `MD5`
- `SHA256`
- `SHA512`
- `MURMUR64`
- `MURMUR128`

## Key Transforms

```yaml
key_transform: "$order_id"
key_transform: "$customer.id"
key_transform: "hash('SHA256', $customer.email)"
key_transform: "construct(tenant=$tenant.id, user=$user.id)"
```

## Pipeline Examples

### Selective Analytics Feed

```yaml
routing:
  routing_type: "filter"
  destinations:
    - output: "analytics-orders"
      filter: "and($region == 'us', $amount >= 100)"
      transform: "construct(order_id=$order_id, customer_id=$customer.id, amount=$amount, region=$region)"
      key_transform: "$order_id"
```

### PII-Safe Partner Feed

```yaml
routing:
  routing_type: "filter"
  destinations:
    - output: "partner-events"
      filter: "and($consent.third_party == true, exists('/properties/non_pii'))"
      transform: "construct(event=$event_type, properties=$properties.non_pii)"
      key_transform: "hash('SHA256', $user.id)"
```

### CDC to Datalake

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

## Authoring Rules

- Prefer `$field.path` for simple JSON paths.
- Use `field('/path')` when a path must be explicit or appears inside `regex()`.
- Use single quotes inside DSL strings so YAML quoting stays simple.
- Keep one route per downstream contract; do not overload a destination with unrelated filters.
- Validate every config with `streamforge-validate` before deploying.
