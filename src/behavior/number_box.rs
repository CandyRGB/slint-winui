use crate::{NumberBoxMath, NumberBoxParseResult, NumberBoxValue};

pub fn install(math: NumberBoxMath<'_>) {
    math.on_parse(|text, expression, separator| parse(&text, expression, &separator));
    math.on_step(|value, change| {
        // Use decimal representations so a series of steps does not accumulate f32 noise.
        let decimal = |value: f32| value.to_string().parse::<f64>().unwrap_or(f64::from(value));
        (decimal(value) + decimal(change)) as f32
    });
}

fn parse(text: &str, expression: bool, separator: &str) -> NumberBoxParseResult {
    let text = text.trim();
    if text.is_empty() {
        return NumberBoxParseResult {
            valid: true,
            ..Default::default()
        };
    }
    let normalized = if separator != "." && !separator.is_empty() {
        if text.contains('.') {
            return NumberBoxParseResult::default();
        }
        text.replace(separator, ".")
    } else {
        text.to_owned()
    };

    let value = if expression {
        let mut parser = Expression { rest: &normalized };
        parser
            .evaluate(0, 0)
            .filter(|_| parser.rest.trim().is_empty())
    } else {
        normalized
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
    };

    match value {
        Some(value) => NumberBoxParseResult {
            valid: true,
            value: NumberBoxValue {
                number: if value.is_nan() { 0.0 } else { value as f32 },
                has_value: !value.is_nan(),
            },
        },
        None => NumberBoxParseResult::default(),
    }
}

struct Expression<'a> {
    rest: &'a str,
}

impl Expression<'_> {
    fn evaluate(&mut self, minimum_precedence: u8, depth: u8) -> Option<f64> {
        if depth == 64 {
            return None;
        }
        self.rest = self.rest.trim_start();
        let mut left = if let Some(inner) = self.rest.strip_prefix('(') {
            self.rest = inner;
            let value = self.evaluate(0, depth + 1)?;
            self.rest = self.rest.trim_start().strip_prefix(')')?;
            value
        } else {
            self.number()?
        };

        loop {
            self.rest = self.rest.trim_start();
            let operator = match self.rest.as_bytes().first() {
                Some(&operator) => operator,
                None => break,
            };
            let precedence = match operator {
                b'+' | b'-' => 1,
                b'*' | b'/' => 2,
                b'^' => 3,
                _ => break,
            };
            if precedence < minimum_precedence {
                break;
            }
            self.rest = &self.rest[1..];
            let right = self.evaluate(precedence + 1, depth + 1)?;
            left = match operator {
                b'+' => left + right,
                b'-' => left - right,
                b'*' => left * right,
                b'/' if right == 0.0 => f64::NAN,
                b'/' => left / right,
                b'^' => left.powf(right),
                _ => unreachable!(),
            };
        }
        Some(left)
    }

    fn number(&mut self) -> Option<f64> {
        let mut end = usize::from(self.rest.starts_with('-'));
        while matches!(self.rest.as_bytes().get(end), Some(b'0'..=b'9' | b'.')) {
            end += 1;
        }
        if matches!(self.rest.as_bytes().get(end), Some(b'e' | b'E')) {
            end += 1;
            if matches!(self.rest.as_bytes().get(end), Some(b'+' | b'-')) {
                end += 1;
            }
            while matches!(self.rest.as_bytes().get(end), Some(b'0'..=b'9')) {
                end += 1;
            }
        }
        let value = self.rest[..end].parse::<f64>().ok()?;
        self.rest = &self.rest[end..];
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expressions_follow_winui_precedence_and_associativity() {
        for (text, expected) in [
            ("2 + 3 * 4", 14.0),
            ("(2 + 3) * 4", 20.0),
            ("2 ^ 3 ^ 2", 64.0),
            ("2--3", 5.0),
            ("1e-2 + 0.5", 0.51),
        ] {
            let result = parse(text, true, ".");
            assert!(result.valid && result.value.has_value);
            assert_eq!(result.value.number, expected);
        }
    }

    #[test]
    fn empty_input_invalid_input_and_division_by_zero_are_distinct() {
        for text in [" ", "1/0"] {
            let result = parse(text, true, ".");
            assert!(result.valid && !result.value.has_value);
        }
        for text in ["two", "2++", "3)", "(2+3", "4 5", "()", "1,4"] {
            assert!(!parse(text, true, ".").valid, "{text}");
        }
        assert!(!parse("1+2", false, ".").valid);
        assert!(!parse(&format!("{}1{}", "(".repeat(65), ")".repeat(65)), true, ".").valid);
    }

    #[test]
    fn decimal_separator_applies_to_plain_input_and_expressions() {
        assert_eq!(parse(" 1,25 ", false, ",").value.number, 1.25);
        assert_eq!(parse("2 * 0,75", true, ",").value.number, 1.5);
        assert!(!parse("1.25", false, ",").valid);
    }
}
