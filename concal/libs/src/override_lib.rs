// Copyright 2023 Google LLC

//! ProtoManager maintains schema of protobuf messages with their dependencies.
#![allow(missing_docs)]

use protobuf::descriptor::{FileDescriptorProto as FdProto, FileDescriptorSet as FdSet};
use protobuf::reflect::FileDescriptor as Fd;

pub struct ProtoManager {
    fd_set: FdSet,
}

impl ProtoManager {
    /// Loads the database.
    pub fn new(fds: FdSet) -> Self {
        Self { fd_set: fds }
    }

    /// Finds a schema for a specific message (full name starting with a .<full package name>.<message name>).
    pub fn find_schema(&self, full_mes_name: &str) -> Option<Fd> {
        for fd_proto in self.fd_set.file.iter() {
            let deps = fd_proto
                .dependency
                .iter()
                .filter_map(|dep| self.get_file_descriptor_with_deps(dep))
                .collect::<Vec<_>>();
            let fd_res = Fd::new_dynamic(fd_proto.clone(), &deps);
            if let Ok(fd) = fd_res {
                if fd.message_by_full_name(full_mes_name).is_some() {
                    let deps = fd_proto
                        .dependency
                        .iter()
                        .filter_map(|dep| self.get_file_descriptor_with_deps(dep))
                        .collect::<Vec<_>>();
                    return Fd::new_dynamic(fd_proto.clone(), &deps.clone()).ok();
                }
            }
        }
        None
    }

    /// Retrieves a file descriptor with dependencies for specified file name.
    fn get_file_descriptor_with_deps(&self, file_name: &str) -> Option<Fd> {
        let fd = self.find_file_descriptor(file_name)?;
        let deps = fd
            .dependency
            .iter()
            .filter_map(|dep| self.get_file_descriptor_with_deps(dep))
            .collect::<Vec<_>>();
        Fd::new_dynamic(fd, &deps).ok()
    }

