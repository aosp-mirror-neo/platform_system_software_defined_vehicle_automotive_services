# Diagnostics middleware generation and implementation example

The example consist of the simple diagnostics manager and diagnostics providerimplementations.

## Service bundles

### Diagnostics provider service bundle

Provides data items via sdv.diagnostics.data_item RPC.

### Diagnostics manager service bundle

Reads / writes data items provided by diagnostics provider via RPC

## Sample code generation

Middleware code is already generated and placed in generated/ folder. However, if you want to see that the code generation works,
run the following command from the repo root folder:

```bash
vsidl_code_generator --vsidl-path core_services/vsidl/syntax/ --catalog-path automotive_services/samples/diagnostics/vsidl_api_sample/catalog --output-path automotive_services/samples/diagnostics/vsidl_api_sample/
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

### Run 'Diagnostics provider' service bundle on VM #1 (RPC server)

```bash
adb -s 0.0.0.0:6520 shell ./vendor/bin/diag_vsidl_service_sample
```

If service started successfully, you should see "Diagnostics provider service bundle started." message printed to the console.

### Run 'Diagnostics manager' service bundle on VM #1 (RPC client)

```bash
adb -s 0.0.0.0:6520 shell ./vendor/bin/diag_vsidl_manager_sample
```

If service started successfully, you should see "Diagnostics Manager service bundle started." message printed to the console.

If the RPC communication is working, you should the following output:

#### In 'Diagnostics provider' terminal

Received read request for "data_item_id".
Received read request for "data_item_id".

#### In the 'Diagnostics manager' terminal

Fetch "data_item_id": Ok(ReadResponse { value: [1, 2, 3], special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } }).

GRPC client got an error: RpcFailure(RpcStatus { code: 7-PERMISSION_DENIED, message: "", details: [], debug_error_string: "UNKNOWN:Error received from peer  {grpc_message:\"\", grpc_status:7, created_time:\"2023-11-22T17:18:35.302319781+00:00\"}" })
Update "data_item_id" to a new value [2]. Response: Err(PermissionDenied).

Fetch "data_item_id" data item after update: Ok(ReadResponse { value: [1, 2, 3], special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } }).

## Known issues

- See general Middleware known issues at REPO_ROOT/core_services/samples/vsidl/implementation_example/README.md
- Write requests are not handled by 'Diagnostics provider', "PermissionDenied" error issued instead
- To use additional diagnostics APIs, we need to add symlink to relevant API definition files (see ./catalog/data_item.proto for example)
