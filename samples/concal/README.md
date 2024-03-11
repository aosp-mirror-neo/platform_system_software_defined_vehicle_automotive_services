# SDV ConCal and OverrideLibrary Sample Implementation

The sample contains a test implementation for the ConCal service and library which can apply ConfigOverride messages to the config. It shows the syntax for ConfigOverride messages.

## SDV ConCal Sample Code Generation

To generate code from the VSIDL definitions, run the following command from this folder:

```bash
vsidl_code_generator \
    --vsidl-path ../../../core_services/vsidl/protos/syntax/ \
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

3. Run tests

    ```bash
    atest lib_override_tests
    atest sdv_con_cal_service_tests
    ```

## ConCal Demo Application

The samples include a reference implementation of the ConCal server along with a sample client.
The client registers a configuration in ConCal, updates it via overrides, and performs a rollback.
To run the demo, follow the steps below:

1. Start the sample ConCal server in one shell

   ```bash
   adb shell /vendor/bin/sdv_con_cal_service
   ```

   The output `ConCalServer service bundle started` means that the server was successfully launched.

2. Run the sample ConCal client in another shell

    ```bash
    adb shell /vendor/bin/sdv_con_cal_client
    ```

3. Once executed the client will register the following configuration and you will see the output

    ```json
    RearViewCamera {
        model: "model 1",
        horizontal_resolution: 720,
        vertical_resolution: 720,
        x_axis_field_of_view: 70.0,
        y_axis_field_of_view: 70.0,
        is_rgb: false,
        special_fields: SpecialFields {
            unknown_fields: UnknownFields {
                fields: None,
            },
            cached_size: CachedSize {
                size: 0,
            },
        },
    }
    Press any key to continue...
    ```

4. Press ENTER. The client will prompt `ConfigOverrides`.

5. Press ENTER. The client will update the configuration through the use of overrides which results in the following configuration and output:

    ```json
    Modified configuration:
    RearViewCamera {
        model: "model 2",
        horizontal_resolution: 1920,
        vertical_resolution: 1080,
        x_axis_field_of_view: 90.0,
        y_axis_field_of_view: 90.0,
        is_rgb: true,
        special_fields: SpecialFields {
            unknown_fields: UnknownFields {
                fields: None,
            },
            cached_size: CachedSize {
                size: 0,
            },
        },
    }
    Press any key to continue...
    ```

6. Press ENTER. Finally, the client will perform a configuration rollback, such that we get the initial configuration:

    ```json
    RearViewCamera {
        model: "model 1",
        horizontal_resolution: 720,
        vertical_resolution: 720,
        x_axis_field_of_view: 70.0,
        y_axis_field_of_view: 70.0,
        is_rgb: false,
        special_fields: SpecialFields {
            unknown_fields: UnknownFields {
                fields: None,
            },
            cached_size: CachedSize {
                size: 0,
            },
        },
    }
    ```
