// Copyright 2023 Google LLC. All Rights Reserved.

package android.sdv.concal;

import android.sdv.concal.Config;
import android.sdv.concal.ConfigId;
import android.sdv.concal.ConfigOverride;
import android.sdv.concal.ConfigVersion;

/**
 * Generally available Configuration/Calibration Service APIs for use by SDV-Services.
 *
 * Allows SDV-Services to deliver customized configuration based on the vehicle options and factors.
 *
 * This is a PoC version for the API for the discussions.
 * It is unstable until API council approval.
 */
interface IConfigurationService {
    // Configuration functionality.

    /**
     * Retrieves service configuration.
     *
     * @param[in] configId is a unique config identifier.
     *
     * @return current service configuration.
     */
    Config getConfig(in ConfigId configId);

    /**
     * Registers configuration schema with default config.
     * If the metadata for this version already exists the metadata is not
     * rewritten.
     * @param[in] configId is a unique config identifier.
     * @param[in] config is a default configuration in the form of a serialized protobuf message,
     * where the parameters are assigned with default values and its schema.
     * This parameter is not fully aligned with the VSIDL version.
     * @param[in] configVersion is a version identifier for the service.
     * If the metadata for this version already exists the metadata is not rewritten.
     */
    void registerConfigMetadata(in ConfigId configId, in byte[] metadata, in String configVersion);

    /**
     * Returns the config metadata for the service.
     *
     * @param[in] configId is a unique config identifier.
     * @param[in] configVersion is a version identifier for the service.
     *
     * @return config schema and default message.
     */
    Config getConfigMetadata(in ConfigId configId, in ConfigVersion configVersion);

    /**
     * Starts a configuration update.
     * No updates can run in parallel.
     * The method will return error if another update session is in progress.
     * Each update can include multiple overrides.
     * This API should be only available to the privileged services like UpdateManager.
     *
     * @return identifier of the configuration update which is a random number existing only during
     *         the update transaction.
     */
    int startConfigUpdate();

    /**
     * Adds or updates a config override for a specific config.
     * Dependent on the sender of the message, configuration service should assign a priority to it.
     * Executed before StartUpdate() and StopUpdate().
     * This API should be only available to the privileged services like UpdateManager.
     *
     * @param[in] updateId is an identifier of the configuration update existing only during the
     * update transaction..
     * @param[in] configId is a unique config identifier.
     * @param[in] configOverride is a serialized ConfigOverride protobuf message.
     */
    void addOrUpdateConfigOverride(
            in int updateId, in ConfigId configId, in ConfigOverride[] configOverride);

    /**
     * Removes a config override for specific config.
     * This API should be only available to the privileged services like UpdateManager.
     *
     * @param[in] updateId is an identifier of the configuration update existing only during the
     * update transaction.
     * @param[in] overrideId is a unique override identifier.
     * @param[in] configId is a unique config identifier.
     */
    void removeConfigOverride(in int updateId, in String overrideId, in ConfigId configId);

    /**
     * Cancels the current update. All ConfigOverrides sent after the last StartUpdate() and now()
     * This API should be only available to the privileged services like UpdateManager.
     * are reverted.
     *
     * @param[in] updateId is an identifier of the configuration update existing only during the
     * update transaction.
     */
    void cancelConfigUpdate(in int updateId);

    /**
     * Marks the end of configuration update.
     * A new configuration snapshot is calculated and stored persistently.
     * This API should be only available to the privileged services like UpdateManager.
     *
     * @param[in] updateId is an identifier of the configuration update existing only during the
     * update transaction.
     */
    void finishConfigUpdate(in int updateId);

    /**
     * Rollbacks the last update to the previous configuration snapshot.
     * This API should be only available to the privileged services like UpdateManager.
     * Can't be performed while another update is in progress.
     *
     * @param[in] updateId is a unique identifier for update, until which the rollback
     * should be performed.
     */
    void rollback(in int updateId);

    /**
     * Removes all configuration updates and default configurations.
     * This API should be only available to the privileged services like UpdateManager.
     * Cleans all services up to the factory state.
     * Can't be performed while another update is in progress.
     *
     * @param[in] configId is a unique config identifier.
     */
    void reset(in ConfigId configId);

    /**
     * Removes a configuration.
     * Called when a service is deinstalled.
     * This API should be only available to the privileged services like UpdateManager.
     * Can't be performed while another update is in progress.
     *
     * @param[in] configId is a unique config identifier.
     */
    void deinstallConfig(in ConfigId configId);

    // Calibration functionality.
    // Should be disabled for the user builds.

    /**
     * Start calibration for a specified config.
     *
     * @param[in] configId is a unique identifier for config whose service is calibrated.
     * @param[in] config is a configuration in the form of a serialized protobuf message.
     *
     * @return calibration callback identifier.
     */
    void startCalibration(in ConfigId configId, in byte[] config);

    /**
     * Removes a previously registered calibration callback.
     *
     * @param[in] configId is a unique config identifier.
     */
    void stopCalibration(in ConfigId configId);
}
