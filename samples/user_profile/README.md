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

```bash
Creating an ephemeral user profile
Successfully created profile with ID 1

Current HVAC settings
Key ENABLED        , Value: Bool(false), Constraint: "None"
Key TEMPERATURE    , Value: Int64(16) , Constraint: "Int64Constraints(Int64Constraints { min_value: Some(16), max_value: Some(32), step: None, special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } })"
Key FAN_SPEED      , Value: Int64(1)  , Constraint: "Int64Constraints(Int64Constraints { min_value: Some(1), max_value: Some(5), step: None, special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } })"



Adjusting HVAC temperature to 32
Received settings change notification HVAC
Key TEMPERATURE     Value: Int64(32)



Attempting to Adjust HVAC temperature to invalid value 100
Received error "Int64(\n    100,\n) is not a valid value for Setting TEMPERATURE"

Creating another user profile
Successfully created profile with ID 2
List of current user ids: "2"

Switched to profile 2
Received settings change notification HVAC
Key ENABLED         Value: Bool(false)
Key TEMPERATURE     Value: Int64(16)
Key FAN_SPEED       Value: Int64(1)



Testing profile deletion, creating profile with ID 3
Successfully created profile with ID 3
Deleted user profile 3
Performed factory reset
```

#### HVAC Service Output

The HVAC service attempts to register the same configuration twice, and we expect the user profile service to ignore the second request.
Additionally HVAC service will get requests to change settings and will respond successfully with the current state.

```bash
Registering settings group with name: HVAC,
Version: 1.0
Key ENABLED        , Type: PER_USER, Value: Bool(false)
Key FAN_SPEED      , Type: PER_USER, Value: Int64(1)
Key TEMPERATURE    , Type: PER_USER, Value: Int64(16)



Registering the same settings again


Settings change request, simulating setting change delay


Applied changes from request with transaction ID: 726
Current settings
Key FAN_SPEED       Value Int64(1         )
Key TEMPERATURE     Value Int64(16        )
Key ENABLED         Value Bool(false     )
Settings change request, simulating setting change delay


Applied changes from request with transaction ID: 841
Current settings
Key FAN_SPEED       Value Int64(1         )
Key TEMPERATURE     Value Int64(32        )
Key ENABLED         Value Bool(false     )
Settings change request, simulating setting change delay


Applied changes from request with transaction ID: 977
Current settings
Key FAN_SPEED       Value Int64(1         )
Key TEMPERATURE     Value Int64(16        )
Key ENABLED         Value Bool(false     )

Performed factory reset
Key FAN_SPEED       Value Int64(1         )
Key ENABLED         Value Bool(false     )
Key TEMPERATURE     Value Int64(16        )
```

#### User Profile Service output

```bash
UserProfileServer service bundle started.
Registered settings HVAC
Key ENABLED        , Type: PER_USER, Value: Bool(false)
Key FAN_SPEED      , Type: PER_USER, Value: Int64(1)
Key TEMPERATURE    , Type: PER_USER, Value: Int64(16)



Settings HVAC with version 1.0 is already registered
Created profile with ID: 1
Loading profile of user 1

Updated the following settings
Key ENABLED         Value: Bool(false)
Key TEMPERATURE     Value: Int64(16)
Key FAN_SPEED       Value: Int64(1)



Successfully registered subscriber to settings: HVAC

Settings Change request to HVAC with the following settings:
Key TEMPERATURE     Value: Int64(32)



Updated the following settings
Key TEMPERATURE     Value: Int64(32)



Notifying HmiService::user_profile_view of settings change

Created profile with ID: 2
Loading profile of user 2

Deleted ephemeral profile: 1
Updated the following settings
Key ENABLED         Value: Bool(false)
Key TEMPERATURE     Value: Int64(16)
Key FAN_SPEED       Value: Int64(1)



Notifying HmiService::user_profile_view of settings change

Created profile with ID: 3
Deleted profile with ID: 3
```
