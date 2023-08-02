package android.sdv.diagnostics.data_item;

import android.sdv.diagnostics.Format.PrimitiveFormat;

/**
 * Description of a data item.
 */
parcelable DataItemDescription {
    /**
     * Unique data item identifier.
     */
    String id;

    /**
     * Definition of the data item format on the data item handler side.
     */
    PrimitiveFormat format;
}
