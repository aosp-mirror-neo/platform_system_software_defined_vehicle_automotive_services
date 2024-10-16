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
