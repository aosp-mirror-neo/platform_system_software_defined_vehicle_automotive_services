# Diagnostics Manager APIs

Currently Diagnostics API is defined in both AIDL (to be removed) and VSIDL formats.
APIs are based on ISO 14229-1:2020 and AUTOSAR Diagnostic Event Manager standards.

All APIs are experimental at the moment.

## AIDL definitions

Defines two Diagnostics API surfaces:

* **IDiagnosticsManager.aidl**: Set of APIs for normal SDV-Services to expose diagnostics
data.

* **IDiagnosticsManagerPlatform.aidl**: Set of privileged APIs for Diagnostics Manager to
delegate parts of its functionality (e.g. Authentication) to privileged SDV-Services.

## VSIDL definitions

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
