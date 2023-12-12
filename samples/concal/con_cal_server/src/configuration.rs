use concal_api::configuration::{config_override_key_value::Value, ConfigOverride};
use core::clone::Clone;
use protobuf::descriptor::FileDescriptorSet;
use protobuf::reflect::ReflectValueBox;
use protobuf::well_known_types::any::Any;
use std::string::String;
use std::vec::Vec;

use concal_api::configuration::config_override_key_value::Value as ConCalValue;

use crate::message_editor::MessageEditor;

#[derive(Default, Debug, Clone)]
pub struct Configuration {
    data: Any,
    descriptor: FileDescriptorSet,
    version: String,
}

impl Configuration {
    pub fn new(data: Any, descriptor: FileDescriptorSet, version: String) -> Self {
        Configuration { data, descriptor, version }
    }

    pub fn with_overrides(&self, overrides: &Vec<ConfigOverride>) -> Result<Self, String> {
        let mut message_editor = MessageEditor::new(self.data.clone(), self.descriptor.clone())?;

        for r#override in overrides {
            for kv in r#override.pairs.iter() {
                if kv.value.is_none() {
                    return Err(format!("Override contains nil value {:?}", kv));
                }
                let reflect_box_value =
                    Configuration::convert_to_reflect_box(kv.value.clone().unwrap())?;
                message_editor.update_value(&kv.key, reflect_box_value)?
            }
        }

        let derived_config = message_editor.to_any()?;

        Ok(Configuration::new(derived_config, self.descriptor.clone(), self.version.clone()))
    }

    pub fn convert_to_reflect_box(value: ConCalValue) -> Result<ReflectValueBox, String> {
        match value {
            Value::ValueBool(x) => Ok(ReflectValueBox::from(x)),
            Value::ValueInt(x) => Ok(ReflectValueBox::from(x)),
            Value::ValueStr(x) => Ok(ReflectValueBox::from(x)),
            Value::ValueFloat(x) => Ok(ReflectValueBox::from(x)),
            _ => Err(format!("Cannot convert unsupported data type {:?} to reflect box", value)),
        }
    }

    pub fn version(&self) -> &String {
        &self.version
    }

    pub fn data(&self) -> &Any {
        &self.data
    }

    pub fn descriptor(&self) -> &FileDescriptorSet {
        &self.descriptor
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports, dead_code)]
    use super::*;
    use crate::tests::utilities::{load_file_descriptors, test_config};
    use concal_api::configuration::ConfigOverrideKeyValue;
    use protobuf::Message;
    use sdvtestconcal_proto_rust::inner_test_configuration::InnerTestConfigurationDifferentFile as InnerTestConfigurationProto;
    use sdvtestconcal_proto_rust::inner_test_configuration_different_package::InnerTestConfigurationDifferentPackage as DifferentPackageProto;
    use sdvtestconcal_proto_rust::test_configuration::{
        ComplexParameterInFile as ComplexProto, TestConfiguration as TestConfigurationProto,
    };
    use std::path::PathBuf;

    #[test]
    fn with_overrides_returns_updated_config_when_it_succeeds() {
        let config = test_config();

        let updated_config = config
            .with_overrides(&vec![ConfigOverride {
                pairs: vec![
                    ConfigOverrideKeyValue {
                        key: String::from("bool_parameter"),
                        value: Some(ConCalValue::ValueBool(true)),
                        ..Default::default()
                    },
                    ConfigOverrideKeyValue {
                        key: String::from("str_parameter"),
                        value: Some(ConCalValue::ValueStr("new_value".to_string())),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }])
            .expect("Expected to get configuration with overrides");

        let updated_test_config =
            TestConfigurationProto::parse_from_bytes(&updated_config.data().value)
                .expect("Expected to unmarshal config to TestConfiguration successfully");

        assert!(updated_test_config.bool_parameter);
        assert_eq!(updated_test_config.str_parameter, String::from("new_value"));
    }

    #[test]
    fn with_overrides_returns_an_error_when_it_encounters_none_values() {
        let config = test_config();

        let updated_config = config.with_overrides(&vec![ConfigOverride {
            pairs: vec![ConfigOverrideKeyValue {
                key: String::from("bool_parameter"),
                value: None,
                ..Default::default()
            }],
            ..Default::default()
        }]);

        assert!(updated_config.is_err());
        assert!(updated_config.unwrap_err().contains("contains nil value"));
    }

    #[test]
    fn with_overrides_returns_an_error_when_override_key_not_found() {
        let config = test_config();

        let updated_config = config.with_overrides(&vec![ConfigOverride {
            pairs: vec![ConfigOverrideKeyValue {
                key: String::from("none_existent_field"),
                value: Some(ConCalValue::ValueBool(true)),
                ..Default::default()
            }],
            ..Default::default()
        }]);

        assert!(updated_config.is_err());
        println!("err: {:?}", updated_config.clone().unwrap_err());
        assert!(updated_config.unwrap_err().contains("Could not find field"));
    }

    #[test]
    fn can_convert_bool_to_reflect_value_box() {
        let bool = ConCalValue::ValueBool(true);

        let result = Configuration::convert_to_reflect_box(bool);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ReflectValueBox::Bool(true));
    }

    #[test]
    fn can_convert_int_to_reflect_value_box() {
        let integer = ConCalValue::ValueInt(1000);

        let result = Configuration::convert_to_reflect_box(integer);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ReflectValueBox::U64(1000));
    }

    #[test]
    fn can_convert_str_to_reflect_value_box() {
        let string = ConCalValue::ValueStr(String::from("String"));

        let result = Configuration::convert_to_reflect_box(string);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ReflectValueBox::String(String::from("String")));
    }

    #[test]
    fn can_convert_float_to_reflect_value_box() {
        let float = ConCalValue::ValueFloat(32.0);

        let result = Configuration::convert_to_reflect_box(float);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ReflectValueBox::F32(32.0));
    }
}
