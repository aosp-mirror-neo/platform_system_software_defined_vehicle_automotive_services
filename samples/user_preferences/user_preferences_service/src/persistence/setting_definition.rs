// Copyright 2024 Google LLC

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use user_preferences_api::setting::SettingDefinition as SettingDefinitionProto;

use crate::persistence::constraint::Constraint;
use crate::persistence::constraint::{
    EnumConstraint, FloatConstraint, Int32Constraint, Int64Constraint,
};
use crate::persistence::setting::Value;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SettingDefinition {
    pub kind: i32,
    pub name: String,
    pub default_value: Value,
    pub constraint: Option<Constraint>,
}

impl From<SettingDefinitionProto> for SettingDefinition {
    fn from(proto: SettingDefinitionProto) -> Self {
        let setting_and_constraints = proto.setting_and_constraints.unwrap();
        let setting = setting_and_constraints.setting.unwrap();
        let constraints = setting_and_constraints.constraints;
        Self {
            name: setting.key,
            kind: proto.kind.value(),
            default_value: setting.value.unwrap().into(),
            constraint: constraints.map(|x| x.into()),
        }
    }
}

impl SettingDefinition {
    pub fn is_valid_value(&self, value: &Value) -> Result<()> {
        match (&self.constraint, value) {
            (Some(Constraint::Int32(constraint)), Value::Int32(value)) => {
                self.validate_int32(constraint, *value)
            }
            (Some(Constraint::Int64(constraint)), Value::Int64(value)) => {
                self.validate_int64(constraint, *value)
            }
            (Some(Constraint::Float(constraint)), Value::Float(value)) => {
                self.validate_float(constraint, *value)
            }
            (Some(Constraint::Enum(constraint)), Value::Enum(value)) => {
                self.validate_enum(constraint, *value)
            }
            (None, _) => Ok(()),
            _ => Err(anyhow!(
                "Invalid value type for setting {}. Expected value similar to {:?}, got {:?}",
                self.name,
                self.default_value,
                value
            )),
        }
    }

    fn validate_int32(&self, constraint: &Int32Constraint, value: i32) -> Result<()> {
        if let Some(min_value) = constraint.min_value {
            if value < min_value {
                bail!(
                    "Value {} for setting {} is less than the minimum allowed value {}",
                    value,
                    self.name,
                    min_value
                );
            }
        }
        if let Some(max_value) = constraint.max_value {
            if value > max_value {
                bail!(
                    "Value {} for setting {} is greater than the maximum allowed value {}",
                    value,
                    self.name,
                    max_value
                );
            }
        }
        if let Some(step) = constraint.step {
            if (value - constraint.min_value.unwrap_or(0)) % step != 0 {
                bail!(
                    "Value {} for setting {} is not a valid step from the minimum value {}",
                    value,
                    self.name,
                    constraint.min_value.unwrap_or(0)
                );
            }
        }
        Ok(())
    }

    fn validate_int64(&self, constraints: &Int64Constraint, value: i64) -> Result<()> {
        if let Some(min_value) = constraints.min_value {
            if value < min_value {
                bail!(
                    "Value {} for setting {} is less than the minimum allowed value {}",
                    value,
                    self.name,
                    min_value
                );
            }
        }
        if let Some(max_value) = constraints.max_value {
            if value > max_value {
                bail!(
                    "Value {} for setting {} is greater than the maximum allowed value {}",
                    value,
                    self.name,
                    max_value
                );
            }
        }
        if let Some(step) = constraints.step {
            if (value - constraints.min_value.unwrap_or(0)) % step != 0 {
                bail!(
                    "Value {} for setting {} is not a valid step from the minimum value {}",
                    value,
                    self.name,
                    constraints.min_value.unwrap_or(0)
                );
            }
        }
        Ok(())
    }

    fn validate_float(&self, constraints: &FloatConstraint, value: f32) -> Result<()> {
        if let Some(min_value) = constraints.min_value {
            if value < min_value {
                bail!(
                    "Value {} for setting {} is less than the minimum allowed value {}",
                    value,
                    self.name,
                    min_value
                );
            }
        }
        if let Some(max_value) = constraints.max_value {
            if value > max_value {
                bail!(
                    "Value {} for setting {} is greater than the maximum allowed value {}",
                    value,
                    self.name,
                    max_value
                );
            }
        }
        if let Some(step) = constraints.step {
            if (value - constraints.min_value.unwrap_or(0.0)) % step != 0.0 {
                bail!(
                    "Value {} for setting {} is not a valid step from the minimum value {}",
                    value,
                    self.name,
                    constraints.min_value.unwrap_or(0.0)
                );
            }
        }
        Ok(())
    }

