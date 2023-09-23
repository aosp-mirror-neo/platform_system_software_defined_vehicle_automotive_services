package android.sdv.config;

import android.sdv.config.ConfigVersion;

/**
 * Return value for `IConfigurationService#getConfig`, `IConfigurationService#getConfigMetadata`.
 * Dependent on the call it returns default configuration with the default values or
 * current service configuration.
 *
 * This is a PoC version for the API for the discussions.
 * It is unstable until API council approval.
 */
parcelable Config {
    /**
     * Service configuration in the form of a serialized protobuf message.
     */
    byte[] config;

    /**
     * Version id of the returned configuration.
     */
    ConfigVersion configVersion;
}
