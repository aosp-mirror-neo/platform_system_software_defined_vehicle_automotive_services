package android.sdv.config;

import android.sdv.config.ConfigId;
import android.sdv.config.ConfigOverrideKeyValue;

/**
 * Format for ConfigOverride messages.
 * See `configuration.proto`.
 *
 * This is a PoC version for the API for the discussions.
 * It is unstable until API council approval.
 */
parcelable ConfigOverride {
    /**
     * Operator string in the form (factor=value) AND/OR (option!=value).
     * If rule is not provided, true is used.
     */
    @nullable String rule;

    /**
     * Unique config identifier.
     */
    ConfigId configId;

    /**
     * Key-value pairs for keys to be updated in the config.
     */
    ConfigOverrideKeyValue[] pair;
}
