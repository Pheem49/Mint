use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CalculationError {
    #[error("expression is required")]
    MissingExpression,
    #[error("calculation error: {0}")]
    EvalFailed(String),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalculationReport {
    pub expression: String,
    pub result: f64,
    pub data: String,
    pub json_payload: Value,
}

pub fn calculate(expr: &str) -> Result<CalculationReport, CalculationError> {
    let clean_expr = expr.trim();
    if clean_expr.is_empty() {
        return Err(CalculationError::MissingExpression);
    }

    let mut parsed_expr = clean_expr.to_string();

    // Convert "25% of 8500" -> "(25 / 100) * 8500"
    if let Ok(re) =
        regex::Regex::new(r"(?i)^(\d+(?:\.\d+)?)\s*%\s*(?:of|ของ|\*)\s*(\d+(?:\.\d+)?)$")
    {
        if let Some(caps) = re.captures(&parsed_expr) {
            if let (Some(p), Some(v)) = (caps.get(1), caps.get(2)) {
                parsed_expr = format!("({} / 100) * {}", p.as_str(), v.as_str());
            }
        }
    }

    // Replace % at the end of number: "25%" -> "(25 / 100)"
    if let Ok(re) = regex::Regex::new(r"(\d+(?:\.\d+)?)\s*%") {
        parsed_expr = re.replace_all(&parsed_expr, "($1 / 100)").to_string();
    }

    let result = eval_math(&parsed_expr)?;

    let json_payload = json!({
        "expression": clean_expr,
        "result": result
    });

    let json_str = serde_json::to_string(&json_payload).unwrap_or_default();
    let text_summary = format!("{clean_expr} = {result}");
    let full_data = format!("{text_summary}\n\n```calculation_json\n{json_str}\n```");

    Ok(CalculationReport {
        expression: clean_expr.to_string(),
        result,
        data: full_data,
        json_payload,
    })
}

fn eval_math(expr: &str) -> Result<f64, CalculationError> {
    let expr = expr.replace(' ', "");
    let mut chars = expr.chars().peekable();
    parse_expr(&mut chars).map_err(|e| CalculationError::EvalFailed(e))
}

fn parse_expr(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<f64, String> {
    let mut term = parse_term(chars)?;
    while let Some(&op) = chars.peek() {
        if op == '+' || op == '-' {
            chars.next();
            let next_term = parse_term(chars)?;
            if op == '+' {
                term += next_term;
            } else {
                term -= next_term;
            }
        } else {
            break;
        }
    }
    Ok(term)
}

fn parse_term(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<f64, String> {
    let mut factor = parse_factor(chars)?;
    while let Some(&op) = chars.peek() {
        if op == '*' || op == '/' {
            chars.next();
            let next_factor = parse_factor(chars)?;
            if op == '*' {
                factor *= next_factor;
            } else {
                if next_factor == 0.0 {
                    return Err("Division by zero".into());
                }
                factor /= next_factor;
            }
        } else {
            break;
        }
    }
    Ok(factor)
}

fn parse_factor(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<f64, String> {
    match chars.peek() {
        Some('(') => {
            chars.next();
            let val = parse_expr(chars)?;
            if chars.next() != Some(')') {
                return Err("Missing closing parenthesis".into());
            }
            Ok(val)
        }
        Some('-') => {
            chars.next();
            Ok(-parse_factor(chars)?)
        }
        Some('+') => {
            chars.next();
            parse_factor(chars)
        }
        Some(c) if c.is_ascii_digit() || *c == '.' => {
            let mut num_str = String::new();
            while let Some(&ch) = chars.peek() {
                if ch.is_ascii_digit() || ch == '.' {
                    num_str.push(ch);
                    chars.next();
                } else {
                    break;
                }
            }
            num_str.parse::<f64>().map_err(|_| "Invalid number".into())
        }
        Some(c) => Err(format!("Unexpected character: {c}")),
        None => Err("Unexpected end of expression".into()),
    }
}
