# StreamForge DSL Specification

**Syntax:** V2 function style
**Status:** Stable
**Last Updated:** 2026-05-27

## Overview

The StreamForge DSL is used in YAML routing destinations to decide whether a record should be published and how its value or key should be shaped. Public docs and examples use V2 syntax only.

## Filter Grammar

```ebnf
filter        = comparison | boolean | regex | existence ;
comparison    = field_ref comparison_op literal ;
boolean       = "and(" filter "," filter { "," filter } ")"
              | "or(" filter "," filter { "," filter } ")"
              | "not(" filter ")" ;
regex         = "regex(" path_ref "," string ")" ;
existence     = "exists(" string ")" | "not_exists(" string ")" ;
field_ref     = "$" identifier { "." identifier }
              | "$(" string ")"
              | "field(" string ")" ;
path_ref      = field_ref | string ;
comparison_op = "==" | "!=" | ">" | ">=" | "<" | "<=" ;
literal       = string | number | boolean | "null" ;
```

## Transform Grammar

```ebnf
transform     = field_ref | construct | hash ;
construct     = "construct(" mapping { "," mapping } ")" ;
mapping       = identifier ( "=" | ":" ) field_ref ;
hash          = "hash(" algorithm "," field_ref [ "," string ] ")" ;
algorithm     = "MD5" | "SHA256" | "SHA512" | "MURMUR64" | "MURMUR128" ;
```

## Field Access

```yaml
filter: "$status == 'active'"
filter: "$customer.tier == 'premium'"
filter: "field('/customer/email') != null"
filter: "$('/field-with-dash') == 'value'"
```

## Filters

```yaml
filter: "$amount >= 100"
filter: "and($status == 'active', $amount >= 100)"
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

## Complete Example

```yaml
appid: "orders-routing"
bootstrap: "localhost:9092"
input: "raw-orders"
offset: "earliest"
threads: 4

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

## Authoring Rules

- Use `$field.path` for normal JSON paths.
- Use `field('/path')` for explicit paths and inside `regex()`.
- Use single quotes inside DSL expressions so YAML quoting stays readable.
- Keep transforms deterministic and side-effect free.
- Validate pipeline files with `streamforge-validate` before deployment.
