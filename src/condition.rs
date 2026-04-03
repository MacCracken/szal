//! Lightweight condition expression evaluator for workflow step conditions.
//!
//! Evaluates predicate expressions against a JSON context built from step results.
//!
//! ## Grammar
//!
//! ```text
//! expr     = or_expr
//! or_expr  = and_expr ("||" and_expr)*
//! and_expr = cmp_expr ("&&" cmp_expr)*
//! cmp_expr = value (("==" | "!=" | ">" | ">=" | "<" | "<=") value)?
//! value    = "!" value | path | string_lit | number_lit | bool_lit | "(" expr ")"
//! path     = ident ("." ident)*
//! ident    = [a-zA-Z_][a-zA-Z0-9_-]*
//! string_lit = "'" [^']* "'"
//! number_lit = [0-9]+ ("." [0-9]+)?
//! bool_lit = "true" | "false"
//! ```
//!
//! ## Example
//!
//! ```
//! use szal::condition::evaluate;
//! use serde_json::json;
//!
//! let ctx = json!({
//!     "steps": {
//!         "build": { "status": "completed" }
//!     }
//! });
//!
//! assert!(evaluate("steps.build.status == 'completed'", &ctx).unwrap());
//! assert!(!evaluate("steps.build.status == 'failed'", &ctx).unwrap());
//! ```

use serde_json::Value;

