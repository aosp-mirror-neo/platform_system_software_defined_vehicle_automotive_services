# VePSM(Vehicle Power State Manager)

This is a sample binary for what the OEM should implement in order to set vehicle/power states or listen to power state machine updates in the SDV VM.

The binary implements the RPC client VSIDL interface defined by SDV VPM (Vehicle Mode and Power Management) agent to set the vehicle state.

```text
package: "sdv.vpm.vehicle.client.rpc"

sdv_service_bundle {
    name: "VpmSystemClient"
    client:  "sdv.vpm.vehicle.VpmSystemService"
}
```

Middleware generated code is used for implementing the VSIDL interface and can be found in the ``` ./generated``` folder. The code was generated using the ```vsidl_code_generator``` as follows:

```bash
vsidl_code_generator --vsidl-path core_services/vsidl/protos/  --catalog-path automotive_services/vpm/vsidl/  --output-path .
```

Only the generated code for the vehicle state RPC client is used in this binary.

Currently the binary supports setting vehicle states. Power state management will be added in a later stage.

**_Warning:_** The current set of vehicle states are not the final ones, its pending OEM clarifications and API council approval.

**Usage**: To set the vehicle states in the SDV VM. The command ```vepsm``` takes one argument which is the vehicle state.

```bash
# vepsm --help
Usage: vepsm <VEHICLE_STATE>

Arguments:
  <VEHICLE_STATE>  The new state of the vehicle [possible values: low-power, software-update, park, life-on-board, vehicle-on, traction-on]

Options:
  -h, --help  Print help

```

1. Launch Cuttlefish if not launched yet.

   ```bash
   cvd start --guest-enforce-security=false
   ```

2. Start the binary with a specific vehicle mode.

   ```bash
   adb shell

   vepsm low-power
   ```

3. The output should look like this

   ```text
   Vehicle mode set successfully LOW_POWER
   ```

When the command is executed, the VPM agent will receive the RPC request and notify all subscribers of the vehicle state ```low_power```.
