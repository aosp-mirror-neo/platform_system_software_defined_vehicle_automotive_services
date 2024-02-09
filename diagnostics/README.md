# Diagnostics Manager APIs

Currently Diagnostics API is defined in VSIDL format.
APIs are based on ISO 14229-1:2020 and AUTOSAR Diagnostic Event Manager standards.

All APIs are experimental at the moment.

## VSIDL API definitions

* **diagnostics_manager.proto**: RPC API for SDV-Service to query information from
Diagnostics Manager.

* **event.proto**: Events-related messages to sent by SDV-Services to Diangostics
Manager via data tunnel.

* **response_code.proto**: Error codes.

* **connection.proto**: Diagnostics connection id message. See diagnostics_manager.proto.

* **\*.proto**: Diagnostics interfaces to be implemented by SDV-Services. Diagnostics
Manager discovers those implementation and interacts with them at runtime.

## External references

* [[**ISO 14229-1:2020**](https://goto.google.com/uds-standard)] (internal link).
* [[**AUTOSAR Diagnostic Event Manager**](https://www.autosar.org/fileadmin/standards/R20-11/CP/AUTOSAR_SWS_DiagnosticEventManager.pdf)]
