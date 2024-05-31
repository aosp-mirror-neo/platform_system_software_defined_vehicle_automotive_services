// Copyright 2023 Google LLC
//! Vehicle and Power Mode Service.

use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;
use log::info;
use sdv_vpm_api::vehicle::VpmVehicleState;
use sdv_vpm_api::vpm::PowerStateChangeRequest;
use sdv_vpm_api::vpm::PowerStateRequest;
use sdv_vpm_api::vpm::SetVehicleStateRequest;
use sdv_vpm_api::vpm::ShutdownType;
use sdvgenerated::sdv_vpm_vpm_system_service_interface::VpmSystemService;
use sdvgenerated::vpm_system_client::VpmSystemClient;

static LOG_TAG: &str = "vepsm";

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
    /// Suspend the VM to ram or disk
    /// Prepare the VM for suspension or shutdown.
    Prepare {
        /// Type of preparation.
        prepare_type: SuspendType,
    },
    /// Cancel the preparation for suspension or shutdown.
    Cancel,
    /// Turn the VM off.
    PowerOff {
        /// Shutdown type of preparation.
        shutdown_type: SuspendType,
    },
}

/// The preparation or suspension type.
#[derive(ValueEnum, Debug, Clone)]
pub enum SuspendType {
    /// Shutdown the VM.
    Off,
    /// Suspend the VM to RAM.
    Ram,
    /// Suspend the VM to DISK.
    Disk,
}

fn main() -> anyhow::Result<()> {
    sdv_log::init_logger(LOG_TAG)?;
    let args = Cli::parse();
    info!("VePSM client starting");
    let client = VpmSystemClient::new();

    let command: VpmCommand = args.command;
    match command {
        VpmCommand::VehicleState(vehicle_state) => {
            let request = SetVehicleStateRequest {
                vehicle_state: <VehicleStateArgs as std::convert::Into<VpmVehicleState>>::into(
                    vehicle_state.clone(),
                )
                .into(),
                ..Default::default()
            };
            client
                .get_sdv_vpm_vpm_system_service_client("VpmSystemServiceBundle")
                .set_vehicle_state(request)?;
            println!("Vehicle mode set successfully {:?}", vehicle_state);
        }
        VpmCommand::PowerState(cmd) => {
            let request = match cmd {
                PowerCommand::PowerOn => PowerStateChangeRequest {
                    power_request: PowerStateRequest::TURN_ON.into(),
                    ..Default::default()
                },
                PowerCommand::Cancel => PowerStateChangeRequest {
                    power_request: PowerStateRequest::CANCEL_SHUTDOWN.into(),
                    ..Default::default()
                },
                PowerCommand::PowerOff { shutdown_type } => match shutdown_type {
                    SuspendType::Off => PowerStateChangeRequest {
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
                    SuspendType::Off => PowerStateChangeRequest {
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

            client
                .get_sdv_vpm_vpm_system_service_client("VpmSystemServiceBundle")
                .set_power_state(request)?;
        }
    }

    Ok(())
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
