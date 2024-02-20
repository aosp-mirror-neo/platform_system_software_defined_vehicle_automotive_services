# VPM (Vehicle Mode and Power Management) Samples

This folder contains code samples for SDV VPM.

## VePSM (Vehicle Power State Manager)

* [VePSM](./vepsm/)

Provides a sample binary that the OEM should implement in order to set vehicle/power states in the SDV VM.

## Vehicle State Client

Provides a sample SDV application that receives vehicle state updates from the VPM (Vehicle Mode and Power Management) agent.

* [Sample Vehicle State Listener](./vehicle_state_client/)

### Usage

1. Launch Cuttlefish if not launched yet.

   ```bash
   cvd start --guest-enforce-security=false
   ```

2. The VPM agent starts with the init.rc in the SDV VM.

3. Start the vehicle state client to start receiving updates for vehicle states

   ```bash
   adb shell

   vpm_vehicle_state_client
   ```

4. Send new vehicle states using the VePSM binary.

   ```bash
   adb shell

   vepsm low-power
   ```
