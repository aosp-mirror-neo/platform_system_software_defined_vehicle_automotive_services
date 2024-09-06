// Copyright 2023 Google LLC

package google.sdv.vpm;

import google.sdv.vpm.IPowerStateReportListener;
import google.sdv.vpm.PowerStateReport;

/**
 * Experimental SDV VPM Client API.
 *
 * Unstable until API council approval.
 *
 * Interface for the VPM client.
 *
 * This interface allows the clients to subscribe to the power state reports.
 *
 */

interface IPowerStateClientApi {
    /**
     * Subscribe to the power state updates.
     * @param[in] IPowerStateListener the callback interface for the power state listener.
     * @param[in] IBinder link to death for callback agent.
     * @return PowerStateReport the current power state report.
     */
    PowerStateReport subscribeToPowerStateReport(
            IPowerStateReportListener listener, IBinder linkToDeath);
}
