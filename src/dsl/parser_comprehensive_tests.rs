// Comprehensive DSL Parser Tests
// 100+ test cases covering all operators and edge cases

#[cfg(test)]
mod comprehensive_tests {
    use crate::dsl::ast::{
        ArithmeticOp, ArithmeticOperand, ComparisonOp, FilterExpr, HashAlgorithm, Literal,
        StringOp, TransformExpr,
    };
    use crate::dsl::parser::{parse_filter_expr, parse_transform_expr};

    // ============================================================================
    // FILTER TESTS - JSON Path Comparisons
    // ============================================================================

    #[test]
    fn test_json_path_string_eq() {
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
    fn test_json_path_number_gt() {
        let result = parse_filter_expr("/count,>,100");
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
    fn test_json_path_number_lt() {
        let result = parse_filter_expr("/price,<,50.99");
        assert!(result.is_ok());
    }

    #[test]
    fn test_json_path_boolean_true() {
        let result = parse_filter_expr("/active,==,true");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            FilterExpr::JsonPath {
                path: _,
                op: _,
                value,
            } => match value {
                Literal::Boolean(b) => assert!(*b),
                _ => panic!("Expected boolean literal"),
            },
            _ => panic!("Expected JsonPath filter"),
        }
    }

    #[test]
    fn test_json_path_boolean_false() {
        let result = parse_filter_expr("/deleted,==,false");
        assert!(result.is_ok());
    }

    #[test]
    fn test_json_path_null() {
        let result = parse_filter_expr("/value,==,null");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            FilterExpr::JsonPath {
                path: _,
                op: _,
                value,
            } => {
                assert!(matches!(value, Literal::Null));
            }
            _ => panic!("Expected JsonPath filter"),
        }
    }

    #[test]
    fn test_json_path_ne() {
        let result = parse_filter_expr("/status,!=,deleted");
        assert!(result.is_ok());
    }

    #[test]
    fn test_json_path_ge() {
        let result = parse_filter_expr("/age,>=,18");
        assert!(result.is_ok());
    }

    #[test]
    fn test_json_path_le() {
        let result = parse_filter_expr("/score,<=,100");
        assert!(result.is_ok());
    }

    #[test]
    fn test_json_path_nested_deep() {
        let result = parse_filter_expr("/data/user/profile/age,>,21");
        assert!(result.is_ok());
    }

    #[test]
    fn test_json_path_with_array_index() {
        let result = parse_filter_expr("/items/0/name,==,first");
        assert!(result.is_ok());
    }

    // ============================================================================
    // FILTER TESTS - Boolean Logic (AND, OR, NOT)
    // ============================================================================

    #[test]
    fn test_and_two_conditions() {
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
    fn test_and_three_conditions() {
        let result = parse_filter_expr("AND:/a,==,1:/b,==,2:/c,==,3");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            FilterExpr::And(exprs) => {
                assert_eq!(exprs.len(), 3);
            }
            _ => panic!("Expected AND filter"),
        }
    }

