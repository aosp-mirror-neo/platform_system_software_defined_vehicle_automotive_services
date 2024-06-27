// Copyright 2024 Google LLC

use crate::configuration::Configuration;
use concal_api::configuration::ConfigId;
use protobuf::descriptor::FileDescriptorSet;
use protobuf::well_known_types::any::Any;
use sdvtestconcal_proto_rust::test_configuration::TestConfiguration;
use std::path::PathBuf;

pub(crate) fn test_config() -> Configuration {
    let data =
        Any::pack(&TestConfiguration::new()).expect("Unable to pack TestConfiguration into Any");
    Configuration::new(data, load_file_descriptors(), String::from("1.0"))
}

pub(crate) fn test_config_id() -> ConfigId {
    ConfigId {
        service_fqin: String::from("testdata.testdata.testdata.testdata.testdata"),
        config_name: String::from("testdata"),
        ..Default::default()
    }
}

pub(crate) fn load_file_descriptors() -> FileDescriptorSet {
    let proto_dir: PathBuf = PathBuf::from("./");
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
    fd_set.unwrap()
}
