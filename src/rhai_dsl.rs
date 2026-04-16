use rhai::{Dynamic, Engine, Scope, AST};
use serde_json::Value;
use std::sync::Arc;
use tracing::debug;

use crate::cache::SyncCacheManager;
use crate::config::DslExpr;
use crate::envelope::MessageEnvelope;
use crate::error::{MirrorMakerError, Result};
use crate::filter::{Filter, Transform};

// ============================================================================
// RhaiEngine
// ============================================================================

/// Shared Rhai engine — pre-configured with all StreamForge built-in functions.
///
/// Create once at startup with [`RhaiEngine::new`], wrap in `Arc`, and share
/// across all processor threads. All expressions are compiled to ASTs at
/// config-load time; per-message overhead is AST evaluation only (~500 ns–1 µs).
///
/// # Scope variables (available in every filter and transform expression)
///
/// | Variable    | Type   | Description                                      |
/// |-------------|--------|--------------------------------------------------|
/// | `msg`       | Map    | Message payload (JSON object fields as map keys) |
/// | `key`       | String | Message key (empty string if absent)             |
/// | `headers`   | Map    | Kafka headers: header-name → UTF-8 string value  |
/// | `timestamp` | i64    | Message timestamp in milliseconds since epoch    |
///
/// # Built-in functions
///
/// | Function                       | Returns | Description                              |
/// |--------------------------------|---------|------------------------------------------|
/// | `is_null(v)`                   | bool    | True if `v` is absent or JSON null       |
/// | `is_empty(v)`                  | bool    | True if `v` is the empty string `""`     |
/// | `is_null_or_empty(v)`          | bool    | True if null, absent, or `""`            |
/// | `not_null(v)`                  | bool    | True if `v` is not null/absent           |
/// | `not_empty(v)`                 | bool    | True if `v` is a non-empty string        |
/// | `now_ms()`                     | i64     | Current wall-clock time in milliseconds  |
/// | `cache_lookup(store, key)`     | Dynamic | Look up a value from a named cache store |
/// | `cache_put(store, key, value)` | unit    | Write a value to a named cache store     |
///
/// # Filter examples
///
/// ```text
/// msg["status"] == "active" && msg["score"] > 80
/// key.starts_with("user-")
/// headers["x-tenant"] == "production"
/// is_null_or_empty(msg["email"])
/// (now_ms() - timestamp) / 1000 < 300
/// msg["tier"] in ["premium", "enterprise"]
/// ```
///
/// # Transform examples
///
/// ```text
/// // Return a new object (CONSTRUCT equivalent)
/// #{id: msg["userId"], email: msg["email"].to_lower()}
///
/// // Conditional based on payload, key, or headers
/// if key.starts_with("vip-") { msg + #{tier: "premium"} } else { msg }
///
/// // Coalesce (first non-null)
/// msg["preferredName"] ?? msg["displayName"] ?? msg["email"]
///
/// // Cache enrichment
/// let profile = cache_lookup("profiles", msg["userId"]);
/// msg + #{tier: profile["tier"] ?? "standard"}
///
/// // Multi-statement script
/// let lower_email = msg["email"].to_lower();
/// cache_put("seen", lower_email, msg);
/// msg + #{email: lower_email, processed: true}
/// ```
pub struct RhaiEngine {
    engine: Arc<Engine>,
}

