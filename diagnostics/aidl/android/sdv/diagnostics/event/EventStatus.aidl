package android.sdv.diagnostics.event;

/**
 * Status of the event.
 *
 * Corresponds to the monitor test result associated with the event.
 *
 * See AUTOSAR Diagnostics Event Manager specification, section 8.6.1.9.
 */
@Backing(type="byte")
enum EventStatus {
    /**
     * Test associated with the event has passed.
     *
     * No further debouncing by diagnostics manager required.
     */
    PASSED = 0x00u8,

    /**
     * Test associated with the event has failed.
     *
     * No further debouncing by diagnostics manager required.
     */
    FAILED = 0x01u8,

    /**
     * Test associated with the event has passed.
     *
     * Debouncing should be handled by diagnostics manager.
     */
    PRE_PASSED = 0x02u8,

    /**
     * Test associated with the event has failed.
     *
     * Debouncing should be handled by diagnostics manager.
     */
    PRE_FAILED = 0x03u8,
}
