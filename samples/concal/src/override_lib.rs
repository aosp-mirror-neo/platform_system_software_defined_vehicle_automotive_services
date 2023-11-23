// Copyright 2023 Google LLC. All Rights Reserved.

//! ProtoManager maintains schema of protobuf messages with their dependencies.
#![allow(
    unused_imports,
    dead_code,
    missing_docs,
    unused_variables,
    clippy::extra_unused_lifetimes,
    clippy::derivable_impls,
    clippy::new_without_default
)]

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
    #![allow(unused_imports)]
    use super::*;
    use concal_api::configuration::{
        ConfigId as ConfigIdProto, ConfigMetadata as ConfigMetadataProto,
        ConfigOverride as ConfigOverrideProto, ConfigOverrideKeyValue as CoPair,
    };
    use protobuf::reflect::MessageDescriptor;
    use protobuf::text_format::merge_from_str;
    use protobuf::text_format::parse_from_str;
    use protobuf::text_format::print_to_string;
    use protobuf::well_known_types::any::Any;
    use protobuf::MessageDyn;
    use protobuf::{MessageField, SpecialFields};
    use sdvtestconcal_proto_rust::inner_test_configuration::InnerTestConfigurationDifferentFile as InnerTestConfigurationProto;
    use sdvtestconcal_proto_rust::inner_test_configuration_different_package::InnerTestConfigurationDifferentPackage as DifferentPackageProto;
    use sdvtestconcal_proto_rust::test_configuration::{
        ComplexParameterInFile as ComplexProto, TestConfiguration as TestConfigurationProto,
    };
    use std::collections::HashMap;
    use std::fs;

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
        let config_override =
            get_config_override_key_value("bool_parameter", "bool_parameter: false");
        let new_config = override_config(default, config_override);
        assert_eq!(print_to_string(&*new_config), "bool_parameter: false int_parameter: 5");
    }

    #[test]
    fn override_textproto_int_succeeds() {
        let default = register_config("bool_parameter: true int_parameter: 5");
        let config_override = get_config_override_key_value("int_parameter", "int_parameter: 10");
        let new_config = override_config(default, config_override);
        assert_eq!(print_to_string(&*new_config), "bool_parameter: true int_parameter: 10");
    }

    #[test]
    fn override_textproto_float_succeeds() {
        let default = register_config("int_parameter: 5 float_parameter: 5.5");
        let config_override =
            get_config_override_key_value("float_parameter", "float_parameter: 10.7");
        let new_config = override_config(default, config_override);
        assert_eq!(print_to_string(&*new_config), "int_parameter: 5 float_parameter: 10.7");
    }

    #[test]
    fn override_textproto_string_succeeds() {
        let default = register_config("float_parameter: 3.4 str_parameter: \"string parameter\"");
        let config_override =
            get_config_override_key_value("str_parameter", "str_parameter: \"updated\"");
        let new_config = override_config(default, config_override);
        assert_eq!(
            print_to_string(&*new_config),
            "float_parameter: 3.4 str_parameter: \"updated\""
        );
    }

    #[test]
    fn override_textproto_complex_add_new_field_succeeds() {
        let default = register_config("str_parameter: \"string parameter\" complex_parameter {}");
        // In Rust protobuf do not place `:` before `{` for messages, see example: `complex_parameter {`.
        let config_override = get_config_override_key_value(
            "complex_parameter",
            "complex_parameter {first_parameter: \"updated\"}",
        );
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
        let config_override = get_config_override_key_value(
            "complex_parameter",
            "complex_parameter {second_parameter: \"updated\"}",
        );
        let new_config = override_config(default, config_override);
        assert_eq!(
            print_to_string(&*new_config),
            "str_parameter: \"string parameter\" complex_parameter {second_parameter: \"updated\"}"
        );
    }

    #[test]
    fn override_textproto_config_succeeds() {
        let default = register_config("bool_parameter: true int_parameter: 5 float_parameter: 3.4 str_parameter: \"string parameter\" nested_parameter {} complex_parameter {} nested_package {}");
        // In Rust protobuf do not place `:` before `{` for messages, see example: `complex_parameter {`.
        let config_override = get_config_override_key_value(
            "",
            "bool_parameter: false int_parameter: 0 float_parameter: 0 str_parameter: \"\"",
        );
        let new_config = override_config(default, config_override);
        assert_eq!(
            print_to_string(&*new_config),
            "bool_parameter: false int_parameter: 0 float_parameter: 0 str_parameter: \"\" nested_parameter {} complex_parameter {} nested_package {}"
        );
    }

    fn get_default_config(textproto: &str) -> TestConfigurationProto {
        let res = parse_from_str::<TestConfigurationProto>(textproto);
        assert!(res.is_ok());
        res.unwrap()
    }

    fn register_config(config_txtpb: &str) -> Any {
        Any::pack(&get_default_config(config_txtpb)).unwrap()
    }

    fn override_config(config: Any, config_override: CoPair) -> Box<dyn MessageDyn> {
        let pm = get_test_proto_manager();
        let fd_dyn = pm
            .find_schema(".android.sdv.config.testconfig.TestConfiguration")
            .expect("Message description has not been found");
        let new_desc: MessageDescriptor = fd_dyn
            .message_by_package_relative_name("TestConfiguration")
            .expect("Message was not found");
        let mut config_dyn = config.unpack_dyn(&new_desc).unwrap().unwrap();
        let result = merge_from_str(&mut *config_dyn, &config_override.value_txtproto);
        assert!(result.is_ok());
        config_dyn
    }

    fn get_config_override_key_value(key: &str, value: &str) -> CoPair {
        let mut copair = CoPair::new();
        copair.key = key.to_string();
        copair.value_txtproto = value.to_string();
        copair
    }

    fn get_test_proto_manager() -> ProtoManager {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_config_file = temp_dir.path().join("test_configuration.proto");
        let test_config = r#"
        syntax = "proto3";

        package android.sdv.config.testconfig;
        import "inner_test_configuration.proto";
        import "inner_test_configuration_different_package.proto";

        message ComplexParameterInFile{
          string first_parameter = 1;
          string second_parameter = 2;
        }

        message TestConfiguration {
          bool bool_parameter = 1;
          int64 int_parameter = 2;
          float float_parameter = 3;
          string str_parameter = 4;
          repeated string list_parameter = 5;
          map<string, int32> map_parameter = 6;
          InnerTestConfigurationDifferentFile nested_parameter = 7;
          ComplexParameterInFile complex_parameter = 8;
          android.sdv.config.diffpackage.InnerTestConfigurationDifferentPackage nested_package = 9;
        }
        "#;
        fs::write(&test_config_file, test_config).unwrap();

        let inner_test_config_file = temp_dir.path().join("inner_test_configuration.proto");
        let inner_test_config = r#"syntax = "proto3";

        package android.sdv.config.testconfig;

        message InnerTestConfigurationDifferentFile{
            bool bool_parameter = 1;
            int64 int_parameter = 2;
            float float_parameter = 3;
            string str_parameter = 4;
            repeated string list_parameter = 5;
            map<string, int32> map_parameter = 6;
        }"#;
        fs::write(&inner_test_config_file, inner_test_config).unwrap();

        let inner_test_config_file_diff_pack =
            temp_dir.path().join("inner_test_configuration_different_package.proto");
        let inner_test_config_diff_pack = r#"syntax = "proto3";

        package android.sdv.config.diffpackage;

        message InnerTestConfigurationDifferentPackage{
            bool bool_parameter = 1;
            int64 int_parameter = 2;
            float float_parameter = 3;
            string str_parameter = 4;
            repeated string list_parameter = 5;
            map<string, int32> map_parameter = 6;
        }"#;
        fs::write(&inner_test_config_file_diff_pack, inner_test_config_diff_pack).unwrap();

        let fd_set = protobuf_parse::Parser::new()
            .pure()
            .include(&temp_dir)
            .input(&test_config_file)
            .input(&inner_test_config_file)
            .input(&inner_test_config_file_diff_pack)
            .file_descriptor_set()
            .unwrap();

        ProtoManager::new(fd_set)
    }
}
