package android.sdv.diagnostics;

import android.sdv.diagnostics.ecu_reset.IECUResetHandler;
import android.sdv.diagnostics.file_transfer.IFileTransferHandler;
import android.sdv.diagnostics.metadata.ConnectionMetadata;

/**
 * Diagnostics APIs available only to privileged SDV-Services.
 *
 * Allows privileged SDV-Services to register as handlers for the supported UDS
 * "services".
 *
 * Handlers are registered with a unique id. It's up to diagnostic manager implementation
 * to decide which handler to use (e.g. depending on configuration or its current state).
 *
 * Handlers unregistration will be supported in the future, after these APIs are redefined
 * in VIDL.
 *
 * TODO(@tantoshchuk): revisit after these APIs are redefined in VIDL.
 * **Common Error conditions**
 * - `ResponseCode::GENERAL_REJECT` - if the request fails for any reason.
 *
 * Based on ISO 14229-1:2020. See README for external links.
 *
 * This APIs are experimental at this point.
 */
interface IDiagnosticsManagerPlatform {
    /**
     * Registers handler for a ECUReset (UDS 0x11) service.
     *
     * See `IECUResetHandler` for more details.
     *
     * @param[in] id unique identifier of the ECUReset service handler.
     * @param[in] handler handler to be used for the ECUReset service.
     */
    void registerECUResetHandler(in String id, in IECUResetHandler handler);

    /**
     * Registers handler for a file transfer related services.
     *
     * Registers handler for a file transfer operations on top of the following services:
     * - RequestDownload (UDS 0x34)
     * - TransferData (UDS 0x36)
     * - RequestTransferExit (UDS 0x37)
     * - RequestFileTransfer (UDS 0x38)
     *
     * See `IFileTransferHandler` for more details.
     *
     * @param[in] id unique identifier of the file transfer handler.
     * @param[in] handler handler to be used for the file transfer.
     */
    void registerFileTransferHandler(in String id, in IFileTransferHandler handler);

    /**
     * Provides connection metadata by its id.
     *
     * Allows service handlers to query the metadata of the current connection. Connection
     * id is provided as a parameter on every method of the following handlers:
     * - IFileTransferHandler
     * - IECUResetHandler
     *
     * **Example**
     * IResetHandler contains method #hardReset(in int *connectionId*),
     * *connectionId* then can be used with #getConnectionMetadata(in int *connectionId*)
     * to retrieve additional information about the connection from the DoIP packet that
     * triggered the IResetHandler#hardReset.
     *
     * @param[in] connectionId id of the connection which metadata is being queried.
     *
     * @return metadata of the connection with connectionId.
     */
    ConnectionMetadata getConnectionMetadata(in int connectionId);
}