impl RhaiEngine {
    /// Build a new engine and register all StreamForge built-in functions.
    pub fn new(cache_manager: Option<Arc<SyncCacheManager>>) -> Self {
        let mut engine = Engine::new();

        // --- Security limits -----------------------------------------------
        // Prevent runaway or malicious user-supplied scripts.
        engine.set_max_operations(100_000);
        engine.set_max_expr_depths(64, 32);

        // --- Null / empty helpers -------------------------------------------
        engine.register_fn("is_null", |v: Dynamic| v.is_unit());
        engine.register_fn("is_empty", |v: Dynamic| {
            v.clone()
                .into_string()
                .map(|s| s.is_empty())
                .unwrap_or(false)
        });
        engine.register_fn("is_null_or_empty", |v: Dynamic| {
            v.is_unit()
                || v.clone()
                    .into_string()
                    .map(|s| s.is_empty())
                    .unwrap_or(false)
        });
        engine.register_fn("not_null", |v: Dynamic| !v.is_unit());
        engine.register_fn("not_empty", |v: Dynamic| {
            v.clone()
                .into_string()
                .map(|s| !s.is_empty())
                .unwrap_or(false)
        });

        // --- Time helper ----------------------------------------------------
        engine.register_fn("now_ms", || -> i64 {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64
        });

        // --- Cache functions ------------------------------------------------
        match cache_manager {
            Some(mgr) => {
                let mgr_get = mgr.clone();
                engine.register_fn(
                    "cache_lookup",
                    move |store: String, key: String| -> Dynamic {
                        let cache = mgr_get.get_or_create(&store);
                        match cache.get(&key) {
                            Some(val) => rhai::serde::to_dynamic(val).unwrap_or(Dynamic::UNIT),
                            None => Dynamic::UNIT,
                        }
                    },
                );
                let mgr_put = mgr.clone();
                engine.register_fn(
                    "cache_put",
                    move |store: String, key: String, val: Dynamic| {
                        if let Ok(json_val) = rhai::serde::from_dynamic::<Value>(&val) {
                            mgr_put.get_or_create(&store).put(key, json_val);
                        }
                    },
                );
            }
            None => {
                // No-op stubs when cache is not configured.
                engine.register_fn("cache_lookup", |_store: String, _key: String| -> Dynamic {
                    Dynamic::UNIT
                });
                engine.register_fn(
                    "cache_put",
                    |_store: String, _key: String, _val: Dynamic| {},
                );
            }
        }

        Self {
            engine: Arc::new(engine),
        }
    }

    /// Compile a filter expression (must evaluate to `bool`).
    ///
    /// Compilation happens once at config-load time. The resulting `RhaiFilter`
    /// holds the compiled AST and can be evaluated cheaply per message.
    ///
    /// On a runtime evaluation error the filter returns `false` (fail-safe).
    pub fn compile_filter(&self, expr: &str) -> Result<Arc<dyn Filter>> {
        let ast = self
            .engine
            .compile_expression(expr)
            .map_err(|e| MirrorMakerError::Config(format!("Filter compile error: {e}")))?;
        Ok(Arc::new(RhaiFilter {
            engine: self.engine.clone(),
            ast: Arc::new(ast),
            source: expr.to_string(),
        }))
    }

    /// Compile a transform expression or multi-statement script.
    ///
    /// The last evaluated expression value becomes the new message payload.
    /// On a runtime evaluation error the transform passes the original
    /// message through unchanged (fail-safe).
    pub fn compile_transform(&self, script: &str) -> Result<Arc<dyn Transform>> {
        let ast = self
            .engine
            .compile(script)
            .map_err(|e| MirrorMakerError::Config(format!("Transform compile error: {e}")))?;
        Ok(Arc::new(RhaiTransform {
            engine: self.engine.clone(),
            ast: Arc::new(ast),
            source: script.to_string(),
        }))
    }

    /// Create an engine with its own internal `SyncCacheManager`.
    /// Use this when `main.rs` does not need a separate handle to the cache.
    pub fn new_with_cache() -> Self {
        let mgr = Arc::new(crate::cache::SyncCacheManager::new());
        Self::new(Some(mgr))
    }

    /// Compile a filter from a [`DslExpr`] — single expression or array.
    ///
    /// An array is compiled by joining all expressions with `&&`, so every
    /// condition in the list must pass for the message to be forwarded.
    pub fn compile_filter_expr(&self, expr: &DslExpr) -> Result<Arc<dyn Filter>> {
        match expr {
            DslExpr::Single(s) => self.compile_filter(s),
            DslExpr::Multi(parts) => {
                if parts.is_empty() {
                    return Err(MirrorMakerError::Config(
                        "filter array must have at least one expression".to_string(),
                    ));
                }
                if parts.len() == 1 {
                    return self.compile_filter(&parts[0]);
                }
                // AND all parts: (expr1) && (expr2) && ...
                let combined = parts
                    .iter()
                    .map(|p| format!("({})", p))
                    .collect::<Vec<_>>()
                    .join(" && ");
                self.compile_filter(&combined)
            }
        }
    }

