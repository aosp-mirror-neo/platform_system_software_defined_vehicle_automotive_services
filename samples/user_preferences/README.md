# SDV User Preferences reference implementation

This package provides a reference implementation for the SDV  User Preferences service. The included sample services demonstrate how to effectively utilize the User Preferences Service APIs.

## Included Services

1. HMIService (Human Machine Interface Service)

    * Purpose: Simulates actions that an AAOS (Android Automotive OS) IVI (In-Vehicle Infotainment) system would perform to modify HVAC
    temperature settings.

    * Key Interactions: Communicates with the User Preferences service to modify the HVAC settings.

2. HVACService

    * Purpose: Represents a service that controls the vehicle's HVAC system, providing an interface to access and adjust temperature settings.

    * Key Interactions: Registers its settings in the User Preferences Service & receives commands to change HVAC settings.

3. CarSeatsService

    * Purpose: Represents a service that controls the position of the driver's seat.

    * Key Interactions: Registers its settings in the User Preferences Service & receives commands to change seat position.

    * Note: This service is specifically used within the AAOS IVI sample Java application.

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

2. Run the User Preferences Service

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
    1. Create a user with ID 1 and switch to it
    2. Subscribe to changes in the HVAC config
    3. Change the TEMPERATURE from 16 to 32
    4. Create a user with ID 2 and switch to it, therefore temperature will change to the default value 16.
    5. Delete the first user with ID 1

```bash
Creating an ephemeral user
Successfully created user 1

Current HVAC settings
Key ENABLED        , Value: Bool(false), Constraint: "None"
Key TEMPERATURE    , Value: Int64(16) , Constraint: "Int64Constraints(Int64Constraints { min_value: Some(16), max_value: Some(32), step: None, special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } })"
Key FAN_SPEED      , Value: Int64(1)  , Constraint: "Int64Constraints(Int64Constraints { min_value: Some(1), max_value: Some(5), step: None, special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } })"



Adjusting HVAC temperature to 32
The following settings were changed in settings group HvacService/HVAC
Key TEMPERATURE     Value: Int64(32)



Attempting to Adjust HVAC temperature to invalid value 100
Received error "Int64(\n    100,\n) is not a valid value for Setting TEMPERATURE"

Creating another user
Successfully created user 2
The following settings were changed in settings group HvacService/HVAC
Key ENABLED         Value: Bool(false)
Key TEMPERATURE     Value: Int64(16)
Key FAN_SPEED       Value: Int64(1)



Switched to user 2
List of available users: "2"

Retrieving settings for user 2
Settings Group: HVAC
Key ENABLED         Value: Bool(false)
Key TEMPERATURE     Value: Int64(16)
Key FAN_SPEED       Value: Int64(1)



Testing user deletion, creating user 3
Successfully created user 3
Deleted user 3
Unsubscribing from setting changes
Performing factory reset
```

#### HVAC Service Output

The HVAC service attempts to register the same configuration twice, and we expect the user preferences service to ignore the second request.
Additionally HVAC service will get requests to change settings and will respond successfully with the current state.

```bash
Registering settings group with name: HVAC,
Version: 1
Key ENABLED        , Type: PER_USER, Value: Bool(false)
Key FAN_SPEED      , Type: PER_USER, Value: Int64(1)
Key TEMPERATURE    , Type: PER_USER, Value: Int64(16)



Started HvacService service bundle
Settings change request


Adjusted the following settings:
Key ENABLED         Value Some(Bool(false     ))
Key TEMPERATURE     Value Some(Int64(16        ))
Key FAN_SPEED       Value Some(Int64(1         ))
Settings change request


Adjusted the following settings:
Key TEMPERATURE     Value Some(Int64(32        ))
Settings change request


Adjusted the following settings:
Key ENABLED         Value Some(Bool(false     ))
Key TEMPERATURE     Value Some(Int64(16        ))
Key FAN_SPEED       Value Some(Int64(1         ))

Performed factory reset
```

#### User Preferences Service output

```bash
UserPreferencesServer service bundle started.
Registered settings HvacService/HVAC
Key ENABLED        , Type: PER_USER, Value: Bool(false)
Key FAN_SPEED      , Type: PER_USER, Value: Int64(1)
Key TEMPERATURE    , Type: PER_USER, Value: Int64(16)



Created user 1
Loading user 1

Updating the following settings within HvacService/HVAC:
Key ENABLED         Value: Bool(false)
Key TEMPERATURE     Value: Int64(16)
Key FAN_SPEED       Value: Int64(1)



Successfully registered subscriber to settings: HvacService/HVAC

Updating the following settings within HvacService/HVAC:
Key TEMPERATURE     Value: Int64(32)



Notifying HmiService of settings change

Created user 2
Loading user 2

Deleted user 1
Deleted ephemeral user: 1
Updating the following settings within HvacService/HVAC:
Key ENABLED         Value: Bool(false)
Key TEMPERATURE     Value: Int64(16)
Key FAN_SPEED       Value: Int64(1)



Notifying HmiService of settings change

Created user 3
Deleted user 3
Factory resetting HvacService
Updated the following settings
Key FAN_SPEED       Value: Int64(1)
Key TEMPERATURE     Value: Int64(16)
Key ENABLED         Value: Bool(false)
```