// ---------------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Path(String),
    StringLit(String),
    NumberLit(f64),
    BoolLit(bool),
    Eq,
    NotEq,
    Gt,
    Gte,
    Lt,
    Lte,
    Not,
    And,
    Or,
    LParen,
    RParen,
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        let b = bytes[i];

        // Skip whitespace
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // Two-char operators
        if i + 1 < len {
            match (b, bytes[i + 1]) {
                (b'=', b'=') => {
                    tokens.push(Token::Eq);
                    i += 2;
                    continue;
                }
                (b'!', b'=') => {
                    tokens.push(Token::NotEq);
                    i += 2;
                    continue;
                }
                (b'>', b'=') => {
                    tokens.push(Token::Gte);
                    i += 2;
                    continue;
                }
                (b'<', b'=') => {
                    tokens.push(Token::Lte);
                    i += 2;
                    continue;
                }
                (b'&', b'&') => {
                    tokens.push(Token::And);
                    i += 2;
                    continue;
                }
                (b'|', b'|') => {
                    tokens.push(Token::Or);
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }

        // Single-char operators: >, <, !
        if b == b'>' {
            tokens.push(Token::Gt);
            i += 1;
            continue;
        }
        if b == b'<' {
            tokens.push(Token::Lt);
            i += 1;
            continue;
        }
        if b == b'!' {
            tokens.push(Token::Not);
            i += 1;
            continue;
        }

        // Parentheses
        if b == b'(' {
            tokens.push(Token::LParen);
            i += 1;
            continue;
        }
        if b == b')' {
            tokens.push(Token::RParen);
            i += 1;
            continue;
        }

        // String literal (single-quoted)
        if b == b'\'' {
            i += 1;
            let start = i;
            while i < len && bytes[i] != b'\'' {
                i += 1;
            }
            if i >= len {
                return Err("unterminated string literal".into());
            }
            let s = &input[start..i];
            tokens.push(Token::StringLit(s.to_owned()));
            i += 1; // skip closing quote
            continue;
        }

        // Number literal
        if b.is_ascii_digit() {
            let start = i;
            while i < len && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i < len && bytes[i] == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit() {
                i += 1; // skip dot
                while i < len && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let num_str = &input[start..i];
            let val: f64 = num_str
                .parse()
                .map_err(|e| format!("invalid number '{num_str}': {e}"))?;
            tokens.push(Token::NumberLit(val));
            continue;
        }

        // Identifier / path / bool literal
        if b.is_ascii_alphabetic() || b == b'_' {
            let start = i;
            while i < len
                && (bytes[i].is_ascii_alphanumeric()
                    || bytes[i] == b'_'
                    || bytes[i] == b'-'
                    || bytes[i] == b'.')
            {
                i += 1;
            }
            let word = &input[start..i];
            match word {
                "true" => tokens.push(Token::BoolLit(true)),
                "false" => tokens.push(Token::BoolLit(false)),
                _ => tokens.push(Token::Path(word.to_owned())),
            }
            continue;
        }

        // Safe conversion for error message — byte is not valid ASCII identifier
        let ch = if b.is_ascii() {
            b as char
        } else {
            return Err(format!("unexpected byte 0x{b:02x}"));
        };
        return Err(format!("unexpected character '{ch}'"));
    }

    Ok(tokens)
}

// ---------------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Expr {
    Literal(Value),
    Path(String),
    Eq(Box<Expr>, Box<Expr>),
    NotEq(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Gte(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Lte(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and()?;
        while self.peek() == Some(&Token::Or) {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_cmp()?;
        while self.peek() == Some(&Token::And) {
            self.advance();
            let right = self.parse_cmp()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_cmp(&mut self) -> Result<Expr, String> {
        let left = self.parse_value()?;
        match self.peek() {
            Some(Token::Eq) => {
                self.advance();
                let right = self.parse_value()?;
                Ok(Expr::Eq(Box::new(left), Box::new(right)))
            }
            Some(Token::NotEq) => {
                self.advance();
                let right = self.parse_value()?;
                Ok(Expr::NotEq(Box::new(left), Box::new(right)))
            }
            Some(Token::Gt) => {
                self.advance();
                let right = self.parse_value()?;
                Ok(Expr::Gt(Box::new(left), Box::new(right)))
            }
            Some(Token::Gte) => {
                self.advance();
                let right = self.parse_value()?;
                Ok(Expr::Gte(Box::new(left), Box::new(right)))
            }
            Some(Token::Lt) => {
                self.advance();
                let right = self.parse_value()?;
                Ok(Expr::Lt(Box::new(left), Box::new(right)))
            }
            Some(Token::Lte) => {
                self.advance();
                let right = self.parse_value()?;
                Ok(Expr::Lte(Box::new(left), Box::new(right)))
            }
            _ => Ok(left),
        }
    }

    fn parse_value(&mut self) -> Result<Expr, String> {
        // Prefix `!` (not)
        if self.peek() == Some(&Token::Not) {
            self.advance();
            let inner = self.parse_value()?;
            return Ok(Expr::Not(Box::new(inner)));
        }
        match self.peek().cloned() {
            Some(Token::StringLit(s)) => {
                self.advance();
                Ok(Expr::Literal(Value::String(s)))
            }
            Some(Token::NumberLit(n)) => {
                self.advance();
                Ok(Expr::Literal(
                    serde_json::Number::from_f64(n)
                        .map(Value::Number)
                        .unwrap_or(Value::Null),
                ))
            }
            Some(Token::BoolLit(b)) => {
                self.advance();
                Ok(Expr::Literal(Value::Bool(b)))
            }
            Some(Token::Path(p)) => {
                self.advance();
                Ok(Expr::Path(p))
            }
            Some(Token::LParen) => {
                self.advance();
                let expr = self.parse_expr()?;
                match self.advance() {
                    Some(Token::RParen) => Ok(expr),
                    _ => Err("expected ')'".into()),
                }
            }
            Some(tok) => Err(format!("unexpected token: {tok:?}")),
            None => Err("unexpected end of expression".into()),
        }
    }
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

/// Resolve a dot-notation path against a JSON value.
///
/// Walks the JSON tree segment by segment. Returns `&Value::Null` if any
/// segment is missing.
///
/// ```
/// use serde_json::json;
/// use szal::condition::resolve_path;
///
/// let ctx = json!({"steps": {"build": {"output": {"url": "https://example.com"}}}});
/// assert_eq!(resolve_path("steps.build.output.url", &ctx), "https://example.com");
/// assert!(resolve_path("steps.missing.field", &ctx).is_null());
/// ```
#[inline]
#[must_use]
pub fn resolve_path<'a>(path: &str, context: &'a Value) -> &'a Value {
    let mut current = context;
    for segment in path.split('.') {
        match current.get(segment) {
            Some(v) => current = v,
            None => return &Value::Null,
        }
    }
    current
}

fn eval_expr(expr: &Expr, context: &Value) -> Result<Value, String> {
    match expr {
        Expr::Literal(v) => Ok(v.clone()),
        Expr::Path(p) => Ok(resolve_path(p, context).clone()),
        Expr::Eq(left, right) => {
            let l = eval_expr(left, context)?;
            let r = eval_expr(right, context)?;
            Ok(Value::Bool(values_equal(&l, &r)))
        }
        Expr::NotEq(left, right) => {
            let l = eval_expr(left, context)?;
            let r = eval_expr(right, context)?;
            Ok(Value::Bool(!values_equal(&l, &r)))
        }
        Expr::Gt(left, right) => {
            let l = eval_expr(left, context)?;
            let r = eval_expr(right, context)?;
            Ok(Value::Bool(
                values_compare(&l, &r) == Some(std::cmp::Ordering::Greater),
            ))
        }
        Expr::Gte(left, right) => {
            let l = eval_expr(left, context)?;
            let r = eval_expr(right, context)?;
            Ok(Value::Bool(matches!(
                values_compare(&l, &r),
                Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
            )))
        }
        Expr::Lt(left, right) => {
            let l = eval_expr(left, context)?;
            let r = eval_expr(right, context)?;
            Ok(Value::Bool(
                values_compare(&l, &r) == Some(std::cmp::Ordering::Less),
            ))
        }
        Expr::Lte(left, right) => {
            let l = eval_expr(left, context)?;
            let r = eval_expr(right, context)?;
            Ok(Value::Bool(matches!(
                values_compare(&l, &r),
                Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
            )))
        }
        Expr::Not(inner) => {
            let v = eval_expr(inner, context)?;
            Ok(Value::Bool(!is_truthy(&v)))
        }
        Expr::And(left, right) => {
            if !is_truthy(&eval_expr(left, context)?) {
                return Ok(Value::Bool(false));
            }
            Ok(Value::Bool(is_truthy(&eval_expr(right, context)?)))
        }
        Expr::Or(left, right) => {
            if is_truthy(&eval_expr(left, context)?) {
                return Ok(Value::Bool(true));
            }
            Ok(Value::Bool(is_truthy(&eval_expr(right, context)?)))
        }
    }
}

/// Compare two JSON values for equality.
///
/// Same-type comparisons use structural equality.
/// Cross-type comparisons always return false.
#[inline]
#[must_use]
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Null, Value::Null) => true,
        _ => false,
    }
}

/// Compare two JSON values for ordering.
///
/// Numbers are compared numerically. Strings are compared lexicographically.
/// Cross-type or non-orderable comparisons return `None`.
#[inline]
#[must_use]
fn values_compare(a: &Value, b: &Value) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Number(a), Value::Number(b)) => {
            let af = a.as_f64()?;
            let bf = b.as_f64()?;
            af.partial_cmp(&bf)
        }
        (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
        _ => None,
    }
}