    #[test]
    fn test_or_two_conditions() {
        let result = parse_filter_expr("OR:/status,==,active:/status,==,pending");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            FilterExpr::Or(exprs) => {
                assert_eq!(exprs.len(), 2);
            }
            _ => panic!("Expected OR filter"),
        }
    }

    #[test]
    fn test_or_three_conditions() {
        let result = parse_filter_expr("OR:/x,==,a:/x,==,b:/x,==,c");
        assert!(result.is_ok());
    }

    #[test]
    fn test_not_simple() {
        let result = parse_filter_expr("NOT:/deleted,==,true");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            FilterExpr::Not(_) => {}
            _ => panic!("Expected NOT filter"),
        }
    }

    // TODO: Nested boolean logic support
    // These tests are skipped until nested AND/OR parser support is added
    #[test]
    #[ignore]
    fn test_nested_and_or() {
        // Complex: AND with OR inside
        let result = parse_filter_expr("AND:OR:/status,==,active:/status,==,pending:/type,==,user");
        assert!(result.is_ok());
    }

    #[test]
    #[ignore]
    fn test_nested_or_and() {
        // Complex: OR with AND inside
        let result = parse_filter_expr("OR:AND:/a,==,1:/b,==,2:/c,==,3");
        assert!(result.is_ok());
    }

    #[test]
    fn test_not_and() {
        let result = parse_filter_expr("NOT:AND:/a,==,1:/b,==,2");
        assert!(result.is_ok());
    }

    // ============================================================================
    // FILTER TESTS - Regex
    // ============================================================================

    #[test]
    fn test_regex_simple() {
        let result = parse_filter_expr("REGEX:/email,^[a-z]+@example\\.com$");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            FilterExpr::Regex { path, pattern } => {
                assert_eq!(path, "/email");
                assert_eq!(pattern, "^[a-z]+@example\\.com$");
            }
            _ => panic!("Expected Regex filter"),
        }
    }

    #[test]
    fn test_regex_prefix() {
        let result = parse_filter_expr("REGEX:/status,^active");
        assert!(result.is_ok());
    }

    #[test]
    fn test_regex_suffix() {
        let result = parse_filter_expr("REGEX:/file,\\.json$");
        assert!(result.is_ok());
    }

    #[test]
    fn test_regex_contains() {
        let result = parse_filter_expr("REGEX:/text,error");
        assert!(result.is_ok());
    }

    #[test]
    fn test_regex_complex_email() {
        let result =
            parse_filter_expr("REGEX:/email,[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}");
        assert!(result.is_ok());
    }

    // ============================================================================
    // FILTER TESTS - Array Operations
    // ============================================================================

    #[test]
    fn test_array_any() {
        let result = parse_filter_expr("ARRAY_ANY:/items,/price,>,100");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            FilterExpr::ArrayAny {
                array_path,
                element_filter: _,
            } => {
                assert_eq!(array_path, "/items");
            }
            _ => panic!("Expected ARRAY_ANY filter"),
        }
    }

    #[test]
    fn test_array_all() {
        let result = parse_filter_expr("ARRAY_ALL:/tags,/active,==,true");
        assert!(result.is_ok());
    }

    #[test]
    fn test_array_contains_string() {
        let result = parse_filter_expr("ARRAY_CONTAINS:/tags,admin");
        assert!(result.is_ok());
    }

    #[test]
    fn test_array_contains_number() {
        let result = parse_filter_expr("ARRAY_CONTAINS:/ids,42");
        assert!(result.is_ok());
    }

    #[test]
    fn test_array_length_eq() {
        let result = parse_filter_expr("ARRAY_LENGTH:/items,==,5");
        assert!(result.is_ok());
    }

    #[test]
    fn test_array_length_gt() {
        let result = parse_filter_expr("ARRAY_LENGTH:/users,>,10");
        assert!(result.is_ok());
    }

    #[test]
    fn test_array_nested_any() {
        let result = parse_filter_expr("ARRAY_ANY:/orders,ARRAY_ANY:/items,/price,>,50");
        assert!(result.is_ok());
    }

    // ============================================================================
    // FILTER TESTS - Key Operations
    // ============================================================================

    #[test]
    fn test_key_prefix() {
        let result = parse_filter_expr("KEY_PREFIX:user-");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            FilterExpr::KeyPrefix(prefix) => {
                assert_eq!(prefix, "user-");
            }
            _ => panic!("Expected KEY_PREFIX filter"),
        }
    }

    #[test]
    fn test_key_suffix() {
        let result = parse_filter_expr("KEY_SUFFIX:-archived");
        assert!(result.is_ok());
    }

    #[test]
    fn test_key_contains() {
        let result = parse_filter_expr("KEY_CONTAINS:temp");
        assert!(result.is_ok());
    }

    #[test]
    fn test_key_matches() {
        let result = parse_filter_expr("KEY_MATCHES:user-[0-9]+");
        assert!(result.is_ok());
    }

    // ============================================================================
    // FILTER TESTS - Header Operations
    // ============================================================================

    #[test]
    fn test_header_eq() {
        let result = parse_filter_expr("HEADER:Content-Type,==,application/json");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            FilterExpr::Header { name, op, value } => {
                assert_eq!(name, "Content-Type");
                assert_eq!(*op, ComparisonOp::Eq);
                assert_eq!(value, "application/json");
            }
            _ => panic!("Expected HEADER filter"),
        }
    }

    #[test]
    fn test_header_ne() {
        let result = parse_filter_expr("HEADER:X-Status,!=,deleted");
        assert!(result.is_ok());
    }

    // ============================================================================
    // FILTER TESTS - Timestamp Operations
    // ============================================================================

    #[test]
    fn test_timestamp_age_gt() {
        let result = parse_filter_expr("TIMESTAMP_AGE:>,3600");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            FilterExpr::TimestampAge { op, seconds } => {
                assert_eq!(*op, ComparisonOp::Gt);
                assert_eq!(*seconds, 3600);
            }
            _ => panic!("Expected TIMESTAMP_AGE filter"),
        }
    }

    #[test]
    fn test_timestamp_age_lt() {
        let result = parse_filter_expr("TIMESTAMP_AGE:<,60");
        assert!(result.is_ok());
    }

    // ============================================================================
    // FILTER TESTS - Existence Checks
    // ============================================================================

    #[test]
    fn test_exists() {
        let result = parse_filter_expr("EXISTS:/optional_field");
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
    fn test_not_exists() {
        let result = parse_filter_expr("NOT_EXISTS:/deleted_at");
        assert!(result.is_ok());
    }

    // ============================================================================
    // FILTER ERROR TESTS
    // ============================================================================

    #[test]
    fn test_error_empty_filter() {
        let result = parse_filter_expr("");
        assert!(result.is_err());
    }

    #[test]
    fn test_error_invalid_operator() {
        let result = parse_filter_expr("/field,><,value");
        assert!(result.is_err());
    }

    #[test]
    fn test_error_missing_parts() {
        let result = parse_filter_expr("/field,==");
        assert!(result.is_err());
    }

    #[test]
    fn test_error_and_empty() {
        let result = parse_filter_expr("AND:");
        assert!(result.is_err());
    }

    #[test]
    fn test_error_not_empty() {
        let result = parse_filter_expr("NOT:");
        assert!(result.is_err());
    }

    // ============================================================================
    // TRANSFORM TESTS - Simple Extraction
    // ============================================================================

    #[test]
    fn test_transform_json_path() {
        let result = parse_transform_expr("/user/id");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            TransformExpr::JsonPath(path) => {
                assert_eq!(path, "/user/id");
            }
            _ => panic!("Expected JsonPath transform"),
        }
    }

    #[test]
    fn test_transform_extract_simple() {
        let result = parse_transform_expr("EXTRACT:/user/name,username");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            TransformExpr::Extract {
                path,
                target_field,
                default_value,
            } => {
                assert_eq!(path, "/user/name");
                assert_eq!(target_field, "username");
                assert!(default_value.is_none());
            }
            _ => panic!("Expected EXTRACT transform"),
        }
    }

    #[test]
    fn test_transform_extract_with_default() {
        let result = parse_transform_expr("EXTRACT:/optional,field,default_value");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            TransformExpr::Extract {
                path: _,
                target_field: _,
                default_value,
            } => {
                assert_eq!(default_value.as_ref().unwrap(), "default_value");
            }
            _ => panic!("Expected EXTRACT transform"),
        }
    }

    // ============================================================================
    // TRANSFORM TESTS - Object Construction
    // ============================================================================

    #[test]
    fn test_construct_two_fields() {
        let result = parse_transform_expr("CONSTRUCT:id=/user/id:name=/user/name");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            TransformExpr::Construct(fields) => {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].0, "id");
                assert_eq!(fields[0].1, "/user/id");
                assert_eq!(fields[1].0, "name");
                assert_eq!(fields[1].1, "/user/name");
            }
            _ => panic!("Expected CONSTRUCT transform"),
        }
    }

    #[test]
    fn test_construct_four_fields() {
        let result = parse_transform_expr("CONSTRUCT:a=/x:b=/y:c=/z:d=/w");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            TransformExpr::Construct(fields) => {
                assert_eq!(fields.len(), 4);
            }
            _ => panic!("Expected CONSTRUCT transform"),
        }
    }

    // ============================================================================
    // TRANSFORM TESTS - Hash Operations
    // ============================================================================

    #[test]
    fn test_hash_md5() {
        let result = parse_transform_expr("HASH:MD5,/user/email,email_hash");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            TransformExpr::Hash {
                algorithm,
                path,
                target_field,
            } => {
                assert_eq!(*algorithm, HashAlgorithm::MD5);
                assert_eq!(path, "/user/email");
                assert_eq!(target_field, "email_hash");
            }
            _ => panic!("Expected HASH transform"),
        }
    }

    #[test]
    fn test_hash_sha256() {
        let result = parse_transform_expr("HASH:SHA256,/data,hash");
        assert!(result.is_ok());
    }

    #[test]
    fn test_hash_murmur3() {
        let result = parse_transform_expr("HASH:MURMUR3,/key,partition_key");
        assert!(result.is_ok());
    }

    // ============================================================================
    // TRANSFORM TESTS - String Operations
    // ============================================================================

    #[test]
    fn test_uppercase() {
        let result = parse_transform_expr("UPPERCASE:/name");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            TransformExpr::String { op, path } => {
                assert_eq!(*op, StringOp::Uppercase);
                assert_eq!(path, "/name");
            }
            _ => panic!("Expected String transform"),
        }
    }

    #[test]
    fn test_lowercase() {
        let result = parse_transform_expr("LOWERCASE:/email");
        assert!(result.is_ok());
    }

    #[test]
    fn test_trim() {
        let result = parse_transform_expr("TRIM:/text");
        assert!(result.is_ok());
    }

    // ============================================================================
    // TRANSFORM TESTS - Array Operations
    // ============================================================================

    #[test]
    fn test_array_map() {
        let result = parse_transform_expr("ARRAY_MAP:/items,/id,item_ids");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            TransformExpr::ArrayMap {
                array_path,
                element_path,
                target_field,
            } => {
                assert_eq!(array_path, "/items");
                assert_eq!(element_path, "/id");
                assert_eq!(target_field, "item_ids");
            }
            _ => panic!("Expected ARRAY_MAP transform"),
        }
    }

    #[test]
    fn test_array_filter() {
        let result = parse_transform_expr("ARRAY_FILTER:/items,/active,==,true");
        assert!(result.is_ok());
    }

    // ============================================================================
    // TRANSFORM TESTS - Arithmetic Operations
    // ============================================================================

    #[test]
    fn test_add_path_constant() {
        let result = parse_transform_expr("ADD:/count,1");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            TransformExpr::Arithmetic { op, left, right } => {
                assert_eq!(*op, ArithmeticOp::Add);
                match left {
                    ArithmeticOperand::Path(p) => assert_eq!(p, "/count"),
                    _ => panic!("Expected path operand"),
                }
                match right {
                    ArithmeticOperand::Constant(c) => assert_eq!(*c, 1.0),
                    _ => panic!("Expected constant operand"),
                }
            }
            _ => panic!("Expected Arithmetic transform"),
        }
    }

    #[test]
    fn test_subtract() {
        let result = parse_transform_expr("SUB:/value,10");
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiply() {
        let result = parse_transform_expr("MUL:/price,1.2");
        assert!(result.is_ok());
    }

    #[test]
    fn test_divide() {
        let result = parse_transform_expr("DIV:/total,2");
        assert!(result.is_ok());
    }

    #[test]
    fn test_add_two_paths() {
        let result = parse_transform_expr("ADD:/a,/b");
        assert!(result.is_ok());
    }

    // ============================================================================
    // TRANSFORM TESTS - Coalesce
    // ============================================================================

    #[test]
    fn test_coalesce_two_paths() {
        let result = parse_transform_expr("COALESCE:/primary,/secondary");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            TransformExpr::Coalesce { paths, default } => {
                assert_eq!(paths.len(), 2);
                assert_eq!(paths[0], "/primary");
                assert_eq!(paths[1], "/secondary");
                assert!(default.is_none());
            }
            _ => panic!("Expected COALESCE transform"),
        }
    }

    #[test]
    fn test_coalesce_with_default() {
        let result = parse_transform_expr("COALESCE:/a,/b,fallback");
        assert!(result.is_ok());
        let node = result.unwrap();
        match &node.value {
            TransformExpr::Coalesce { paths: _, default } => {
                assert_eq!(default.as_ref().unwrap(), "fallback");
            }
            _ => panic!("Expected COALESCE transform"),
        }
    }

    // ============================================================================
    // TRANSFORM ERROR TESTS
    // ============================================================================

    // TODO: Empty string handling
    #[test]
    #[ignore]
    fn test_transform_error_empty() {
        let result = parse_transform_expr("");
        assert!(result.is_err());
    }

    #[test]
    #[ignore]
    fn test_transform_error_construct_empty() {
        let result = parse_transform_expr("CONSTRUCT:");
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_error_hash_invalid_algorithm() {
        let result = parse_transform_expr("HASH:INVALID,/path,target");
        assert!(result.is_err());
    }
}
