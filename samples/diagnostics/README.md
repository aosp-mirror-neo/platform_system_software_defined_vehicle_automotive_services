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
vsidl_code_generator --vsidl-path core_services/vsidl/protos/ --catalog-path automotive_services/samples/diagnostics/catalog --output-path automotive_services/samples/diagnostics/ --dependency-paths automotive_services/diagnostics/vsidl
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
cvd start --noresume -vsock_guest_cid=3 -guest-enforce-security=false -base_instance_num=1 --extra_bootconfig_args="androidboot.virt.vmname=instance1 androidboot.virt.address=3"

cvd start --noresume -vsock_guest_cid=4 -guest-enforce-security=false -base_instance_num=2 --extra_bootconfig_args="androidboot.virt.vmname=instance2 androidboot.virt.address=4"

# Waits a few seconds for message "Virtual device booted successfully"

adb devices

# Expects "List of devices attached" and two devices below it
```

### Run 'Diagnostics provider 1' service bundle (RPC server)

```bash
adb -s <VM address> shell diag_provider1_sample
```

If service started successfully, you should see "Diagnostics provider service bundle started." message printed to the console.

### Run 'Diagnostics provider 2' service bundle (RPC server)

```bash
adb -s <VM address> shell diag_provider2_sample
```

If service started successfully, you should see "Diagnostics provider service bundle started." message printed to the console.

### Run 'Diagnostics manager' service bundle (RPC client)

```bash
adb -s <VM address> shell diag_manager_sample
```

If service started successfully, you should see "Diagnostics Manager service bundle started." message printed to the console.

If the RPC communication is working, you should the following output:

#### In 'Diagnostics provider 1' terminal

```
Received read request for "data_item_id1".
Received write request for "data_item_id1".
Received read request for "data_item_id1".
```

#### In 'Diagnostics provider 2' terminal

```
Received read request for "data_item_id1".
Received read request for "data_item_id2".
```

#### In the 'Diagnostics manager' terminal

See `$REPO_ROOT/automotive_services/diagnostics/integration_test/tests/manager_stdout.txt`

## Known issues

- See general Middleware known issues at `$REPO_ROOT/core_services/samples/vsidl/implementation_example/README.md`
