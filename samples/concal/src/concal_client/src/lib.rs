// Copyright 2024 Google LLC
#![allow(missing_docs)]

//! ConCal Sample Client

use com_sdv_google_concal_con_cal_registration_service_rpc::Client as RegistrationClient;
use com_sdv_google_concal_con_cal_update_service_rpc::Client as UpdateClient;
use concal_api::concal_registration_service::*;
use concal_api::concal_update_service::*;
use concal_api::config_override::config_override_key_value::Value;
use concal_api::config_override::{ConfigOverride, ConfigOverrideKeyValue};
use concal_api::configuration::{ConfigId, ConfigMetadata};
use concal_api::service_fqin::ServiceFqin as ProtoFqin;
use log::{debug, info, trace, warn};
use protobuf::descriptor::FileDescriptorSet as FdSet;
use protobuf::well_known_types::any::Any;
use protobuf::{Message, MessageField};
use sdv::mw::ClientOptions;
use sdv::mw::RpcClient;
use sdv::mw::SdvComms;
use sdv::mw::UnitName;
use sdv::status::{SdvStatus, SdvStatusCode};
use sdv_mw_rs_com_sdv_oem_sample_concal_sample_oem_con_cal_client_service_bundle::SampleOemConCalClientServiceBundle;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::vec::Vec;
use tokio_util::sync::CancellationToken;

use sdvtestconcal_proto_rust::rear_view_camera::RearViewCamera;

pub struct SampleOemConCalClientServiceBundleService {
    // Entry point for comm stack APIs.
    context: ContextRef,
    // Used to shutdown a runtime after on_stop request
    cancellation_token: Option<CancellationToken>,
    // Execution thread to get results when the bundle is stopped.
    execution_thread: Option<JoinHandle<()>>,
}

// Register the new service bundle.
sdv::lifecycle::register_service_bundle!(SampleOemConCalClientServiceBundleService);

impl ServiceBundle for SampleOemConCalClientServiceBundleService {
    /// Creates a new instance of the SampleOemConCalClientServiceBundleService.
    /// Called when service bundle is created by the system.
    ///
    /// Context object is provided as a parameter that gives access to the
    /// communication stack APIs.
    fn new(context: ContextRef) -> SampleOemConCalClientServiceBundleService {
        // Memory allocations and static data loading should be done as
        // part of this method.
        //
        // Loading of the dynamic resources (sockets, files, etc)
        // is strongly discouraged due to possible Suspend-to-RAM scenario.
        //
        // The dynamic data can be loaded in the [Self::on_start()] method.
        debug!("Creating {}.", context.get_self_fqin());
        SampleOemConCalClientServiceBundleService {
            context,
            cancellation_token: None,
            execution_thread: None,
        }
    }

    /// Called when the service bundle is started by the system.
    fn on_start(&mut self) {
        // Dynamic resources(sockets, files, etc) should be allocated during this call.
        debug!("Starting {}.", self.context.get_self_fqin());
        // Spawn new execution thread to avoid blocking the `on_start` method.
        // New thread creates tokio runtime and starts MW and Comm Stack.
        let new_thread = spawn_execution_thread(self.create_cancellation_token(), self.context);
        self.execution_thread = Some(new_thread);
    }

    /// Called when the service bundle is stopped by the system in preparation
    /// for shutdown or suspend to RAM/Disc.
    fn on_stop(&mut self) {
        // Stop phase requires the service bundle to delete the dynamic resources
        // (sockets, files, etc) that were previously allocated in the [Self::on_start()] method.
        debug!("Stopping {}.", self.context.get_self_fqin());
        // Trigger cancellation token to exit Tokio runtime execution and drop it.
        if let Some(cancellation_token) = self.cancellation_token.as_mut() {
            cancellation_token.cancel();
            self.cancellation_token = None;
        } else {
            warn!("Cancellation token must have been created on_start.");
        }
        trace!("Joining execution thread {:#?}.", self.execution_thread);
        // TODO (b/392541025): Add waiting timeout to avoid JoinHandle::join be blocked.
        let result = self.execution_thread.take().map(JoinHandle::join);
        trace!("Joined execution thread with {result:?}.");
    }
}

/// Called when the service bundle is destroyed by the system.
impl Drop for SampleOemConCalClientServiceBundleService {
    fn drop(&mut self) {
        // Static resources deallocation needs to be implemented in the drop method.
        debug!("Drop {}.", self.context.get_self_fqin());
    }
}

impl SampleOemConCalClientServiceBundleService {
    fn create_cancellation_token(&mut self) -> CancellationToken {
        let cancellation_token = CancellationToken::new();
        self.cancellation_token = Some(cancellation_token.clone());
        cancellation_token
    }
}
fn spawn_execution_thread(
    cancellation_token: CancellationToken,
    context: ContextRef,
) -> JoinHandle<()> {
    let Ok(runtime) =
        tokio::runtime::Builder::new_current_thread().enable_time().enable_io().build()
    else {
        panic!("Failed to start the tokio runtime!");
    };
    std::thread::spawn(move || {
        // Due to async nature of MW/Comms APIs we have
        // to execute a service bundle inside a runtime (Tokio).
        runtime.block_on(async move {
            debug!("Starting {} execution loop", context.get_self_fqin());

            // Call `foo_main` which contains a business logic.
            // The function is not supposed to return.
            // The execution must be halted only by `on_stop` request coming from the
            // lifecycle manager.
            let res = tokio::select! {
                res = sample_oem_concal_client_service_bundle_main(context) => res,
                () = cancellation_token.cancelled() => Err(SdvStatus::new(SdvStatusCode::Cancelled)),
            };
            debug!("Stopped {} execution loop with result: {res:?}.", context.get_self_fqin());
        });
        // Dropping tokio runtime immediately stops all the execution of a business logic
        // and gracefully free all the allocated resources.
        // Discovering of service units offered by this service bundle are no longer possible.
        //
        // This does not stop a thread created with `tokio::spawn_blocking` or `std::thread::spawn`.
        // The created execution thread will be joined in `onStop` method.
        drop(runtime);
    })
}

