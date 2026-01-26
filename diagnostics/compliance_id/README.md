# Diagnostics Compliance IDs

This library provides a shared enum, `DiagComplianceId`, for mapping between diagnostic identifiers (DIDs) and their corresponding string representations of Diagnostics Data Item identifiers. These mappings serve as a single source of truth for host tests, the Diagnostics Manager, and the Diagnostics Compliance Service Bundle.

## Building and Testing

You can build and run unit tests using the following Soong targets:

- **Library**: `m libsdv_diag_compliance_id`
- **Tests**: `atest libsdv_diag_compliance_id_tests`
