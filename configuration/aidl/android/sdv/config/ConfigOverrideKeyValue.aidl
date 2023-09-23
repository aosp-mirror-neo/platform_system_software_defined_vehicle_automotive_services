package android.sdv.config;

import android.sdv.config.ConfigValue;

/**
 * Format for parameter-value pair for ConfigOverride messages.
 * See `configuration.proto`.
 *
 * This is a PoC version for the API for the discussions.
 * It is unstable until API council approval.
 */
parcelable ConfigOverrideKeyValue {
    /**
     * The name of the key (configuration parameter).
     */
    String key;

    /**
     * New value of the key.
     */
    ConfigValue value;
}
