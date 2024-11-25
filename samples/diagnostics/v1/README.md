<!-- Copyright 2024 Google LLC -->

# Diagnostics middleware generation and implementation example

The example consist of the simple diagnostics provider implementation that works
with Diagnostics Agent and exposes a number of diagnostics entities.

## Running the samples

### Build the image

Image `sdv_core_cf` includes samples already. So just run the following command
to build it:

`m`

### Start the cloud virtual device

```bash
cvd create --noresume -vsock_guest_cid=3 -base_instance_num=1 --extra_bootconfig_args="androidboot.sdv.instance_name=instance1 androidboot.virt.address=3"

# Waits a few seconds for message "Virtual device booted successfully"

adb devices

# Expects "List of devices attached" and two devices below it
```

On VM boot Diagnostics Agent is started, which subscribes to any discovered
diagnostics Events publications as well as waits for `Diagnostics provider 1`
servers to appear.

On receiving Event publications and connecting to server, diagnostics agents
performs a basic interaction with them and log the results.

Use the following command to observe the outputs:

```bash
adb logcat | grep sdv_diagnostics_agent
```

### Run 'Diagnostics provider 1' service bundle

Using root, because the binary is on the `/product` partition.

```bash
adb -s <VM address> shell su root diag_v1_provider1_sample
```

Diagnostics provider 1 registers RPC servers for diagnostics interfaces as well
as publishes Events.

### Expected Output

#### Diagnostics provider 1

```bash
Diagnostics provider service bundle started.
...
Start PressurizationRoutine called, pressure []
Freeze current state for PressureIoControl called
On fault status change for LowPressureFault called: status [1], data [None]
```

#### Diagnostics_Agent

Logs at boot, before launching sample binary:

