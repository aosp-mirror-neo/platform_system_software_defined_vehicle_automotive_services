// Copyright 2023 Google LLC. All Rights Reserved.

package android.sdv.config;

import android.sdv.config.ConfigId;

/**
 * Calibration callback.
 *
 * Service can register callback on configuration changes.
 * Implementation of this API should not be available in the user builds.
 *
 * This is a PoC version for the API for the discussions.
 * It is unstable until API council approval.
 */
interface ICalibrationCallback {
    /**
     * Called when configuration changes.
     * @param[in] configId is a unique config identifier.
     * @return changed config.
     */
    byte[] onConfigChange(in ConfigId configId);
}
