<!-- Copyright 2024 Google LLC -->

# Sample Vehicle State Listener

The sample SDV application receives vehicle state updates from the VPM (Vehicle
Mode and Power Management) agent.

The binary implements a subscription for the VSIDL interface using Data Tunnel
communication to exchange vehicle state information defined by SDV VPM (Vehicle
Mode and Power Management).

```text
package: "com.sdv.google.vpm.vehicle.client.subscriber"

sdv_service_bundle {
    name: "VehicleStateSubscriber"
    # Subscriber to the vehicle state updates.
    subscriber {
        type: "com.sdv.google.vpm.vehicle.VehicleStateChange"
    }
}

```

Middleware generated code is used for implementing the VSIDL interface and can
be found in the `./generated_rs` folder. The code was generated using the
`vsidlc` as follows:

```bash
vsidlc --catalog-path vpm/stable/vsidl/  --output-path vpm/
```

Only the generated code for the vehicle state subscription is used in this
binary.

***Warning:*** The current set of vehicle states are not final, and are pending
OEM clarifications and API council approval.

**Usage**: To start receiving the vehicle state updates in the SDV VM

1.  Launch Cuttlefish if not launched yet.

    ```bash
    cvd start
    ```

2.  Start the binary.

    ```bash
    adb shell

    vpm_vehicle_state_client
    ```

3.  The output should look like this, if a new vehicle state was sent via VePSM

    ```text
    Received vehicle state LOW_POWER
    ```
