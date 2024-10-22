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
Read Pressure DataItem called
Start PressurizationRoutine called, pressure []
Freeze current state for PressureIoControl called
On fault status change for LowPressureFault called: status [1], data [None]
```

#### Diagnostics_Agent

```bash
10-22 15:53:52.442   301   340 I sdv_diagnostics_agent: main: Response for RPC call [FreezeCurrentState]: Ok(IoControlResponse { value: [13, 51, 51, 200, 66], response_code: POSITIVE_RESPONSE, special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } })
10-22 15:53:52.466   301   340 I sdv_diagnostics_agent: main: Response for RPC call [OnFaultStatusChange]: Ok(OnFaultStatusChangeResponse { special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } })
10-22 15:53:52.466   301   340 I sdv_diagnostics_agent: main: Response for RPC call [StartRoutine]: Ok(RoutineControlResponse { routine_info: 0, response: [], response_code: POSITIVE_RESPONSE, special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } })
10-22 15:53:52.466   301   340 I sdv_diagnostics_agent: main: Response for RPC call [ReadDataItem]: Ok(ReadDataItemResponse { value: [13, 51, 51, 200, 66], response_code: POSITIVE_RESPONSE, special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } })

10-22 15:53:52.603   301   339 I sdv_diagnostics_agent: main: Subscribe to event with msg size: 24
10-22 15:53:52.603   301   339 I sdv_diagnostics_agent: main: Available to read: 1
10-22 15:53:52.603   301   339 I sdv_diagnostics_agent: main: Event received: Ok(
10-22 15:53:52.603   301   339 I sdv_diagnostics_agent:     Event {
10-22 15:53:52.603   301   339 I sdv_diagnostics_agent:         status: PRE_FAIL,
10-22 15:53:52.603   301   339 I sdv_diagnostics_agent:         extended_data: None,
10-22 15:53:52.603   301   339 I sdv_diagnostics_agent:         special_fields: SpecialFields {
10-22 15:53:52.603   301   339 I sdv_diagnostics_agent:             unknown_fields: UnknownFields {
10-22 15:53:52.603   301   339 I sdv_diagnostics_agent:                 fields: None,
10-22 15:53:52.603   301   339 I sdv_diagnostics_agent:             },
10-22 15:53:52.603   301   339 I sdv_diagnostics_agent:             cached_size: CachedSize {
10-22 15:53:52.603   301   339 I sdv_diagnostics_agent:                 size: 0,
10-22 15:53:52.603   301   339 I sdv_diagnostics_agent:             },
10-22 15:53:52.603   301   339 I sdv_diagnostics_agent:         },
10-22 15:53:52.603   301   339 I sdv_diagnostics_agent:     },
10-22 15:53:52.603   301   339 I sdv_diagnostics_agent: )
```
