// Copyright 2023 Google LLC. All Rights Reserved.

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
            let fd = Fd::new_dynamic(fd_proto.clone(), &deps).unwrap();
            if fd.message_by_full_name(full_mes_name).is_some() {
                let deps = fd_proto
                    .dependency
                    .iter()
                    .filter_map(|dep| self.get_file_descriptor_with_deps(dep))
                    .collect::<Vec<_>>();
                return Some(Fd::new_dynamic(fd_proto.clone(), &deps.clone()).unwrap());
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
        Some(Fd::new_dynamic(fd, &deps).unwrap())
    }

    fn find_file_descriptor(&self, file_name: &str) -> Option<FdProto> {
        self.fd_set
            .file
            .iter()
            .find(|f| f.name.to_owned().map_or(false, |n| n == file_name))
            .to_owned()
            .map(|f| f.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use concal_api::configuration::ConfigOverrideKeyValue as CoPair;
    use protobuf::reflect::ReflectMapRef;
    use protobuf::reflect::{FieldDescriptor, MessageDescriptor, RuntimeType};
    use protobuf::text_format::merge_from_str;
    use protobuf::text_format::parse_from_str;
    use protobuf::text_format::print_to_string;
    use protobuf::well_known_types::any::Any;
    use protobuf::MessageDyn;
    use sdvtestconcal_proto_rust::test_configuration::TestConfiguration as TestConfigurationProto;
    use std::collections::VecDeque;
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
    fn override_textproto_int_succeeds() {
        let default = register_config("bool_parameter: true int_parameter: 5");
        let config_override = get_config_override_key_value("int_parameter", "10");
        let new_config = override_config(default, config_override);
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
    fn override_textproto_string_succeeds() {
        let default = register_config("float_parameter: 3.4 str_parameter: \"string parameter\"");
        let config_override = get_config_override_key_value("str_parameter", "\"updated\"");
        let new_config = override_config(default, config_override);
        assert_eq!(
            print_to_string(&*new_config),
            "float_parameter: 3.4 str_parameter: \"updated\""
        );
    }

    #[test]
    fn override_textproto_complex_add_new_field_succeeds() {
        let default = register_config("str_parameter: \"string parameter\"");
        // In Rust protobuf do not place `:` before `{` for messages, see example: `complex_parameter {`.
        let config_override =
            get_config_override_key_value("complex_parameter", "{first_parameter: \"updated\"}");
        let new_config = override_config(default, config_override);
        assert_eq!(
            print_to_string(&*new_config),
            "str_parameter: \"string parameter\" complex_parameter {first_parameter: \"updated\"}"
        );
    }

    #[test]
    fn override_textproto_complex_full_rewrite_succeeds() {
        // See that the complex parameter is rewritten completely.
        let default = register_config(
            "str_parameter: \"string parameter\" complex_parameter {first_parameter: \"initial\"}",
        );
        let config_override =
            get_config_override_key_value("complex_parameter", "{second_parameter: \"updated\"}");
        let new_config = override_config(default, config_override);
        assert_eq!(
            print_to_string(&*new_config),
            "str_parameter: \"string parameter\" complex_parameter {second_parameter: \"updated\"}"
        );
    }

    #[test]
    fn clear_textproto_config_succeeds() {
        let default = register_config("bool_parameter: true int_parameter: 5 float_parameter: 3.4 str_parameter: \"string parameter\" nested_parameter {} complex_parameter {second_parameter: \"initial\"} nested_package {}");
        // In Rust protobuf do not place `:` before `{` for messages, see example: `complex_parameter {`.
        let config_override = get_config_override_key_value("", "");
        let new_config = override_config(default, config_override);
        assert_eq!(print_to_string(&*new_config), "");
    }

    #[test]
    fn override_textproto_config_succeeds() {
        let default = register_config("bool_parameter: true int_parameter: 5 float_parameter: 3.4 str_parameter: \"string parameter\" nested_parameter {} complex_parameter {second_parameter: \"initial\"} nested_package {}");
        // In Rust protobuf do not place `:` before `{` for messages, see example: `complex_parameter {`.
        let config_override = get_config_override_key_value(
            "",
            "int_parameter: 10 complex_parameter {first_parameter: \"initial\"} map_parameter {key: \"first_element\" value: 1}",
        );
        let new_config = override_config(default, config_override);
        assert_eq!(
            print_to_string(&*new_config),
            "int_parameter: 10 map_parameter {key: \"first_element\" value: 1} complex_parameter {first_parameter: \"initial\"}",
        );
    }

    #[test]
    fn override_textproto_clean_map_succeeds() {
        let default = register_config("map_parameter {key: \"first_element\" value: 1}");
        let config_override = get_config_override_key_value("map_parameter", "");
        let new_config = override_config(default, config_override);
        assert_eq!(print_to_string(&*new_config), "");
    }

    // my_map { key: "entry1" value: 1 }
    #[test]
    fn override_textproto_add_newkey_to_map_succeeds() {
        let default = register_config("map_parameter {key: \"initial key\" value: 1}");
        // In Rust protobuf do not place `:` before `{` for messages, see example: `complex_parameter {`.
        let config_override = get_config_override_key_value("map_parameter.new key", "2");
        let new_config = override_config(default, config_override);

        // We cannot compare maps as strings with multiple keys, as the order of elements is not guaranteed.
        // Therefore, we need to sort the entries first.
        let map_desc = new_config.descriptor_dyn().field_by_name("map_parameter").unwrap();
        let map = map_desc.get_map(&*new_config);
        assert_eq!(print_pretty_map(map), "map_parameter {key: \"initial key\" value: 1} map_parameter {key: \"new key\" value: 2}");
    }

    #[test]
    fn override_textproto_assign_new_value_to_key_in_map_succeeds() {
        let default = register_config("map_parameter {key: \"first_element\" value: 1}");
        // In Rust protobuf do not place `:` before `{` for messages, see example: `complex_parameter {`.
        let config_override = get_config_override_key_value("map_parameter.first_element", "2");
        let new_config = override_config(default, config_override);
        assert_eq!(
            print_to_string(&*new_config),
            "map_parameter {key: \"first_element\" value: 2}"
        );
    }

    fn get_default_config(textproto: &str) -> TestConfigurationProto {
        let res = parse_from_str::<TestConfigurationProto>(textproto);
        assert!(res.is_ok(), "{:?}", res);
        res.unwrap()
    }

    fn register_config(config_txtpb: &str) -> Any {
        Any::pack(&get_default_config(config_txtpb)).unwrap()
    }

    fn override_config_fo_subkey(
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

            let mut override_str: String = key;
            match field.singular_runtime_type() {
                RuntimeType::Message(_) => {}
                _ => override_str.push(':'),
            }
            override_str.push_str(&value);
            let result = merge_from_str(&mut *config_dyn, &override_str);
            assert!(result.is_ok(), "override: {:?}, error: {:?}", override_str, result);
        }
        config_dyn
    }

    fn merge(key: String, field: &FieldDescriptor, value: String, config_dyn: &mut dyn MessageDyn) {
        let mut override_str: String = key;
        match field.singular_runtime_type() {
            RuntimeType::Message(_) => {}
            _ => override_str.push(':'),
        }
        override_str.push_str(&value);
        let result = merge_from_str(&mut *config_dyn, &override_str);
        assert!(result.is_ok(), "override: {:?}, error: {:?}", override_str, result);
    }

    fn print_pretty_map(map: ReflectMapRef) -> String {
        let mut pairs = map
            .into_iter()
            .map(|(k, v)| format!("map_parameter {{key: \"{}\" value: {}}}", k, v))
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

        override_config_fo_subkey(config, &mut subkeys, config_override.value_txtproto)
    }

    fn get_config_override_key_value(key: &str, value: &str) -> CoPair {
        let mut copair = CoPair::new();
        copair.key = key.into();
        copair.value_txtproto = value.into();
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
