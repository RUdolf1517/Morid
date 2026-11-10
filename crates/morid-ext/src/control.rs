//! Progress predicates follow legacy/src/engines.cpp:1494–1582.
use serde_json::Value;

/// A validated predicate; only the first present key participates.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Predicate {
    Never,
    Starts(String),
    Ends(String),
    Contains(String),
    Any(Vec<String>),
    All(Vec<String>),
}

impl Predicate {
    fn parse(value: &Value) -> Result<Self, String> {
        let object = value.as_object().ok_or("progress side must be an object")?;
        for key in [
            "startsWith",
            "endsWith",
            "contains",
            "containsAny",
            "containsAll",
        ] {
            if let Some(value) = object.get(key) {
                return match key {
                    "startsWith" | "endsWith" | "contains" => {
                        let text = value
                            .as_str()
                            .ok_or_else(|| format!("{key} must be a string"))?
                            .to_owned();
                        Ok(match key {
                            "startsWith" => Self::Starts(text),
                            "endsWith" => Self::Ends(text),
                            _ => Self::Contains(text),
                        })
                    }
                    _ => {
                        let words: Vec<String> = value
                            .as_array()
                            .ok_or_else(|| format!("{key} must be a string array"))?
                            .iter()
                            .map(|word| {
                                word.as_str()
                                    .map(str::to_owned)
                                    .ok_or_else(|| format!("{key} must contain strings"))
                            })
                            .collect::<Result<_, _>>()?;
                        Ok(if key == "containsAny" {
                            Self::Any(words)
                        } else {
                            Self::All(words)
                        })
                    }
                };
            }
        }
        Ok(Self::Never)
    }

    fn matches(&self, line: &str) -> bool {
        match self {
            Self::Never => false,
            Self::Starts(text) => line.starts_with(text),
            Self::Ends(text) => line.ends_with(text),
            Self::Contains(text) => line.contains(text),
            Self::Any(words) => words.iter().any(|word| line.contains(word)),
            Self::All(words) => words.iter().all(|word| line.contains(word)),
        }
    }
}

/// Empty structure means no progress definition, unlike an empty side predicate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control(Option<Expression>);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Expression {
    Single(Predicate),
    And(Predicate, Predicate),
    Or(Predicate, Predicate),
}

impl Control {
    /// Parsing errors are wrapped by the extension reader with file and rule.
    pub fn parse(value: &Value) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or("ControlJsonStructure must be an object")?;
        if object.is_empty() {
            return Ok(Self(None));
        }
        let connector = match object.get("Connector") {
            None => "",
            Some(value) => value.as_str().ok_or("Connector must be a string")?,
        };
        let lhs = Predicate::parse(object.get("lhs").ok_or("progress rule requires lhs")?)?;
        let expression = match connector {
            "" => Expression::Single(lhs),
            "&&" | "||" => {
                let rhs = Predicate::parse(object.get("rhs").ok_or("connector requires rhs")?)?;
                if connector == "&&" {
                    Expression::And(lhs, rhs)
                } else {
                    Expression::Or(lhs, rhs)
                }
            }
            _ => return Err("Connector must be empty, && or ||".to_owned()),
        };
        Ok(Self(Some(expression)))
    }

    pub fn is_defined(&self) -> bool {
        self.0.is_some()
    }

    pub fn matches(&self, line: &str) -> bool {
        match &self.0 {
            None => false,
            Some(Expression::Single(lhs)) => lhs.matches(line),
            Some(Expression::And(lhs, rhs)) => lhs.matches(line) && rhs.matches(line),
            Some(Expression::Or(lhs, rhs)) => lhs.matches(line) || rhs.matches(line),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn control_json_structure_evaluates() {
        let cases = [
            (
                json!({"Connector":"&&","lhs":{"startsWith":"[download]"},"rhs":{"contains":"ETA"}}),
                "[download] 1% ETA 1",
                true,
            ),
            (
                json!({"Connector":"&&","lhs":{"startsWith":"[download]"},"rhs":{"contains":"ETA"}}),
                "[download] done",
                false,
            ),
            (
                json!({"Connector":"||","lhs":{"startsWith":"[gallery-dl]"},"rhs":{"contains":"% "}}),
                "20% done",
                true,
            ),
            (
                json!({"Connector":"||","lhs":{"startsWith":"[gallery-dl]"},"rhs":{"contains":"% "}}),
                "noise",
                false,
            ),
            (json!({"lhs":{"contains":"ETA"}}), "ETA 2", true),
            (
                json!({"lhs":{"containsAny":["%[","% [","<=>"]}}),
                "1% [done]",
                true,
            ),
            (
                json!({"Connector":"","lhs":{"containsAll":["[","]"]},"rhs":42}),
                "[done]",
                true,
            ),
            (json!({"lhs":{}}), "anything", false),
            (json!({}), "anything", false),
        ];
        for (value, line, expected) in cases {
            assert_eq!(
                Control::parse(&value).expect("valid rule").matches(line),
                expected,
                "{value} on {line}"
            );
        }
        assert!(!Control::parse(&json!({})).expect("empty").is_defined());
        assert!(Control::parse(&json!({"lhs":{}}))
            .expect("empty side")
            .is_defined());
        for value in [
            json!(null),
            json!([]),
            json!({"Connector":"xor","lhs":{},"rhs":{}}),
            json!({"Connector":"&&","lhs":{}}),
            json!({"Connector":"||","rhs":{}}),
            json!({"lhs":false}),
            json!({"lhs":{"containsAny":[1]}}),
        ] {
            assert!(Control::parse(&value).is_err(), "{value}");
        }
    }

    #[test]
    fn predicate_priority_is_legacy_order_not_conjunction() {
        let keys = [
            "startsWith",
            "endsWith",
            "contains",
            "containsAny",
            "containsAll",
        ];
        for (index, key) in keys.iter().enumerate() {
            let mut side = serde_json::Map::new();
            for later in &keys[index..] {
                side.insert(
                    (*later).to_owned(),
                    if later.starts_with("containsA") {
                        json!(["absent"])
                    } else {
                        json!("absent")
                    },
                );
            }
            side.insert(
                (*key).to_owned(),
                if key.starts_with("containsA") {
                    json!(["x"])
                } else {
                    json!("x")
                },
            );
            assert!(
                Control::parse(&json!({"lhs":side}))
                    .expect("valid")
                    .matches("x"),
                "first key {key}"
            );
        }
    }
}
