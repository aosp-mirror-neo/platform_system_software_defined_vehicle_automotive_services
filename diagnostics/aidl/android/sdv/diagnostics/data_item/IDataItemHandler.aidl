// Copyright 2023 Google LLC. All Rights Reserved.

package android.sdv.diagnostics.data_item;

/**
 * Handler for a data-by-identifier related services.
 *
 * Handles read / write operations on data items.
 *
 * Data items represent a primitive pieces of data, which DID consists of, e.g.:
 * - DiagnosticsManager internally defines DID that represents position of the car as two
 * primitive values (latitude and longitude).
 * - SDV-Service (or multiple services) can expose those values as individual data items.
 * - It's up to DiagnosticsManager implementation to define mapping between DIDs and data
 * items.
 */
interface IDataItemHandler {
    /**
     * Read data item by identifier.
     *
     * Corresponds to ReadDataByIdentifier (UDS 0x22) service.
     *
     * @param[in] id unique id of the data item.
     * @return data item value.
     */
    byte[] onRead(in String id);

    /**
     * Write data item by identifier.
     *
     * Corresponds to WriteDataByIdentifier (UDS 0x2E) service.
     *
     * @param[in] id unique id of the data item.
     * @param[in] data new value to assign to data item.
     */
    void onWrite(in String identifier, in byte[] data);
}
