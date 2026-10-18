//! Settings schema that drives the generated options page, plus validation of a `[features.<id>]` table.

use crate::error::FeatureError;
use serde::Serialize;

/// All options of one feature.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSchema {
    /// Fields in display order.
    pub fields: Vec<Field>,
}

/// One option.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    /// Key inside `[features.<id>]`.
    pub key: String,
    /// Label shown in the UI.
    pub label: String,
    /// Help text under the label.
    pub help: String,
    /// Control type.
    pub kind: FieldKind,
    /// Default value.
    pub default: serde_json::Value,
    /// True when changing it needs a stop then start.
    pub restart: bool,
}

/// Column of a [`FieldKind::KeyedList`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    /// Key in each row table.
    pub key: String,
    /// Column header.
    pub label: String,
}

/// Control type of a field.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum FieldKind {
    /// Toggle.
    Bool,
    /// Number with bounds.
    Number {
        /// Lowest allowed value.
        min: f64,
        /// Highest allowed value.
        max: f64,
        /// UI step.
        step: f64,
    },
    /// Free text.
    Text {
        /// Placeholder text.
        placeholder: String,
    },
    /// One of a fixed set of strings.
    Select {
        /// Allowed values.
        options: Vec<String>,
    },
    /// List of strings.
    StringList,
    /// List of rows with string columns (for example web shortcuts).
    KeyedList {
        /// Columns of each row.
        columns: Vec<Column>,
    },
    /// Folder or file path.
    Path,
    /// Secret entered through the Add-key flow; never stored in config.
    #[serde(rename_all = "camelCase")]
    Secret {
        /// Credential Manager entry name template.
        name_template: String,
    },
}

fn bad(key: &str, why: &str) -> FeatureError {
    FeatureError::Options(format!("{key}: {why}"))
}

fn check(f: &Field, v: &toml::Value) -> Result<(), FeatureError> {
    let k = f.key.as_str();
    match &f.kind {
        FieldKind::Bool if !v.is_bool() => Err(bad(k, "expected true or false")),
        FieldKind::Number { min, max, .. } => {
            let n = match v {
                toml::Value::Integer(i) => *i as f64,
                toml::Value::Float(x) => *x,
                _ => return Err(bad(k, "expected a number")),
            };
            if n < *min || n > *max {
                return Err(bad(k, &format!("must be between {min} and {max}")));
            }
            Ok(())
        }
        FieldKind::Text { .. } | FieldKind::Path if !v.is_str() => Err(bad(k, "expected text")),
        FieldKind::Select { options } => match v.as_str() {
            Some(s) if options.iter().any(|o| o == s) => Ok(()),
            _ => Err(bad(k, "not one of the allowed values")),
        },
        FieldKind::StringList => match v.as_array() {
            Some(a) if a.iter().all(toml::Value::is_str) => Ok(()),
            _ => Err(bad(k, "expected a list of text")),
        },
        FieldKind::KeyedList { columns } => {
            let rows = v
                .as_array()
                .ok_or_else(|| bad(k, "expected a list of rows"))?;
            for row in rows {
                let t = row
                    .as_table()
                    .ok_or_else(|| bad(k, "each row must be a table"))?;
                if columns
                    .iter()
                    .any(|c| t.get(&c.key).is_some_and(|x| !x.is_str()))
                {
                    return Err(bad(k, "row values must be text"));
                }
            }
            Ok(())
        }
        FieldKind::Secret { .. } => Err(bad(k, "secrets are never stored in config")),
        _ => Ok(()),
    }
}

/// Checks every key of `table` that the schema knows. Unknown keys (such as `enabled`) pass.
pub fn validate(schema: &SettingsSchema, table: &toml::Table) -> Result<(), FeatureError> {
    for f in &schema.fields {
        if let Some(v) = table.get(&f.key) {
            check(f, v)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema() -> SettingsSchema {
        SettingsSchema {
            fields: vec![
                Field {
                    key: "days".into(),
                    label: "Days".into(),
                    help: String::new(),
                    kind: FieldKind::Number {
                        min: 1.0,
                        max: 30.0,
                        step: 1.0,
                    },
                    default: serde_json::json!(7),
                    restart: false,
                },
                Field {
                    key: "key".into(),
                    label: "Key".into(),
                    help: String::new(),
                    kind: FieldKind::Secret {
                        name_template: "x/{id}".into(),
                    },
                    default: serde_json::Value::Null,
                    restart: true,
                },
            ],
        }
    }

    #[test]
    fn json_shape_is_tagged_camel_case() {
        let v = serde_json::to_value(schema()).unwrap();
        assert_eq!(v["fields"][0]["kind"]["type"], "number");
        assert_eq!(v["fields"][0]["kind"]["min"], 1.0);
        assert_eq!(v["fields"][1]["kind"]["type"], "secret");
        assert_eq!(v["fields"][1]["kind"]["nameTemplate"], "x/{id}");
    }

    #[test]
    fn rejects_out_of_range_and_secret() {
        let s = schema();
        assert!(validate(&s, &"days = 99".parse().unwrap()).is_err());
        assert!(validate(&s, &"days = 7\nother = 1".parse().unwrap()).is_ok());
        assert!(validate(&s, &"key = \"abc\"".parse().unwrap()).is_err());
    }
}
