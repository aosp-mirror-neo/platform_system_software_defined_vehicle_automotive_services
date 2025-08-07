// Copyright 2023 Google LLC
//! Vehicle and Power Mode Service.
use anyhow::anyhow;
use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;
use com_sdv_google_vpm_client_power_notification_service_rpc::Interface as NotificationInterface;
use com_sdv_google_vpm_vpm_system_service_rpc::Client;
use log::{error, info};
use sdv_comms::id::ServiceFqin;
use sdv_comms::ContextRef;
use sdv_mw_rs_com_sdv_google_vpm_client_vpm_system_client::VpmSystemClient;
use sdv_mw_rs_comms::ClientOptions;
use sdv_mw_rs_comms::RpcClient;
use sdv_mw_rs_comms::UnitName;
use sdv_mw_rs_sdv_comms::SdvComms;
use sdv_vpm_api::vehicle::VpmVehicleState;
use sdv_vpm_api::vpm::power_state_change_response::PowerStateChangeRequestError;
use sdv_vpm_api::vpm::GetPowerStateRequest;
use sdv_vpm_api::vpm::PowerStateChangeRequest;
use sdv_vpm_api::vpm::PowerStateRequest;
use sdv_vpm_api::vpm::SetVehicleStateRequest;
use sdv_vpm_api::vpm::ShutdownType;
use sdv_vpm_api::vpm_notifications::PowerStateNotificationResponse;
use std::sync::Arc;

static LOG_TAG: &str = "vepsm";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    sdv_log::init_logger(LOG_TAG)?;
    let context = create_context();
    let comms = SdvComms { context };
    let bundle = VpmSystemClient::new(Arc::new(comms.clone()), (Arc::new(VePSM {}),)).await?;
    let client = create_client(&bundle, comms).await?;
    let mut parse_from_cli = true;
    loop {
        let input_args = if parse_from_cli { Cli::try_parse().ok() } else { parse_input() };
        if let Some(args) = input_args {
            match args.command {
                VpmCommand::VehicleState(vehicle_state) => {
                    handle_vehicle_state(vehicle_state, &client).await?;
                }
                VpmCommand::PowerState(cmd) => {
                    handle_power_state(cmd, &client).await?;
                }
            }
        }
        parse_from_cli = false;
    }
}

fn create_context() -> ContextRef {
    let fqin = ServiceFqin::builder()
        .sdv_vm_name("vm")
        .sdv_package_name("com.sdv.google.vpm.client")
        .service_bundle_name("VpmSystemClient")
        .service_instance_name("default")
        .build()
        .unwrap();

    ContextRef::create(fqin)
}

fn parse_input() -> Option<Cli> {
    println!("Enter VePSM command, press enter for help or cntl + c to exit!");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    let args: Vec<&str> = input.split_whitespace().collect();
    match Cli::try_parse_from(args) {
        Ok(cli) => Some(cli),
        Err(err) => {
            eprintln!("Error: {err}");
            None
        }
    }
}

async fn handle_vehicle_state(
    vehicle_state_args: VehicleStateArgs,
    client: &Client,
) -> anyhow::Result<()> {
    let vehicle_state: VpmVehicleState = vehicle_state_args.clone().into();
    let request =
        SetVehicleStateRequest { vehicle_state: vehicle_state.into(), ..Default::default() };
    client.SetVehicleState(&request).await?;
    println!("Vehicle mode set successfully {vehicle_state_args:?}");
    Ok(())
}

async fn handle_power_state(power_cmd: PowerCommand, client: &Client) -> anyhow::Result<()> {
    let timeout_ms = 10000;
    let request = match power_cmd {
        PowerCommand::Get => {
            let result = client.GetPowerState(&GetPowerStateRequest { ..Default::default() }).await;
            println!("{:?}", result.unwrap().power_state.unwrap());
            return Ok(());
        }

        PowerCommand::PowerOn => PowerStateChangeRequest {
            power_request: PowerStateRequest::TURN_ON.into(),
            timeout_ms,
            ..Default::default()
        },
        PowerCommand::Cancel => PowerStateChangeRequest {
            power_request: PowerStateRequest::CANCEL_SHUTDOWN.into(),
            timeout_ms,
            ..Default::default()
        },
        PowerCommand::Finish { shutdown_type } => match shutdown_type {
            SuspendType::ShutDown => PowerStateChangeRequest {
                power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
                shutdown_type: ShutdownType::POWER_OFF.into(),
                timeout_ms,
                ..Default::default()
            },
            SuspendType::Ram => PowerStateChangeRequest {
                power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
                shutdown_type: ShutdownType::SUSPEND_TO_RAM.into(),
                timeout_ms,
                ..Default::default()
            },
            SuspendType::Disk => PowerStateChangeRequest {
                power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
                shutdown_type: ShutdownType::SUSPEND_TO_DISK.into(),
                timeout_ms,
                ..Default::default()
            },
        },
        PowerCommand::Prepare { prepare_type } => match prepare_type {
            SuspendType::ShutDown => PowerStateChangeRequest {
                power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
                shutdown_type: ShutdownType::POWER_OFF.into(),
                timeout_ms,
                ..Default::default()
            },
            SuspendType::Ram => PowerStateChangeRequest {
                power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
                shutdown_type: ShutdownType::SUSPEND_TO_RAM.into(),
                timeout_ms,
                ..Default::default()
            },
            SuspendType::Disk => PowerStateChangeRequest {
                power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
                shutdown_type: ShutdownType::SUSPEND_TO_DISK.into(),
                timeout_ms,
                ..Default::default()
            },
        },
    };

    let result = client.SetPowerState(&request).await;

    if let Ok(response) = result {
        if response.error == PowerStateChangeRequestError::NO_ERROR.into() {
            info!("Request was sent successfully!");
        } else {
            error!("Request was invalid {response:?}");
        }
    } else {
        error!("Failed to send power state change request, reason={:?}", result.err())
    }
    Ok(())
}