```bash
11-26 08:11:37.416     0     0 I init    : starting service 'sdv_diagnostics_agent'...
11-26 08:11:37.422     0     0 I init    : ... started service 'sdv_diagnostics_agent' has pid 331
11-26 08:11:37.606   331   331 V sdv_diagnostics_agent: sdv_log: Panics are redirected to logger errors.
11-26 08:11:37.606   331   331 V sdv_diagnostics_agent: sdv_log: Default logger is initialized.
11-26 08:11:37.615     0     0 I servicemanager: Caller(pid=331,uid=1000,sid=u:r:sdv_diagnostics_agent:s0) Since 'google.sdv.service_discovery.discovery.IServiceRegistrationAgent/default' could not be found trying to start it as a lazy AIDL service. (if it's not configured to be a lazy service, it may be stuck starting or still starting).
11-26 08:11:37.654     0     0 I servicemanager: Caller(pid=331,uid=1000,sid=u:r:sdv_diagnostics_agent:s0) Tried to start aidl service google.sdv.service_discovery.discovery.IServiceRegistrationAgent/default as a lazy service, but was unable to. Usually this happens when a service is not installed, but if the service is intended to be used as a lazy service, then it may be configured incorrectly.
11-26 08:11:38.617   331   331 D sdv_diagnostics_agent: sdv_config_provider::vsidl_provider: Vsidl provider somip config path: "/apex/com.sdv.someip/etc/vsidl_v1/someip_config/libsomeip_messages_catalog-someip-config.binpb"
11-26 08:11:38.617   331   331 D sdv_diagnostics_agent: sdv_config_provider::vsidl_provider: Vsidl provider somip config path: "/apex/com.sdv.google.sample.service_bundles_registry.apex_configurable/etc/external_protocol_mapping.binpb"
11-26 08:11:38.617   331   331 D sdv_diagnostics_agent: sdv_config_provider::vsidl_provider: Vsidl provider bundle diag config path: "/apex/com.sdv.oem.sample.diagnostics/etc/data_item/diag.binpb"
11-26 08:11:38.617   331   331 D sdv_diagnostics_agent: sdv_config_provider::vsidl_provider: Vsidl provider bundle diag config path: "/apex/com.sdv.oem.sample.diagnostics/etc/event/diag.binpb"
11-26 08:11:38.617   331   331 D sdv_diagnostics_agent: sdv_config_provider::vsidl_provider: Vsidl provider bundle diag config path: "/apex/com.sdv.google.sample.vsidl_provider/etc/diag_service/vsidl/diagnostics_config/diag_service-diag-config.binpb"
11-26 08:11:38.617   331   331 D sdv_diagnostics_agent: sdv_config_provider::vsidl_provider: Vsidl provider bundle vsidl config path: "/apex/com.sdv.oem.sample.diagnostics/etc/data_item/vsidl.binpb"
11-26 08:11:38.617   331   331 D sdv_diagnostics_agent: sdv_config_provider::vsidl_provider: Vsidl provider bundle vsidl config path: "/apex/com.sdv.google.sample.vsidl_provider/etc/other_manager/vsidl/vsidl_config/other_manager-vsidl-config.binpb"
11-26 08:11:38.617   331   331 D sdv_diagnostics_agent: sdv_config_provider::vsidl_provider: Vsidl provider bundle vsidl config path: "/apex/com.sdv.oem.sample.diagnostics/etc/event/vsidl.binpb"
11-26 08:11:38.617   331   331 D sdv_diagnostics_agent: sdv_config_provider::vsidl_provider: Vsidl provider bundle vsidl config path: "/apex/com.sdv.google.sample.vsidl_provider/etc/diag_service/vsidl/vsidl_config/diag_service-vsidl-config.binpb"
11-26 08:11:38.617   331   331 D sdv_diagnostics_agent: sdv_config_provider::vsidl_provider: Vsidl provider bundle vsidl config path: "/apex/com.sdv.google.sample.vsidl_provider/etc/monitor/vsidl/vsidl_config/monitor-vsidl-config.binpb"
11-26 08:11:38.617   331   331 D sdv_diagnostics_agent: sdv_config_provider::vsidl_provider: Vsidl provider bundle vsidl config path: "/apex/com.sdv.google.sample.vsidl_provider/etc/manager/vsidl/vsidl_config/manager-vsidl-config.binpb"
11-26 08:11:38.617   331   331 D sdv_diagnostics_agent: sdv_config_provider::vsidl_provider: Vsidl provider bundle vsidl config path: "/apex/com.sdv.google.sample.vsidl_provider/etc/other_monitor/vsidl/vsidl_config/other_monitor-vsidl-config.binpb"
11-26 08:11:38.617   331   331 D sdv_diagnostics_agent: sdv_config_provider::vsidl_provider: Vsidl provider bundle vsidl config path: "/apex/com.sdv.google.sample.vsidl_provider/etc/hello_service/vsidl/vsidl_config/hello_service-vsidl-config.binpb"
11-26 08:11:38.617   331   331 D sdv_diagnostics_agent: sdv_config_provider::vsidl_provider: Vsidl provider bundle vsidl config path: "/apex/com.sdv.google.health/etc/libhealth_messages_stable_catalog-vsidl-config.binpb"
11-26 08:11:38.643   331   331 I sdv_diagnostics_agent: main: Starting Diagnostics Agent
11-26 08:11:38.649   331   373 I sdv_diagnostics_agent: main: Starting interaction with Events
11-26 08:11:38.650   331   372 I sdv_diagnostics_agent: main: Starting interaction with DataItems
11-26 08:11:38.650   331   371 I sdv_diagnostics_agent: main: Starting interaction with IoControls
11-26 08:11:38.650   331   373 I sdv_diagnostics_agent: main: Starting interaction with RoutineControls
11-26 08:11:38.650   331   373 I sdv_diagnostics_agent: main: Waiting for service unit: Some("com-sdv-google-diagnostics-routine-control-routine-control-service")
11-26 08:11:38.650   331   373 I sdv_diagnostics_agent: main: Starting interaction with FaultListeners
11-26 08:11:38.650   331   373 I sdv_diagnostics_agent: main: Waiting for service unit: Some("com-sdv-google-diagnostics-fault-fault-listener-service")
11-26 08:11:38.650   331   372 I sdv_diagnostics_agent: main: Listening for service units registration by type : com.sdv.google.diagnostics.DiagnosticsManager#data_item.DataItemService
11-26 08:11:38.650   331   371 I sdv_diagnostics_agent: main: Waiting for service unit: Some("com-sdv-google-diagnostics-io-control-io-control-service")
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent: main: Service unit change received: Registered(
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:     ServiceUnitDefinition(
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:         0x00007176abadf540,
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:     ),
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent: )
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent: main: Service unit registered: instance1:com.sdv.oem.sample.diagnostics.DiagnosticsSampleWithDataItem/instance / com-sdv-google-diagnostics-data-item-data-item-service
11-26 08:11:40.180   331   370 I sdv_diagnostics_agent: main: Discovered unit: instance1:com.sdv.oem.sample.diagnostics.DiagnosticsSampleWithDataItem/instance : com-sdv-google-diagnostics-data-item-data-item-service
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent: main: Diagnostics declaration of a instance1:com.sdv.oem.sample.diagnostics.DiagnosticsSampleWithDataItem/instance : Some(
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:     DiagnosticsDeclaration {
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:         data_item: [
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:             DataItem {
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:                 id: "pressure",
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:                 message_name: "com.sdv.oem.sample.diagnostics.Pressure",
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:                 is_writable: false,
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:                 special_fields: SpecialFields {
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:                     unknown_fields: UnknownFields {
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:                         fields: None,
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:                     },
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:                     cached_size: CachedSize {
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:                         size: 0,
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:                     },
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:                 },
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:             },
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:         ],
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:         io_control: [],
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:         routine: [],
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:         event: [],
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:         special_fields: SpecialFields {
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:             unknown_fields: UnknownFields {
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:                 fields: None,
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:             },
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:             cached_size: CachedSize {
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:                 size: 0,
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:             },
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:         },
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent:     },
11-26 08:11:40.180   331   370 D sdv_diagnostics_agent: )
11-26 08:11:40.197   331   370 D sdv_diagnostics_agent: main: Trying to read data item: DataItem {
11-26 08:11:40.197   331   370 D sdv_diagnostics_agent:     id: "pressure",
11-26 08:11:40.197   331   370 D sdv_diagnostics_agent:     message_name: "com.sdv.oem.sample.diagnostics.Pressure",
11-26 08:11:40.197   331   370 D sdv_diagnostics_agent:     is_writable: false,
11-26 08:11:40.197   331   370 D sdv_diagnostics_agent:     special_fields: SpecialFields {
11-26 08:11:40.197   331   370 D sdv_diagnostics_agent:         unknown_fields: UnknownFields {
11-26 08:11:40.197   331   370 D sdv_diagnostics_agent:             fields: None,
11-26 08:11:40.197   331   370 D sdv_diagnostics_agent:         },
11-26 08:11:40.197   331   370 D sdv_diagnostics_agent:         cached_size: CachedSize {
11-26 08:11:40.197   331   370 D sdv_diagnostics_agent:             size: 0,
11-26 08:11:40.197   331   370 D sdv_diagnostics_agent:         },
11-26 08:11:40.197   331   370 D sdv_diagnostics_agent:     },
11-26 08:11:40.197   331   370 D sdv_diagnostics_agent: }
11-26 08:11:40.197   331   370 D sdv_diagnostics_agent: main: Performing RPC call: com.sdv.google.diagnostics.DiagnosticsManager#data_item.DataItemService:ReadDataItem
11-26 08:11:40.203   331   370 D sdv_diagnostics_agent: sdv_comms::rpc::client: Client's request path was set to: /com.sdv.google.diagnostics.data_item.DataItemService/
11-26 08:11:40.204   331   370 V sdv_diagnostics_agent: mio::poll: registering event source with poller: token=Token(124756647852800), interests=READABLE | WRITABLE
11-26 08:11:40.282   331   373 I sdv_diagnostics_agent: main: Response for RPC call [ReadDataItem]: Ok(ReadDataItemResponse { value: [13, 51, 51, 200, 66], response_code: POSITIVE_RESPONSE, special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } })
11-26 08:11:40.285   331   373 D sdv_diagnostics_agent: main: Read data item: instance1:com.sdv.oem.sample.diagnostics.DiagnosticsSampleWithDataItem/instance#pressure, proto_value: [13, 51, 51, 200, 66], uds_value [66, 200, 51, 51]
11-26 08:11:40.285   331   366 V sdv_diagnostics_agent: mio::poll: deregistering event source from poller
11-26 08:11:40.671   331   372 I sdv_diagnostics_agent: main: Subscribing to new event: instance1:com.sdv.oem.sample.diagnostics.DiagnosticsSampleWithEvent/instance#overheating-event
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent: main: Subscribe to event with msg size: 26
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent: main: Available to read: 1
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent: main: Event received: Ok(
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:     Event {
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:         status: PRE_PASS,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:         extended_data: Some(
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:             [
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:                 10,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:                 10,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:                 8,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:                 116,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:                 111,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:                 111,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:                 32,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:                 104,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:                 111,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:                 116,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:                 33,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:             ],
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:         ),
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:         special_fields: SpecialFields {
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:             unknown_fields: UnknownFields {
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:                 fields: None,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:             },
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:             cached_size: CachedSize {
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:                 size: 0,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:             },
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:         },
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:     },
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent: )
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent: main: Event extended data received: [
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:     10,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:     10,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:     8,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:     116,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:     111,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:     111,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:     32,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:     104,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:     111,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:     116,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent:     33,
11-26 08:11:40.872   331   372 I sdv_diagnostics_agent: ]
```

