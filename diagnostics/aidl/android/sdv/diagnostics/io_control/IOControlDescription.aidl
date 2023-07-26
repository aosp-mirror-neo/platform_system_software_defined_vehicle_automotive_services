package android.sdv.diagnostics.io_control;

import android.sdv.diagnostics.Format;

/**
 * Description of a data identifier for input/output control.
 */
parcelable IOControlDescription {
    /**
     * Identifier that references the input signals, internal parameters and/or
     * output signals
     */
    String id;

    /**
     * Format of the identifier's data record.
     */
    Format format;
}
