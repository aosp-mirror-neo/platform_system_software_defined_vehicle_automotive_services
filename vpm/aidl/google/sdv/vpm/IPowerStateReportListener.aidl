// Copyright 2024 Google LLC

package google.sdv.vpm;

import google.sdv.vpm.PowerStateReport;

/**
 * Experimental SDV VPM Client API.
 * Unstable until API council approval.
 *
 * Callback interface for power state machine updates.
 *
 * The VPM will wait on the execution of onPowerStateReportChanged method.
 *
 * Critical changes caused by power state changes should be executed before the return on this
 * callback.
 *
 * The VPM will have a timeout for the callback execution.
 *
 * Subsequent callbacks can be received sequentially, the client should cancel/interrupt the older
 * callbacks and execute the more recent one.
 */
interface IPowerStateReportListener {
    /**
     * Callback method when the power state report has changed.
     * @param[in] PowerStateReport the new power state report.
     */
    void onPowerStateReportChanged(PowerStateReport PowerStateReport);
}