After launching sample binary:

```bash
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent: main: Service unit change received: Registered(
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent:     ServiceUnitDefinition(
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent:         0x00007176abaeff30,
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent:     ),
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent: )
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent: main: Service unit registered: instance1:com.sdv.oem.sample.diagnostics.DiagnosticsProvider1/instance / com-sdv-google-diagnostics-data-item-data-item-service
11-26 08:16:01.500   331   373 I sdv_diagnostics_agent: main: Discovered unit: instance1:com.sdv.oem.sample.diagnostics.DiagnosticsProvider1/instance : com-sdv-google-diagnostics-data-item-data-item-service
11-26 08:16:01.500   331   373 I sdv_diagnostics_agent: sdv_config_provider::vsidl_provider: VSIDL: using fallback for instance1:com.sdv.oem.sample.diagnostics.DiagnosticsProvider1/instance
11-26 08:16:01.500   331   373 I sdv_diagnostics_agent: sdv_config_provider::vsidl_provider: VSIDL: using fallback for instance1:com.sdv.oem.sample.diagnostics.DiagnosticsProvider1/instance
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent: main: Diagnostics declaration of a instance1:com.sdv.oem.sample.diagnostics.DiagnosticsProvider1/instance : Some(
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent:     DiagnosticsDeclaration {
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent:         data_item: [],
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent:         io_control: [],
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent:         routine: [],
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent:         event: [],
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent:         special_fields: SpecialFields {
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent:             unknown_fields: UnknownFields {
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent:                 fields: None,
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent:             },
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent:             cached_size: CachedSize {
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent:                 size: 0,
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent:             },
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent:         },
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent:     },
11-26 08:16:01.500   331   373 D sdv_diagnostics_agent: )
11-26 08:16:01.502   331   366 I sdv_diagnostics_agent: main: Event received: Registered(
11-26 08:16:01.502   331   366 I sdv_diagnostics_agent:     ServiceUnitDefinition(
11-26 08:16:01.502   331   366 I sdv_diagnostics_agent:         0x00007176abaee120,
11-26 08:16:01.502   331   366 I sdv_diagnostics_agent:     ),
11-26 08:16:01.502   331   366 I sdv_diagnostics_agent: )
11-26 08:16:01.502   331   366 I sdv_diagnostics_agent: main: Discovered unit: instance1:com.sdv.oem.sample.diagnostics.DiagnosticsProvider1/instance : com-sdv-google-diagnostics-routine-control-routine-control-service
11-26 08:16:01.502   331   366 D sdv_diagnostics_agent: main: Performing RPC call: com.sdv.google.diagnostics.DiagnosticsManager#routine_control.RoutineControlService:StartRoutine
11-26 08:16:01.514   331   373 D sdv_diagnostics_agent: sdv_comms::rpc::client: Client's request path was set to: /com.sdv.google.diagnostics.routine_control.RoutineControlService/
11-26 08:16:01.515   331   373 I sdv_diagnostics_agent: main: Event received: Registered(
11-26 08:16:01.515   331   373 I sdv_diagnostics_agent:     ServiceUnitDefinition(
11-26 08:16:01.515   331   373 I sdv_diagnostics_agent:         0x00007176abaddbd0,
11-26 08:16:01.515   331   373 I sdv_diagnostics_agent:     ),
11-26 08:16:01.515   331   373 I sdv_diagnostics_agent: )
11-26 08:16:01.515   331   373 I sdv_diagnostics_agent: main: Discovered unit: instance1:com.sdv.oem.sample.diagnostics.DiagnosticsProvider1/instance : com-sdv-google-diagnostics-io-control-io-control-service
11-26 08:16:01.515   331   373 D sdv_diagnostics_agent: main: Performing RPC call: com.sdv.google.diagnostics.DiagnosticsManager#io_control.IoControlService:FreezeCurrentState
11-26 08:16:01.515   331   369 V sdv_diagnostics_agent: mio::poll: registering event source with poller: token=Token(124756647855488), interests=READABLE | WRITABLE
11-26 08:16:01.516   331   369 I sdv_diagnostics_agent: main: Event received: Registered(
11-26 08:16:01.516   331   369 I sdv_diagnostics_agent:     ServiceUnitDefinition(
11-26 08:16:01.516   331   369 I sdv_diagnostics_agent:         0x00007176abaeb780,
11-26 08:16:01.516   331   369 I sdv_diagnostics_agent:     ),
11-26 08:16:01.516   331   369 I sdv_diagnostics_agent: )
11-26 08:16:01.516   331   369 I sdv_diagnostics_agent: main: Discovered unit: instance1:com.sdv.oem.sample.diagnostics.DiagnosticsProvider1/instance : com-sdv-google-diagnostics-fault-fault-listener-service
11-26 08:16:01.516   331   369 D sdv_diagnostics_agent: main: Performing RPC call: com.sdv.google.diagnostics.DiagnosticsManager#fault.FaultListenerService:OnFaultStatusChange
11-26 08:16:01.518   331   369 D sdv_diagnostics_agent: sdv_comms::rpc::client: Client's request path was set to: /com.sdv.google.diagnostics.io_control.IoControlService/
11-26 08:16:01.519   331   369 D sdv_diagnostics_agent: sdv_comms::rpc::client: Client's request path was set to: /com.sdv.google.diagnostics.fault.FaultListenerService/
11-26 08:16:01.519   331   369 V sdv_diagnostics_agent: mio::poll: registering event source with poller: token=Token(124756647858176), interests=READABLE | WRITABLE
11-26 08:16:01.520   331   373 V sdv_diagnostics_agent: mio::poll: registering event source with poller: token=Token(124756647859968), interests=READABLE | WRITABLE
11-26 08:16:01.551   331   369 I sdv_diagnostics_agent: main: Response for RPC call [OnFaultStatusChange]: Ok(OnFaultStatusChangeResponse { special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } })
11-26 08:16:01.551   331   369 V sdv_diagnostics_agent: mio::poll: deregistering event source from poller
11-26 08:16:01.556   331   369 I sdv_diagnostics_agent: main: Response for RPC call [StartRoutine]: Ok(RoutineControlResponse { routine_info: 0, response: [], response_code: POSITIVE_RESPONSE, special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } })
11-26 08:16:01.556   331   369 V sdv_diagnostics_agent: mio::poll: deregistering event source from poller
11-26 08:16:01.566   331   371 I sdv_diagnostics_agent: main: Subscribing to new event: instance1:com.sdv.oem.sample.diagnostics.DiagnosticsProvider1/instance#com-sdv-google-diagnostics-event-event-lowpressure
11-26 08:16:01.567   331   368 I sdv_diagnostics_agent: main: Response for RPC call [FreezeCurrentState]: Ok(IoControlResponse { value: [13, 51, 51, 200, 66], response_code: POSITIVE_RESPONSE, special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } })
11-26 08:16:01.567   331   366 V sdv_diagnostics_agent: mio::poll: deregistering event source from poller
11-26 08:16:01.766   331   371 I sdv_diagnostics_agent: main: Subscribe to event with msg size: 25
11-26 08:16:01.767   331   371 I sdv_diagnostics_agent: main: Available to read: 1
11-26 08:16:01.767   331   371 I sdv_diagnostics_agent: main: Event received: Ok(
11-26 08:16:01.767   331   371 I sdv_diagnostics_agent:     Event {
11-26 08:16:01.767   331   371 I sdv_diagnostics_agent:         status: PASS,
11-26 08:16:01.767   331   371 I sdv_diagnostics_agent:         extended_data: None,
11-26 08:16:01.767   331   371 I sdv_diagnostics_agent:         special_fields: SpecialFields {
11-26 08:16:01.767   331   371 I sdv_diagnostics_agent:             unknown_fields: UnknownFields {
11-26 08:16:01.767   331   371 I sdv_diagnostics_agent:                 fields: None,
11-26 08:16:01.767   331   371 I sdv_diagnostics_agent:             },
11-26 08:16:01.767   331   371 I sdv_diagnostics_agent:             cached_size: CachedSize {
11-26 08:16:01.767   331   371 I sdv_diagnostics_agent:                 size: 0,
11-26 08:16:01.767   331   371 I sdv_diagnostics_agent:             },
11-26 08:16:01.767   331   371 I sdv_diagnostics_agent:         },
11-26 08:16:01.767   331   371 I sdv_diagnostics_agent:     },
11-26 08:16:01.767   331   371 I sdv_diagnostics_agent: )
11-26 08:16:02.767   331   371 I sdv_diagnostics_agent: main: Available to read: 1
11-26 08:16:02.767   331   371 I sdv_diagnostics_agent: main: Event received: Ok(
11-26 08:16:02.767   331   371 I sdv_diagnostics_agent:     Event {
11-26 08:16:02.767   331   371 I sdv_diagnostics_agent:         status: PRE_FAIL,
11-26 08:16:02.767   331   371 I sdv_diagnostics_agent:         extended_data: None,
11-26 08:16:02.767   331   371 I sdv_diagnostics_agent:         special_fields: SpecialFields {
11-26 08:16:02.767   331   371 I sdv_diagnostics_agent:             unknown_fields: UnknownFields {
11-26 08:16:02.767   331   371 I sdv_diagnostics_agent:                 fields: None,
11-26 08:16:02.767   331   371 I sdv_diagnostics_agent:             },
11-26 08:16:02.767   331   371 I sdv_diagnostics_agent:             cached_size: CachedSize {
11-26 08:16:02.767   331   371 I sdv_diagnostics_agent:                 size: 0,
11-26 08:16:02.767   331   371 I sdv_diagnostics_agent:             },
11-26 08:16:02.767   331   371 I sdv_diagnostics_agent:         },
11-26 08:16:02.767   331   371 I sdv_diagnostics_agent:     },
11-26 08:16:02.767   331   371 I sdv_diagnostics_agent: )
```
