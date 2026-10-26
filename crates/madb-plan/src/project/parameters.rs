//! Closed parameter values and deterministic default expansion; no regex engine.
mod reserved;
use super::ProjectPlanError;
use madb_model::Id;
use std::collections::BTreeMap;

pub const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;
pub const IDENTIFIER_PATTERN: &str = "[A-Za-z][A-Za-z0-9_]{0,47}";
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParameterValue {
    String(String),
    Boolean(bool),
    Integer(i64),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StringPattern {
    Identifier,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Parameter {
    String {
        id: Id,
        required: bool,
        default: Option<String>,
        min_length: u32,
        max_length: u32,
        pattern: Option<StringPattern>,
        choices: Vec<String>,
    },
    Boolean {
        id: Id,
        required: bool,
        default: Option<bool>,
    },
    Integer {
        id: Id,
        required: bool,
        default: Option<i64>,
        minimum: i64,
        maximum: i64,
    },
}
pub type ParameterValues = BTreeMap<Id, ParameterValue>;
impl Parameter {
    pub fn id(&self) -> &Id {
        match self {
            Self::String { id, .. } | Self::Boolean { id, .. } | Self::Integer { id, .. } => id,
        }
    }
    pub fn required(&self) -> bool {
        match self {
            Self::String { required, .. }
            | Self::Boolean { required, .. }
            | Self::Integer { required, .. } => *required,
        }
    }
    pub fn default_value(&self) -> Option<ParameterValue> {
        match self {
            Self::String { default, .. } => default.clone().map(ParameterValue::String),
            Self::Boolean { default, .. } => default.map(ParameterValue::Boolean),
            Self::Integer { default, .. } => default.map(ParameterValue::Integer),
        }
    }
    pub fn validate_value(&self, value: &ParameterValue) -> Result<(), ProjectPlanError> {
        validate_value(self, value)
    }
}

/// `output_basename` is already obtained lexically by the request owner, not IO.
/// A missing name uses this basename before the template default. Other optional
/// parameters with no value/default remain absent (null is never a value).
pub fn resolve_parameters(
    parameters: &[Parameter],
    supplied: &ParameterValues,
    output_basename: &str,
) -> Result<ParameterValues, ProjectPlanError> {
    check_parameters(parameters)?;
    let by_id: BTreeMap<_, _> = parameters.iter().map(|p| (p.id(), p)).collect();
    for id in supplied.keys() {
        if !by_id.contains_key(id) {
            return Err(ProjectPlanError::new(
                id.as_str(),
                "unknown template parameter",
            ));
        }
    }
    let mut result = BTreeMap::new();
    for parameter in parameters {
        let value = supplied.get(parameter.id()).cloned().or_else(|| {
            if parameter.id().as_str() == "name" {
                Some(ParameterValue::String(output_basename.to_owned()))
            } else {
                parameter.default_value()
            }
        });
        match value {
            Some(value) => {
                parameter.validate_value(&value)?;
                result.insert(parameter.id().clone(), value);
            }
            None if parameter.required() => {
                return Err(ProjectPlanError::new(
                    parameter.id().as_str(),
                    "required parameter has no value or default",
                ));
            }
            None => {}
        }
    }
    Ok(result)
}

pub(super) fn check_parameters(parameters: &[Parameter]) -> Result<(), ProjectPlanError> {
    if parameters.len() > 4096 {
        return Err(ProjectPlanError::new(
            "parameters",
            "parameter array exceeds item budget",
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for parameter in parameters {
        if !seen.insert(parameter.id()) {
            return Err(ProjectPlanError::new(
                parameter.id().as_str(),
                "duplicate parameter identity",
            ));
        }
        match parameter {
            Parameter::String {
                min_length,
                max_length,
                choices,
                ..
            } => {
                if min_length > max_length || *max_length > 4096 {
                    return Err(ProjectPlanError::new(
                        parameter.id().as_str(),
                        "invalid string parameter length bounds",
                    ));
                }
                if choices.len() > 4096 {
                    return Err(ProjectPlanError::new(
                        parameter.id().as_str(),
                        "parameter choices exceed item budget",
                    ));
                }
                let mut unique = std::collections::BTreeSet::new();
                for choice in choices {
                    if !unique.insert(choice) {
                        return Err(ProjectPlanError::new(
                            parameter.id().as_str(),
                            "duplicate parameter choice",
                        ));
                    }
                    parameter.validate_value(&ParameterValue::String(choice.clone()))?;
                }
            }
            Parameter::Integer {
                minimum, maximum, ..
            } if minimum > maximum
                || *minimum < -MAX_SAFE_INTEGER
                || *maximum > MAX_SAFE_INTEGER =>
            {
                return Err(ProjectPlanError::new(
                    parameter.id().as_str(),
                    "invalid safe integer parameter bounds",
                ));
            }
            _ => {}
        }
        if let Some(default) = parameter.default_value() {
            parameter.validate_value(&default)?;
        }
    }
    Ok(())
}
fn validate_value(parameter: &Parameter, value: &ParameterValue) -> Result<(), ProjectPlanError> {
    let valid = match (parameter, value) {
        (
            Parameter::String {
                min_length,
                max_length,
                pattern,
                choices,
                ..
            },
            ParameterValue::String(value),
        ) => {
            let len = value.chars().count();
            len >= *min_length as usize
                && len <= *max_length as usize
                && (choices.is_empty() || choices.contains(value))
                && (pattern.is_none() || is_identifier_shape(value))
        }
        (Parameter::Boolean { .. }, ParameterValue::Boolean(_)) => true,
        (
            Parameter::Integer {
                minimum, maximum, ..
            },
            ParameterValue::Integer(value),
        ) => {
            (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(value)
                && value >= minimum
                && value <= maximum
        }
        _ => false,
    };
    if !valid {
        return Err(ProjectPlanError::new(
            parameter.id().as_str(),
            "parameter type or constraints do not match",
        ));
    }
    Ok(())
}
pub fn is_identifier_shape(value: &str) -> bool {
    (1..=48).contains(&value.len())
        && value.as_bytes()[0].is_ascii_alphabetic()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
}
/// Fixed C11 / Rust 2024 keyword and C standard-library reservation table. This
/// restricts identifiers only; it is not a claim that arbitrary source is safe.
pub fn validate_identifier(value: &str) -> Result<(), ProjectPlanError> {
    if !is_identifier_shape(value) || reserved::is_reserved(value) {
        return Err(ProjectPlanError::new(
            value,
            "identifier is malformed or reserved by C or Rust",
        ));
    }
    Ok(())
}
