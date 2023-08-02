package android.sdv.diagnostics.event;

/**
 * Status of the DTC fault.
 *
 * See ISO 14229-1:2020, section D.2.
 */
@Backing(type="byte")
enum FaultStatus {
    /**
     * The fault associated with the event is confirmed.
     *
     * Number of the fault related event reports with `FAILED`/`PRE_FAILED` status has
     * reached the confirmation threshold.
     *
     * See ISO 14229-1:2020, table D.6.
     */
    CONFIRMED,

    /**
     * The fault is no longer confirmed.
     *
     * This status change is a result of one of:
     * - ClearDiagnosticInformation (UDS 0x14) service.
     * - Aging threshold.
     * - Fault memory overflow.
     *
     * See ISO 14229-1:2020, table D.6.
     */
    CLEARED,
}
