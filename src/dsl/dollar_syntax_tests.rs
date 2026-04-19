// Tests for $ syntax (shorthand field access)

#[cfg(test)]
mod tests {
    use crate::dsl::{parse_filter_expr, ComparisonOp, FilterExpr, Literal};

    #[test]
    fn test_dollar_simple_field() {
        // $status == 'active'
        let result = parse_filter_expr("$status == 'active'");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::JsonPath { path, op, value } => {
                assert_eq!(path, "/status");
                assert_eq!(*op, ComparisonOp::Eq);
                match value {
                    Literal::String(s) => assert_eq!(s, "active"),
                    _ => panic!("Expected string literal"),
                }
            }
            _ => panic!("Expected JsonPath filter"),
        }
    }

    #[test]
    fn test_dollar_dot_notation() {
        // $user.email == 'admin@example.com'
        let result = parse_filter_expr("$user.email == 'admin@example.com'");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::JsonPath { path, op, value } => {
                assert_eq!(path, "/user/email");
                assert_eq!(*op, ComparisonOp::Eq);
                match value {
                    Literal::String(s) => assert_eq!(s, "admin@example.com"),
                    _ => panic!("Expected string literal"),
                }
            }
            _ => panic!("Expected JsonPath filter"),
        }
    }

    #[test]
    fn test_dollar_deep_nested() {
        // $data.user.profile.age > 18
        let result = parse_filter_expr("$data.user.profile.age > 18");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::JsonPath { path, op, value } => {
                assert_eq!(path, "/data/user/profile/age");
                assert_eq!(*op, ComparisonOp::Gt);
                match value {
                    Literal::Number(n) => assert_eq!(*n, 18.0),
                    _ => panic!("Expected number literal"),
                }
            }
            _ => panic!("Expected JsonPath filter"),
        }
    }

    #[test]
    fn test_dollar_explicit_path() {
        // $('/field$1/name') handles special characters
        let result = parse_filter_expr("$('/field$1/name') == 'test'");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::JsonPath { path, op, value } => {
                assert_eq!(path, "/field$1/name");
                assert_eq!(*op, ComparisonOp::Eq);
                match value {
                    Literal::String(s) => assert_eq!(s, "test"),
                    _ => panic!("Expected string literal"),
                }
            }
            _ => panic!("Expected JsonPath filter"),
        }
    }

    #[test]
    fn test_dollar_number_comparison() {
        // $count > 100
        let result = parse_filter_expr("$count > 100");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::JsonPath { path, op, value } => {
                assert_eq!(path, "/count");
                assert_eq!(*op, ComparisonOp::Gt);
                match value {
                    Literal::Number(n) => assert_eq!(*n, 100.0),
                    _ => panic!("Expected number literal"),
                }
            }
            _ => panic!("Expected JsonPath filter"),
        }
    }

    #[test]
    fn test_dollar_in_and() {
        // and($status == 'active', $count > 10)
        let result = parse_filter_expr("and($status == 'active', $count > 10)");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::And(exprs) => {
                assert_eq!(exprs.len(), 2);

                // Check first expression
                match &exprs[0].value {
                    FilterExpr::JsonPath { path, .. } => {
                        assert_eq!(path, "/status");
                    }
                    _ => panic!("Expected JsonPath in AND"),
                }

                // Check second expression
                match &exprs[1].value {
                    FilterExpr::JsonPath { path, .. } => {
                        assert_eq!(path, "/count");
                    }
                    _ => panic!("Expected JsonPath in AND"),
                }
            }
            _ => panic!("Expected AND filter"),
        }
    }

    #[test]
    fn test_dollar_with_dot_in_or() {
        // or($user.active == true, $user.status == 'trial')
        let result = parse_filter_expr("or($user.active == true, $user.status == 'trial')");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::Or(exprs) => {
                assert_eq!(exprs.len(), 2);

                match &exprs[0].value {
                    FilterExpr::JsonPath { path, .. } => {
                        assert_eq!(path, "/user/active");
                    }
                    _ => panic!("Expected JsonPath in OR"),
                }

                match &exprs[1].value {
                    FilterExpr::JsonPath { path, .. } => {
                        assert_eq!(path, "/user/status");
                    }
                    _ => panic!("Expected JsonPath in OR"),
                }
            }
            _ => panic!("Expected OR filter"),
        }
    }

    #[test]
    fn test_dollar_all_operators() {
        // Test all comparison operators with $
        let tests = vec![
            ("$value == 10", ComparisonOp::Eq),
            ("$value != 10", ComparisonOp::Ne),
            ("$value > 10", ComparisonOp::Gt),
            ("$value >= 10", ComparisonOp::Ge),
            ("$value < 10", ComparisonOp::Lt),
            ("$value <= 10", ComparisonOp::Le),
        ];

        for (expr, expected_op) in tests {
            let result = parse_filter_expr(expr);
            assert!(result.is_ok(), "Failed to parse: {}", expr);

            let node = result.unwrap();
            match &node.value {
                FilterExpr::JsonPath { path, op, .. } => {
                    assert_eq!(path, "/value");
                    assert_eq!(*op, expected_op);
                }
                _ => panic!("Expected JsonPath filter for: {}", expr),
            }
        }
    }

    #[test]
    fn test_dollar_boolean_literal() {
        // $active == true
        let result = parse_filter_expr("$active == true");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::JsonPath { path, value, .. } => {
                assert_eq!(path, "/active");
                match value {
                    Literal::Boolean(b) => assert!(*b),
                    _ => panic!("Expected boolean literal"),
                }
            }
            _ => panic!("Expected JsonPath filter"),
        }
    }

    #[test]
    fn test_dollar_null_literal() {
        // $value == null
        let result = parse_filter_expr("$value == null");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::JsonPath { path, value, .. } => {
                assert_eq!(path, "/value");
                assert!(matches!(value, Literal::Null));
            }
            _ => panic!("Expected JsonPath filter"),
        }
    }

    #[test]
    fn test_dollar_complex_nested() {
        // and($user.email == 'admin@example.com', or($tier == 'premium', $tier == 'enterprise'))
        let result = parse_filter_expr(
            "and($user.email == 'admin@example.com', or($tier == 'premium', $tier == 'enterprise'))"
        );
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::And(exprs) => {
                assert_eq!(exprs.len(), 2);

                // First condition: $user.email
                match &exprs[0].value {
                    FilterExpr::JsonPath { path, .. } => {
                        assert_eq!(path, "/user/email");
                    }
                    _ => panic!("Expected JsonPath"),
                }

                // Second condition: OR
                match &exprs[1].value {
                    FilterExpr::Or(or_exprs) => {
                        assert_eq!(or_exprs.len(), 2);
                    }
                    _ => panic!("Expected OR filter"),
                }
            }
            _ => panic!("Expected AND filter"),
        }
    }

    #[test]
    fn test_dollar_with_underscores() {
        // $user_id == 123
        let result = parse_filter_expr("$user_id == 123");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::JsonPath { path, .. } => {
                assert_eq!(path, "/user_id");
            }
            _ => panic!("Expected JsonPath filter"),
        }
    }

    #[test]
    fn test_dollar_explicit_with_special_chars() {
        // Test explicit form with special characters
        let tests = vec![
            "$('/field-with-dash') == 'test'",
            "$('/field.with.dots') == 'test'",
            "$('/field$special') == 'test'",
            "$('/field:colon') == 'test'",
        ];

        for expr in tests {
            let result = parse_filter_expr(expr);
            assert!(result.is_ok(), "Failed to parse: {}", expr);
        }
    }

    #[test]
    fn test_autodetect_dollar_vs_v1() {
        // V1 syntax should still work
        let v1_result = parse_filter_expr("/status,==,active");
        assert!(v1_result.is_ok());

        // V2 $ syntax should work
        let v2_result = parse_filter_expr("$status == 'active'");
        assert!(v2_result.is_ok());

        // Both should produce equivalent results
        let v1_node = v1_result.unwrap();
        let v2_node = v2_result.unwrap();

        match (&v1_node.value, &v2_node.value) {
            (FilterExpr::JsonPath { path: p1, .. }, FilterExpr::JsonPath { path: p2, .. }) => {
                assert_eq!(p1, p2);
            }
            _ => panic!("Both should be JsonPath filters"),
        }
    }

    #[test]
    fn test_dollar_mixed_with_field() {
        // Can mix $ and field() in same expression
        let result = parse_filter_expr("and($status == 'active', field('/legacy/path') > 10)");
        assert!(result.is_ok());
    }
}
