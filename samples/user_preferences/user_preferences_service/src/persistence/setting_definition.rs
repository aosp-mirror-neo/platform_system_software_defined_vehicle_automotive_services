use super::constraint::Constraint;
use super::setting::Value;
use serde::{Deserialize, Serialize};
use user_preferences_api::setting::SettingDefinition as SettingDefinitionProto;

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
