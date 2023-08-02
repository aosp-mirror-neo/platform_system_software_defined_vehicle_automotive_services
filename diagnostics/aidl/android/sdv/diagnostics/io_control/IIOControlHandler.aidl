package android.sdv.diagnostics.io_control;

/**
 * Handler for the InputOutputControlByIdentifier (UDS 0x2F) service.
 */
interface IIOControlHandler {
    /**
     * Request that indicates that the client no longer has control about the input
     * signals, internal parameters and/or output signals referenced by the identifier.
     *
     * @param[in] id identifies a server local input signals, internal parameters and/or
     *            output signals.
     * @param[in] controlMask in case identifier references more than one parameter,
     *            specifies which of referenced parameters are affected.
     * @return identifier's data record.
     */
    byte[] returnControlToEcu(in String id, in byte[] controlMask);

    /**
     * Request to reset the input signals, internal parameters and/or output signals
     * referenced by the identifier to its default state.
     *
     * @param[in] id identifies a server local input signals, internal parameters and/or
     *            output signals.
     * @param[in] controlMask in case identifier references more than one parameter,
     *            specifies which of referenced parameters are affected.
     * @return identifier's data record.
     */
    byte[] resetToDefault(in String id, in byte[] controlMask);

    /**
     * Request to freeze the current state of the input signals, internal parameters
     * and/or output signals referenced by the identifier.
     *
     * @param[in] id identifies a server local input signals, internal parameters and/or
     *            output signals.
     * @param[in] controlMask in case identifier references more than one parameter,
     *            specifies which of referenced parameters are affected.
     * @return identifier's data record.
     */
    byte[] freezeCurrentState(in String id, in byte[] controlMask);

    /**
     * Request to adjust the current state of the input signals, internal parameters
     * and/or output signals referenced by the identifier in RAM to the values included in
     * the controlState parameter.
     *
     * @param[in] id identifies a server local input signals, internal parameters and/or
     *            output signals.
     * @param[in] controlState adjusted state of controls referenced by the identifier.
     * @param[in] controlMask in case identifier references more than one parameter,
     *            specifies which of referenced parameters are affected.
     * @return identifier's data record.
     */
    byte[] shortTermAdjustment(in String id, in byte[] controlState, in byte[] controlMask);
}
