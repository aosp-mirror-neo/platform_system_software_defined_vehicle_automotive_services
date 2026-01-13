<!--
  Copyright (C) 2026 The Android Open Source Project

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
-->
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
