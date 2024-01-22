# Diagnostics middleware generation and implementation example

The example consist of the simple diagnostics manager and diagnostics providerimplementations.

## Service bundles

### Diagnostics provider service bundle

Provides data items via sdv.diagnostics.data_item RPC.

### Diagnostics manager service bundles

Reads / writes data items provided by diagnostics provider via RPC

## Sample code generation

Middleware code is already generated and placed in generated/ folder. However, if you want to see that the code generation works,
run the following command from the `$REPO_ROOT/system/software_defined_vehicle` folder:

```bash
vsidl_code_generator --vsidl-path core_services/vsidl/protos/ --catalog-path automotive_services/samples/diagnostics/vsidl_api_sample/catalog --output-path automotive_services/samples/diagnostics/vsidl_api_sample/ --dependency-paths automotive_services/diagnostics/vsidl --dependency-paths automotive_services/diagnostics/serde/proto
```

If you get an error that *vsidl_code_generator* is not found, build it first:

```bash
mm vsidl_code_generator
```

## Running the samples

### Build the image

Image `sdv_core_cf` includes samples already. So just run the following command to build it:

```m```

### Start the cloud virtual devices

Two devices need to be started for cross-VM communication.

```bash
launch_cvd --noresume -vsock_guest_cid=3 -guest-enforce-security=false -base_instance_num=1 --extra_bootconfig_args="androidboot.virt.vmname=instance1 androidboot.virt.address=3"

launch_cvd --noresume -vsock_guest_cid=4 -guest-enforce-security=false -base_instance_num=2 --extra_bootconfig_args="androidboot.virt.vmname=instance2 androidboot.virt.address=4"

// Waits a few seconds for message "Virtual device booted successfully"

adb devices

// Expects "List of devices attached" and two devices below it
```

### Run 'Diagnostics provider 1' service bundle (RPC server)

```bash
adb -s <VM address> shell ./vendor/bin/diag_vsidl_service_sample1
```

If service started successfully, you should see "Diagnostics provider service bundle started." message printed to the console.

### Run 'Diagnostics provider 2' service bundle (RPC server)

```bash
adb -s <VM address> shell ./vendor/bin/diag_vsidl_service_sample2
```

If service started successfully, you should see "Diagnostics provider service bundle started." message printed to the console.

### Run 'Diagnostics manager' service bundle (RPC client)

```bash
adb -s <VM address> shell ./vendor/bin/diag_vsidl_manager_sample
```

If service started successfully, you should see "Diagnostics Manager service bundle started." message printed to the console.

If the RPC communication is working, you should the following output:

#### In 'Diagnostics provider 1' terminal

Received read request for "data_item_id1".
Received write request for "data_item_id1".
Received read request for "data_item_id1".

#### In 'Diagnostics provider 2' terminal

Received read request for "data_item_id1".
Received read request for "data_item_id2".

#### In the 'Diagnostics manager' terminal

Fetch "data_item_id1": Ok(ReadResponse { value: [7, 8, 1, 18, 3, 104, 101, 121], error_code: POSITIVE_RESPONSE, special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } }).

Fields parsed from UDS bytes: code = '1'
Fields parsed from UDS bytes: name = 'hey'

Update "data_item_id1" to a new value [9, 8, 15, 18, 5, 104, 105, 0, 0, 0]. Response: Ok(EmptyResponse { error_code: POSITIVE_RESPONSE, special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } }).

Fetch "data_item_id1" data item after update: Ok(ReadResponse { value: [9, 8, 15, 18, 5, 104, 105, 0, 0, 0], error_code: POSITIVE_RESPONSE, special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } }).

Fields parsed from UDS bytes: code = '15'
Fields parsed from UDS bytes: name = 'hi'

Trying to fetch "data_item_id1" from the second provider, fails with response: Ok(ReadResponse { value: [], error_code: REQUEST_OUT_OF_RANGE, special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } }).

Fetch "data_item_id2" from the second provider: Ok(ReadResponse { value: [10, 8, 1, 8, 2, 8, 3, 8, 4, 8, 5], error_code: POSITIVE_RESPONSE, special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } }).

## Known issues

- See general Middleware known issues at REPO_ROOT/core_services/samples/vsidl/implementation_example/README.md
