// Copyright 2023 Google LLC. All Rights Reserved.

//! ProtoManager maintains schema of protobuf messages with their dependencies.

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

    fn get_test_proto_manager() -> ProtoManager {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_config_file = temp_dir.path().join("test_configuration.proto");
        let test_config = r#"
        syntax = "proto3";

        package android.sdv.config.testconfig;
        import "inner_test_configuration.proto";
        import "inner_test_configuration_different_package.proto";

        message ComplexParameterInFile{
          string str_parameter = 1;
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
