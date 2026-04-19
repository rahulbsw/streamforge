// Abstract Syntax Tree for StreamForge DSL

use super::error::Span;

/// AST node with position information
#[derive(Debug, Clone)]
pub struct Node<T> {
    pub value: T,
    pub span: Span,
}

impl<T> Node<T> {
    pub fn new(value: T, span: Span) -> Self {
        Self { value, span }
    }
}

/// Comparison operator
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComparisonOp {
    Eq,   // ==
    Ne,   // !=
    Gt,   // >
    Ge,   // >= (not implemented yet, for future)
    Lt,   // <
    Le,   // <= (not implemented yet, for future)
}

impl ComparisonOp {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "==" => Some(Self::Eq),
            "!=" => Some(Self::Ne),
            ">" => Some(Self::Gt),
            ">=" => Some(Self::Ge),
            "<" => Some(Self::Lt),
            "<=" => Some(Self::Le),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Gt => ">",
            Self::Ge => ">=",
            Self::Lt => "<",
            Self::Le => "<=",
        }
    }
}

/// Literal value
#[derive(Debug, Clone)]
pub enum Literal {
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
}

/// Filter expression AST
#[derive(Debug, Clone)]
pub enum FilterExpr {
    /// Simple JSON path comparison: /path,op,value
    JsonPath {
        path: String,
        op: ComparisonOp,
        value: Literal,
    },

    /// Boolean AND: AND:expr1:expr2:...
    And(Vec<Node<FilterExpr>>),

    /// Boolean OR: OR:expr1:expr2:...
    Or(Vec<Node<FilterExpr>>),

    /// Boolean NOT: NOT:expr
    Not(Box<Node<FilterExpr>>),

    /// Regex match: REGEX:path,pattern
    Regex {
        path: String,
        pattern: String,
    },

    /// Array filter: ARRAY_ANY:array_path,element_filter
    ArrayAny {
        array_path: String,
        element_filter: Box<Node<FilterExpr>>,
    },

    /// Array filter: ARRAY_ALL:array_path,element_filter
    ArrayAll {
        array_path: String,
        element_filter: Box<Node<FilterExpr>>,
    },

    /// Array contains: ARRAY_CONTAINS:array_path,value
    ArrayContains {
        array_path: String,
        value: Literal,
    },

    /// Array length: ARRAY_LENGTH:array_path,op,value
    ArrayLength {
        array_path: String,
        op: ComparisonOp,
        length: usize,
    },

    /// Key prefix: KEY_PREFIX:prefix
    KeyPrefix(String),

    /// Key matches regex: KEY_MATCHES:pattern
    KeyMatches(String),

    /// Key suffix (deprecated): KEY_SUFFIX:suffix
    KeySuffix(String),

    /// Key contains (deprecated): KEY_CONTAINS:substring
    KeyContains(String),

    /// Header filter: HEADER:name,op,value
    Header {
        name: String,
        op: ComparisonOp,
        value: String,
    },

    /// Timestamp age: TIMESTAMP_AGE:op,seconds
    TimestampAge {
        op: ComparisonOp,
        seconds: u64,
    },

    /// Field exists: EXISTS:path
    Exists(String),

    /// Field not exists: NOT_EXISTS:path
    NotExists(String),
}

/// Transform expression AST
#[derive(Debug, Clone)]
pub enum TransformExpr {
    /// Simple JSON path extraction: /path
    JsonPath(String),

    /// Extract with target field: EXTRACT:path,target_field
    Extract {
        path: String,
        target_field: String,
        default_value: Option<String>,
    },

    /// Construct object: CONSTRUCT:field1=path1:field2=path2:...
    Construct(Vec<(String, String)>), // (field_name, json_path)

    /// Hash transform: HASH:algorithm,path,target_field
    Hash {
        algorithm: HashAlgorithm,
        path: String,
        target_field: String,
    },

    /// String transform: UPPERCASE:path, LOWERCASE:path, TRIM:path
    String {
        op: StringOp,
        path: String,
    },

    /// Array map: ARRAY_MAP:array_path,element_path,target_field
    ArrayMap {
        array_path: String,
        element_path: String,
        target_field: String,
    },

    /// Array filter: ARRAY_FILTER:array_path,filter_expr
    ArrayFilter {
        array_path: String,
        filter: Box<Node<FilterExpr>>,
    },

