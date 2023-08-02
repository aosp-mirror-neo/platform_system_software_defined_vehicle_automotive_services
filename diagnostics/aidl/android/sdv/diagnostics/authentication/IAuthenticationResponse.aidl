package android.sdv.diagnostics.authentication;

/**
 * Response values for responses of authentication handler callbacks.
 *
 * Corresponds to "AuthenticationReturnParameter" as per ISO 14229-1:2020 table B.5.
 */
interface IAuthenticationResponse {
    /** Request was successful. */
    const byte REQUEST_ACCEPTED = 0x00;

    /** Request was not successful. */
    const byte GENERAL_REJECTED = 0x01;

    /**
     * Indicates the provided authentication configuration as Authentication with PKI
     * Certificate Exchange (APCE).
     */
    const byte APCE_CONFIGURATION = 0x02;

    /**
     * Indicates the provided authentication configuration as Authentication with
     * Challenge-Response (ACR) and asymmetric cryptography.
     */
    const byte ACR_ASYMMETRIC_CONFIGURATION = 0x03;

    /**
     * Indicates the provided authentication configuration as Authentication with
     * Challenge-Response (ACR) and symmetric cryptography.
     */
    const byte ACR_SYMMETRIC_CONFIGURATION = 0x04;

    /** Deauthentication was successful, server is protected again. */
    const byte DEAUTHENTICATION_SUCCESSFUL = 0x10;

    /** Certificate could be verified in first step, second step is pending. */
    const byte CERTIFICATE_VERIFIED_OWNERSHIP_VERIFICATION_NECESSARY = 0x11;

    /** Proof of Ownership could be verified, Authentication is complete.*/
    const byte OWNERSHIP_VERIFIED_AUTH_COMPLETE = 0x12;

    /** Certificate could be verified.*/
    const byte CERTIFICATE_VERIFIED = 0x13;

    // 0x14 to 0x9F: Reserved.
    // 0xA0 to 0xCF: Vehicle manufacturer specific.
    // 0xD0 to 0xFE: System supplier specific.
    // 0xFF: Reserved.
}
