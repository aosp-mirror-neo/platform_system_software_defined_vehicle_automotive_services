// Copyright 2023 Google LLC. All Rights Reserved.

package android.sdv.vpm;

import android.sdv.vpm.PowerState;

/**
 * Experimental SDV VPM Client API.
 *
 * Unstable until API council approval.
 *
 * Callback interface for power state machine updates.
 *
 */
oneway interface IPowerStateListener {
    /**
     * Callback method when the power state has changed.
     * @param[in] PowerState the new power state.
     */
    void onPowerStateChanged(PowerState powerState);
}
