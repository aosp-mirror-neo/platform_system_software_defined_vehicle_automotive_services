# SDV User Profile reference implementation

The sample contains a reference implementation for the User Profile service, along with sample clients that illustrate how the User Profile Service APIs can be used.

## VSIDL Code Generation

The Middleware code is already generated and placed in the `generated/` folder. If you would like to regenerate the code run the following command from this folder:

```bash
vsidl_code_generator \
    --vsidl-path ../../../core_services/vsidl/protos/syntax/ \
    --catalog-path ./catalog/ \
    --dependency-paths ../../user_preferences/vsidl/ \
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

    adb shell sdv_user_preferences_service

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
Key TEMPERATURE    , Value: Int64(16) , Constraint: "Int64Constraints(Int64Constraints { min_value: Some(16), max_value: Some(32), step: None, special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } })"
Key FAN_SPEED      , Value: Int64(1)  , Constraint: "Int64Constraints(Int64Constraints { min_value: Some(1), max_value: Some(5), step: None, special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } })"
Key ENABLED        , Value: Bool(false), Constraint: "None"



Adjusting HVAC temperature to 32
Received settings change notification HVAC
Key TEMPERATURE     Value: Int64(32)



Attempting to Adjust HVAC temperature to invalid value 100
Received error "Int64(\n    100,\n) is not a valid value for Setting TEMPERATURE"

Creating another user profile
Successfully created profile with ID 2
Switched to profile 2
List of available profiles: "2"

Received settings change notification HVAC
Key TEMPERATURE     Value: Int64(16)
Key FAN_SPEED       Value: Int64(1)
Key ENABLED         Value: Bool(false)



Retrieving settings within user profile 2
Settings Group: HVAC
Key TEMPERATURE     Value: Int64(16)
Key FAN_SPEED       Value: Int64(1)
Key ENABLED         Value: Bool(false)



Testing profile deletion, creating profile with ID 3
Successfully created profile with ID 3
Deleted user profile 3
Unsubscribing from setting changes
Performing factory reset
```

#### HVAC Service Output

The HVAC service attempts to register the same configuration twice, and we expect the user profile service to ignore the second request.
Additionally HVAC service will get requests to change settings and will respond successfully with the current state.

```bash
Registering settings group with name: HVAC,
Version: 1.0
Key TEMPERATURE    , Type: PER_USER, Value: Int64(16)
Key FAN_SPEED      , Type: PER_USER, Value: Int64(1)
Key ENABLED        , Type: PER_USER, Value: Bool(false)



Registering the same settings again


Settings change request, simulating setting change delay


Applied changes from request with transaction ID: 783
Current settings
Key ENABLED         Value Bool(false     )
Key FAN_SPEED       Value Int64(1         )
Key TEMPERATURE     Value Int64(16        )
Settings change request, simulating setting change delay


Applied changes from request with transaction ID: 297
Current settings
Key ENABLED         Value Bool(false     )
Key FAN_SPEED       Value Int64(1         )
Key TEMPERATURE     Value Int64(32        )
Settings change request, simulating setting change delay


Applied changes from request with transaction ID: 441
Current settings
Key ENABLED         Value Bool(false     )
Key FAN_SPEED       Value Int64(1         )
Key TEMPERATURE     Value Int64(16        )

Performed factory reset
Settings change request, simulating setting change delay


Applied changes from request with transaction ID: 0
Current settings
Key ENABLED         Value Bool(false     )
Key FAN_SPEED       Value Int64(1         )
Key TEMPERATURE     Value Int64(16        )
```

#### User Profile Service output

```bash
UserPreferencesServer service bundle started.
Registered settings HVAC
Key TEMPERATURE    , Type: PER_USER, Value: Int64(16)
Key FAN_SPEED      , Type: PER_USER, Value: Int64(1)
Key ENABLED        , Type: PER_USER, Value: Bool(false)



Settings HVAC with version 1.0 is already registered
Created profile with ID: 1
Loading profile of user 1

Updated the following settings
Key TEMPERATURE     Value: Int64(16)
Key FAN_SPEED       Value: Int64(1)
Key ENABLED         Value: Bool(false)



Successfully registered subscriber to settings: HVAC

Settings Change request to HVAC with the following settings:
Key TEMPERATURE     Value: Int64(32)



Updated the following settings
Key TEMPERATURE     Value: Int64(32)



Notifying HmiService of settings change

Created profile with ID: 2
Loading profile of user 2

Deleted ephemeral profile: 1
Updated the following settings
Key TEMPERATURE     Value: Int64(16)
Key FAN_SPEED       Value: Int64(1)
Key ENABLED         Value: Bool(false)



Notifying HmiService of settings change

Created profile with ID: 3
Deleted profile with ID: 3
Updated the following settings
Key TEMPERATURE     Value: Int64(16)
Key FAN_SPEED       Value: Int64(1)
Key ENABLED         Value: Bool(false)
```
