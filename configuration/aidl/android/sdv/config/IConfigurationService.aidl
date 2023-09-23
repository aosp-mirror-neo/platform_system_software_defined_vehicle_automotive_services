// Copyright 2023 Google LLC. All Rights Reserved.

package android.sdv.config;

import android.sdv.config.Config;
import android.sdv.config.ConfigId;
import android.sdv.config.ConfigOverride;
import android.sdv.config.ICalibrationCallback;

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
     * The service can decide on its own, if it wants to add instance id or not.
     *
     * @param[in] configId is a unique config identifier.
     *
     * @return current service configuration.
     */
    Config getConfig(in ConfigId configId);

    /**
     * Registers default service configuration.
     * TODO: consider renaming to getDefaultConfig.
     *
     * @param[in] configId is a unique config identifier.
     * @param[in] config is a default configuration in the form of a serialized protobuf message,
     * where the parameters are assigned with default values.
     * @param[in] version is a version identifier for the service.
     * If the metadata for this version already exists the metadata is not rewritten.     *
     */
    void registerConfigMetadata(in ConfigId configId, in byte[] config, in String version);

    /**
     * Returns the config metadata for the service.
     *
     * @param[in] configId is a unique config identifier.
     *
     * @return default configuration.
     */
    Config getConfigMetadata(in ConfigId configId);

    /**
     * Starts a configuration update.
     * No updates can run in parallel.
     * The method will return error if another update session is in progress.
     * Each update can include multiple overrides.
     * This API should be only available to the privileged services like UpdateManager.
     *
     * @return identifier of the configuration update.
     */
    int startConfigUpdate();

    /**
     * Adds a config override for a specific config.
     * Dependent on the sender of the message, configuration service should assign a priority to it.
     * Executed before StartUpdate() and StopUpdate().
     * This API should be only available to the privileged services like UpdateManager.
     *
     * @param[in] updateId is an identifier of the configuration update.
     * @param[in] configId is a unique config identifier.
     * @param[in] configOverride is a serialized ConfigOverride protobuf message.
     */
    void addConfigOverride(
            in int updateId, in ConfigId configId, in ConfigOverride[] configOverride);

    /**
     * Removes a config override for specific config.
     * This API should be only available to the privileged services like UpdateManager.
     *
     * @param[in] updateId is an identifier of the configuration update.
     * @param[in] configId is a unique config identifier.
     */
    void removeConfigOverride(in int updateId, in ConfigId configId);

    /**
     * Cancels the current update. All ConfigOverrides sent after the last StartUpdate() and now()
     * This API should be only available to the privileged services like UpdateManager.
     * are reverted.
     *
     * @param[in] updateId is an identifier of the configuration update.
     */
    void cancelConfigUpdate(in int updateId);

    /**
     * Marks the end of configuration update.
     * A new configuration snapshot is calculated and stored persistently.
     * This API should be only available to the privileged services like UpdateManager.
     *
     * @param[in] updateId is an identifier of the configuration update.
     */
    void stopConfigUpdate(in int updateId);

    /**
     * Rollbacks the last update or update with id.
     * If updateId is provided, rollback all update till updateId.
     * This API should be only available to the privileged services like UpdateManager.
     * Can't be performed while another update is in progress.
     *
     * @param[in] updateId is a unique identifier for update, until which the rollback
     * should be performed.
     *
     * @return update identifier which is valid and current after the rollback is performed.
     * If there was no previous update, 0 is returned.
     */
    int rollback(in int updateId);

    /**
     * Removes all configuration updates and default configurations.
     * This API should be only available to the privileged services like UpdateManager.
     * Cleans all services up to the factory state.
     * Can't be performed while another update is in progress.
     */
    void reset();

    /**
     * Removes all configurations for service.
     * Called when a service is deinstalled.
     * This API should be only available to the privileged services like UpdateManager.
     * Can't be performed while another update is in progress.
     *
     * @param[in] serviceId is a unique service identifier.
     */
    void deinstallConfig(in String serviceId);

    /**
     * Adds a new factor or option.
     * It might be required if new hardware or software options are available.
     * This API should be only available to the privileged services like UpdateManager.
     *
     * Can't be performed while another update is in progress.
     * @param[in] id is a unique identifier for factor or option.
     * @param[in] values is an array of enums which the factor or option can take.
     */
    void addFactorOrOption(in String keyId, in String[] values);

    // Calibration functionality.
    // Should be disabled for the user builds.

    /**
     * Registers calibration callback.
     *
     * @param[in] configId is a unique identifier for config whose service is calibrated.
     * @param[in] cb is a callback function.
     *
     * @return calibration callback identifier.
     */
    String registerCalibCallback(in ConfigId configId, in ICalibrationCallback cb);

    /**
     * Removes a previously registered calibration callback.
     *
     * @param[in] configId is a unique config identifier.
     * @param[in] callbackId is a unique identifier for calibration callback.
     */
    void unregisterCallback(in ConfigId configId, in String callbackId);
}
