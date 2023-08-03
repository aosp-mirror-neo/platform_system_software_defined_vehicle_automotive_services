// Copyright 2023 Google LLC. All Rights Reserved.

package android.sdv.vpm;

/**
 * Experimental SDV VPM Client API.
 *
 * Unstable until API council approval.
 *
 * Callback interface for custom state machines' updates.
 *
 */
oneway interface ICustomStateListener {
    /**
     * Callback method when a custom state machine is changed.
     * @param[in] stateMachineName the name of the custom state machine.
     * @param[in] stateMachineValue description of this parameter.
     */
    void onCustomStateChanged(String stateMachineName, String stateMachineValue);
}
