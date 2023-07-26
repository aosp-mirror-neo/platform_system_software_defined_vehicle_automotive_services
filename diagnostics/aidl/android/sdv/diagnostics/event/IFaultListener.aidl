package android.sdv.diagnostics.event;

import android.sdv.diagnostics.event.FaultStatus;

/**
 * Listener for event-related fault status change.
 *
 * See `IDiagnosticsManager#registerEventListener`.
 */
interface IFaultListener {
    /**
     * Callback for event's fault status change.
     *
     * This callback is triggered when fault status changes.
     * See `FaultStatus` for details.
     *
     * @param[in] id id of the event.
     * @param[in] status new status of the event's fault.
     */
    void onFaultStatusChange(in String id, in FaultStatus status);
}
