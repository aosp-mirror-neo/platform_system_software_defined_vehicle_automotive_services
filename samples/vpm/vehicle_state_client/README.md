# Sample Vehicle State Listener

The sample SDV application receives vehicle state updates from the VPM (Vehicle Mode and Power Management) agent.

The binary implements a subscription for the VSIDL interface using Data Tunnel communication to exchange vehicle state information defined by SDV VPM (Vehicle Mode and Power Management).

```text
package: "sdv.vpm.vehicle.client.subscriber"

sdv_service_bundle {
    name: "VehicleStateSubscriber"
    subscriber:  "sdv.vpm.vehicle.VehicleStateChange::VehicleStateChange"
}
```

Middleware generated code is used for implementing the VSIDL interface and can be found in the ```./generated``` folder. The code was generated using the ```vsidl_code_generator``` as follows:

```bash
vsidl_code_generator --vsidl-path core_services/vsidl/protos/  --catalog-path automotive_services/vpm/vsidl/  --output-path .
```

Only the generated code for the vehicle state subscription is used in this binary.

**_Warning:_** The current set of vehicle states are not the final ones, and are pending OEM clarifications and API council approval.

**Usage**: To start receiving the vehicle state updates in the SDV VM

1. Launch Cuttlefish if not launched yet.

   ```bash
   cvd start --guest-enforce-security=false
   ```

2. Start the binary.

   ```bash
   adb shell

   vpm_vehicle_state_client
   ```

3. The output should look like this, if a new vehicle state was sent via VePSM

   ```text
   Received vehicle state LOW_POWER
   ```
