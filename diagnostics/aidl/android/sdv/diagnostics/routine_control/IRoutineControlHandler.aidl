// Copyright 2023 Google LLC
package android.sdv.diagnostics.routine_control;

/**
 * Handler for the RoutineControl (UDS 0x31) service.
 *
 * Formats of the raw data (i.e. `byte[]` methods params/return values) that this
 * interface operates on are provided via
 * `DiagnosticsManager::registerRoutineControlHandler`.
 *
 * See ISO 14229-1:2020, section 14.2.
 *
 * ** Common error codes **
 * See ISO 14229-1:2020, section 14.2.4.
 */
interface IRoutineControlHandler {
    /**
     * Start the routine specified by the id.
     *
     * @param[in] id routine identifier.
     * @param[in] requestData parameters of the routine start request.
     * @return additional information about the status of the server following the start
     *         of the routine.
     */
    byte[] start(in String id, in byte[] requestData);

    /**
     * Stop the routine specified by the id.
     *
     * @param[in] id routine identifier.
     * @param[in] requestData parameters of the routine stop request.
     * @return additional information about the status of the server after the routine has
     *         been stopped.
     */
    byte[] stop(in String id, in byte[] requestData);

    /**
     * Return result values of the routine specified by the id.
     *
     * @param[in] id routine identifier.
     * @param[in] requestData parameters of the routine results request.
     * @return results of the routine, which has been stopped previoiusly.
     */
    byte[] requestResult(in String id, in byte[] requestData);
}
