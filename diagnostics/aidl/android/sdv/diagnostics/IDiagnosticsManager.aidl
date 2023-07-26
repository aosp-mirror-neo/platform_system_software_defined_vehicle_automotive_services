package android.sdv.diagnostics;

import android.sdv.diagnostics.Format;
import android.sdv.diagnostics.data_item.DataItemDescription;
import android.sdv.diagnostics.data_item.IDataItemHandler;
import android.sdv.diagnostics.event.EventStatus;
import android.sdv.diagnostics.event.IFaultListener;
import android.sdv.diagnostics.io_control.IIOControlHandler;
import android.sdv.diagnostics.io_control.IOControlDescription;
import android.sdv.diagnostics.routine_control.IRoutineControlHandler;
import android.sdv.diagnostics.routine_control.RoutineControlDescription;

/**
 * Generally available Diagnostics APIs for use by SDV-Services.
 *
 * Allows normal (non-privileged) SDV-Services to expose diagnostics data via the
 * supported subset of UDS "services".
 *
 * Event / routine / data item / IO control ids are defined by SDV-Services. Diagnostics
 * manager implementation should be configured to handle those ids.
 *
 * Handlers and listeners unregistration will be supported in the future, after these APIs
 * are redefined in VIDL.
 *
 * Based on ISO 14229-1:2020 and AUTOSAR Diagnostic Event Manager standards.
 * See README for external links.
 *
 * TODO(@tantoshchuk): revisit after these APIs are redefined in VIDL.
 * **Common Error conditions**
 * - `ResponseCode::GENERAL_REJECT` - if the request fails for any reason.
 *
 * This APIs are experimental at this point.
 */
interface IDiagnosticsManager {
    /**
     * Report the status of an event.
     *
     * @param[in] id unique id of the event.
     * @param[in] status the status of the event.
     */
    void setEventStatus(in String id, in EventStatus status);

    /**
     * Registers a listener for event-related fault status.
     *
     * - the listener being registered is immediately notified about the current status of
     * faults it is subscribed to.
     * - multiple listeners from different SDV-Services can listen to the same fault.
     *
     * @param[in] ids ids of the events to register the listener for.
     * @param[in] listener listnener that gets notified on event's fault status changes.
     */
    void registerFaultListener(in String[] ids, in IFaultListener listener);

    /**
     * Set the state of an enable condition.
     *
     * If the enable condition is not fulfilled, reports of the events linked to the
     * condition are discarded. Links between the enable condition and events are defined
     * by the configuration of a diagnostics manager implementation.
     *
     * See AUTOSAR Diagnostics Event Manager spec, section 7.7.6.
     *
     * @param[in] id unique id of the enable condition.
     * @param[on] fulfilled true if this enable condition is fulfilled, false otherwise.
     */
    void setEnableCondition(in String id, in boolean fulfilled);

    /**
     * Commands for the operation cycle.
     */
    @Backing(type="byte")
    enum OperationCycleCommand {
        /**
         * Command to trigger operation cycle start.
         */
        START = 0x00u8,
        /**
         * Command to trigger operation cycle stop.
         */
        STOP = 0x01u8,
        /**
         * Command to trigger operation cycle restart.
         *
         * If RESTART is issued, any event reported in parallel will be on the operation
         * cycle (before or after restart).
         */
        RESTART = 0x02u8,
    }

    /**
     * Issue a command for an operation cycle.
     *
     * Operation cycle generally defines a time period of interest, e.g.:
     * - ignition on/off cycle
     * - power up / power down cycle
     * - engine warm up cycle
     * An event can be linked to operation cycle. An operation cycle then serves as the
     * basis for calculation of cycle-dependent event status information.
     *
     * See AUTOSAR Diagnostics Event Manager spec, section 7.6.
     * See ISO 14229-1:2020, table D.2.
     *
     * @param[in] id unique id of the operation cycle.
     * @param[in] command the command to apply to the operation cycle.
     */
    void setOperationCycle(in String id, in OperationCycleCommand command);

    /**
     * Register data items handler for read / write operations.
     *
     * Registers handler for ReadDataByIdentifier (UDS 0x22) and WriteDataByIdentifier
     * (UDS 0x2E) services. While UDS services support structured data to be referenced by
     * a single id, data item corresponds to a single primitive data point within
     * that structured data.
     *
     * See `IDataItemHandler` for more details.
     *
     * @param[in] descriptions descriptions of data items for which to register the handler.
     * @param[in] handler handler for data items read/write operations.
     */
    void registerDataItemHandler(
            in DataItemDescription[] descriptions, in IDataItemHandler handler);

    /**
     * Register handler for RoutineControl (UDS 0x31) service.
     *
     * @param[in] descriptions descriptions of routines for which to register the handler.
     * @param[in] handler handler for routine operations.
     *
     * See `IRoutineControlHandler` for more details.
     */
    void registerRoutineControlHandler(
            in RoutineControlDescription[] descriptions, in IRoutineControlHandler handler);

    /**
     * Register handler for IOControlByIdentifier (UDS 0x2F) service.
     *
     * @param[in] descriptions descriptions of IO control identifiers for which to
     * register the handler.
     * @param[in] handler handler for IO controlidentifiers operations.
     *
     * See `IIOControlHandler` for more details.
     */
    void registerIOControlHandler(
            in IOControlDescription[] descriptions, in IIOControlHandler callback);

    /**
     * Register caller-related configuration data.
     *
     * This is a temporary solution to allow for configuration updates at service update
     * time.
     *
     * Long-term this would be a responsibility of Update component to update Diagnostics
     * Manager configuration at service update time.
     *
     * @param[in] configurationBlob an opaque blob of vendor-specific configuration data.
     */
    void registerServiceConfiguration(in byte[] configurationBlob);
}
