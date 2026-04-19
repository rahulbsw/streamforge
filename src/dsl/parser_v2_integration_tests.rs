// Integration tests for v1/v2 syntax auto-detection

#[cfg(test)]
mod tests {
    use crate::dsl::{parse_filter_expr, ComparisonOp, FilterExpr, Literal};

    #[test]
    fn test_autodetect_v1_simple() {
        // V1 colon-delimited syntax
        let result = parse_filter_expr("/status,==,active");
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
    fn test_autodetect_v2_simple() {
        // V2 function-style syntax
        let result = parse_filter_expr("field('/status') == 'active'");
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
    fn test_autodetect_v1_and() {
        let result = parse_filter_expr("AND:/status,==,active:/count,>,10");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::And(exprs) => {
                assert_eq!(exprs.len(), 2);
            }
            _ => panic!("Expected AND filter"),
        }
    }

    #[test]
    fn test_autodetect_v2_and() {
        let result = parse_filter_expr("and(field('/status') == 'active', field('/count') > 10)");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::And(exprs) => {
                assert_eq!(exprs.len(), 2);
            }
            _ => panic!("Expected AND filter"),
        }
    }

    #[test]
    fn test_autodetect_v1_regex() {
        let result = parse_filter_expr("REGEX:/email,^[a-z]+@example\\.com$");
        assert!(result.is_ok());
    }

    #[test]
    fn test_autodetect_v2_regex() {
        let result = parse_filter_expr("regex(field('/email'), '^[a-z]+@example\\\\.com$')");
        assert!(result.is_ok());
    }

    #[test]
    fn test_autodetect_v1_exists() {
        let result = parse_filter_expr("EXISTS:/optional_field");
        assert!(result.is_ok());
    }

    #[test]
    fn test_autodetect_v2_exists() {
        let result = parse_filter_expr("exists('/optional_field')");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::Exists(path) => {
                assert_eq!(path, "/optional_field");
            }
            _ => panic!("Expected EXISTS filter"),
        }
    }

    #[test]
    fn test_v2_is_null() {
        let result = parse_filter_expr("is_null('/value')");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::IsNull(path) => {
                assert_eq!(path, "/value");
            }
            _ => panic!("Expected IS_NULL filter"),
        }
    }

    #[test]
    fn test_v2_is_not_null() {
        let result = parse_filter_expr("is_not_null('/value')");
        assert!(result.is_ok());
    }

    #[test]
    fn test_v2_is_empty() {
        let result = parse_filter_expr("is_empty('/string')");
        assert!(result.is_ok());
    }

    #[test]
    fn test_v2_is_not_empty() {
        let result = parse_filter_expr("is_not_empty('/array')");
        assert!(result.is_ok());
    }

    #[test]
    fn test_v2_is_blank() {
        let result = parse_filter_expr("is_blank('/text')");
        assert!(result.is_ok());
    }

    #[test]
    fn test_v2_not_with_is_null() {
        let result = parse_filter_expr("not(is_null('/value'))");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::Not(inner) => match &inner.value {
                FilterExpr::IsNull(path) => {
                    assert_eq!(path, "/value");
                }
                _ => panic!("Expected IS_NULL inside NOT"),
            },
            _ => panic!("Expected NOT filter"),
        }
    }

    #[test]
    fn test_v2_nested_and_with_null_checks() {
        let result =
            parse_filter_expr("and(is_not_null('/user/id'), field('/status') == 'active')");
        assert!(result.is_ok());

        let node = result.unwrap();
        match &node.value {
            FilterExpr::And(exprs) => {
                assert_eq!(exprs.len(), 2);
            }
            _ => panic!("Expected AND filter"),
        }
    }
}