    /// Compile a transform from a [`DslExpr`] — single script or pipeline.
    ///
    /// An array is compiled as a sequential pipeline: each script's output
    /// value becomes the `msg` input for the next script in the list.
    pub fn compile_transform_expr(&self, expr: &DslExpr) -> Result<Arc<dyn Transform>> {
        match expr {
            DslExpr::Single(s) => self.compile_transform(s),
            DslExpr::Multi(parts) => {
                if parts.is_empty() {
                    return Err(MirrorMakerError::Config(
                        "transform array must have at least one expression".to_string(),
                    ));
                }
                if parts.len() == 1 {
                    return self.compile_transform(&parts[0]);
                }
                let steps = parts
                    .iter()
                    .map(|p| self.compile_transform(p))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Arc::new(ChainedTransform { steps }))
            }
        }
    }

    /// Return the `Arc<Engine>` for sharing across threads.
    pub fn engine(&self) -> Arc<Engine> {
        self.engine.clone()
    }
}

// ============================================================================
// ChainedTransform — pipeline of sequential transform steps
// ============================================================================

/// Runs a sequence of transforms in order. The output `Value` of each step
/// becomes the `msg` payload passed to the next step.
struct ChainedTransform {
    steps: Vec<Arc<dyn Transform>>,
}

impl Transform for ChainedTransform {
    fn transform(&self, envelope: &MessageEnvelope) -> Result<Value> {
        let mut current = envelope.clone();
        for step in &self.steps {
            let new_value = step.transform(&current)?;
            current.value = new_value;
        }
        Ok(current.value)
    }
}

// ============================================================================
// RhaiFilter
// ============================================================================

struct RhaiFilter {
    engine: Arc<Engine>,
    ast: Arc<AST>,
    source: String,
}

impl Filter for RhaiFilter {
    fn evaluate(&self, value: &Value) -> Result<bool> {
        let envelope = MessageEnvelope::new(value.clone());
        self.evaluate_envelope(&envelope)
    }

    fn evaluate_envelope(&self, envelope: &MessageEnvelope) -> Result<bool> {
        let mut scope = build_scope(envelope)?;
        match self
            .engine
            .eval_ast_with_scope::<bool>(&mut scope, &self.ast)
        {
            Ok(b) => Ok(b),
            Err(e) => {
                debug!(
                    "Filter '{}' runtime error (treating as no-match): {}",
                    self.source, e
                );
                Ok(false) // fail-safe: evaluation errors = message does not match
            }
        }
    }
}

// ============================================================================
// RhaiTransform
// ============================================================================

struct RhaiTransform {
    engine: Arc<Engine>,
    ast: Arc<AST>,
    source: String,
}

impl Transform for RhaiTransform {
    fn transform(&self, envelope: &MessageEnvelope) -> Result<Value> {
        let mut scope = build_scope(envelope)?;
        match self
            .engine
            .eval_ast_with_scope::<Dynamic>(&mut scope, &self.ast)
        {
            Ok(result) => match rhai::serde::from_dynamic::<Value>(&result) {
                Ok(v) => Ok(v),
                Err(e) => {
                    debug!(
                        "Transform '{}' result conversion error (passing through): {}",
                        self.source, e
                    );
                    Ok(envelope.value.clone()) // fail-safe
                }
            },
            Err(e) => {
                debug!(
                    "Transform '{}' runtime error (passing through): {}",
                    self.source, e
                );
                Ok(envelope.value.clone()) // fail-safe
            }
        }
    }
}

// ============================================================================
// Scope builder — shared by filter and transform evaluation
// ============================================================================

