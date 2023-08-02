package android.sdv.diagnostics.routine_control;

import android.sdv.diagnostics.Format;

/**
 * Description of a routine.
 */
parcelable RoutineControlDescription {
    /**
     * Routine id.
     */
    String id;

    /**
     * Format of the `IRoutineControlHandler#start` `requestData` parameter.
     */
    Format startRequestFormat;

    /**
     * Format of the `IRoutineControlHandler#start` return value.
     */
    Format startResponseFormat;

    /**
     * Format of the `IRoutineControlHandler#stop` `requestData` parameter.
     */
    Format stopRequestFormat;

    /**
     * Format of the `IRoutineControlHandler#stop` return value.
     */
    Format stopResponseFormat;

    /**
     * Format of the `IRoutineControlHandler#requestResult` `requestData` parameter.
     */
    Format resultRequestFormat;

    /**
     * Format of the `IRoutineControlHandler#starequestResultrt` return value.
     */
    Format resultResponseFormat;
}
