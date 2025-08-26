<!-- Copyright 2024 Google LLC -->

# SDV User Preferences reference implementation

This package provides a reference implementation for the SDV User Preferences
service. The included sample services demonstrate how to effectively utilize the
User Preferences Service APIs. The User Preferences service is packaged as an
APEX (Android Pony EXpress) and is automatically started by the Orchestrator
service upon system startup.

## Included Services

1.  HMIService (Human Machine Interface Service)

    *   Purpose: Simulates actions that an AAOS (Android Automotive OS) IVI
        (In-Vehicle Infotainment) system would perform to modify HVAC
        temperature settings.

    *   Key Interactions: Communicates with the User Preferences service to
        modify the HVAC settings.

2.  HVACService

    *   Purpose: Represents a service that controls the vehicle's HVAC system,
        providing an interface to access and adjust temperature settings.

    *   Key Interactions: Registers its settings in the User Preferences Service
        & receives commands to change HVAC settings.

## VSIDL Code Generation

The Middleware code is already generated and placed in the `generated_rs/`
folder. If you would like to regenerate the code run the following command from
this folder:

```bash
vsidlc --catalog-path ${ANDROID_BUILD_TOP}/system/software_defined_vehicle/automotive_services/user_preferences/vsidl/v1 --output-path .
```

## Running the sample

1.  Launch Cuttlefish

    ```bash
    cvd create
    ```

2.  Launch the HVAC Service

    ```bash

    adb shell sdv_hvac_service_stable

    ```

3.  Launch the HMI Service

    ```bash

    adb shell sdv_hmi_service_stable

    ```
### Expected output

#### HMI Service output

The HMI service performs the following:

*   **User Management:**

    *   Creates an ephemeral user (ID 1) and switches to it.
    *   Creates a regular user (ID 2) and switches to it, observing the default
        temperature.
    *   Lists users to confirm the automatic deletion of the ephemeral user.
    *   Creates and deletes a user (ID 3) to demonstrate user deletion.
    *   Performs a factory reset to clear all user data.
    *   Creates a new user (ID 4) and switches to it.

*   **HVAC Settings Interaction:**

    *   Subscribes to changes in the HVAC configuration.
    *   Changes the temperature from 16 to 32.
    *   Attempts to set an invalid temperature (100) and handles the error.
    *   Sets the temperature to 24 for the final user.
    *   Unsubscribes from setting changes.

```bash
Creating an ephemeral user
Subscribing to setting changes
Current HVAC settings
Key ENABLED        , Value: Bool(false), Constraint: "None"
Key FAN_SPEED      , Value: Int64(1)  , Constraint: "Int64Constraints(Int64Constraints { min_value: Some(1), max_value: Some(5), step: None, special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } })"
Key TEMPERATURE    , Value: Int64(16) , Constraint: "Int64Constraints(Int64Constraints { min_value: Some(16), max_value: Some(32), step: None, special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } })"



Adjusting HVAC temperature to 32


The following settings were changed in settings group instance1:com.sdv.oem.user_preferences.HvacService/default/HVAC
Key TEMPERATURE     Value: Int64(32)



Attempting to Adjust HVAC temperature to invalid value 100
Request to set invalid temperature denied SdvStatusCode::InvalidArgument(3)

Creating another user


The following settings were changed in settings group instance1:com.sdv.oem.user_preferences.HvacService/default/HVAC
Key ENABLED         Value: Bool(false)
Key FAN_SPEED       Value: Int64(1)
Key TEMPERATURE     Value: Int64(16)



Switched to user 2
List of available users: "2"

Creating another user
Deleted user 3
Unsubscribing from setting changes
---------------------------
Scenario 2: User Preference configuring HVAC temperature to persisted temperature
Creating user
Set preferred temperature to 24
Subscribing from setting changes
Please restart the HVAC sample binary
User Preferences will automatically detect it starting and set HVAC temperature to the user's desired temperature 24
```

#### HVAC Service Output

The HVAC service attempts to register the same configuration twice, and we
expect the user preferences service to ignore the second request. Additionally
HVAC service will get requests to change settings and will respond successfully
with the current state.

```bash
Registering settings group with name: HVAC,
Version: 1
Started service bundle
Adjusted the following settings:
Key FAN_SPEED       Value Some(Int64(1         ))
Key ENABLED         Value Some(Bool(false     ))
Key TEMPERATURE     Value Some(Int64(16        ))
Adjusted the following settings:
Key TEMPERATURE     Value Some(Int64(32        ))
Adjusted the following settings:
Key ENABLED         Value Some(Bool(false     ))
Key TEMPERATURE     Value Some(Int64(16        ))
Key FAN_SPEED       Value Some(Int64(1         ))

Performed factory reset
Adjusted the following settings:
Key ENABLED         Value Some(Bool(false     ))
Key FAN_SPEED       Value Some(Int64(1         ))
Key TEMPERATURE     Value Some(Int64(16        ))
Adjusted the following settings:
Key TEMPERATURE     Value Some(Int64(24        ))
```

#### Once HVAC service is started

Upon restarting the HVAC service, the SDV User Preferences service automatically
detects its availability and configures it with the active user's preferred
settings. This is evident from the HVAC service log, which indicates that the
temperature is set to 24 (the user's preference) instead of the default value of
16.

```bash
Adjusted the following settings:
Key TEMPERATURE     Value Some(Int64(24        ))
Key FAN_SPEED       Value Some(Int64(1         ))
Key ENABLED         Value Some(Bool(false     ))
```

The HMI service will also display a message indicating that it has received the
latest HVAC service status from User Preferences.

```bash
The following settings were changed in settings group instance1:com.sdv.oem.user_preferences.HvacService/default/HVAC
Key ENABLED         Value: Bool(false)
Key FAN_SPEED       Value: Int64(1)
Key TEMPERATURE     Value: Int64(24)
```