static LOG_TAG: &str = "concal_sample_client";

async fn sample_oem_concal_client_service_bundle_main(
    context: ContextRef,
) -> sdv::status::SdvResult<()> {
    sdv_log::init_logger(LOG_TAG)?;

    let sdv_comms = SdvComms { context };

    info!("Context for Sample ConCal Client is created.");

    let service_bundle = SampleOemConCalClientServiceBundle::new(Arc::new(sdv_comms)).await?;
    let client = service_bundle
        .create_rpc_client::<RegistrationClient>(
            UnitName::builder()
                .package_name("com.sdv.oem.sample.concal")
                .bundle_name("SampleOemConCalServiceBundle")
                .service_unit_name(RegistrationClient::DEFAULT_UNIT_NAME)
                .build()?,
            ClientOptions::default(),
        )
        .await
        .expect("Failed to create an RPC client");

    info!("Sample ConCal RPC Client started.");

    // using the prebuilt_etc build rule within automotive_services/samples/concal/proto/Android.bp,
    // a sample proto rear_view_camera.proto has been placed within the /etc/protos directory for this demo
    let protos_path = String::from("/etc/protos/");
    let config = test_config();
    let config_id = test_config_id();
    let fds = test_file_descriptor_set(protos_path);

    info!("Registering config:\n{config:#?}");

    register_config(&client, &config_id, &config, &fds).await;

    let overrides = overrides(&config_id);
    info!("\nApplying the following overrides:\n{:#?}", &overrides);
    let update_client = service_bundle
        .create_rpc_client::<UpdateClient>(
            UnitName::builder()
                .package_name("com.sdv.oem.sample.concal")
                .bundle_name("SampleOemConCalServiceBundle")
                .service_unit_name(UpdateClient::DEFAULT_UNIT_NAME)
                .build()?,
            ClientOptions::default(),
        )
        .await
        .expect("Failed to create an RPC client");

    update_config(&update_client, &overrides).await;

    let config = get_config(&client, &config_id).await;
    let config = parse_test_config_from_bytes(&config);

    info!("\nModified configuration:\n{config:#?}");

    info!("\nRolling back previous changes");

    rollback(&update_client).await;

    let config = get_config(&client, &config_id).await;
    let config = parse_test_config_from_bytes(&config);

    info!("\n{config:#?}");

    Ok(())
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
        package_name: String::from("com.sdv.oem.sample.concal"),
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
        "Unable to find proto file {inner_test_config_file_diff_pack:?}."
    );

    let fd_set = protobuf_parse::Parser::new()
        .pure()
        .include(&proto_dir)
        .input(&inner_test_config_file_diff_pack)
        .file_descriptor_set();
    assert!(fd_set.is_ok(), "Could not parse protobuf files {fd_set:?}");
    fd_set.unwrap()
}

async fn register_config(
    client: &RegistrationClient,
    config_id: &ConfigId,
    config: &RearViewCamera,
    fds: &FdSet,
) {
    let config = Any::pack(config).expect("Failed to pack config");
    let config_metadata = ConfigMetadata {
        descriptor_set: MessageField::some(fds.clone()),
        default_config: MessageField::some(config.clone()),
        ..Default::default()
    };

    client
        .RegisterConfigMetadata(&RegisterConfigMetadataRequest {
            config_id: MessageField::some(config_id.clone()),
            metadata: MessageField::some(config_metadata),
            config_version: String::from("1.0"),
            ..Default::default()
        })
        .await
        .expect("RegisterConfigMetadata() failed");
}

async fn update_config(client: &UpdateClient, overrides: &[ConfigOverride]) {
    let update_id = client
        .StartConfigUpdate(&StartConfigUpdateRequest::new())
        .await
        .expect("StartConfigUpdate() failed")
        .update_id;

    client
        .AddOrUpdateConfigOverride(&AddOrUpdateConfigOverrideRequest {
            update_id,
            configOverride: overrides.to_owned(),
            ..Default::default()
        })
        .await
        .expect("AddOrUpdateConfigOverride() failed");

    client
        .FinishConfigUpdate(&FinishConfigUpdateRequest { update_id, ..Default::default() })
        .await
        .expect("FinishConfigUpdate() failed");
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
    client
        .GetConfig(&GetConfigRequest {
            config_id: MessageField::some(config_id.clone()),
            ..Default::default()
        })
        .await
        .expect("FetConfig() failed")
        .config
}

fn parse_test_config_from_bytes(bytes: &[u8]) -> RearViewCamera {
    let any_proto = Any::parse_from_bytes(bytes).expect("Parse Proto Any from bytes");

    RearViewCamera::parse_from_bytes(&any_proto.value).expect("parse_from_bytes failed")
}

async fn rollback(client: &UpdateClient) {
    client.Rollback(&RollbackRequest::new()).await.expect("Rollback() failed");
}
