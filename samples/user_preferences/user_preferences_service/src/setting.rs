use protobuf::MessageField;
use std::sync::Arc;
use user_preferences_api::setting::Setting as SettingProto;
use user_preferences_api::setting::SettingAndConstraints;
use user_preferences_api::setting::SettingDefinition;
use user_preferences_api::setting::{
    setting::Value, setting_and_constraints::Constraints, SettingKind,
};

#[derive(Clone)]
pub(crate) struct Setting {
    pub name: String,
    value: Value,
    pub kind: Arc<SettingKind>,
    default_value: Arc<Value>,
    constraints: Option<Arc<Constraints>>,
}

impl Setting {
    pub fn set_value(&mut self, value: Value) -> Result<(), String> {
        if !self.is_valid_value(&value) {
            return Err(format!(
                "Value {:?} violated constraints {:?}",
                self.value, self.constraints
            ));
        }

        self.value = value;
        Ok(())
    }

    pub fn reset(&mut self) {
        self.value = self.default_value.as_ref().clone();
    }

    pub fn is_valid_value(&self, value: &Value) -> bool {
        if std::mem::discriminant(value) != std::mem::discriminant(&self.value) {
            return false;
        }
        if let Some(constraints) = &self.constraints {
            return match (constraints.as_ref(), value) {
                (Constraints::FloatConstraints(x), Value::Float(val)) => {
                    x.min_value.map_or(true, |min| val >= &min)
                        && x.max_value.map_or(true, |max| val <= &max)
                }
                (Constraints::Int32Constraints(x), Value::Int32(val)) => {
                    x.min_value.map_or(true, |min| val >= &min)
                        && x.max_value.map_or(true, |max| val <= &max)
                }
                (Constraints::Int64Constraints(x), Value::Int64(val)) => {
                    x.min_value.map_or(true, |min| val >= &min)
                        && x.max_value.map_or(true, |max| val <= &max)
                }
                (Constraints::EnumConstraints(x), Value::Enum(val)) => {
                    x.possible_values.contains(val)
                }
                _ => false,
            };
        }
        true
    }
}

impl From<Setting> for SettingProto {
    fn from(val: Setting) -> Self {
        SettingProto { key: val.name, value: Some(val.value), ..Default::default() }
    }
}

impl From<SettingDefinition> for Setting {
    fn from(def: SettingDefinition) -> Self {
        let setting_and_constrants = def.setting_and_constraints.unwrap();
        let constraints = setting_and_constrants.constraints.map(Arc::new);
        let setting = setting_and_constrants.setting.unwrap();
        let value = setting.value.unwrap();

        Self {
            name: setting.key,
            value: value.clone(),
            kind: Arc::new(def.kind.unwrap()),
            default_value: Arc::new(value),
            constraints,
        }
    }
}

impl From<Setting> for SettingAndConstraints {
    fn from(val: Setting) -> Self {
        SettingAndConstraints {
            setting: MessageField::some(SettingProto {
                key: val.name,
                value: Some(val.value),
                ..Default::default()
            }),
            constraints: val.constraints.map(|x| x.as_ref().clone()),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]
    use super::*;
    use user_preferences_api::setting::{
        EnumConstraints, FloatConstraints, Int32Constraints, Int64Constraints,
    };

    macro_rules! numeric_value_validation_test_logic {
        ($constraint_type:ident, $min_val:expr, $max_val:expr, $val_type:ident, $val:expr, $validation_result:expr) => {
            let setting = Setting {
                name: String::from("test"),
                value: Value::$val_type($min_val),
                kind: Arc::new(SettingKind::PER_USER),
                default_value: Arc::new(Value::$val_type($min_val)),
                constraints: Some(Arc::new(Constraints::$constraint_type($constraint_type {
                    min_value: Some($min_val),
                    max_value: Some($max_val),
                    ..Default::default()
                }))),
            };

            assert_eq!(setting.is_valid_value(&Value::$val_type($val)), $validation_result);
        };
    }

    #[test]
    fn is_valid_value_returns_true_for_valid_int_32() {
        numeric_value_validation_test_logic!(Int32Constraints, 1, 10, Int32, 2, true);
    }

    #[test]
    fn is_valid_value_returns_false_for_too_low_int_32() {
        numeric_value_validation_test_logic!(Int32Constraints, 1, 10, Int32, 0, false);
    }

    #[test]
    fn is_valid_value_returns_false_for_too_high_int_32() {
        numeric_value_validation_test_logic!(Int32Constraints, 1, 10, Int32, 100, false);
    }

    #[test]
    fn is_valid_value_returns_true_for_valid_int_64() {
        numeric_value_validation_test_logic!(Int64Constraints, 1, 10, Int64, 2, true);
    }

    #[test]
    fn is_valid_value_returns_false_for_too_low_int_64() {
        numeric_value_validation_test_logic!(Int64Constraints, 1, 10, Int64, 0, false);
    }

    #[test]
    fn is_valid_value_returns_false_for_too_high_int_64() {
        numeric_value_validation_test_logic!(Int64Constraints, 1, 10, Int64, 100, false);
    }

    #[test]
    fn is_valid_value_returns_true_for_valid_float() {
        numeric_value_validation_test_logic!(FloatConstraints, 1.0, 10.0, Float, 2.0, true);
    }

    #[test]
    fn is_valid_value_returns_false_for_too_low_float() {
        numeric_value_validation_test_logic!(FloatConstraints, 1.0, 10.0, Float, 0.0, false);
    }

    #[test]
    fn is_valid_value_returns_false_for_too_high_float() {
        numeric_value_validation_test_logic!(FloatConstraints, 1.0, 10.0, Float, 100.0, false);
    }

    #[test]
    fn is_valid_value_returns_false_when_current_and_new_value_have_different_types() {
        let setting = Setting {
            name: String::from("test"),
            value: Value::Int32(1),
            kind: Arc::new(SettingKind::PER_USER),
            default_value: Arc::new(Value::Int32(2)),
            constraints: None,
        };
        assert!(!setting.is_valid_value(&Value::Float(1.0)));
    }
}
