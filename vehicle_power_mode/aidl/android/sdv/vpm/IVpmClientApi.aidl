// Copyright 2023 Google LLC. All Rights Reserved.

package android.sdv.vpm;

import android.sdv.vpm.ICustomStateListener;
import android.sdv.vpm.IPowerStateListener;
import android.sdv.vpm.PowerState;

/**
 * Experimental SDV VPM Client API.
 *
 * Unstable until API council approval.
 *
 * Interface for the VPM client.
 *
 * This interface allows the clients to subscribe to the power state machine and any other custom
 * state machine that is sent to the VPM via IVPMSystemApi.
 *
 */

interface IVpmClientApi {
    const int SERVICE_PORT = 2222;

    const String SERVICE_NAME = "vpm_client_api";

    /** Get the current power state */
    PowerState getPowerState();

    /**
     * Subscribe to the power state updates.
     * @param[in] IPowerStateListener the callback interface for the power state listener.
     */
    void subscribeToPowerStateChange(IPowerStateListener listener);

    /**
     * Subscribe to any custom state machine updates.
     * The call returns Ok if there was a value published on the customStateMachine otherwise
     * returns Err.
     * @param[in] stateMachineName the name of the custom state machine to subscribe to.
     * @param[in] ICustomStateListener the callback interface for the custom state machine.
     */
    void subscribeToCustomStateChange(String stateMachineName, ICustomStateListener listener);
}
