// Copyright 2023 Google LLC
//! Vehicle and Power Mode Service.

use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;
use log::{error, info};
use sdv_vpm_api::vehicle::VpmVehicleState;
use sdv_vpm_api::vpm::power_state_change_response::PowerStateChangeRequestError;
use sdv_vpm_api::vpm::PowerStateChangeRequest;
use sdv_vpm_api::vpm::PowerStateNotificationRequest;
use sdv_vpm_api::vpm::PowerStateNotificationResponse;
use sdv_vpm_api::vpm::PowerStateRequest;
use sdv_vpm_api::vpm::SetVehicleStateRequest;
use sdv_vpm_api::vpm::ShutdownType;
use sdvgenerated::sdv_vpm_power_notification_service_interface::PowerNotificationService;
use sdvgenerated::sdv_vpm_vpm_system_service_interface::VpmSystemService;
use sdvgenerated::vpm_system_client::VpmSystemClient;
use sdvmiddleware_rpc::RpcResult;
use std::sync::Arc;

static LOG_TAG: &str = "vepsm";

fn main() -> anyhow::Result<()> {
    sdv_log::init_logger(LOG_TAG)?;
    let client = VpmSystemClient::new();
    client.sdv_vpm_power_notification_service_server.register_service_interface(Arc::new(VePSM {}));

    let mut parse_from_cli = true;
    loop {
        let input_args = if parse_from_cli { Cli::try_parse().ok() } else { parse_input() };
        if let Some(args) = input_args {
            match args.command {
                VpmCommand::VehicleState(vehicle_state) => {
                    handle_vehicle_state(vehicle_state, &client)?;
                }
                VpmCommand::PowerState(cmd) => {
                    handle_power_state(cmd, &client)?;
                }
            }
        }
        parse_from_cli = false;
    }
}

fn parse_input() -> Option<Cli> {
    println!("Enter VePSM command, press enter for help or cntl + c to exit!");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    let args: Vec<&str> = input.split_whitespace().collect();
    match Cli::try_parse_from(args) {
        Ok(cli) => Some(cli),
        Err(err) => {
            eprintln!("Error: {}", err);
            None
        }
    }
}

fn handle_vehicle_state(
    vehicle_state_args: VehicleStateArgs,
    client: &VpmSystemClient,
) -> anyhow::Result<()> {
    let vehicle_state: VpmVehicleState = vehicle_state_args.clone().into();

    let request =
        SetVehicleStateRequest { vehicle_state: vehicle_state.into(), ..Default::default() };

    client
        .get_sdv_vpm_vpm_system_service_client("VpmSystemServiceBundle")
        .set_vehicle_state(request)?;

    println!("Vehicle mode set successfully {:?}", vehicle_state_args);
    Ok(())
}

fn handle_power_state(power_cmd: PowerCommand, client: &VpmSystemClient) -> anyhow::Result<()> {
    let request = match power_cmd {
        PowerCommand::PowerOn => PowerStateChangeRequest {
            power_request: PowerStateRequest::TURN_ON.into(),
            ..Default::default()
        },
        PowerCommand::Cancel => PowerStateChangeRequest {
            power_request: PowerStateRequest::CANCEL_SHUTDOWN.into(),
            ..Default::default()
        },
        PowerCommand::Finish { shutdown_type } => match shutdown_type {
            SuspendType::ShutDown => PowerStateChangeRequest {
                power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
                shutdown_type: ShutdownType::POWER_OFF.into(),
                ..Default::default()
            },
            SuspendType::Ram => PowerStateChangeRequest {
                power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
                shutdown_type: ShutdownType::SUSPEND_TO_RAM.into(),
                ..Default::default()
            },
            SuspendType::Disk => PowerStateChangeRequest {
                power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
                shutdown_type: ShutdownType::SUSPEND_TO_DISK.into(),
                ..Default::default()
            },
        },
        PowerCommand::Prepare { prepare_type } => match prepare_type {
            SuspendType::ShutDown => PowerStateChangeRequest {
                power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
                shutdown_type: ShutdownType::POWER_OFF.into(),
                ..Default::default()
            },
            SuspendType::Ram => PowerStateChangeRequest {
                power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
                shutdown_type: ShutdownType::SUSPEND_TO_RAM.into(),
                ..Default::default()
            },
            SuspendType::Disk => PowerStateChangeRequest {
                power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
                shutdown_type: ShutdownType::SUSPEND_TO_DISK.into(),
                ..Default::default()
            },
        },
    };

    let result = client
        .get_sdv_vpm_vpm_system_service_client("VpmSystemServiceBundle")
        .set_power_state(request);

    if let Ok(response) = result {
        if response.error == PowerStateChangeRequestError::NO_ERROR.into() {
            info!("Request was sent successfully!");
        } else {
            error!("Request was invalid {:?}", response);
        }
    } else {
        error!("Failed to send power state change request, reason={:?}", result.err())
    }
    Ok(())
}

struct VePSM {}

impl PowerNotificationService for VePSM {
    fn notify_power_state(
        &self,
        request: PowerStateNotificationRequest,
    ) -> RpcResult<PowerStateNotificationResponse> {
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
    /// Set the power state to on
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
