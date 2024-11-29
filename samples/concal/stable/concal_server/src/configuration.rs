// Copyright 2024 Google LLC

use concal_api_stable::config_override::ConfigOverride;
use core::clone::Clone;
use protobuf::descriptor::FileDescriptorSet;
use protobuf::well_known_types::any::Any;
use std::string::String;
use std::vec::Vec;

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
                if kv.has_value_txtproto() {
                    message_editor.update_value(&kv.key, kv.value_txtproto())?
                } else {
                    panic!("Support for Any as an override value is not implemented");
                }
            }
        }

        let derived_config = message_editor.to_any()?;

        Ok(Configuration::new(derived_config, self.descriptor.clone(), self.version.clone()))
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
    use concal_api_stable::config_override::config_override_key_value::Value;
    use concal_api_stable::config_override::ConfigOverrideKeyValue;
    use protobuf::Message;
    use sdvtestconcal_proto_rust::inner_test_configuration::InnerTestConfigurationDifferentFile as InnerTestConfigurationProto;
    use sdvtestconcal_proto_rust::inner_test_configuration_different_package::InnerTestConfigurationDifferentPackage as DifferentPackageProto;
    use sdvtestconcal_proto_rust::test_configuration::{
        ComplexParameterInFile as ComplexProto, TestConfiguration as TestConfigurationProto,
    };
    use std::option::Option;
    use std::path::PathBuf;

    #[test]
    fn with_overrides_returns_updated_config_when_it_succeeds() {
        let config = test_config();

        let updated_config = config
            .with_overrides(&vec![ConfigOverride {
                pairs: vec![
                    ConfigOverrideKeyValue {
                        key: String::from("bool_parameter"),
                        value: Option::Some(Value::ValueTxtproto("true".into())),
                        ..Default::default()
                    },
                    ConfigOverrideKeyValue {
                        key: String::from("str_parameter"),
                        value: Option::Some(Value::ValueTxtproto("\"new_value\"".into())),
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
    fn with_overrides_returns_an_error_when_override_key_not_found() {
        let config = test_config();

        let updated_config = config.with_overrides(&vec![ConfigOverride {
            pairs: vec![ConfigOverrideKeyValue {
                key: String::from("none_existent_field"),
                value: Option::Some(Value::ValueTxtproto("true".into())),
                ..Default::default()
            }],
            ..Default::default()
        }]);

        assert!(updated_config.is_err());
        println!("err: {:?}", updated_config.clone().unwrap_err());
        assert!(updated_config.unwrap_err().contains("Could not find field"));
    }
}
