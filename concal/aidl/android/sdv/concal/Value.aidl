package android.sdv.concal;

/**
 * Format for the parameter value.
 * Only one of the parameters should be set.
 * See `configuration.proto`.
 *
 * This is a PoC version for the API for the discussions.
 * It is unstable until API council approval.
 */
parcelable Value {
    boolean valueBool;
    float valueFloat;
    int valueInt;
    @nullable String valueStr;
    @nullable byte[] valueBlob;
}
