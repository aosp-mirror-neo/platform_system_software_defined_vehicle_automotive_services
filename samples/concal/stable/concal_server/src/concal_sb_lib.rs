// Copyright 2024 Google LLC

//! SDV ConCal Server Sample Library to be used by the Lifecycle Manager Runner.

/// Service bundle struct definition.
pub struct ConCalServiceBundle {
    _context: ContextRef,
}

// Register the new service bundle.
sdv_lifecycle_client::register_service_bundle!(ConCalServiceBundle);

impl ServiceBundle for ConCalServiceBundle {
    /// Creates a new instance of the ConCalServiceBundle.
    /// Called when service bundle is created by the system.
    ///
    /// Context object is provided as a parameter that gives access to the
    /// communication stack APIs.
    fn new(_context: ContextRef) -> ConCalServiceBundle {
        // Memory allocations and static data loading should be done as
        // part of this method.
        //
        // Loading of the dynamic resources (sockets, files, etc)
        // is strongly discouraged due to possible Suspend-to-RAM scenario.
        //
        // The dynamic data can be loaded in the [Self::on_start()] method.
        ConCalServiceBundle { _context }
    }

    /// Called when the service bundle is started by the system.
    fn on_start(&mut self) {
        // Dynamic resources(sockets, files, etc) should be allocated during this call.
    }

    /// Called when the service bundle is stopped by the system in preparation
    /// for shutdown or suspend to RAM/Disc.
    fn on_stop(&mut self) {
        // Stop phase requires the service bundle to delete the dynamic resources
        // (sockets, files, etc) that were previously allocated in the [Self::on_start()] method.
    }
}

/// Called when the service bundle is destroyed by the system.
impl Drop for ConCalServiceBundle {
    fn drop(&mut self) {
        // Static resources deallocation needs to be implemented in the drop method.
    }
}
