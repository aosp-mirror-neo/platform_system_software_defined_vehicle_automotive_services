// Copyright 2023 Google LLC
package android.sdv.diagnostics.authentication;

/**
 * Return value for `IAuthenticationHandler#proofOfOwnership`.
 */
parcelable ProofOfOwnershipResponse {
    /**
     * See `IAuthenticationResponse`.
     */
    byte authenticationResponse;

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
