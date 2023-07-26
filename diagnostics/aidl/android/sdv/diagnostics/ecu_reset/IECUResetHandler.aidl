// Copyright 2023 Google LLC. All Rights Reserved.

package android.sdv.diagnostics.ecu_reset;

/**
 * Handler for ECUReset (UDS 0x11) service.
 *
 * See ISO 14229-1:2020, section 10.3.
 */
interface IECUResetHandler {
    /**
     * Identifies a "hard reset" condition which simulates the power-on/start-up sequence.
     *
     * @param[in] connectionId see `DiagnosticsManagerPlatform#getConnectionMetadata`.
     */
    void hardReset(in int connectionId);

    /**
     * Identifies a condition similar to the driver turning the ignition key off and back
     * on.
     *
     * @param[in] connectionId see `DiagnosticsManagerPlatform#getConnectionMetadata`.
     */
    void keyOnOffReset(in int connectionId);

    /**
     * Identifies a "soft reset" condition, which causes the server to immediately restart
     * the application program if applicable.
     *
     * @param[in] connectionId see `DiagnosticsManagerPlatform#getConnectionMetadata`.
     */
    void softReset(in int connectionId);

    /**
     * Applies to ECUs which a not ignition powered but battery powered only.
     *
     * Shutdown forces the sleep mode rather than a power off.
     *
     * @param[in] connectionId see `DiagnosticsManagerPlatform#getConnectionMetadata`.
     * @return the minimum time of the stand-by-sequence the server will remain in the
     *         power down sequence.
     *         The resolution of this parameter is one second per count.
     *         0x00 to 0xFE: 0 to 254 seconds power down time.
     *         0xFF: indicates a failure or time not available.
     */
    byte enableRapidPowerShutdown(in int connectionId);

    /**
     * Requests to disable the previously enabled "rapid power shut down".
     *
     * @param[in] connectionId see `DiagnosticsManagerPlatform#getConnectionMetadata`.
     */
    void disableRapidPowerShutdown(in int connectionId);

    /**
     * Performs vehicle manufacturer / system supplier specific reset.
     *
     * @param[in] connectionId see `DiagnosticsManagerPlatform#getConnectionMetadata`.
     * @param[in] resetType reset type id
     *            0x40 to 0x5F: vehicle manufacturer specific reset;
     *            0x60 to 0x75: system supplier specific reset.
     */
    void customReset(in int connectionId, in byte resetType);
}
