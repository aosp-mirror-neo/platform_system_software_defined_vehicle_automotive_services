// Copyright 2023 Google LLC
package android.sdv.diagnostics.authentication;

/**
 * Return value for `IAuthenticationHandler#requestChallengeForAuthentication`.
 */
parcelable RequestChallengeResponse {
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
     * The challenge contains vehicle manufacturer specific formatted server data or is a
     * random number.
     *
     * Format of this field is OEM-specific.
     */
    byte[] challenge;

    /**
     * Indicate what additional parameters, if needed, are expected by the server.
     *
     * Format of this field is OEM-specific.
     */
    @nullable byte[] neededAdditionalParameter;
}