/// Build a Rhai `Scope` populated with the standard StreamForge envelope variables:
/// `msg`, `key`, `headers`, `timestamp`.
fn build_scope(envelope: &MessageEnvelope) -> Result<Scope<'static>> {
    let mut scope = Scope::new();

    // msg — message payload as a Rhai Map (JSON object) or scalar
    let msg = rhai::serde::to_dynamic(envelope.value.clone())
        .map_err(|e| MirrorMakerError::Processing(format!("Scope build error: {e}")))?;
    scope.push("msg", msg);

    // key — message key as a String; empty string when absent
    let key: String = match &envelope.key {
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => String::new(),
    };
    scope.push("key", key);

    // headers — Kafka headers as a Rhai Map (name → UTF-8 string value)
    let headers: rhai::Map = envelope
        .headers
        .iter()
        .map(|(name, bytes)| {
            let value = String::from_utf8_lossy(bytes).into_owned();
            (name.clone().into(), Dynamic::from(value))
        })
        .collect();
    scope.push("headers", headers);

    // timestamp — milliseconds since Unix epoch; 0 when absent
    scope.push("timestamp", envelope.timestamp.unwrap_or(0_i64));

    Ok(scope)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::SyncCacheManager;
    use serde_json::json;

    fn engine() -> RhaiEngine {
        RhaiEngine::new(None)
    }

    fn engine_with_cache() -> (RhaiEngine, Arc<SyncCacheManager>) {
        let mgr = Arc::new(SyncCacheManager::new());
        let engine = RhaiEngine::new(Some(mgr.clone()));
        (engine, mgr)
    }

    // --- Filter tests -------------------------------------------------------

    #[test]
    fn test_filter_simple_eq() {
        let e = engine();
        let f = e.compile_filter(r#"msg["status"] == "active""#).unwrap();
        assert!(f.evaluate(&json!({"status": "active"})).unwrap());
        assert!(!f.evaluate(&json!({"status": "inactive"})).unwrap());
    }

    #[test]
    fn test_filter_numeric_gt() {
        let e = engine();
        let f = e.compile_filter("msg[\"score\"] > 80").unwrap();
        assert!(f.evaluate(&json!({"score": 90})).unwrap());
        assert!(!f.evaluate(&json!({"score": 70})).unwrap());
    }

    #[test]
    fn test_filter_and() {
        let e = engine();
        let f = e
            .compile_filter(r#"msg["status"] == "active" && msg["score"] > 80"#)
            .unwrap();
        assert!(f
            .evaluate(&json!({"status": "active", "score": 90}))
            .unwrap());
        assert!(!f
            .evaluate(&json!({"status": "active", "score": 70}))
            .unwrap());
        assert!(!f
            .evaluate(&json!({"status": "inactive", "score": 90}))
            .unwrap());
    }

    #[test]
    fn test_filter_or() {
        let e = engine();
        let f = e
            .compile_filter(r#"msg["type"] == "login" || msg["type"] == "signup""#)
            .unwrap();
        assert!(f.evaluate(&json!({"type": "login"})).unwrap());
        assert!(f.evaluate(&json!({"type": "signup"})).unwrap());
        assert!(!f.evaluate(&json!({"type": "logout"})).unwrap());
    }

    #[test]
    fn test_filter_not() {
        let e = engine();
        let f = e.compile_filter(r#"msg["status"] != "banned""#).unwrap();
        assert!(f.evaluate(&json!({"status": "active"})).unwrap());
        assert!(!f.evaluate(&json!({"status": "banned"})).unwrap());
    }

    #[test]
    fn test_filter_in_operator() {
        let e = engine();
        let f = e
            .compile_filter(r#"msg["tier"] in ["premium", "enterprise"]"#)
            .unwrap();
        assert!(f.evaluate(&json!({"tier": "premium"})).unwrap());
        assert!(f.evaluate(&json!({"tier": "enterprise"})).unwrap());
        assert!(!f.evaluate(&json!({"tier": "basic"})).unwrap());
    }

    #[test]
    fn test_filter_regex_contains() {
        let e = engine();
        let f = e
            .compile_filter(r#"msg["email"].contains("@company.com")"#)
            .unwrap();
        assert!(f.evaluate(&json!({"email": "user@company.com"})).unwrap());
        assert!(!f.evaluate(&json!({"email": "user@other.com"})).unwrap());
    }

    #[test]
    fn test_filter_string_starts_with() {
        let e = engine();
        let f = e
            .compile_filter(r#"msg["id"].starts_with("usr-")"#)
            .unwrap();
        assert!(f.evaluate(&json!({"id": "usr-123"})).unwrap());
        assert!(!f.evaluate(&json!({"id": "org-456"})).unwrap());
    }

    #[test]
    fn test_filter_key_access() {
        let e = engine();
        let f = e.compile_filter(r#"key.starts_with("premium-")"#).unwrap();

        let mut env = MessageEnvelope::new(json!({}));
        env.key = Some(json!("premium-user1"));
        assert!(f.evaluate_envelope(&env).unwrap());

        env.key = Some(json!("basic-user1"));
        assert!(!f.evaluate_envelope(&env).unwrap());
    }

    #[test]
    fn test_filter_header_access() {
        let e = engine();
        let f = e
            .compile_filter(r#"headers["x-tenant"] == "production""#)
            .unwrap();

        let mut env = MessageEnvelope::new(json!({}));
        env.headers
            .insert("x-tenant".to_string(), b"production".to_vec());
        assert!(f.evaluate_envelope(&env).unwrap());

        env.headers
            .insert("x-tenant".to_string(), b"staging".to_vec());
        assert!(!f.evaluate_envelope(&env).unwrap());
    }

    #[test]
    fn test_filter_timestamp_age() {
        let e = engine();
        let f = e
            .compile_filter("(now_ms() - timestamp) / 1000 < 3600")
            .unwrap();

        // Recent message (1 second ago)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let mut env = MessageEnvelope::new(json!({}));
        env.timestamp = Some(now - 1_000);
        assert!(f.evaluate_envelope(&env).unwrap());

        // Old message (2 hours ago)
        env.timestamp = Some(now - 7_200_000);
        assert!(!f.evaluate_envelope(&env).unwrap());
    }

    #[test]
    fn test_filter_is_null() {
        let e = engine();
        let f = e.compile_filter(r#"is_null(msg["email"])"#).unwrap();
        assert!(f.evaluate(&json!({"email": null})).unwrap());
        assert!(f.evaluate(&json!({"other": "x"})).unwrap()); // absent = null
        assert!(!f.evaluate(&json!({"email": "a@b.com"})).unwrap());
    }

    #[test]
    fn test_filter_is_empty() {
        let e = engine();
        let f = e.compile_filter(r#"is_empty(msg["name"])"#).unwrap();
        assert!(f.evaluate(&json!({"name": ""})).unwrap());
        assert!(!f.evaluate(&json!({"name": "Alice"})).unwrap());
        assert!(!f.evaluate(&json!({"other": "x"})).unwrap()); // absent != empty string
    }

    #[test]
    fn test_filter_is_null_or_empty() {
        let e = engine();
        let f = e
            .compile_filter(r#"is_null_or_empty(msg["email"])"#)
            .unwrap();
        assert!(f.evaluate(&json!({"email": null})).unwrap());
        assert!(f.evaluate(&json!({"email": ""})).unwrap());
        assert!(f.evaluate(&json!({"other": "x"})).unwrap()); // absent
        assert!(!f.evaluate(&json!({"email": "a@b.com"})).unwrap());
    }

    #[test]
    fn test_filter_runtime_error_returns_false() {
        let e = engine();
        // Type error: can't compare a map with a number
        let f = e.compile_filter(r#"msg > 5"#).unwrap();
        // Should not panic; returns false
        assert!(!f.evaluate(&json!({"a": 1})).unwrap());
    }

    #[test]
    fn test_filter_missing_field_safe() {
        let e = engine();
        // Accessing missing key returns () in Rhai; comparing () == "x" is false
        let f = e
            .compile_filter(r#"msg["nonexistent"] == "value""#)
            .unwrap();
        assert!(!f.evaluate(&json!({"other": 1})).unwrap());
    }

    // --- Transform tests ----------------------------------------------------

    #[test]
    fn test_transform_field_extraction() {
        let e = engine();
        let t = e.compile_transform(r#"msg["user"]"#).unwrap();
        let env = MessageEnvelope::new(json!({"user": {"id": 1, "name": "Alice"}}));
        let result = t.transform(&env).unwrap();
        assert_eq!(result, json!({"id": 1, "name": "Alice"}));
    }

    #[test]
    fn test_transform_object_construction() {
        let e = engine();
        let t = e
            .compile_transform(r#"#{ id: msg["userId"], email: msg["email"].to_lower() }"#)
            .unwrap();
        let env = MessageEnvelope::new(json!({"userId": "u1", "email": "USER@EXAMPLE.COM"}));
        let result = t.transform(&env).unwrap();
        assert_eq!(result["id"], json!("u1"));
        assert_eq!(result["email"], json!("user@example.com"));
    }

    #[test]
    fn test_transform_if_else() {
        let e = engine();
        let t = e
            .compile_transform(
                r#"if msg["score"] > 90 { "A" } else if msg["score"] > 80 { "B" } else { "C" }"#,
            )
            .unwrap();
        assert_eq!(
            t.transform(&MessageEnvelope::new(json!({"score": 95})))
                .unwrap(),
            json!("A")
        );
        assert_eq!(
            t.transform(&MessageEnvelope::new(json!({"score": 85})))
                .unwrap(),
            json!("B")
        );
        assert_eq!(
            t.transform(&MessageEnvelope::new(json!({"score": 70})))
                .unwrap(),
            json!("C")
        );
    }

    #[test]
    fn test_transform_coalesce() {
        let e = engine();
        let t = e
            .compile_transform(r#"msg["preferredName"] ?? msg["displayName"] ?? msg["email"]"#)
            .unwrap();
        assert_eq!(
            t.transform(&MessageEnvelope::new(
                json!({"preferredName": "Alice", "displayName": "A. Smith"})
            ))
            .unwrap(),
            json!("Alice")
        );
        assert_eq!(
            t.transform(&MessageEnvelope::new(
                json!({"displayName": "A. Smith", "email": "a@b.com"})
            ))
            .unwrap(),
            json!("A. Smith")
        );
        assert_eq!(
            t.transform(&MessageEnvelope::new(json!({"email": "a@b.com"})))
                .unwrap(),
            json!("a@b.com")
        );
    }

    #[test]
    fn test_transform_uses_key() {
        let e = engine();
        let t = e.compile_transform(r#"msg + #{ fromKey: key }"#).unwrap();
        let mut env = MessageEnvelope::new(json!({"x": 1}));
        env.key = Some(json!("my-key"));
        let result = t.transform(&env).unwrap();
        assert_eq!(result["fromKey"], json!("my-key"));
    }

    #[test]
    fn test_transform_uses_header() {
        let e = engine();
        let t = e
            .compile_transform(r#"msg + #{ tenant: headers["x-tenant"] }"#)
            .unwrap();
        let mut env = MessageEnvelope::new(json!({"x": 1}));
        env.headers.insert("x-tenant".to_string(), b"acme".to_vec());
        let result = t.transform(&env).unwrap();
        assert_eq!(result["tenant"], json!("acme"));
    }

    #[test]
    fn test_transform_switch_case() {
        let e = engine();
        let t = e
            .compile_transform(
                r#"switch msg["tier"] {
                    "premium" => "GOLD",
                    "basic" => "SILVER",
                    _ => "BRONZE"
                }"#,
            )
            .unwrap();
        assert_eq!(
            t.transform(&MessageEnvelope::new(json!({"tier": "premium"})))
                .unwrap(),
            json!("GOLD")
        );
        assert_eq!(
            t.transform(&MessageEnvelope::new(json!({"tier": "other"})))
                .unwrap(),
            json!("BRONZE")
        );
    }

    #[test]
    fn test_transform_array_map() {
        let e = engine();
        let t = e
            .compile_transform(r#"msg["users"].map(|u| u["id"])"#)
            .unwrap();
        let env = MessageEnvelope::new(
            json!({"users": [{"id": 1, "name": "Alice"}, {"id": 2, "name": "Bob"}]}),
        );
        let result = t.transform(&env).unwrap();
        assert_eq!(result, json!([1, 2]));
    }

    #[test]
    fn test_transform_multistatement_script() {
        let e = engine();
        let t = e
            .compile_transform(
                r#"
                let lower = msg["email"].to_lower();
                let domain = lower.split("@")[1];
                #{ email: lower, domain: domain }
                "#,
            )
            .unwrap();
        let env = MessageEnvelope::new(json!({"email": "User@EXAMPLE.COM"}));
        let result = t.transform(&env).unwrap();
        assert_eq!(result["email"], json!("user@example.com"));
        assert_eq!(result["domain"], json!("example.com"));
    }

    #[test]
    fn test_transform_missing_field_passes_through() {
        let e = engine();
        let t = e.compile_transform(r#"msg["nonexistent"]"#).unwrap();
        let msg = json!({"other": "value"});
        let env = MessageEnvelope::new(msg.clone());
        // Returns null (()) which converts to null JSON — this is correct behavior
        // since accessing a missing key returns ()
        let result = t.transform(&env).unwrap();
        assert_eq!(result, json!(null));
    }

    #[test]
    fn test_transform_runtime_error_passes_through() {
        let e = engine();
        // Calling a method that doesn't exist on a number — genuine runtime error
        let t = e.compile_transform(r#"msg["count"].to_upper()"#).unwrap();
        let msg = json!({"count": 42});
        let env = MessageEnvelope::new(msg.clone());
        let result = t.transform(&env).unwrap();
        assert_eq!(result, msg); // passes through on error
    }

    // --- Cache integration tests --------------------------------------------

    #[test]
    fn test_cache_put_and_lookup_in_transform() {
        let (e, _mgr) = engine_with_cache();

        // First transform writes to cache
        let put_t = e
            .compile_transform(
                r#"
                cache_put("profiles", msg["userId"], msg);
                msg
                "#,
            )
            .unwrap();
        let env1 = MessageEnvelope::new(json!({"userId": "u1", "tier": "premium"}));
        put_t.transform(&env1).unwrap();

        // Second transform reads from cache
        let lookup_t = e
            .compile_transform(
                r#"
                let profile = cache_lookup("profiles", msg["userId"]);
                msg + #{ cachedTier: profile["tier"] ?? "unknown" }
                "#,
            )
            .unwrap();
        let env2 = MessageEnvelope::new(json!({"userId": "u1", "event": "login"}));
        let result = lookup_t.transform(&env2).unwrap();
        assert_eq!(result["cachedTier"], json!("premium"));
    }

    #[test]
    fn test_cache_lookup_miss_returns_unit() {
        let (e, _mgr) = engine_with_cache();
        let t = e
            .compile_transform(
                r#"
                let v = cache_lookup("empty_store", "missing-key");
                is_null(v)
                "#,
            )
            .unwrap();
        let env = MessageEnvelope::new(json!({}));
        let result = t.transform(&env).unwrap();
        assert_eq!(result, json!(true));
    }

    // --- DslExpr array tests -----------------------------------------------

    #[test]
    fn test_filter_array_ands_all_conditions() {
        let e = engine();
        let expr = crate::config::DslExpr::Multi(vec![
            r#"msg["status"] == "active""#.to_string(),
            r#"msg["score"] > 80"#.to_string(),
            r#"key.starts_with("user-")"#.to_string(),
        ]);
        let f = e.compile_filter_expr(&expr).unwrap();

        let mut env_pass = MessageEnvelope::new(json!({"status": "active", "score": 90}));
        env_pass.key = Some(json!("user-1"));
        assert!(
            f.evaluate_envelope(&env_pass).unwrap(),
            "all conditions met"
        );

        let mut env_fail = MessageEnvelope::new(json!({"status": "active", "score": 90}));
        env_fail.key = Some(json!("admin-1")); // key doesn't match
        assert!(!f.evaluate_envelope(&env_fail).unwrap(), "key fails");
    }

    #[test]
    fn test_filter_single_element_array() {
        let e = engine();
        let expr = crate::config::DslExpr::Multi(vec![r#"msg["x"] == 1"#.to_string()]);
        let f = e.compile_filter_expr(&expr).unwrap();
        assert!(f.evaluate(&json!({"x": 1})).unwrap());
        assert!(!f.evaluate(&json!({"x": 2})).unwrap());
    }

    #[test]
    fn test_transform_pipeline_chains_steps() {
        let e = engine();
        let expr = crate::config::DslExpr::Multi(vec![
            r#"msg + #{ email: msg["email"].to_lower() }"#.to_string(),
            r#"msg + #{ processed: true }"#.to_string(),
            r#"msg + #{ step3: "done" }"#.to_string(),
        ]);
        let t = e.compile_transform_expr(&expr).unwrap();
        let env = MessageEnvelope::new(json!({"email": "USER@EXAMPLE.COM", "name": "Alice"}));
        let result = t.transform(&env).unwrap();

        assert_eq!(result["email"], json!("user@example.com"));
        assert_eq!(result["processed"], json!(true));
        assert_eq!(result["step3"], json!("done"));
        assert_eq!(result["name"], json!("Alice")); // preserved through pipeline
    }

    #[test]
    fn test_transform_pipeline_each_step_sees_previous_output() {
        let e = engine();
        // Each step adds a field — next step must see all previous fields
        let expr = crate::config::DslExpr::Multi(vec![
            r#"msg + #{ a: 1 }"#.to_string(),
            r#"msg + #{ b: msg["a"] + 1 }"#.to_string(), // reads `a` from step 1
            r#"msg + #{ c: msg["b"] + 1 }"#.to_string(), // reads `b` from step 2
        ]);
        let t = e.compile_transform_expr(&expr).unwrap();
        let env = MessageEnvelope::new(json!({}));
        let result = t.transform(&env).unwrap();

        assert_eq!(result["a"], json!(1));
        assert_eq!(result["b"], json!(2));
        assert_eq!(result["c"], json!(3));
    }

    #[test]
    fn test_transform_single_element_array() {
        let e = engine();
        let expr = crate::config::DslExpr::Multi(vec![r#"msg["user"]"#.to_string()]);
        let t = e.compile_transform_expr(&expr).unwrap();
        let env = MessageEnvelope::new(json!({"user": {"id": "u1"}}));
        let result = t.transform(&env).unwrap();
        assert_eq!(result, json!({"id": "u1"}));
    }

    #[test]
    fn test_dsl_expr_serde_single_string() {
        // A plain YAML string deserializes as Single
        let yaml = r#"active"#;
        let expr: crate::config::DslExpr = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(expr, crate::config::DslExpr::Single(_)));
    }

    #[test]
    fn test_dsl_expr_serde_array() {
        // A YAML array deserializes as Multi
        let yaml = r#"- expr1
- expr2
- expr3
"#;
        let expr: crate::config::DslExpr = serde_yaml::from_str(yaml).unwrap();
        if let crate::config::DslExpr::Multi(parts) = expr {
            assert_eq!(parts, vec!["expr1", "expr2", "expr3"]);
        } else {
            panic!("Expected Multi variant");
        }
    }

    // --- Compile-time validation tests -------------------------------------

    #[test]
    fn test_invalid_filter_syntax_errors_at_compile() {
        let e = engine();
        assert!(e.compile_filter("msg[\"status\" ==").is_err());
    }

    #[test]
    fn test_invalid_transform_syntax_errors_at_compile() {
        let e = engine();
        assert!(e.compile_transform("let x = ; x").is_err());
    }
}
