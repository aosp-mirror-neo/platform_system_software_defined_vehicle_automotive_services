// Copyright 2023 Google LLC
#![allow(missing_docs)]
use concal_api::concal_service::*;
use concal_api::configuration::{
    config_override_key_value::Value::*, ConfigId, ConfigMetadata, ConfigOverride,
    ConfigOverrideKeyValue,
};
use protobuf::descriptor::FileDescriptorSet as FdSet;
use protobuf::well_known_types::any::Any;
use protobuf::{Message, MessageField};
use sdvgenerated::con_cal_client::ConCalClient;
use std::io;
use std::io::prelude::*;
use std::path::PathBuf;
use std::vec::Vec;

use sdvgenerated::con_cal_service_interface::ConCalService;
use sdvtestconcal_proto_rust::rear_view_camera::RearViewCamera;

fn main() {
    let client = ConCalClient::new();

    // using the prebuilt_etc build rule within automotive_services/samples/concal/proto/Android.bp,
    // a test proto rear_view_camera.proto has been placed within the /etc/protos directory for this demo
    let protos_path = String::from("/etc/protos/");
    let config = test_config();
    let config_id = test_config_id();
    let fds = test_file_descriptor_set(protos_path);

    println!("Registering config:\n{:#?}", config);

    register_config(&client, &config_id, &config, &fds);

    wait_for_key_press();

    let overrides = overrides();
    println!("\nApplying the following overrides:\n{:#?}", &overrides);
    update_config(&client, &config_id, &overrides);

    wait_for_key_press();

    let config = get_config(&client, &config_id);
    let config = parse_test_config_from_bytes(&config);

    println!("\nModified configuration:\n{:#?}", config);

    wait_for_key_press();

    println!("\nRolling back previous changes");

    rollback(&client);

    let config = get_config(&client, &config_id);
    let config = parse_test_config_from_bytes(&config);

    println!("\n{:#?}", config);
}

fn test_config() -> RearViewCamera {
    RearViewCamera {
        model: String::from("model 1"),
        horizontal_resolution: 720,
        vertical_resolution: 720,
        x_axis_field_of_view: 70.0,
        y_axis_field_of_view: 70.0,
        is_rgb: false,
        ..Default::default()
    }
}

fn test_config_id() -> ConfigId {
    ConfigId {
        service_fqin: "company_domain.domain.apex.service.instance".to_string(),
        config_name: "config".to_string(),
        ..Default::default()
    }
}

fn test_file_descriptor_set(protos_dir: String) -> FdSet {
    let proto_dir: PathBuf = PathBuf::from(protos_dir);
    let inner_test_config_file_diff_pack = proto_dir.join("rear_view_camera.proto");
    assert!(
        inner_test_config_file_diff_pack.exists(),
        "Unable to find proto file {:?}.",
        inner_test_config_file_diff_pack
    );

    let fd_set = protobuf_parse::Parser::new()
        .pure()
        .include(&proto_dir)
        .input(&inner_test_config_file_diff_pack)
        .file_descriptor_set();
    assert!(fd_set.is_ok(), "Could not parse protobuf files {:?}", fd_set);
    fd_set.unwrap()
}

fn register_config(
    client: &ConCalClient,
    config_id: &ConfigId,
    config: &RearViewCamera,
    fds: &FdSet,
) {
    let config = Any::pack(config).unwrap();
    let config_metadata = ConfigMetadata {
        descriptor_set: MessageField::some(fds.clone()),
        default_config: MessageField::some(config.clone()),
        ..Default::default()
    };

    let resp = client
        .con_cal_service_client
        .register_config_metadata(RegisterConfigMetadataRequest {
            config_id: MessageField::some(config_id.clone()),
            metadata: MessageField::some(config_metadata),
            config_version: String::from("1.0"),
            ..Default::default()
        })
        .expect("register_config_metadata failed");

    if resp.error.is_some() {
        panic!("register_config_metadata failed: {:?}", resp.error.unwrap());
    }
}

