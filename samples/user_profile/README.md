# SDV User Profile reference implementation

The sample contains a reference implementation for the User Profile service, along with sample clients that illustrate how the User Profile Service APIs can be used.

## VSIDL Code Generation

The Middleware code is already generated and placed in the `generated/` folder. If you would like to regenerate the code run the following command from this folder:

```bash
# Please note as of now, Middleware generated code does not allow multiple servers to be implemented in the same service bundle, manual patches have been done to the generated code.
vsidl_code_generator \
    --vsidl-path ../../../core_services/vsidl/protos/syntax/ \
    --catalog-path ./catalog/ \
    --dependency-paths ../../user_profile/vsidl/ \
    --output-path ./
```

## Running the sample

1. Launch Cuttlefish

    ```bash
    cvd start --guest-enforce-security=false
    ```

2. Run the User Profile Service

    ```bash

    adb root

    adb shell sdv_user_profile_service

    ```

3. Launch the HVAC Service

    ```bash

    adb shell sdv_hvac_service

    ```

4. Launch the HMI Service

    ```bash

    adb shell sdv_hmi_service

    ```

### Expected output

#### HMI Service output

The HMI service perform the following steps:
    1. Create a user profile with ID 1 and switch to it
    2. Subscribe to changes in the HVAC config
    3. Change the TEMPERATURE from 16 to 32
    4. Create a user profile with ID 2 and switch to it, therefore temperature will return to the profile's default 16.
    5. Delete the first profile with ID 1

Creating a user profile
Successfully created profile with ID 1

Current HVAC configuration
Key FAN_SPEED       Value Int64(1)
Key ENABLED         Value Bool(false)
Key TEMPERATURE     Value Int64(16)



Adjusting HVAC temperature to 32
Received config change notification HVAC
Key TEMPERATURE     Value Int64(32)



Creating another user profile
Successfully created profile with ID 2
Switched to profile 2
Received config change notification HVAC
Key ENABLED         Value Bool(false)
Key TEMPERATURE     Value Int64(16)
Key FAN_SPEED       Value Int64(1)



Deleted user profile 1
Performed factory reset
```

#### HVAC Service Output

The HVAC service attempts to register the same configuration twice, and we expect the user profile service to ignore the second request.
Additionally HVAC service will get requests to change settings and will respond successfully with the current state.

```bash
Registering config with name: HVAC,
Version: 1.0
Key FAN_SPEED      , Type: PER_USER, Value Int64(1)
Key ENABLED        , Type: PER_USER, Value Bool(false)
Key TEMPERATURE    , Type: PER_USER, Value Int64(16)



Registering the same configuration again


Config change request, simulating setting change delay


Applied changes from request with transaction ID: 89
Current settings
Key FAN_SPEED       Value Int64(1         )
Key TEMPERATURE     Value Int64(16        )
Key ENABLED         Value Bool(false     )
Config change request, simulating setting change delay


Applied changes from request with transaction ID: 590
Current settings
Key FAN_SPEED       Value Int64(1         )
Key TEMPERATURE     Value Int64(32        )
Key ENABLED         Value Bool(false     )
Config change request, simulating setting change delay


Applied changes from request with transaction ID: 754
Current settings
Key FAN_SPEED       Value Int64(1         )
Key TEMPERATURE     Value Int64(16        )
Key ENABLED         Value Bool(false     )

Performed factory reset
Key ENABLED         Value Bool(false     )
Key FAN_SPEED       Value Int64(1         )
Key TEMPERATURE     Value Int64(16        )
```

#### User Profile Service output

```bash
UserProfileServer service bundle started.
Registered config HVAC
Key FAN_SPEED      , Type: PER_USER, Value Int64(1)
Key TEMPERATURE    , Type: PER_USER, Value Int64(16)
Key ENABLED        , Type: PER_USER, Value Bool(false)



Config HVAC with version 1.0 is already registered
Created profile with ID: 1
Loading profile of user 1

Updated the following settings
Key ENABLED         Value Bool(false)
Key TEMPERATURE     Value Int64(16)
Key FAN_SPEED       Value Int64(1)



Config Change request to HVAC with the following settings:
Key TEMPERATURE     Value Int64(32)



Updated the following settings
Key TEMPERATURE     Value Int64(32)



Successfully registered subscriber to config: HVAC

Created profile with ID: 2
Loading profile of user 2

Updated the following settings
Key ENABLED         Value Bool(false)
Key FAN_SPEED       Value Int64(1)
Key TEMPERATURE     Value Int64(16)
```
