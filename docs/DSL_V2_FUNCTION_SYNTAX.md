# V2 Function-Style DSL

**Status:** Stable
**Last Updated:** 2026-05-27

StreamForge uses V2 function-style DSL in public documentation and examples.

## Filters

```yaml
filter: "$status == 'active'"
filter: "$customer.tier == 'premium'"
filter: "and($region == 'us', $amount >= 100)"
filter: "or($priority == 'high', $priority == 'urgent')"
filter: "not($test == true)"
filter: "regex(field('/email'), '^[^@]+@[^@]+\\.[^@]+$')"
filter: "exists('/customer/id')"
filter: "not_exists('/deleted_at')"
```

## Transforms

```yaml
transform: "$customer.id"
transform: "field('/payload/after')"
transform: "construct(order_id=$order.id, amount=$order.amount, region=$region)"
transform: "hash('SHA256', $customer.email, 'email_hash')"
```

## Key Transforms

```yaml
key_transform: "$order_id"
key_transform: "$customer.id"
key_transform: "hash('SHA256', $customer.email)"
key_transform: "construct(tenant=$tenant.id, user=$user.id)"
```

## Example

```yaml
routing:
  routing_type: "filter"
  destinations:
    - output: "analytics-orders"
      filter: "and($region == 'us', $amount >= 100)"
      transform: "construct(order_id=$order_id, customer_id=$customer.id, amount=$amount, region=$region)"
      key_transform: "$order_id"

    - output: "pii-safe-orders"
      filter: "regex(field('/customer/email'), '^[^@]+@[^@]+\\.[^@]+$')"
      transform: "construct(order_id=$order_id, amount=$amount, region=$region)"
      key_transform: "hash('SHA256', $customer.email)"
```