async fn create_client(bundle: &VpmSystemClient, comms: SdvComms) -> anyhow::Result<Client> {
    let server_name = UnitName::builder()
        .vm_name(comms.context.get_self_fqin().get_sdv_vm_name())
        .package_name("com.sdv.google.vpm")
        .bundle_name("VpmSystemServiceBundle")
        .service_unit_name(Client::DEFAULT_UNIT_NAME)
        .build()
        .map_err(|_| anyhow!("invalid server UnitName"))?;
    let result = bundle.create_rpc_client(server_name, ClientOptions::default()).await;
    if result.is_err() {
        error!("Could not get a client for vpm notifications");
        return Err(anyhow!("Could not get a client for vpm notifications"));
    }
    let client = result.unwrap();
    Ok(client)
}

struct VePSM {}

#[async_trait::async_trait]
impl NotificationInterface for VePSM {
    async fn NotifyPowerState(
        &self,
        _caller_id: sdv_comms::id::ServiceFqin,
        request: &sdv_vpm_api::vpm_notifications::PowerStateNotificationRequest,
    ) -> sdv_status_rs::SdvResult<sdv_vpm_api::vpm_notifications::PowerStateNotificationResponse>
    {
        println!(
            "Received power-state-report from VPM {:?} , reason {:?}",
            request.power_state, request.reason
        );
        Ok(PowerStateNotificationResponse { ..Default::default() })
    }
}

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: VpmCommand,
}

#[derive(Subcommand, Debug)]
enum VpmCommand {
    /// Vpm Vehicle state change command
    #[command(subcommand)]
    VehicleState(VehicleStateArgs),

    /// Vpm Power state change command
    #[command(subcommand)]
    PowerState(PowerCommand),
}

/// Set the power state for the VM.
#[derive(Subcommand, Debug, Clone)]
pub enum PowerCommand {
    /// Gets the current power state.
    Get,
    /// Set the power state to on.
    PowerOn,
    /// Prepare the VM for suspension or shutdown.
    Prepare {
        /// Type of preparation.
        prepare_type: SuspendType,
    },
    /// Cancel the preparation for suspension or shutdown.
    Cancel,
    /// Finish the suspend or shutdown process.
    Finish {
        /// Shutdown type of preparation.
        shutdown_type: SuspendType,
    },
}

/// The preparation or suspension type.
#[derive(ValueEnum, Debug, Clone)]
pub enum SuspendType {
    /// Shutdown the VM.
    ShutDown,
    /// Suspend the VM to RAM.
    Ram,
    /// Suspend the VM to DISK.
    Disk,
}

#[derive(Subcommand, Clone, Debug)]
enum VehicleStateArgs {
    LowPower,
    SoftwareUpdate,
    Park,
    LifeOnBoard,
    VehicleOn,
    TractionOn,
}

impl From<VehicleStateArgs> for VpmVehicleState {
    fn from(other: VehicleStateArgs) -> VpmVehicleState {
        match other {
            VehicleStateArgs::LowPower => VpmVehicleState::LOW_POWER,
            VehicleStateArgs::SoftwareUpdate => VpmVehicleState::SOFTWARE_UPDATE,
            VehicleStateArgs::Park => VpmVehicleState::PARK,
            VehicleStateArgs::LifeOnBoard => VpmVehicleState::LIFE_ON_BOARD,
            VehicleStateArgs::VehicleOn => VpmVehicleState::VEHICLE_ON,
            VehicleStateArgs::TractionOn => VpmVehicleState::TRACTION_ON,
        }
    }
}
