<!-- Copyright 2024 Google LLC -->

# SDV ConCal and OverrideLibrary Sample Implementation

The sample contains a test implementation for the ConCal service and library
which can apply ConfigOverride messages to the config. It shows the syntax for
ConfigOverride messages.

## SDV ConCal Sample Code Generation (Old Syntax)

To generate code from the VSIDL definitions, run the following command from this
folder:

```bash
sdv_catalog_update --catalog con-cal
```

If you get an error that *sdv_catalog_update* is not found, build it first:

```bash
mm sdv_catalog_update
```

## ConCal Config Override Library

1.  Setup environment

    ```bash
    source build/envsetup.sh
    lunch sdv_core_cf-userdebug
    m
    m lib_override_tests
    ```

2.  Run the device

    ```bash
    cvd start
    ```

3.  Run tests

    ```bash
    atest lib_override_tests sdv_con_cal_service_tests sdv_concal_integration_test
    ```

## ConCal Demo Application

The samples include a reference implementation of the ConCal server along with a
sample client. The client registers a configuration in ConCal, updates it via
overrides, and performs a rollback. To run the demo, follow the steps below:

1.  Start the sample ConCal server in one shell

    Using root, because the binary is on the `/product` partition.

    ```bash
    adb shell su root /product/bin/sdv_con_cal_service
    ```

    The output `ConCalServer service bundle started` means that the server was
    successfully launched.

2.  Run the sample ConCal client in another shell

    Using root, because the binary is on the `/product` partition.

    ```bash
    adb shell su root /product/bin/sdv_con_cal_client
    ```

3.  Once executed the client will register the following configuration and you
    will see the output as in
    [expected client stdout file](../../concal/integration_test/tests/client_stdout.txt)

## SDV ConCal Sample Code Generation (New Syntax)

To generate code from the VSIDL definitions, run the following command from
`system/software_defined_vehicle/automotive_services/concal/stable`:

```bash
vsidlc   --catalog-path catalog/     --output-path ../../samples/concal/
```

If you get an error that *vsidlc* is not found, build it first:

```bash
mm vsidlc
```
