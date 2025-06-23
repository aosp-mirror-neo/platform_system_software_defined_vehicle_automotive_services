// Copyright 2024 Google LLC

use super::setting_definition::SettingDefinition;

use sdv_status_rs::{SdvResult, SdvStatus, SdvStatusCode};
use serde::{Deserialize, Serialize};
use user_preferences_api::setting::{setting::Value as ValueProto, Setting as SettingProto};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Setting {
    pub name: String,
    pub value: Value,
}

impl From<SettingDefinition> for Setting {
    fn from(setting: SettingDefinition) -> Self {
        Self { name: setting.name, value: setting.default_value }
    }
}

impl From<Setting> for SettingProto {
    fn from(setting: Setting) -> Self {
        Self { key: setting.name, value: Some(setting.value.into()), ..Default::default() }
    }
}

impl TryFrom<SettingProto> for Setting {
    type Error = SdvStatus;

    fn try_from(setting: SettingProto) -> SdvResult<Self> {
        Ok(Self {
            name: setting.key,
            value: setting.value.ok_or(SdvStatus::new(SdvStatusCode::NotFound))?.into(),
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Float(f32),
    Int32(i32),
    Int64(i64),
    Blob(::std::vec::Vec<u8>),
    Enum(i32),
}

impl From<ValueProto> for Value {
    fn from(value: ValueProto) -> Self {
        match value {
            ValueProto::Bool(v) => Value::Bool(v),
            ValueProto::Float(v) => Value::Float(v),
            ValueProto::Int32(v) => Value::Int32(v),
            ValueProto::Int64(v) => Value::Int64(v),
            ValueProto::Blob(v) => Value::Blob(v),
            ValueProto::Enum(v) => Value::Enum(v),
            _ => todo!(),
        }
    }
}

impl From<Value> for ValueProto {
    fn from(val: Value) -> Self {
        match val {
            Value::Bool(v) => ValueProto::Bool(v),
            Value::Float(v) => ValueProto::Float(v),
            Value::Int32(v) => ValueProto::Int32(v),
            Value::Int64(v) => ValueProto::Int64(v),
            Value::Blob(v) => ValueProto::Blob(v),
            Value::Enum(v) => ValueProto::Enum(v),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use user_preferences_api::setting::{setting::Value as ValueProto, Setting as SettingProto};

    #[test]
    fn setting_from_setting_proto() {
        let setting_proto = SettingProto {
            key: "float_setting".to_string(),
            value: Some(ValueProto::Float(10.5)),
            ..Default::default()
        };

        let setting: Setting = setting_proto.try_into().unwrap();

        assert_eq!(setting.name, "float_setting");
        assert_eq!(setting.value, Value::Float(10.5));
    }

    #[test]
    fn setting_from_setting_definition() {
        let setting_definition = SettingDefinition {
            kind: 1,
            name: "bool_setting".to_string(),
            default_value: Value::Bool(true),
            constraint: None,
        };

        let setting: Setting = setting_definition.into();

        assert_eq!(setting.name, "bool_setting");
        assert_eq!(setting.value, Value::Bool(true));
    }

    #[test]
    fn setting_to_setting_proto() {
        let setting = Setting { name: "int32_setting".to_string(), value: Value::Int32(10) };

        let setting_proto: SettingProto = setting.into();

        assert_eq!(setting_proto.key, "int32_setting");
        assert_eq!(setting_proto.value.unwrap(), ValueProto::Int32(10));
    }

    #[test]
    fn value_from_value_proto_bool() {
        let value_proto = ValueProto::Bool(true);

        let value: Value = value_proto.into();

        assert_eq!(value, Value::Bool(true));
    }

    #[test]
    fn value_to_value_proto_bool() {
        let value = Value::Bool(false);

        let value_proto: ValueProto = value.into();

        assert_eq!(value_proto, ValueProto::Bool(false));
    }

    #[test]
    fn value_from_value_proto_float() {
        let value_proto = ValueProto::Float(123.456);

        let value: Value = value_proto.into();

        assert_eq!(value, Value::Float(123.456));
    }

    #[test]
    fn value_to_value_proto_float() {
        let value = Value::Float(789.012);

        let value_proto: ValueProto = value.into();

        assert_eq!(value_proto, ValueProto::Float(789.012));
    }

    #[test]
    fn value_from_value_proto_int32() {
        let value_proto = ValueProto::Int32(12345);

        let value: Value = value_proto.into();

        assert_eq!(value, Value::Int32(12345));
    }

    #[test]
    fn value_to_value_proto_int32() {
        let value = Value::Int32(67890);

        let value_proto: ValueProto = value.into();

        assert_eq!(value_proto, ValueProto::Int32(67890));
    }

    #[test]
    fn value_from_value_proto_int64() {
        let value_proto = ValueProto::Int64(1234567890);

        let value: Value = value_proto.into();

        assert_eq!(value, Value::Int64(1234567890));
    }

    #[test]
    fn value_to_value_proto_int64() {
        let value = Value::Int64(9876543210);

        let value_proto: ValueProto = value.into();

        assert_eq!(value_proto, ValueProto::Int64(9876543210));
    }

    #[test]
    fn value_from_value_proto_blob() {
        let value_proto = ValueProto::Blob(vec![1, 2, 3]);

        let value: Value = value_proto.into();

        assert_eq!(value, Value::Blob(vec![1, 2, 3]));
    }

    #[test]
    fn value_to_value_proto_blob() {
        let value = Value::Blob(vec![4, 5, 6]);

        let value_proto: ValueProto = value.into();

        assert_eq!(value_proto, ValueProto::Blob(vec![4, 5, 6]));
    }

    #[test]
    fn value_from_value_proto_enum() {
        let value_proto = ValueProto::Enum(10);

        let value: Value = value_proto.into();

        assert_eq!(value, Value::Enum(10));
    }

    #[test]
    fn value_to_value_proto_enum() {
        let value = Value::Enum(20);

        let value_proto: ValueProto = value.into();

        assert_eq!(value_proto, ValueProto::Enum(20));
    }
}