    /// Arithmetic: ADD:path,value, MULTIPLY:path,value, etc.
    Arithmetic {
        op: ArithmeticOp,
        left: ArithmeticOperand,
        right: ArithmeticOperand,
    },

    /// Coalesce: COALESCE:path1,path2,default
    Coalesce {
        paths: Vec<String>,
        default: Option<String>,
    },
}

/// Hash algorithm
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashAlgorithm {
    MD5,
    SHA256,
    Murmur3,
}

impl HashAlgorithm {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "MD5" => Some(Self::MD5),
            "SHA256" => Some(Self::SHA256),
            "MURMUR3" => Some(Self::Murmur3),
            _ => None,
        }
    }
}

/// String operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringOp {
    Uppercase,
    Lowercase,
    Trim,
}

impl StringOp {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "UPPERCASE" => Some(Self::Uppercase),
            "LOWERCASE" => Some(Self::Lowercase),
            "TRIM" => Some(Self::Trim),
            _ => None,
        }
    }
}

/// Arithmetic operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArithmeticOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl ArithmeticOp {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "ADD" => Some(Self::Add),
            "SUB" => Some(Self::Sub),
            "SUBTRACT" => Some(Self::Sub),
            "MUL" => Some(Self::Mul),
            "MULTIPLY" => Some(Self::Mul),
            "DIV" => Some(Self::Div),
            "DIVIDE" => Some(Self::Div),
            _ => None,
        }
    }
}

/// Arithmetic operand (path or constant)
#[derive(Debug, Clone)]
pub enum ArithmeticOperand {
    Path(String),
    Constant(f64),
}

/// Top-level DSL expression
#[derive(Debug, Clone)]
pub enum DslExpr {
    Filter(Node<FilterExpr>),
    Transform(Node<TransformExpr>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::error::Position;

    #[test]
    fn test_comparison_op_from_str() {
        assert_eq!(ComparisonOp::from_str("=="), Some(ComparisonOp::Eq));
        assert_eq!(ComparisonOp::from_str("!="), Some(ComparisonOp::Ne));
        assert_eq!(ComparisonOp::from_str(">"), Some(ComparisonOp::Gt));
        assert_eq!(ComparisonOp::from_str("<"), Some(ComparisonOp::Lt));
        assert_eq!(ComparisonOp::from_str("invalid"), None);
    }

    #[test]
    fn test_hash_algorithm_from_str() {
        assert_eq!(HashAlgorithm::from_str("MD5"), Some(HashAlgorithm::MD5));
        assert_eq!(HashAlgorithm::from_str("md5"), Some(HashAlgorithm::MD5));
        assert_eq!(HashAlgorithm::from_str("SHA256"), Some(HashAlgorithm::SHA256));
        assert_eq!(HashAlgorithm::from_str("invalid"), None);
    }

    #[test]
    fn test_string_op_from_str() {
        assert_eq!(StringOp::from_str("UPPERCASE"), Some(StringOp::Uppercase));
        assert_eq!(StringOp::from_str("lowercase"), Some(StringOp::Lowercase));
        assert_eq!(StringOp::from_str("TRIM"), Some(StringOp::Trim));
        assert_eq!(StringOp::from_str("invalid"), None);
    }

    #[test]
    fn test_arithmetic_op_from_str() {
        assert_eq!(ArithmeticOp::from_str("ADD"), Some(ArithmeticOp::Add));
        assert_eq!(ArithmeticOp::from_str("SUBTRACT"), Some(ArithmeticOp::Sub));
        assert_eq!(ArithmeticOp::from_str("MUL"), Some(ArithmeticOp::Mul));
        assert_eq!(ArithmeticOp::from_str("DIVIDE"), Some(ArithmeticOp::Div));
        assert_eq!(ArithmeticOp::from_str("invalid"), None);
    }

    #[test]
    fn test_node_creation() {
        let span = Span::new(Position::zero(), Position::new(1, 10, 10));
        let filter = FilterExpr::JsonPath {
            path: "/status".to_string(),
            op: ComparisonOp::Eq,
            value: Literal::String("active".to_string()),
        };
        let node = Node::new(filter, span);

        assert_eq!(node.span.start.line, 1);
        assert_eq!(node.span.end.column, 10);
    }
}