/// Determine the truthiness of a JSON value.
#[inline]
#[must_use]
fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::String(s) => !s.is_empty(),
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Evaluate a condition expression against a JSON context.
///
/// Returns `Ok(true)` if the condition passes, `Ok(false)` if it does not,
/// or `Err` if the expression is malformed.
///
/// An empty expression is vacuously true.
///
/// # Examples
///
/// ```
/// use szal::condition::evaluate;
/// use serde_json::json;
///
/// let ctx = json!({"status": "ok"});
/// assert!(evaluate("status == 'ok'", &ctx).unwrap());
/// assert!(evaluate("", &ctx).unwrap());
/// ```
pub fn evaluate(expr: &str, context: &Value) -> Result<bool, String> {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return Ok(true);
    }

    let tokens = tokenize(trimmed)?;
    let mut parser = Parser::new(tokens);
    let ast = parser.parse_expr()?;

    if parser.pos < parser.tokens.len() {
        return Err(format!(
            "unexpected trailing token: {:?}",
            parser.tokens[parser.pos]
        ));
    }

    let result = eval_expr(&ast, context)?;
    Ok(is_truthy(&result))
}

/// Render a template string by resolving `{{path}}` placeholders against a JSON context.
///
/// Supports dot-notation paths: `{{steps.build.output.url}}` walks into nested JSON.
/// Missing paths resolve to empty string. Non-string values are JSON-serialized.
///
/// ```
/// use serde_json::json;
/// use szal::condition::render_template;
///
/// let ctx = json!({"name": "Alice", "meta": {"role": "admin"}});
/// assert_eq!(render_template("Hello {{name}}, role={{meta.role}}", &ctx), "Hello Alice, role=admin");
/// ```
#[must_use]
pub fn render_template(template: &str, context: &Value) -> String {
    let mut result = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if i + 1 < len && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            // Find closing }}
            if let Some(end) = template[i + 2..].find("}}") {
                let path = template[i + 2..i + 2 + end].trim();
                let resolved = resolve_path(path, context);
                match resolved {
                    Value::Null => {} // missing path → empty
                    Value::String(s) => result.push_str(s),
                    other => {
                        use std::fmt::Write;
                        let _ = write!(result, "{other}");
                    }
                }
                i += 2 + end + 2; // skip past }}
                continue;
            }
        }
        // Safe: we only branch on ASCII bytes above, so this is a valid char boundary
        // for non-ASCII bytes, they fall through and we need to advance by char width
        if bytes[i].is_ascii() {
            result.push(bytes[i] as char);
            i += 1;
        } else {
            // Multi-byte UTF-8: find the char at this byte offset
            let ch = template[i..].chars().next().unwrap();
            result.push(ch);
            i += ch.len_utf8();
        }
    }

    result
}