    fn find_file_descriptor(&self, file_name: &str) -> Option<FdProto> {
        self.fd_set
            .file
            .iter()
            .find(|f| f.name.to_owned().is_some_and(|n| n == file_name))
            .to_owned()
            .map(|f| f.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use concal_api::config_override::config_override_key_value::Value;
    use concal_api::config_override::ConfigOverrideKeyValue as CoPair;
    use protobuf::reflect::ReflectMapRef;
    use protobuf::reflect::RuntimeFieldType;
    use protobuf::reflect::{FieldDescriptor, MessageDescriptor, RuntimeType};
    use protobuf::text_format::merge_from_str;
    use protobuf::text_format::parse_from_str;
    use protobuf::text_format::print_to_string;
    use protobuf::well_known_types::any::Any;
    use protobuf::well_known_types::struct_::value::Kind::StringValue as PStringValue;
    use protobuf::well_known_types::struct_::{ListValue, Value as PValue};
    use protobuf::well_known_types::wrappers::{
        BoolValue, BytesValue, DoubleValue, FloatValue, Int32Value, Int64Value, StringValue,
        UInt32Value, UInt64Value,
    };
    use protobuf::MessageDyn;
    use protobuf::SpecialFields;
    use regex::Regex;
    use sdvtestconcal_proto_rust::test_configuration::ComplexParameterInFile;
    use sdvtestconcal_proto_rust::test_configuration::TestConfiguration as TestConfigurationProto;
    use std::collections::VecDeque;
    use std::option::Option;
    use std::path::PathBuf;

    #[test]
    fn find_schema_full_name_succeeds() {
        let pm = get_test_proto_manager();
        assert!(pm.find_schema(".android.sdv.config.testconfig.TestConfiguration").is_some());
    }

    #[test]
    fn find_schema_unknown_name_returns_none() {
        let pm = get_test_proto_manager();
        assert!(pm.find_schema(".TestConfiguration").is_none());
    }

    #[test]
    #[should_panic]
    fn find_schema_short_name_fails() {
        let pm = get_test_proto_manager();
        assert!(pm.find_schema("TestConfiguration").is_none());
    }

    #[test]
    fn find_schema_imported_name_from_different_package_succeeds() {
        let pm = get_test_proto_manager();
        assert!(pm
            .find_schema(".android.sdv.config.diffpackage.InnerTestConfigurationDifferentPackage")
            .is_some());
    }

    #[test]
    fn find_schema_imported_name_from_different_file_succeeds() {
        let pm = get_test_proto_manager();
        assert!(pm
            .find_schema(".android.sdv.config.testconfig.InnerTestConfigurationDifferentFile")
            .is_some());
    }

    #[test]
    fn override_textproto_bool_succeeds() {
        let default = register_config("bool_parameter: true int_parameter: 5");
        let config_override = get_config_override_key_value("bool_parameter", "false");
        let new_config = override_config(default, config_override);
        assert_eq!(print_to_string(&*new_config), "bool_parameter: false int_parameter: 5");
    }

    #[test]
    fn override_any_bool_succeeds() {
        let default = register_config("bool_parameter: true int_parameter: 5");
        let mut copair = CoPair::new();
        copair.key = "bool_parameter".to_string();
        let mut bool_param = BoolValue::new();
        bool_param.value = false;
        copair.value = Option::Some(Value::ValueAny(Any::pack(&bool_param).unwrap()));
        let new_config = override_config(default, copair);
        assert_eq!(print_to_string(&*new_config), "bool_parameter: false int_parameter: 5");
    }

    #[test]
    fn override_textproto_int_succeeds() {
        let default = register_config("bool_parameter: true int_parameter: 5");
        let config_override = get_config_override_key_value("int_parameter", "10");
        let new_config = override_config(default, config_override);
        assert_eq!(print_to_string(&*new_config), "bool_parameter: true int_parameter: 10");
    }

    #[test]
    fn override_any_int_succeeds() {
        let default = register_config("bool_parameter: true int_parameter: 5");
        let mut copair = CoPair::new();
        copair.key = "int_parameter".to_string();
        let mut param = Int64Value::new();
        param.value = 10;
        copair.value = Option::Some(Value::ValueAny(Any::pack(&param).unwrap()));
        let new_config = override_config(default, copair);
        assert_eq!(print_to_string(&*new_config), "bool_parameter: true int_parameter: 10");
    }

    #[test]
    fn override_textproto_float_succeeds() {
        let default = register_config("int_parameter: 5 float_parameter: 5.5");
        let config_override = get_config_override_key_value("float_parameter", "10.7");
        let new_config = override_config(default, config_override);
        assert_eq!(print_to_string(&*new_config), "int_parameter: 5 float_parameter: 10.7");
    }

    #[test]
    fn override_any_float_succeeds() {
        let default = register_config("int_parameter: 5 float_parameter: 5.5");
        let mut copair = CoPair::new();
        copair.key = "float_parameter".to_string();
        let mut param = FloatValue::new();
        param.value = 10.7;
        copair.value = Option::Some(Value::ValueAny(Any::pack(&param).unwrap()));
        let new_config = override_config(default, copair);
        assert_eq!(print_to_string(&*new_config), "int_parameter: 5 float_parameter: 10.7");
    }

    #[test]
    fn override_textproto_clean_float_succeeds() {
        let default = register_config("int_parameter: 5 float_parameter: 5.5");
        let config_override = get_config_override_key_value("float_parameter", "");
        let new_config = override_config(default, config_override);
        assert_eq!(print_to_string(&*new_config), "int_parameter: 5");
    }

    #[test]
    fn override_textproto_string_succeeds() {
        let default = register_config(r#"float_parameter: 3.4 str_parameter: "string parameter""#);
        let config_override = get_config_override_key_value("str_parameter", r#""updated""#);
        let new_config = override_config(default, config_override);
        assert_eq!(
            print_to_string(&*new_config),
            r#"float_parameter: 3.4 str_parameter: "updated""#
        );
    }

    #[test]
    fn override_any_string_succeeds() {
        let default = register_config(r#"float_parameter: 3.4 str_parameter: "string parameter""#);
        let mut copair = CoPair::new();
        copair.key = "str_parameter".to_string();
        let mut param = StringValue::new();
        param.value = "updated".to_string();
        copair.value = Option::Some(Value::ValueAny(Any::pack(&param).unwrap()));
        let new_config = override_config(default, copair);
        assert_eq!(
            print_to_string(&*new_config),
            r#"float_parameter: 3.4 str_parameter: "updated""#
        );
    }

    #[test]
    fn override_textproto_clean_field_succeeds() {
        let default = register_config(
            r#"str_parameter: "string parameter" complex_parameter {first_parameter: "new_parameter"}"#,
        );
        // In Rust protobuf do not place `:` before `{` for messages, see example: `complex_parameter {`.
        let config_override = get_config_override_key_value("complex_parameter", "");
        let new_config = override_config(default, config_override);
        assert_eq!(print_to_string(&*new_config), r#"str_parameter: "string parameter""#);
    }

    #[test]
    fn override_textproto_add_new_field_succeeds() {
        let default = register_config(r#"str_parameter: "string parameter""#);
        // In Rust protobuf do not place `:` before `{` for messages, see example: `complex_parameter {`.
        let config_override =
            get_config_override_key_value("complex_parameter", r#"{first_parameter: "updated"}"#);
        let new_config = override_config(default, config_override);
        assert_eq!(
            print_to_string(&*new_config),
            r#"str_parameter: "string parameter" complex_parameter {first_parameter: "updated"}"#
        );
    }

    #[test]
    fn override_textproto_complex_full_rewrite_succeeds() {
        // See that the complex parameter is rewritten completely.
        let default = register_config(
            r#"str_parameter: "string parameter" complex_parameter {first_parameter: "initial"}"#,
        );
        let config_override =
            get_config_override_key_value("complex_parameter", r#"{second_parameter: "updated"}"#);
        let new_config = override_config(default, config_override);
        assert_eq!(
            print_to_string(&*new_config),
            r#"str_parameter: "string parameter" complex_parameter {second_parameter: "updated"}"#
        );
    }

    #[test]
    fn override_any_complex_full_rewrite_succeeds() {
        // See that the complex parameter is rewritten completely.
        let default = register_config(
            r#"str_parameter: "string parameter" complex_parameter {first_parameter: "initial"}"#,
        );
        let mut copair = CoPair::new();
        copair.key = "complex_parameter".to_string();
        let mut param = ComplexParameterInFile::new();
        param.second_parameter = "updated".to_string();
        copair.value = Option::Some(Value::ValueAny(Any::pack(&param).unwrap()));
        let new_config = override_config(default, copair);
        assert_eq!(
            print_to_string(&*new_config),
            r#"str_parameter: "string parameter" complex_parameter {second_parameter: "updated"}"#
        );
    }

    #[test]
    fn override_textproto_bytes_succeeds() {
        let mut value = format!(r#"bytes_parameter: "{:?}""#, "Initial text".as_bytes());
        let default = register_config(&value);
        value = format!(r#""{:?}""#, "Updated text".as_bytes());
        let config_override = get_config_override_key_value("bytes_parameter", &value);
        let new_config = override_config(default, config_override);
        let upd_config = format!(r#"bytes_parameter: "{:?}""#, "Updated text".as_bytes());
        assert_eq!(print_to_string(&*new_config), upd_config);
    }

    #[test]
    fn override_any_bytes_succeeds() {
        let value = format!(r#"bytes_parameter: "{:?}""#, "Initial text".as_bytes());
        let default = register_config(&value);
        let mut copair = CoPair::new();
        copair.key = "bytes_parameter".to_string();
        let mut param = BytesValue::new();
        param.value = vec![85, 112, 100, 97, 116, 101, 100, 32, 116, 101, 120, 116]; //Updated text
        copair.value = Option::Some(Value::ValueAny(Any::pack(&param).unwrap()));
        let new_config = override_config(default, copair);
        let upd_config = format!(r#"bytes_parameter: {:?}"#, "Updated text");
        assert_eq!(print_to_string(&*new_config), upd_config);
    }

    #[test]
    fn clear_textproto_config_succeeds() {
        let default = register_config(
            r#"bool_parameter: true int_parameter: 5 float_parameter: 3.4 str_parameter: "string parameter" nested_parameter {} complex_parameter {second_parameter: "initial"} nested_package {}"#,
        );
        // In Rust protobuf do not place `:` before `{` for messages, see example: `complex_parameter {`.
        let config_override = get_config_override_key_value("", "");
        let new_config = override_config(default, config_override);
        assert_eq!(print_to_string(&*new_config), "");
    }

    #[test]
    fn clear_no_value_config_succeeds() {
        let default = register_config(
            r#"bool_parameter: true int_parameter: 5 float_parameter: 3.4 str_parameter: "string parameter" nested_parameter {} complex_parameter {second_parameter: "initial"} nested_package {}"#,
        );
        let mut copair = CoPair::new();
        copair.key = "".to_string();
        let new_config = override_config(default, copair);
        assert_eq!(print_to_string(&*new_config), "");
    }

    #[test]
    fn override_textproto_config_succeeds() {
        let default = register_config(
            r#"bool_parameter: true int_parameter: 5 float_parameter: 3.4 str_parameter: "string parameter" nested_parameter {} complex_parameter {second_parameter: "initial"} nested_package {}"#,
        );
        // In Rust protobuf do not place `:` before `{` for messages, see example: `complex_parameter {`.
        let config_override = get_config_override_key_value(
            "",
            r#"int_parameter: 10 complex_parameter {first_parameter: "initial"} map_parameter {key: "first_element" value: 1}"#,
        );
        let new_config = override_config(default, config_override);
        assert_eq!(
            print_to_string(&*new_config),
            r#"int_parameter: 10 map_parameter {key: "first_element" value: 1} complex_parameter {first_parameter: "initial"}"#,
        );
    }

    #[test]
    fn override_any_config_succeeds() {
        let default = register_config(
            r#"bool_parameter: true int_parameter: 5 float_parameter: 3.4 str_parameter: "string parameter" nested_parameter {} complex_parameter {second_parameter: "initial"} nested_package {}"#,
        );
        let any_config = register_config(
            r#"int_parameter: 10 complex_parameter {first_parameter: "initial"} map_parameter {key: "first_element" value: 1}"#,
        );

        let mut copair = CoPair::new();
        copair.key = "".into();
        copair.value = Option::Some(Value::ValueAny(any_config));
        let new_config = override_config(default, copair);
        assert_eq!(
            print_to_string(&*new_config),
            r#"int_parameter: 10 map_parameter {key: "first_element" value: 1} complex_parameter {first_parameter: "initial"}"#,
        );
    }

    #[test]
    fn override_textproto_clean_map_succeeds() {
        let default = register_config(r#"map_parameter {key: "first_element" value: 1}"#);
        let config_override = get_config_override_key_value("map_parameter", "");
        let new_config = override_config(default, config_override);
        assert_eq!(print_to_string(&*new_config), "");
    }

    #[test]
    fn override_textproto_assign_new_value_map_succeeds() {
        let default = register_config(r#"map_parameter {key: "first_element" value: 1}"#);
        let config_override = get_config_override_key_value(
            "map_parameter",
            r#"[{key: "second_element" value: 2}, {key: "third_element" value: 3}]"#,
        );
        let new_config = override_config(default, config_override);

        // We cannot compare maps as strings with multiple keys, as the order of elements is not guaranteed.
        // Therefore, we need to sort the entries first.
        let map_desc = new_config.descriptor_dyn().field_by_name("map_parameter").unwrap();
        let map = map_desc.get_map(&*new_config);
        assert_eq!(
            print_pretty_map(map),
            r#"map_parameter {key: "second_element" value: 2} map_parameter {key: "third_element" value: 3}"#
        );
    }

    #[test]
    fn override_textproto_add_newkey_to_map_succeeds() {
        let default = register_config(r#"map_parameter {key: "initial key" value: 1}"#);
        // In Rust protobuf do not place `:` before `{` for messages, see example: `complex_parameter {`.
        let config_override = get_config_override_key_value("map_parameter.new key", "2");
        let new_config = override_config(default, config_override);

        // We cannot compare maps as strings with multiple keys, as the order of elements is not guaranteed.
        // Therefore, we need to sort the entries first.
        let map_desc = new_config.descriptor_dyn().field_by_name("map_parameter").unwrap();
        let map = map_desc.get_map(&*new_config);
        assert_eq!(
            print_pretty_map(map),
            r#"map_parameter {key: "initial key" value: 1} map_parameter {key: "new key" value: 2}"#
        );
    }

    #[test]
    fn override_textproto_assign_new_value_to_key_in_map_succeeds() {
        let default = register_config(r#"map_parameter {key: "first_element" value: 1}"#);
        // In Rust protobuf do not place `:` before `{` for messages, see example: `complex_parameter {`.
        let config_override = get_config_override_key_value("map_parameter.first_element", "2");
        let new_config = override_config(default, config_override);
        assert_eq!(
            print_to_string(&*new_config),
            r#"map_parameter {key: "first_element" value: 2}"#
        );
    }

    #[test]
    fn override_textproto_clean_repeated_succeeds() {
        let default =
            register_config(r#"int_parameter: 1 list_parameter: "first" list_parameter: "second""#);
        let config_override = get_config_override_key_value("list_parameter", "");
        let new_config = override_config(default, config_override);
        assert_eq!(print_to_string(&*new_config), "int_parameter: 1");
    }

    #[test]
    fn override_any_clean_repeated_succeeds() {
        let default =
            register_config(r#"int_parameter: 1 list_parameter: "first" list_parameter: "second""#);
        let mut copair = CoPair::new();
        copair.key = "list_parameter".to_string();
        let new_config = override_config(default, copair);
        assert_eq!(print_to_string(&*new_config), "int_parameter: 1");
    }

    #[test]
    fn override_textproto_assign_new_value_repeated_succeeds() {
        let default = register_config(r#"list_parameter: "first" list_parameter: "second""#);
        let config_override = get_config_override_key_value(
            "list_parameter",
            r#""first updated", "second updated", "third updated""#,
        );
        let new_config = override_config(default, config_override);
        assert_eq!(
            print_to_string(&*new_config),
            r#"list_parameter: "first updated" list_parameter: "second updated" list_parameter: "third updated""#
        );
    }

    #[test]
    fn override_any_assign_new_value_repeated_succeeds() {
        let default = register_config(r#"list_parameter: "first" list_parameter: "second""#);

        let mut copair = CoPair::new();
        copair.key = "list_parameter".to_string();
        let mut param = ListValue::new();
        param.values = vec![
            PValue {
                kind: Some(PStringValue("first updated".to_string())),
                special_fields: SpecialFields::new(),
            },
            PValue {
                kind: Some(PStringValue("second updated".to_string())),
                special_fields: SpecialFields::new(),
            },
            PValue {
                kind: Some(PStringValue("third updated".to_string())),
                special_fields: SpecialFields::new(),
            },
        ];
        copair.value = Option::Some(Value::ValueAny(Any::pack(&param).unwrap()));
        let new_config = override_config(default, copair);
        assert_eq!(
            print_to_string(&*new_config),
            r#"list_parameter: "first updated" list_parameter: "second updated" list_parameter: "third updated""#
        );
    }

    fn get_default_config(textproto: &str) -> TestConfigurationProto {
        let res = parse_from_str::<TestConfigurationProto>(textproto);
        assert!(res.is_ok(), "String: {}, error: {:?}", textproto, res);
        res.unwrap()
    }

    fn register_config(config_txtpb: &str) -> Any {
        Any::pack(&get_default_config(config_txtpb)).unwrap()
    }

    fn override_config_for_subkey(
        config: Any,
        subkeys: &mut VecDeque<String>,
        value: String,
    ) -> Box<dyn MessageDyn> {
        // Does not consider nested rewritings, only for maps.
        let pm = get_test_proto_manager();
        let fd_dyn = pm
            .find_schema(".android.sdv.config.testconfig.TestConfiguration")
            .expect("Message description has not been found");
        let desc: MessageDescriptor = fd_dyn
            .message_by_package_relative_name("TestConfiguration")
            .expect("Message was not found");

        // If the key is undefined the complete config should be rewritten.
        if subkeys.is_empty() {
            let mut new_config = desc.new_instance();
            let result = merge_from_str(&mut *new_config, &value);
            assert!(result.is_ok(), "error: {:?}", result);
            return new_config;
        }
        let key = subkeys
            .pop_front()
            .expect("This should never happen because the empty case is already handled");

        let mut config_dyn = config.unpack_dyn(&desc).unwrap().unwrap();
        if let Some(field) = desc.field_by_name(&key) {
            if value.is_empty() {
                field.clear_field(&mut *config_dyn);
                return config_dyn;
            }

            if field.is_map() {
                // If there is no next subkey => complete map is rewritten
                if subkeys.is_empty() {
                    field.clear_field(&mut *config_dyn);
                    merge(key, &field, value, &mut *config_dyn);
                    return config_dyn;
                }
                // Extract the key for the map.
                let map_key = subkeys
                    .pop_front()
                    .expect("This should never happen because the empty case is already handled");

                // Construct an override string as <map_name> {key: <key_name> value: <value>}.
                let mut override_str: String = key.clone();
                override_str.push_str(" {");
                // ToDo: Adjust to different types. Currently, it assumes a string as a key.
                override_str.push_str("key: \"");
                override_str.push_str(&map_key);
                // ToDo: Adjust to different types. Currently, it assumes a string as a key.
                override_str.push_str("\" ");
                override_str.push_str("value:");
                override_str.push_str(&value);
                override_str.push('}');
                let result = merge_from_str(&mut *config_dyn, &override_str);
                assert!(result.is_ok(), "override: {:?}, error: {:?}", override_str, result);
                return config_dyn;
            }

            let mut override_str: String = "".to_string();
            if field.is_singular() {
                override_str = construct_singular_textproto(key, value, &field);
            } else if field.is_repeated() {
                field.clear_field(&mut *config_dyn);
                override_str = construct_repeated_textproto(key, value);
            }
            let result = merge_from_str(&mut *config_dyn, &override_str);
            assert!(result.is_ok(), "override: {:?}, error: {:?}", override_str, result);
        };
        config_dyn
    }

    fn override_config_for_subkey_any(
        config: Any,
        subkeys: &mut VecDeque<String>,
        value: Any,
    ) -> Box<dyn MessageDyn> {
        // Does not consider nested rewritings, only for maps.
        let pm = get_test_proto_manager();
        let fd_dyn = pm
            .find_schema(".android.sdv.config.testconfig.TestConfiguration")
            .expect("Message description has not been found");
        let desc: MessageDescriptor = fd_dyn
            .message_by_package_relative_name("TestConfiguration")
            .expect("Message was not found");

        // If the key is undefined the complete config should be rewritten.
        if subkeys.is_empty() {
            let mut new_config = desc.new_instance();
            assert!(value.is::<TestConfigurationProto>(), "value is {:?}", value.type_url);
            let result = new_config.merge_from_bytes_dyn(&value.value);
            assert!(result.is_ok(), "error: {:?}", result);
            return new_config;
        }
        let key = subkeys
            .pop_front()
            .expect("This should never happen because the empty case is already handled");

        let mut config_dyn = config.unpack_dyn(&desc).unwrap().unwrap();

        if let Some(field) = desc.field_by_name(&key) {
            match field.runtime_field_type() {
                RuntimeFieldType::Singular(_) => match value.type_url.as_str() {
                    "type.googleapis.com/google.protobuf.BoolValue" => field.set_singular_field(
                        config_dyn.as_mut(),
                        protobuf::reflect::ReflectValueBox::from(
                            value.unpack::<BoolValue>().unwrap().unwrap().value,
                        ),
                    ),
                    "type.googleapis.com/google.protobuf.BytesValue" => {
                        field.set_singular_field(
                            config_dyn.as_mut(),
                            protobuf::reflect::ReflectValueBox::from(
                                value.unpack::<BytesValue>().unwrap().unwrap().value,
                            ),
                        );
                    }
                    "type.googleapis.com/google.protobuf.DoubleValue" => field.set_singular_field(
                        config_dyn.as_mut(),
                        protobuf::reflect::ReflectValueBox::from(
                            value.unpack::<DoubleValue>().unwrap().unwrap().value,
                        ),
                    ),
                    "type.googleapis.com/google.protobuf.FloatValue" => field.set_singular_field(
                        config_dyn.as_mut(),
                        protobuf::reflect::ReflectValueBox::from(
                            value.unpack::<FloatValue>().unwrap().unwrap().value,
                        ),
                    ),
                    "type.googleapis.com/google.protobuf.Int32Value" => field.set_singular_field(
                        config_dyn.as_mut(),
                        protobuf::reflect::ReflectValueBox::from(
                            value.unpack::<Int32Value>().unwrap().unwrap().value,
                        ),
                    ),
                    "type.googleapis.com/google.protobuf.Int64Value" => field.set_singular_field(
                        config_dyn.as_mut(),
                        protobuf::reflect::ReflectValueBox::from(
                            value.unpack::<Int64Value>().unwrap().unwrap().value,
                        ),
                    ),
                    "type.googleapis.com/google.protobuf.StringValue" => field.set_singular_field(
                        config_dyn.as_mut(),
                        protobuf::reflect::ReflectValueBox::from(
                            value.unpack::<StringValue>().unwrap().unwrap().value,
                        ),
                    ),
                    "type.googleapis.com/google.protobuf.UInt32Value" => field.set_singular_field(
                        config_dyn.as_mut(),
                        protobuf::reflect::ReflectValueBox::from(
                            value.unpack::<UInt32Value>().unwrap().unwrap().value,
                        ),
                    ),
                    "type.googleapis.com/google.protobuf.UInt64Value" => field.set_singular_field(
                        config_dyn.as_mut(),
                        protobuf::reflect::ReflectValueBox::from(
                            value.unpack::<UInt64Value>().unwrap().unwrap().value,
                        ),
                    ),
                    &_ => {
                        field.clear_field(&mut *config_dyn);
                        let field_mes = field.mut_message(&mut *config_dyn);
                        let _ = field_mes.merge_from_bytes_dyn(&value.value);
                    }
                },
                protobuf::reflect::RuntimeFieldType::Repeated(_) => {
                    field.clear_field(&mut *config_dyn);
                    let mut repeated = field.mut_repeated(&mut *config_dyn);
                    for repeated_mes in &mut value.unpack::<ListValue>().unwrap().unwrap().values {
                        repeated.push(protobuf::reflect::ReflectValueBox::from(
                            repeated_mes.take_string_value(),
                        ));
                    }
                }
                protobuf::reflect::RuntimeFieldType::Map(_, _) => todo!(),
            }
        }
        config_dyn
    }

    fn construct_singular_textproto(key: String, value: String, field: &FieldDescriptor) -> String {
        let mut override_str: String = key;
        match field.singular_runtime_type() {
            RuntimeType::Message(_) => {}
            _ => override_str.push(':'),
        }
        override_str.push_str(&value);
        override_str
    }

    fn construct_repeated_textproto(key: String, value: String) -> String {
        value
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| key.clone() + ": " + s)
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn merge(
        key: String,
        field: &FieldDescriptor,
        mut value: String,
        config_dyn: &mut dyn MessageDyn,
    ) {
        let mut override_str: String = "".to_string();
        if field.is_singular() {
            override_str.clone_from(&key);
            match field.singular_runtime_type() {
                RuntimeType::Message(_) => {}
                _ => override_str.push(':'),
            }
            override_str.push_str(&value);
        }
        // As the merge_from_str does not understand [] syntax, transform into map{} map{}.
        if field.is_map() {
            override_str.clear();
            if value.starts_with('[') & value.ends_with(']') {
                value.remove(0);
                value.pop();
            }
            override_str = Regex::new(r"\{.*\}")
                .unwrap()
                .find_iter(&value)
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(" ")
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| key.clone() + " " + s)
                .collect::<Vec<_>>()
                .join(" ")
        }
        let result = merge_from_str(&mut *config_dyn, &override_str);
        assert!(result.is_ok(), "override: {:?}, error: {:?}", override_str, result);
    }

    fn print_pretty_map(map: ReflectMapRef) -> String {
        let mut pairs = map
            .into_iter()
            .map(|(k, v)| format!(r#"map_parameter {{key: "{}" value: {}}}"#, k, v))
            .collect::<Vec<_>>();
        pairs.sort();
        pairs.join(" ")
    }

    fn override_config(config: Any, config_override: CoPair) -> Box<dyn MessageDyn> {
        let mut subkeys: VecDeque<String> = config_override
            .key
            .split('.')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        if config_override.has_value_txtproto() {
            override_config_for_subkey(
                config,
                &mut subkeys,
                config_override.value_txtproto().into(),
            )
        } else if config_override.has_value_any() {
            override_config_for_subkey_any(
                config,
                &mut subkeys,
                config_override.value_any().clone(),
            )
        } else {
            override_config_for_subkey(config, &mut subkeys, "".into())
        }
    }

    fn get_config_override_key_value(key: &str, value: &str) -> CoPair {
        let mut copair = CoPair::new();
        copair.key = key.into();
        copair.value = Option::Some(Value::ValueTxtproto(value.into()));
        copair
    }

    fn get_test_proto_manager() -> ProtoManager {
        let proto_dir: PathBuf = PathBuf::from("./proto");
        let test_config_file = proto_dir.join("test_configuration.proto");
        assert!(test_config_file.exists(), "Unable to find proto file {:?}.", test_config_file);

        let inner_test_config_file = proto_dir.join("inner_test_configuration.proto");
        assert!(
            inner_test_config_file.exists(),
            "Unable to find proto file {:?}.",
            inner_test_config_file
        );

        let inner_test_config_file_diff_pack =
            proto_dir.join("inner_test_configuration_different_package.proto");
        assert!(
            inner_test_config_file_diff_pack.exists(),
            "Unable to find proto file {:?}.",
            inner_test_config_file_diff_pack
        );

        let fd_set = protobuf_parse::Parser::new()
            .pure()
            .include(&proto_dir)
            .input(&test_config_file)
            .input(&inner_test_config_file)
            .input(&inner_test_config_file_diff_pack)
            .file_descriptor_set();
        assert!(fd_set.is_ok(), "Could not parse protobuf files {:?}", fd_set);
        ProtoManager::new(fd_set.unwrap())
    }
}
