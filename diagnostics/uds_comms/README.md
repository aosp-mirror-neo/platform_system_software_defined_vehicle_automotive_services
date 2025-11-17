<!-- Copyright 2025 Google LLC -->
# `uds_comms`

The `uds_comms` Rust library provides communication components for selected Unified Diagnostic Services (UDS) over a pluggable transport layer.

Currently, it supports Diagnostics over IP (DoIP) as transport layer that can be used with `tokio` to encode and decode DoIP messages.


## Build `uds_comms`

```bash
 m libsdv_uds_comms
```

## Test `uds_comms`

### 1. Preapre the Device
Create a new CF device and restart adbd with root permissions
```bash
cvd create && adb root
```

### 2. Run tests
```bash
atest sdv_uds_comms_tests
```
