//! Math expression evaluator tool.

use crate::mcp::{Tool, result_error, result_ok, tool_def};
use bote::ToolDef as BoteToolDef;
use serde_json::json;
use std::pin::Pin;

/// Evaluate a basic math expression.
pub struct MathEval;

impl Tool for MathEval {
    fn definition(&self) -> BoteToolDef {
        tool_def(
            "szal_math_eval",
            "Evaluate a basic math expression (+, -, *, /, %, ^)",
            json!({ "expression": { "type": "string", "description": "Math expression to evaluate (e.g. '2 + 3 * 4')" } }),
            vec!["expression".into()],
        )
    }

    fn call(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn std::future::Future<Output = serde_json::Value> + Send + '_>> {
        Box::pin(async move {
            let expr = match args.get("expression").and_then(|v| v.as_str()) {
                Some(e) => e,
                None => return result_error("missing required field: expression"),
            };

            let valid = expr
                .chars()
                .all(|c| c.is_ascii_digit() || " +-*/.%()".contains(c));
            if !valid {
                return result_error(
                    "expression contains invalid characters — only digits, +, -, *, /, %, (, ), . allowed",
                );
            }

            match eval_expr(expr) {
                Ok(val) => {
                    // Format: strip trailing zeros for clean output
                    if val.fract() == 0.0 && val.abs() < i64::MAX as f64 {
                        result_ok(&format!("{}", val as i64))
                    } else {
                        result_ok(&format!("{val}"))
                    }
                }
                Err(e) => result_error(e),
            }
        })
    }
}

/// Simple recursive descent math evaluator.
fn eval_expr(input: &str) -> Result<f64, String> {
    let tokens = tokenize(input)?;
    let mut pos = 0;
    let result = parse_add_sub(&tokens, &mut pos)?;
    if pos < tokens.len() {
        return Err(format!("unexpected token at position {pos}"));
    }
    Ok(result)
}

#[derive(Debug, Clone)]
enum Token {
    Num(f64),
    Plus,
    Minus,
    Mul,
    Div,
    Mod,
    LParen,
    RParen,
}

fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            ' ' => {
                chars.next();
            }
            '+' => {
                tokens.push(Token::Plus);
                chars.next();
            }
            '-' => {
                // Unary minus: after operator, open paren, or at start
                let is_unary = tokens.is_empty()
                    || matches!(
                        tokens.last(),
                        Some(
                            Token::Plus
                                | Token::Minus
                                | Token::Mul
                                | Token::Div
                                | Token::Mod
                                | Token::LParen
                        )
                    );
                chars.next();
                if is_unary {
                    let mut num = String::with_capacity(16);
                    num.push('-');
                    while let Some(&d) = chars.peek() {
                        if d.is_ascii_digit() || d == '.' {
                            num.push(d);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    tokens.push(Token::Num(num.parse::<f64>().map_err(|e| e.to_string())?));
                } else {
                    tokens.push(Token::Minus);
                }
            }
            '*' => {
                tokens.push(Token::Mul);
                chars.next();
            }
            '/' => {
                tokens.push(Token::Div);
                chars.next();
            }
            '%' => {
                tokens.push(Token::Mod);
                chars.next();
            }
            '(' => {
                tokens.push(Token::LParen);
                chars.next();
            }
            ')' => {
                tokens.push(Token::RParen);
                chars.next();
            }
            _ if c.is_ascii_digit() || c == '.' => {
                let mut num = String::with_capacity(16);
                while let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() || d == '.' {
                        num.push(d);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Num(num.parse::<f64>().map_err(|e| e.to_string())?));
            }
            _ => return Err(format!("unexpected character: {c}")),
        }
    }
    Ok(tokens)
}

fn parse_add_sub(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_mul_div(tokens, pos)?;
    while *pos < tokens.len() {
        match tokens[*pos] {
            Token::Plus => {
                *pos += 1;
                left += parse_mul_div(tokens, pos)?;
            }
            Token::Minus => {
                *pos += 1;
                left -= parse_mul_div(tokens, pos)?;
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_mul_div(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_atom(tokens, pos)?;
    while *pos < tokens.len() {
        match tokens[*pos] {
            Token::Mul => {
                *pos += 1;
                left *= parse_atom(tokens, pos)?;
            }
            Token::Div => {
                *pos += 1;
                let right = parse_atom(tokens, pos)?;
                if right == 0.0 {
                    return Err("division by zero".into());
                }
                left /= right;
            }
            Token::Mod => {
                *pos += 1;
                let right = parse_atom(tokens, pos)?;
                if right == 0.0 {
                    return Err("modulo by zero".into());
                }
                left %= right;
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_atom(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    if *pos >= tokens.len() {
        return Err("unexpected end of expression".into());
    }
    match &tokens[*pos] {
        Token::Num(n) => {
            let v = *n;
            *pos += 1;
            Ok(v)
        }
        Token::LParen => {
            *pos += 1;
            let val = parse_add_sub(tokens, pos)?;
            if *pos >= tokens.len() || !matches!(tokens[*pos], Token::RParen) {
                return Err("missing closing parenthesis".into());
            }
            *pos += 1;
            Ok(val)
        }
        _ => Err(format!("unexpected token at position {pos}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn math_eval_basic() {
        let result = MathEval.call(json!({"expression": "2 + 3"})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.starts_with("5"));
    }

    #[tokio::test]
    async fn math_eval_complex() {
        let result = MathEval.call(json!({"expression": "10 * 5 + 3"})).await;
        assert_eq!(result["isError"], false);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.starts_with("53"));
    }

    #[tokio::test]
    async fn math_eval_rejects_injection() {
        let result = MathEval.call(json!({"expression": "1; rm -rf /"})).await;
        assert_eq!(result["isError"], true);
    }
}