    fn validate_enum(&self, constraints: &EnumConstraint, value: i32) -> Result<()> {
        if !constraints.possible_values.contains(&value) {
            bail!("Value {} for setting {} is not a valid enum value", value, self.name);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::constraint::{Constraint, Int32Constraint};
    use user_preferences_api::setting::{
        setting::Value as ValueProto, setting_and_constraints::Constraints as ConstraintsProto,
        Int32Constraints as Int32ConstraintsProto, Setting as SettingProto,
        SettingAndConstraints as SettingAndConstraintsProto, SettingKind as SettingKindProto,
    };

    #[test]
    fn is_valid_value_int32_valid() {
        let setting_definition = SettingDefinition {
            kind: SettingKindProto::PER_USER as i32,
            name: "int32_setting".to_string(),
            default_value: Value::Int32(10),
            constraint: Some(Constraint::Int32(Int32Constraint {
                min_value: Some(0),
                max_value: Some(20),
                step: Some(1),
            })),
        };

        assert!(setting_definition.is_valid_value(&Value::Int32(10)).is_ok());
    }

    #[test]
    fn is_valid_value_int32_below_min() {
        let setting_definition = SettingDefinition {
            kind: SettingKindProto::PER_USER as i32,
            name: "int32_setting".to_string(),
            default_value: Value::Int32(10),
            constraint: Some(Constraint::Int32(Int32Constraint {
                min_value: Some(10),
                max_value: Some(20),
                step: Some(1),
            })),
        };

        assert!(setting_definition.is_valid_value(&Value::Int32(9)).is_err());
    }

    #[test]
    fn is_valid_value_int32_above_max() {
        let setting_definition = SettingDefinition {
            kind: SettingKindProto::PER_USER as i32,
            name: "int32_setting".to_string(),
            default_value: Value::Int32(10),
            constraint: Some(Constraint::Int32(Int32Constraint {
                min_value: Some(0),
                max_value: Some(10),
                step: Some(1),
            })),
        };

        assert!(setting_definition.is_valid_value(&Value::Int32(11)).is_err());
    }

    #[test]
    fn is_valid_value_int32_invalid_step() {
        let setting_definition = SettingDefinition {
            kind: SettingKindProto::PER_USER as i32,
            name: "int32_setting".to_string(),
            default_value: Value::Int32(10),
            constraint: Some(Constraint::Int32(Int32Constraint {
                min_value: Some(0),
                max_value: Some(20),
                step: Some(2),
            })),
        };

        assert!(setting_definition.is_valid_value(&Value::Int32(11)).is_err());
    }

    #[test]
    fn is_valid_value_int64_valid() {
        let setting_definition = SettingDefinition {
            kind: SettingKindProto::PER_USER as i32,
            name: "int64_setting".to_string(),
            default_value: Value::Int64(10),
            constraint: Some(Constraint::Int64(Int64Constraint {
                min_value: Some(0),
                max_value: Some(20),
                step: Some(1),
            })),
        };

        assert!(setting_definition.is_valid_value(&Value::Int64(10)).is_ok());
    }

    #[test]
    fn is_valid_value_int64_below_min() {
        let setting_definition = SettingDefinition {
            kind: SettingKindProto::PER_USER as i32,
            name: "int64_setting".to_string(),
            default_value: Value::Int64(10),
            constraint: Some(Constraint::Int64(Int64Constraint {
                min_value: Some(10),
                max_value: Some(20),
                step: Some(1),
            })),
        };

        assert!(setting_definition.is_valid_value(&Value::Int64(9)).is_err());
    }

    #[test]
    fn is_valid_value_int64_above_max() {
        let setting_definition = SettingDefinition {
            kind: SettingKindProto::PER_USER as i32,
            name: "int64_setting".to_string(),
            default_value: Value::Int64(10),
            constraint: Some(Constraint::Int64(Int64Constraint {
                min_value: Some(0),
                max_value: Some(10),
                step: Some(1),
            })),
        };

        assert!(setting_definition.is_valid_value(&Value::Int64(11)).is_err());
    }

    #[test]
    fn is_valid_value_int64_invalid_step() {
        let setting_definition = SettingDefinition {
            kind: SettingKindProto::PER_USER as i32,
            name: "int64_setting".to_string(),
            default_value: Value::Int64(10),
            constraint: Some(Constraint::Int64(Int64Constraint {
                min_value: Some(0),
                max_value: Some(20),
                step: Some(2),
            })),
        };

        assert!(setting_definition.is_valid_value(&Value::Int64(11)).is_err());
    }

    #[test]
    fn is_valid_value_float_valid() {
        let setting_definition = SettingDefinition {
            kind: SettingKindProto::PER_USER as i32,
            name: "float_setting".to_string(),
            default_value: Value::Float(10.0),
            constraint: Some(Constraint::Float(FloatConstraint {
                min_value: Some(0.0),
                max_value: Some(20.0),
                step: Some(1.0),
            })),
        };

        assert!(setting_definition.is_valid_value(&Value::Float(10.0)).is_ok());
    }

    #[test]
    fn is_valid_value_float_below_min() {
        let setting_definition = SettingDefinition {
            kind: SettingKindProto::PER_USER as i32,
            name: "float_setting".to_string(),
            default_value: Value::Float(10.0),
            constraint: Some(Constraint::Float(FloatConstraint {
                min_value: Some(10.0),
                max_value: Some(20.0),
                step: Some(1.0),
            })),
        };

        assert!(setting_definition.is_valid_value(&Value::Float(9.0)).is_err());
    }

    #[test]
    fn is_valid_value_float_above_max() {
        let setting_definition = SettingDefinition {
            kind: SettingKindProto::PER_USER as i32,
            name: "float_setting".to_string(),
            default_value: Value::Float(10.0),
            constraint: Some(Constraint::Float(FloatConstraint {
                min_value: Some(0.0),
                max_value: Some(10.0),
                step: Some(1.0),
            })),
        };

        assert!(setting_definition.is_valid_value(&Value::Float(11.0)).is_err());
    }

    #[test]
    fn is_valid_value_float_invalid_step() {
        let setting_definition = SettingDefinition {
            kind: SettingKindProto::PER_USER as i32,
            name: "float_setting".to_string(),
            default_value: Value::Float(10.0),
            constraint: Some(Constraint::Float(FloatConstraint {
                min_value: Some(0.0),
                max_value: Some(20.0),
                step: Some(2.0),
            })),
        };

        assert!(setting_definition.is_valid_value(&Value::Float(11.0)).is_err());
    }

    #[test]
    fn is_valid_value_enum_valid() {
        let setting_definition = SettingDefinition {
            kind: SettingKindProto::PER_USER as i32,
            name: "enum_setting".to_string(),
            default_value: Value::Enum(1),
            constraint: Some(Constraint::Enum(EnumConstraint { possible_values: vec![1, 2, 3] })),
        };

        assert!(setting_definition.is_valid_value(&Value::Enum(1)).is_ok());
    }

    #[test]
    fn is_valid_value_enum_invalid() {
        let setting_definition = SettingDefinition {
            kind: SettingKindProto::PER_USER as i32,
            name: "enum_setting".to_string(),
            default_value: Value::Enum(1),
            constraint: Some(Constraint::Enum(EnumConstraint { possible_values: vec![1, 2, 3] })),
        };

        assert!(setting_definition.is_valid_value(&Value::Enum(4)).is_err());
    }

    #[test]
    fn is_valid_value_invalid_value_type() {
        let setting_definition = SettingDefinition {
            kind: SettingKindProto::PER_USER as i32,
            name: "int32_setting".to_string(),
            default_value: Value::Int32(10),
            constraint: Some(Constraint::Int32(Int32Constraint {
                min_value: Some(0),
                max_value: Some(20),
                step: Some(1),
            })),
        };

        assert!(setting_definition.is_valid_value(&Value::Bool(true)).is_err());
    }

    #[test]
    fn is_valid_value_no_constraint() {
        let setting_definition = SettingDefinition {
            kind: SettingKindProto::PER_USER as i32,
            name: "no_constraint_setting".to_string(),
            default_value: Value::Int32(10),
            constraint: None,
        };

        assert!(setting_definition.is_valid_value(&Value::Int32(10)).is_ok());
        assert!(setting_definition.is_valid_value(&Value::Int32(5)).is_ok());
        assert!(setting_definition.is_valid_value(&Value::Bool(true)).is_ok());
    }

    #[test]
    fn setting_definition_from_setting_definition_proto() {
        let proto = SettingDefinitionProto {
            kind: SettingKindProto::PER_USER.into(),
            setting_and_constraints: Some(SettingAndConstraintsProto {
                setting: Some(SettingProto {
                    key: "int32_setting".to_string(),
                    value: Some(ValueProto::Int32(10)),
                    ..Default::default()
                })
                .into(),
                constraints: Some(ConstraintsProto::Int32Constraints(Int32ConstraintsProto {
                    min_value: Some(0),
                    max_value: Some(20),
                    step: Some(1),
                    ..Default::default()
                })),
                ..Default::default()
            })
            .into(),
            ..Default::default()
        };

        let setting_definition: SettingDefinition = proto.into();

        assert_eq!(setting_definition.kind, SettingKindProto::PER_USER as i32);
        assert_eq!(setting_definition.name, "int32_setting");
        assert_eq!(setting_definition.default_value, Value::Int32(10));
        assert_eq!(
            setting_definition.constraint,
            Some(Constraint::Int32(Int32Constraint {
                min_value: Some(0),
                max_value: Some(20),
                step: Some(1),
            }))
        );
    }
}
