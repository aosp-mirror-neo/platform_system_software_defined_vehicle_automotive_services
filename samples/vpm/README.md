# VPM (Vehicle Mode and Power Management) Samples

This folder contains code samples for SDV VPM.

## VePSM (Vehicle Power State Manager)

*   [VePSM](./vepsm/)

Provides a sample binary that the OEM should implement in order to set
vehicle/power states in the SDV VM and receive power state reports.

## Vehicle State Client

Provides a sample SDV application that receives vehicle state updates from the
VPM (Vehicle Mode and Power Management) agent.

*   [Sample Vehicle State Listener](./vehicle_state_client/)

## Power State Client

Provides a sample SDV application that receives power state callbacks from the
VPM (Vehicle Mode and Power Management) agent. This is only intended for SDV
agents and not for OEM usage.

*   [Sample Power State Listener](./power_state_client/)

## Usage

1.  Launch Cuttlefish if not launched yet.

    ```bash
    cvd start
    ```

2.  The VPM agent starts with the init.rc in the SDV VM.

3.  Start the vehicle state client to start receiving updates for vehicle states

    ```bash
    adb shell

    vpm_vehicle_state_client

    or

    vpm_power_state_client
    ```

4.  Send new vehicle/power states using the VePSM binary.

    ```bash

    $adb shell

    $vepsm

    $vepsm vehicle-state low-power

    or

    $vepsm power-state power-on

    ```