fn update_config(client: &ConCalClient, config_id: &ConfigId, overrides: &[ConfigOverride]) {
    let resp = client
        .con_cal_service_client
        .start_config_update(StartConfigUpdateRequest::new())
        .expect("start_config_update failed");

    if resp.error.is_some() {
        panic!("start_config_update failed: {:?}", resp.error.unwrap());
    }

    let update_id = resp.update_id;

    let resp = client
        .con_cal_service_client
        .add_or_update_config_override(AddOrUpdateConfigOverrideRequest {
            update_id,
            config_id: MessageField::some(config_id.to_owned()),
            configOverride: overrides.to_owned(),
            ..Default::default()
        })
        .expect("add_or_update_config_override failed");

    if resp.error.is_some() {
        panic!("add_or_update_config_override failed: {:?}", resp.error.unwrap());
    }

    let resp = client
        .con_cal_service_client
        .finish_config_update(FinishConfigUpdateRequest { update_id, ..Default::default() })
        .expect("finish_config_update failed failed");

    if resp.error.is_some() {
        panic!("finish_config_update failed failed: {:?}", resp.error.unwrap())
    }
}

fn overrides() -> Vec<ConfigOverride> {
    let mut overrides =
        ConfigOverride { override_id: "1".to_string(), pairs: Vec::new(), ..Default::default() };

    overrides.pairs.push(ConfigOverrideKeyValue {
        key: String::from("model"),
        value: Some(ValueStr("model 2".to_string())),
        ..Default::default()
    });

    overrides.pairs.push(ConfigOverrideKeyValue {
        key: String::from("horizontal_resolution"),
        value: Some(ValueInt(1920)),
        ..Default::default()
    });

    overrides.pairs.push(ConfigOverrideKeyValue {
        key: String::from("vertical_resolution"),
        value: Some(ValueInt(1080)),
        ..Default::default()
    });

    overrides.pairs.push(ConfigOverrideKeyValue {
        key: String::from("x_axis_field_of_view"),
        value: Some(ValueFloat(90.0)),
        ..Default::default()
    });

    overrides.pairs.push(ConfigOverrideKeyValue {
        key: String::from("y_axis_field_of_view"),
        value: Some(ValueFloat(90.0)),
        ..Default::default()
    });

    overrides.pairs.push(ConfigOverrideKeyValue {
        key: String::from("is_rgb"),
        value: Some(ValueBool(true)),
        ..Default::default()
    });

    vec![overrides]
}

fn get_config(client: &ConCalClient, config_id: &ConfigId) -> Vec<u8> {
    let resp = client
        .con_cal_service_client
        .get_config(GetConfigRequest {
            config_id: MessageField::some(config_id.clone()),
            ..Default::default()
        })
        .expect("get_config failed");

    if resp.error.is_some() {
        panic!("get_config failed: {:?}", resp.error.unwrap());
    }
    resp.config
}

fn parse_test_config_from_bytes(bytes: &[u8]) -> RearViewCamera {
    let any_proto = Any::parse_from_bytes(bytes).expect("Parse Proto Any from bytes");

    RearViewCamera::parse_from_bytes(&any_proto.value).expect("parse_from_bytes failed")
}

fn rollback(client: &ConCalClient) {
    let resp =
        client.con_cal_service_client.rollback(RollbackRequest::new()).expect("rollback failed");
    if resp.error.is_some() {
        panic!("rollback failed: {:?}", resp.error.unwrap())
    }
}

fn wait_for_key_press() {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();

    // We want the cursor to stay at the end of the line, so we print without a newline and flush manually.
    write!(stdout, "Press any key to continue...").unwrap();
    stdout.flush().unwrap();

    // Read a single byte and discard
    let _ = stdin.read(&mut [0u8]).unwrap();
}
