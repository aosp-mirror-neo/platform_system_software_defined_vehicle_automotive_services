<!-- Copyright 2024 Google LLC -->

# SDV ConCal and OverrideLibrary Sample Implementation

The sample contains a test implementation for the ConCal service and library
which can apply ConfigOverride messages to the config. It shows the syntax for
ConfigOverride messages.

## ConCal Demo Application

The samples include a reference implementation of the ConCal server along with a
sample client. The client registers a configuration in ConCal, updates it via
overrides, and performs a rollback. The ConCal service is defined in an apex. It
implements Lifecycle Manager interface. To run the demo, follow the steps below:

1.  Start the sample ConCal server

    ```bash
    adb root
    adb shell sdv_service_bundle launch local-vm:com.sdv.oem.sample.concal.SampleOemConCalServiceBundle/default
    ```

    The output `ConCalServer service bundle started` printed in `logcat` means
    that the server was successfully launched.

2.  Run the sample ConCal client

    ```bash
    adb root
    adb shell /product/bin/sdv_concal_sample_client
    ```

3.  Once executed the client will register the following configuration and you
    will see the output as in
    [expected client stdout file](../../concal/tests/client_stdout.txt)

## SDV ConCal Sample Code Generation

To generate code from the VSIDL definitions, run the following command from
`system/software_defined_vehicle/automotive_services/concal/`:

```bash
vsidlc   --catalog-path catalog/     --output-path ../../samples/concal/
```

If you get an error that *vsidlc* is not found, build it first:

```bash
mm vsidlc
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
    atest lib_override_tests
    ```
