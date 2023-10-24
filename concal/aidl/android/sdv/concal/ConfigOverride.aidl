package android.sdv.concal;

import android.sdv.concal.ConfigId;
import android.sdv.concal.ConfigOverrideKeyValue;

/**
 * Format for ConfigOverride messages.
 * See `configuration.proto`.
 *
 * This is a PoC version for the API for the discussions.
 * It is unstable until API council approval.
 */
parcelable ConfigOverride {
    /**
     * Override identifier defined by the caller.
     * Unique per caller.
     */
    String overrideId;

    /**
     * Unique config identifier.
     * If config override should be applied to multiple configs, some fields in the config_id might
     * be undefined.
     */
    ConfigId configId;

    /**
     * Key-value pairs for keys to be updated in the config.
     */
    ConfigOverrideKeyValue[] pairs;
}
