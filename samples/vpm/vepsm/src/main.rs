// Copyright 2023 Google LLC
//! Vehicle and Power Mode Service.

use clap::Parser;
use clap::ValueEnum;
use log::info;
use sdv_vpm_api::vehicle::VpmVehicleState;
use sdv_vpm_api::vpm::SetVehicleStateRequest;
use sdvgenerated::vpm_system_client::VpmSystemClient;
use sdvgenerated::vpm_system_service_interface::VpmSystemService;

static LOG_TAG: &str = "vepsm";

#[derive(Parser)]
struct Cli {
    /// The new state of the vehicle
    vehicle_state: VehicleStateArgs,
}

fn main() -> anyhow::Result<()> {
    sdv_log::init_logger(LOG_TAG)?;
    let args = Cli::parse();
    info!("VePSM client starting");
    let client = VpmSystemClient::new();
    let vehicle_state: VpmVehicleState = args.vehicle_state.into();
    let request =
        SetVehicleStateRequest { vehicle_state: vehicle_state.into(), ..Default::default() };
    client.vpm_system_service_client.set_vehicle_state(request)?;
    println!("Vehicle mode set successfully {:?}", vehicle_state);
    Ok(())
}

#[derive(ValueEnum, Clone)]
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
