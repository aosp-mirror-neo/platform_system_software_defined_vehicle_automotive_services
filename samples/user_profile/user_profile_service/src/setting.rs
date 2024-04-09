use std::sync::Arc;
use user_profile_api::setting::Setting as SettingProto;
use user_profile_api::setting::SettingDefinition;
use user_profile_api::setting::{
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
        if self
            .constraints
            .as_ref()
            .filter(|constraints| Self::validate_value(constraints.as_ref(), &value))
            .is_some()
        {
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

    fn validate_value(constraint: &Constraints, val: &Value) -> bool {
        match (constraint, val) {
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
            (Constraints::EnumConstraints(x), Value::Enum(val)) => x.possible_values.contains(val),
            _ => true,
        }
    }
}

impl From<Setting> for SettingProto {
    fn from(val: Setting) -> Self {
        SettingProto { key: val.name, value: Some(val.value), ..Default::default() }
    }
}

impl From<SettingDefinition> for Setting {
    fn from(def: SettingDefinition) -> Self {
        let setting_and_constrants = def.setting_and_constaints.unwrap();
        let constraints = setting_and_constrants.constraints.map(Arc::new);
        let setting = setting_and_constrants.setting.unwrap();
        let value = setting.value.unwrap();

        Self {
            name: setting.key,
            value: value.clone(),
            kind: Arc::new(def.type_.unwrap()),
            default_value: Arc::new(value),
            constraints,
        }
    }
}
