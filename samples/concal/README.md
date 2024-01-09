# SDV ConCal and OverrideLibrary Sample Implementation

The sample contains a test implementation for the ConCal service and library which can apply ConfigOverride messages to the config. It shows the syntax for ConfigOverride messages.

## SDV ConCal Sample Code Generation

To generate code from the VSIDL definitions, run the following command from this folder:

```bash
vsidl_code_generator \
    --vsidl-path ../../../core_services/vsidl/syntax/ \
    --catalog-path ../../concal/vsidl/ \
    --output-path ./
```

If you get an error that *vsidl_code_generator* is not found, build it first:

```bash
mm vsidl_code_generator
```

## ConCal Config Override Library

1. Setup environment

   ```bash
   source build/envsetup.sh
   lunch sdv_core_cf-userdebug
   m
   m lib_override_tests
   ```

2. Run the device

    ```bash
    launch_cvd -guest-enforce-security false
    ```

3. Run the test

    ```bash
    atest lib_override_tests
    ```
