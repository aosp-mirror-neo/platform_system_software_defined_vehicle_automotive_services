// Copyright 2025 Google LLC
//! UDS communication library
//!
//! Implements parts of the Unified Diagnostic Services (UDS) and Diagnostics over IP (DoIP)
//! communication protocols according to ISO-14229-1 (UDS) and ISO-13400 (DoIP).

/// Core models like UDS request and response structures, as well as error definitions.
pub mod models;

/// Transport layer abstractions for UDS communication.
pub mod traits;
