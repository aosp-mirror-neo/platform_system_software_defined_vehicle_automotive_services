package android.sdv.diagnostics.authentication;

/**
 * Return value for `IAuthenticationHandler#verifyProofOfOwnership`.
 */
parcelable VerifyProofOfOwnershipResponse {
    /**
     * See `IAuthenticationResponse`.
     */
    byte authenticationResponse;

    /**
     * Indicates the algorithm used in the generating and verifying proof of ownership.
     *
     * Format of this field is OEM-specific.
     */
    byte[] algorithmIndicator;

    /**
     * This value shall contain session key information, e.g. the encrypted session key(s)
     * for securing further communication in the actual session and/or proof value(s)
     * (e.g. a hash value) for the validation of the session key(s) on the client side.
     *
     * Non-null if secure diagnostics communication is in use.
     *
     * Format of this field is OEM-specific.
     */
    @nullable byte[] sessionKeyInfo;
}
