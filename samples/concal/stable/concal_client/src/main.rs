// Copyright 2024 Google LLC

//! ConCal Sample Client

use com_sdv_concal_con_cal_registration_service_rpc::Client as RegistrationClient;
use com_sdv_concal_con_cal_update_service_rpc::Client as UpdateClient;
use concal_api_stable::concal_registration_service::*;
use concal_api_stable::concal_update_service::*;
use concal_api_stable::config_override::config_override_key_value::Value;
use concal_api_stable::config_override::{ConfigOverride, ConfigOverrideKeyValue};
use concal_api_stable::configuration::{ConfigId, ConfigMetadata};
use concal_api_stable::service_fqin::ServiceFqin as ProtoFqin;
use protobuf::descriptor::FileDescriptorSet as FdSet;
use protobuf::well_known_types::any::Any;
use protobuf::{Message, MessageField};
use sdv::comms::id::ServiceFqin as CommsFqin;
use sdv::comms::ContextRef;
use sdv::mw::ClientOptions;
use sdv::mw::SdvComms;
use sdv_mw_rs_com_sdv_concal_sample_oem_sample_oem_con_cal_client_service_bundle::SampleOemConCalClientServiceBundle;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::vec::Vec;

use sdvsampleconcal_proto_rust::concal_rear_view_camera::RearViewCamera;

#[tokio::main]
async fn main() {
    let sdv_comms = SdvComms {
        context: ContextRef::create(
            CommsFqin::from_str(
                "local-vm:sdv.concal.sample.SampleOemConCalClientServiceBundle/sample",
            )
            .unwrap(),
        ),
    };

    println!("Context for Sample ConCal Client is created.");

    let service_bundle =
        SampleOemConCalClientServiceBundle::new(Arc::new(sdv_comms)).await.unwrap();
    let client = service_bundle
        .create_rpc_client::<RegistrationClient>(
            "com.sdv.oem.sample.concal".to_string(),
            "SampleOemConCalServiceBundle".to_string(),
            ClientOptions::default(),
        )
        .await;
    if client.is_err() {
        panic!("Failed to create an RPC client");
    }
    let client = client.unwrap();
    println!("Sample ConCal RPC Client started.");

    // using the prebuilt_etc build rule within automotive_services/samples/concal/stable/proto/Android.bp,
    // a sample proto rear_view_camera.proto has been placed within the /etc/protos directory for this demo
    let protos_path = String::from("/etc/protos/");
    let config = test_config();
    let config_id = test_config_id();
    let fds = test_file_descriptor_set(protos_path);

    println!("Registering config:\n{:#?}", config);

    register_config(&client, &config_id, &config, &fds).await;

    let overrides = overrides(&config_id);
    println!("\nApplying the following overrides:\n{:#?}", &overrides);
    let update_client = service_bundle
        .create_rpc_client::<UpdateClient>(
            "com.sdv.oem.sample.concal".to_string(),
            "SampleOemConCalServiceBundle".to_string(),
            ClientOptions::default(),
        )
        .await
        .unwrap();
    update_config(&update_client, &overrides).await;

    let config = get_config(&client, &config_id).await;
    let config = parse_test_config_from_bytes(&config);

    println!("\nModified configuration:\n{:#?}", config);

    println!("\nRolling back previous changes");

    rollback(&update_client).await;

    let config = get_config(&client, &config_id).await;
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

fn test_service_fqin() -> ProtoFqin {
    ProtoFqin {
        vm_name: String::from("local-vm"),
        package_name: String::from("sdv.concal.sample"),
        service_name: String::from("ConCalClientServiceBundle"),
        instance_name: String::from("sample"),
        ..Default::default()
    }
}

fn test_config_id() -> ConfigId {
    let mut config = ConfigId::new();
    config.config_name = String::from("config");
    config.service_fqin = MessageField::some(test_service_fqin());
    config
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

async fn register_config(
    client: &RegistrationClient,
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
        .RegisterConfigMetadata(&RegisterConfigMetadataRequest {
            config_id: MessageField::some(config_id.clone()),
            metadata: MessageField::some(config_metadata),
            config_version: String::from("1.0"),
            ..Default::default()
        })
        .await;

    if resp.is_err() {
        panic!("RegisterConfigMetadata() failed: {:?}", resp);
    }
}

async fn update_config(client: &UpdateClient, overrides: &[ConfigOverride]) {
    let resp = client.StartConfigUpdate(&StartConfigUpdateRequest::new()).await;

    if resp.is_err() {
        panic!("StartConfigUpdate() failed: {:?}", resp);
    }

    let update_id = resp.unwrap().update_id;

    let resp = client
        .AddOrUpdateConfigOverride(&AddOrUpdateConfigOverrideRequest {
            update_id,
            configOverride: overrides.to_owned(),
            ..Default::default()
        })
        .await;

    if resp.is_err() {
        panic!("AddOrUpdateConfigOverride() failed: {:?}", resp);
    }

    let resp = client
        .FinishConfigUpdate(&FinishConfigUpdateRequest { update_id, ..Default::default() })
        .await;

    if resp.is_err() {
        panic!("FinishConfigUpdate() failed: {:?}", resp);
    };
}

fn overrides(config_id: &ConfigId) -> Vec<ConfigOverride> {
    let mut overrides = ConfigOverride {
        config_id: MessageField::some(config_id.to_owned()),
        override_id: "1".to_string(),
        pairs: Vec::new(),
        ..Default::default()
    };

    overrides.pairs.push(ConfigOverrideKeyValue {
        key: String::from("model"),
        value: Option::Some(Value::ValueTxtproto(String::from("\"model 2\""))),
        ..Default::default()
    });

    overrides.pairs.push(ConfigOverrideKeyValue {
        key: String::from("horizontal_resolution"),
        value: Option::Some(Value::ValueTxtproto(String::from("1920"))),
        ..Default::default()
    });

    overrides.pairs.push(ConfigOverrideKeyValue {
        key: String::from("vertical_resolution"),
        value: Option::Some(Value::ValueTxtproto(String::from("1080"))),
        ..Default::default()
    });

    overrides.pairs.push(ConfigOverrideKeyValue {
        key: String::from("x_axis_field_of_view"),
        value: Option::Some(Value::ValueTxtproto(String::from("90.0"))),
        ..Default::default()
    });

    overrides.pairs.push(ConfigOverrideKeyValue {
        key: String::from("y_axis_field_of_view"),
        value: Option::Some(Value::ValueTxtproto(String::from("90.0"))),
        ..Default::default()
    });

    overrides.pairs.push(ConfigOverrideKeyValue {
        key: String::from("is_rgb"),
        value: Option::Some(Value::ValueTxtproto(String::from("true"))),
        ..Default::default()
    });

    vec![overrides]
}

async fn get_config(client: &RegistrationClient, config_id: &ConfigId) -> Vec<u8> {
    let resp = client
        .GetConfig(&GetConfigRequest {
            config_id: MessageField::some(config_id.clone()),
            ..Default::default()
        })
        .await;

    if resp.is_err() {
        panic!("FetConfig() failed: {:?}", resp);
    }
    resp.unwrap().config
}

fn parse_test_config_from_bytes(bytes: &[u8]) -> RearViewCamera {
    let any_proto = Any::parse_from_bytes(bytes).expect("Parse Proto Any from bytes");

    RearViewCamera::parse_from_bytes(&any_proto.value).expect("parse_from_bytes failed")
}

async fn rollback(client: &UpdateClient) {
    let resp = client.Rollback(&RollbackRequest::new()).await;
    if resp.is_err() {
        panic!("Rollback() failed: {:?}", resp);
    }
}
