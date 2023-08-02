package android.sdv.diagnostics;

/**
 * Defines a format of the raw byte array.
 *
 * Maps byte array to a typed data structure.
 *
 * To be defined when support for type adapter is added. Currently used to indicate APIs
 * for which type adapter support is planned to be added.
 */
parcelable Format {
    /**
     * Defines a format of the raw byte array.
     *
     * Maps byte array to a single primitive data point.
     *
     * To be defined when support for type adapter is added. Currently used to indicate
     * APIs for which type adapter support is planned to be added.
     */
    parcelable PrimitiveFormat {}
}
