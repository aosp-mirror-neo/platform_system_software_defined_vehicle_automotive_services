// Copyright 2023 Google LLC. All Rights Reserved.

package android.sdv.vpm;

import android.sdv.vpm.PowerState;

/**
 * Experimental SDV VPM System API.
 *
 * Unstable until API council approval.
 *
 * This interface allows the OEM agents to publish new power states and new custom state machines
 * with their corresponding values.
 *
 */
interface IVpmSystemApi {
    const int SERVICE_PORT = 1111;

    const String SERVICE_NAME = "vpm_system_api";

    /**
     * Set the power state of the VPM.
     * @param[in] state the value or state name of the power state machine.
     */
    PowerState setPowerState(PowerState state);

    /**
     * Set the value of a custom state machine.
     * @param[in] stateMachineName the name of the custom state machine.
     * @param[in] stateMachineName the value or state name for the custom state machine.
     */
    String setCustomState(String stateMachineName, String stateMachineValue);
}