/// Build a step result context for condition evaluation.
///
/// Creates a JSON object:
/// ```json
/// {
///   "steps": {
///     "<step_name>": {
///       "status": "<completed|failed|skipped>",
///       "output": <step output value>,
///       "error": <error string or null>
///     }
///   }
/// }
/// ```
///
/// Maps `step_id` from each [`StepResult`](crate::step::StepResult) back to the
/// step name via the [`StepDef`](crate::step::StepDef) slice.
#[must_use]
pub fn build_step_context(
    results: &[crate::step::StepResult],
    steps: &[crate::step::StepDef],
) -> Value {
    use serde_json::json;
    use std::collections::HashMap;

    let id_to_name: HashMap<_, _> = steps.iter().map(|s| (s.id, s.name.as_str())).collect();

    let mut step_map = serde_json::Map::new();

    for result in results {
        let name = match id_to_name.get(&result.step_id) {
            Some(n) => *n,
            None => {
                tracing::warn!(
                    step_id = %result.step_id,
                    "step result references unknown step id, skipping"
                );
                continue;
            }
        };

        let entry = json!({
            "status": result.status.to_string(),
            "output": result.output,
            "error": result.error,
        });

        step_map.insert(name.to_owned(), entry);
    }

    json!({ "steps": step_map })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- Path resolution --

    #[test]
    fn path_resolution() {
        let ctx = json!({
            "steps": {
                "build": {
                    "status": "completed"
                }
            }
        });
        assert!(evaluate("steps.build.status == 'completed'", &ctx).unwrap());
    }

    #[test]
    fn missing_path_returns_null() {
        let ctx = json!({});
        // null != 'completed' so comparison is false
        assert!(!evaluate("steps.build.status == 'completed'", &ctx).unwrap());
        // bare missing path is null → falsy
        assert!(!evaluate("steps.build.status", &ctx).unwrap());
    }

    // -- Literal comparisons --

    #[test]
    fn string_comparison() {
        let ctx = json!({});
        assert!(evaluate("'completed' == 'completed'", &ctx).unwrap());
        assert!(!evaluate("'completed' == 'failed'", &ctx).unwrap());
    }

    #[test]
    fn number_comparison() {
        let ctx = json!({});
        assert!(evaluate("42 == 42", &ctx).unwrap());
        assert!(!evaluate("42 == 43", &ctx).unwrap());
    }

    #[test]
    fn float_number_comparison() {
        let ctx = json!({});
        assert!(evaluate("3.14 == 3.14", &ctx).unwrap());
        assert!(!evaluate("3.14 == 3.15", &ctx).unwrap());
    }

    #[test]
    fn boolean_literals() {
        let ctx = json!({});
        assert!(evaluate("true", &ctx).unwrap());
        assert!(!evaluate("false", &ctx).unwrap());
    }

    // -- Logical operators --

    #[test]
    fn and_operator() {
        let ctx = json!({});
        assert!(evaluate("true && true", &ctx).unwrap());
        assert!(!evaluate("true && false", &ctx).unwrap());
        assert!(!evaluate("false && true", &ctx).unwrap());
        assert!(!evaluate("false && false", &ctx).unwrap());
    }

    #[test]
    fn or_operator() {
        let ctx = json!({});
        assert!(evaluate("true || true", &ctx).unwrap());
        assert!(evaluate("true || false", &ctx).unwrap());
        assert!(evaluate("false || true", &ctx).unwrap());
        assert!(!evaluate("false || false", &ctx).unwrap());
    }

    #[test]
    fn not_equal() {
        let ctx = json!({});
        assert!(evaluate("'a' != 'b'", &ctx).unwrap());
        assert!(!evaluate("'a' != 'a'", &ctx).unwrap());
    }

    // -- Parentheses --

    #[test]
    fn parentheses() {
        let ctx = json!({});
        assert!(evaluate("(true || false) && true", &ctx).unwrap());
        assert!(!evaluate("(true || false) && false", &ctx).unwrap());
        assert!(evaluate("true || (false && false)", &ctx).unwrap());
    }

    // -- Complex expressions --

    #[test]
    fn complex_step_conditions() {
        let ctx = json!({
            "steps": {
                "build": { "status": "completed" },
                "test": { "status": "completed" }
            }
        });
        assert!(
            evaluate(
                "steps.build.status == 'completed' && steps.test.status == 'completed'",
                &ctx,
            )
            .unwrap()
        );

        let ctx_fail = json!({
            "steps": {
                "build": { "status": "completed" },
                "test": { "status": "failed" }
            }
        });
        assert!(
            !evaluate(
                "steps.build.status == 'completed' && steps.test.status == 'completed'",
                &ctx_fail,
            )
            .unwrap()
        );
    }

    #[test]
    fn missing_step() {
        let ctx = json!({
            "steps": {
                "build": { "status": "completed" }
            }
        });
        assert!(!evaluate("steps.missing.status == 'completed'", &ctx).unwrap());
    }

    // -- Error cases --

    #[test]
    fn malformed_expression() {
        let ctx = json!({});
        assert!(evaluate("== 'foo'", &ctx).is_err());
    }

    #[test]
    fn unterminated_string() {
        let ctx = json!({});
        assert!(evaluate("'unterminated", &ctx).is_err());
    }

    #[test]
    fn unmatched_paren() {
        let ctx = json!({});
        assert!(evaluate("(true", &ctx).is_err());
    }

    #[test]
    fn empty_expression_is_true() {
        let ctx = json!({});
        assert!(evaluate("", &ctx).unwrap());
        assert!(evaluate("   ", &ctx).unwrap());
    }

    // -- Cross-type comparison --

    #[test]
    fn cross_type_comparison_is_false() {
        let ctx = json!({});
        assert!(!evaluate("42 == 'forty-two'", &ctx).unwrap());
        assert!(!evaluate("true == 'true'", &ctx).unwrap());
        assert!(!evaluate("true == 1", &ctx).unwrap());
    }

    // -- Null equality --

    #[test]
    fn null_equals_null() {
        let ctx = json!({});
        // Two missing paths both resolve to null — they should be equal
        assert!(evaluate("missing.a == missing.b", &ctx).unwrap());
    }

    // -- build_step_context --

    #[test]
    fn build_context_structure() {
        use crate::step::{StepDef, StepResult, StepStatus};

        let build = StepDef::new("build");
        let test = StepDef::new("test");
        let steps = vec![build.clone(), test.clone()];

        let results = vec![
            StepResult {
                step_id: build.id,
                status: StepStatus::Completed,
                output: json!({"artifact": "build.tar.gz"}),
                duration_ms: 1200,
                attempts: 1,
                error: None,
            },
            StepResult {
                step_id: test.id,
                status: StepStatus::Failed,
                output: json!(null),
                duration_ms: 500,
                attempts: 2,
                error: Some("assertion failed".into()),
            },
        ];

        let ctx = build_step_context(&results, &steps);

        // Verify structure
        assert_eq!(ctx["steps"]["build"]["status"], "completed");
        assert_eq!(ctx["steps"]["build"]["output"]["artifact"], "build.tar.gz");
        assert!(ctx["steps"]["build"]["error"].is_null());

        assert_eq!(ctx["steps"]["test"]["status"], "failed");
        assert_eq!(ctx["steps"]["test"]["error"], "assertion failed");

        // Use with evaluate
        assert!(evaluate("steps.build.status == 'completed'", &ctx).unwrap());
        assert!(!evaluate("steps.test.status == 'completed'", &ctx).unwrap());
        assert!(evaluate("steps.test.status == 'failed'", &ctx).unwrap());
    }

    #[test]
    fn build_context_skips_unknown_step_ids() {
        use crate::step::{StepDef, StepResult, StepStatus};
        use uuid::Uuid;

        let build = StepDef::new("build");
        let steps = vec![build.clone()];

        let results = vec![
            StepResult {
                step_id: build.id,
                status: StepStatus::Completed,
                output: json!(null),
                duration_ms: 100,
                attempts: 1,
                error: None,
            },
            StepResult {
                step_id: Uuid::new_v4(), // unknown
                status: StepStatus::Failed,
                output: json!(null),
                duration_ms: 0,
                attempts: 1,
                error: Some("orphan".into()),
            },
        ];

        let ctx = build_step_context(&results, &steps);
        // Only the known step should appear
        assert_eq!(ctx["steps"]["build"]["status"], "completed");
        assert!(ctx["steps"].as_object().unwrap().len() == 1);
    }

    // -- Operator precedence --

    #[test]
    fn and_binds_tighter_than_or() {
        let ctx = json!({});
        // false || true && true  →  false || (true && true) → true
        assert!(evaluate("false || true && true", &ctx).unwrap());
        // true || true && false  →  true || (true && false) → true
        assert!(evaluate("true || true && false", &ctx).unwrap());
        // false || false && true →  false || (false && true) → false
        assert!(!evaluate("false || false && true", &ctx).unwrap());
    }

    // -- Trailing token error --

    #[test]
    fn trailing_tokens_are_error() {
        let ctx = json!({});
        assert!(evaluate("true true", &ctx).is_err());
    }

    // -- Short-circuit evaluation --

    #[test]
    fn and_short_circuits_on_false() {
        let ctx = json!({});
        // false && <anything> should return false without evaluating the right side
        // Here, the right side resolves to null (falsy) — but the result should be
        // false regardless, because the left side is false.
        assert!(!evaluate("false && missing.path", &ctx).unwrap());
    }

    #[test]
    fn or_short_circuits_on_true() {
        let ctx = json!({});
        // true || <anything> should return true without evaluating the right side
        assert!(evaluate("true || missing.path", &ctx).unwrap());
    }

    // -- String literals with non-ASCII content --

    #[test]
    fn string_literal_with_unicode() {
        let ctx = json!({"greeting": "héllo"});
        assert!(evaluate("greeting == 'héllo'", &ctx).unwrap());
        assert!(!evaluate("greeting == 'hello'", &ctx).unwrap());
    }

    #[test]
    fn render_template_with_unicode() {
        let ctx = json!({"name": "André"});
        assert_eq!(render_template("Hello {{name}}!", &ctx), "Hello André!");
    }

    #[test]
    fn render_template_with_unicode_literal_text() {
        let ctx = json!({"x": "y"});
        assert_eq!(render_template("café {{x}}", &ctx), "café y");
    }

    // -- Comparison operators --

    #[test]
    fn greater_than() {
        let ctx = json!({});
        assert!(evaluate("10 > 5", &ctx).unwrap());
        assert!(!evaluate("5 > 10", &ctx).unwrap());
        assert!(!evaluate("5 > 5", &ctx).unwrap());
    }

    #[test]
    fn greater_than_or_equal() {
        let ctx = json!({});
        assert!(evaluate("10 >= 5", &ctx).unwrap());
        assert!(evaluate("5 >= 5", &ctx).unwrap());
        assert!(!evaluate("4 >= 5", &ctx).unwrap());
    }

    #[test]
    fn less_than() {
        let ctx = json!({});
        assert!(evaluate("5 < 10", &ctx).unwrap());
        assert!(!evaluate("10 < 5", &ctx).unwrap());
        assert!(!evaluate("5 < 5", &ctx).unwrap());
    }

    #[test]
    fn less_than_or_equal() {
        let ctx = json!({});
        assert!(evaluate("5 <= 10", &ctx).unwrap());
        assert!(evaluate("5 <= 5", &ctx).unwrap());
        assert!(!evaluate("6 <= 5", &ctx).unwrap());
    }

    #[test]
    fn comparison_with_paths() {
        let ctx = json!({"a": 10, "b": 5});
        assert!(evaluate("a > b", &ctx).unwrap());
        assert!(!evaluate("b > a", &ctx).unwrap());
        assert!(evaluate("a >= 10", &ctx).unwrap());
        assert!(evaluate("b < a", &ctx).unwrap());
    }

    #[test]
    fn comparison_with_floats() {
        let ctx = json!({});
        assert!(evaluate("3.14 > 2.71", &ctx).unwrap());
        assert!(evaluate("2.71 < 3.14", &ctx).unwrap());
        assert!(evaluate("3.14 >= 3.14", &ctx).unwrap());
    }

    #[test]
    fn string_comparison_ordering() {
        let ctx = json!({});
        assert!(evaluate("'banana' > 'apple'", &ctx).unwrap());
        assert!(evaluate("'apple' < 'banana'", &ctx).unwrap());
        assert!(evaluate("'apple' <= 'apple'", &ctx).unwrap());
    }

    #[test]
    fn cross_type_comparison_returns_false() {
        let ctx = json!({});
        assert!(!evaluate("42 > 'hello'", &ctx).unwrap());
        assert!(!evaluate("'hello' < 42", &ctx).unwrap());
    }

    // -- Not operator --

    #[test]
    fn not_operator() {
        let ctx = json!({});
        assert!(!evaluate("!true", &ctx).unwrap());
        assert!(evaluate("!false", &ctx).unwrap());
    }

    #[test]
    fn not_with_path() {
        let ctx = json!({"enabled": false, "missing": null});
        assert!(evaluate("!enabled", &ctx).unwrap());
        assert!(evaluate("!missing", &ctx).unwrap());
    }

    #[test]
    fn not_with_comparison() {
        let ctx = json!({});
        assert!(evaluate("!(5 > 10)", &ctx).unwrap());
        assert!(!evaluate("!(10 > 5)", &ctx).unwrap());
    }

    #[test]
    fn double_not() {
        let ctx = json!({});
        assert!(evaluate("!!true", &ctx).unwrap());
        assert!(!evaluate("!!false", &ctx).unwrap());
    }

    #[test]
    fn not_in_compound_expression() {
        let ctx = json!({"status": "failed"});
        // ! binds tighter than ==, so use parens for negating a comparison
        assert!(evaluate("!(status == 'completed') && status == 'failed'", &ctx).unwrap());
        // Without parens: (!status) == 'completed' → false == 'completed' → false
        assert!(!evaluate("!status == 'completed'", &ctx).unwrap());
    }
}
