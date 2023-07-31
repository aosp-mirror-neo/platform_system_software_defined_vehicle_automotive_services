// Copyright 2023 Google LLC. All Rights Reserved.

package android.sdv.vpm;
/**
 * An example of power states that could be defined for the VPM.
 * The values defined here are just for testing purposes.
 */

@Backing(type="int")
enum PowerState {
    WAITING_FOR_SIGNAL = 0,
    ON = 1,
    SHUTDOWN_PREPARE = 2,
    CANCEL_SHUTDOWN = 3,
    FINISHED = 4,
}
