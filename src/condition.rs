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
//! cmp_expr = value (("==" | "!=") value)?
//! value    = path | string_lit | number_lit | bool_lit | "(" expr ")"
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
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Skip whitespace
        if chars[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // Two-char operators
        if i + 1 < len {
            let two = &input[i..i + 2];
            match two {
                "==" => {
                    tokens.push(Token::Eq);
                    i += 2;
                    continue;
                }
                "!=" => {
                    tokens.push(Token::NotEq);
                    i += 2;
                    continue;
                }
                "&&" => {
                    tokens.push(Token::And);
                    i += 2;
                    continue;
                }
                "||" => {
                    tokens.push(Token::Or);
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }

        // Parentheses
        if chars[i] == '(' {
            tokens.push(Token::LParen);
            i += 1;
            continue;
        }
        if chars[i] == ')' {
            tokens.push(Token::RParen);
            i += 1;
            continue;
        }

        // String literal (single-quoted)
        if chars[i] == '\'' {
            i += 1;
            let start = i;
            while i < len && chars[i] != '\'' {
                i += 1;
            }
            if i >= len {
                return Err("unterminated string literal".into());
            }
            let s: String = chars[start..i].iter().collect();
            tokens.push(Token::StringLit(s));
            i += 1; // skip closing quote
            continue;
        }

        // Number literal
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < len && chars[i].is_ascii_digit() {
                i += 1;
            }
            if i < len && chars[i] == '.' && i + 1 < len && chars[i + 1].is_ascii_digit() {
                i += 1; // skip dot
                while i < len && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let num_str: String = chars[start..i].iter().collect();
            let val: f64 = num_str
                .parse()
                .map_err(|e| format!("invalid number '{num_str}': {e}"))?;
            tokens.push(Token::NumberLit(val));
            continue;
        }

        // Identifier / path / bool literal
        if chars[i].is_ascii_alphabetic() || chars[i] == '_' {
            let start = i;
            while i < len
                && (chars[i].is_ascii_alphanumeric()
                    || chars[i] == '_'
                    || chars[i] == '-'
                    || chars[i] == '.')
            {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            match word.as_str() {
                "true" => tokens.push(Token::BoolLit(true)),
                "false" => tokens.push(Token::BoolLit(false)),
                _ => tokens.push(Token::Path(word)),
            }
            continue;
        }

        return Err(format!("unexpected character '{}'", chars[i]));
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
            _ => Ok(left),
        }
    }

    fn parse_value(&mut self) -> Result<Expr, String> {
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
        Expr::And(left, right) => {
            let l = is_truthy(&eval_expr(left, context)?);
            let r = is_truthy(&eval_expr(right, context)?);
            Ok(Value::Bool(l && r))
        }
        Expr::Or(left, right) => {
            let l = is_truthy(&eval_expr(left, context)?);
            let r = is_truthy(&eval_expr(right, context)?);
            Ok(Value::Bool(l || r))
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
    let chars: Vec<char> = template.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if i + 1 < len && chars[i] == '{' && chars[i + 1] == '{' {
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
        result.push(chars[i]);
        i += 1;
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
}
