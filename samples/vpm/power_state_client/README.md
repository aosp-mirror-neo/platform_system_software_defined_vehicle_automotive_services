# Sample Power State Listener

The sample SDV application receives power state updates from the VPM (Vehicle
Mode and Power Management) agent.

The binary implements AIDL callbacks for power state information for SDV agents.

***Warning:*** The current APIs are not final ones and pending API council
approval.

**Usage**: To start receiving the power state updates in the SDV VM

1.  Launch Cuttlefish if not launched yet.

    ```bash
    cvd start
    ```

2.  Start the binary.

    ```bash
    adb shell

    vpm_power_state_client
    ```

3.  The output should look like this, if a new power state was sent via VePSM

    ```text
    VPM client received power state change
    ```
