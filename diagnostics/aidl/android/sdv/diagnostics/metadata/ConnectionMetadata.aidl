package android.sdv.diagnostics.metadata;

/**
 * Metadata of the underlying diagnostics connection.
 *
 * This data is retrieved from the UDS payload that triggered a handler callback.
 */
parcelable ConnectionMetadata {
    /**
     * Application layer source address.
     *
     * See ISO 14229-1:2020 section 7.4.1.3.
     */
    char sourceAddress;

    /**
     * Application layer target address.
     *
     * See ISO 14229-1:2020 section 7.4.1.4.
     */
    char targetAddress;
}
