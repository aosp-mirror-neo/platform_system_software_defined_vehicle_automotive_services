# SDV User Profile reference implementation

The sample contains a reference implementation for the User Profile service, along with sample clients that illustrate how the User Profile Service APIs can be used.

## VSIDL Code Generation

The Middleware code is already generated and placed in the `generated/` folder. If you would like to regenerate the code run the following command from this folder:

```bash
vsidl_code_generator \
    --vsidl-path ../../../core_services/vsidl/protos/syntax/ \
    --catalog-path ./catalog/ \
    --dependency-paths ../../user_profile/vsidl/ \
    --output-path ./
```

## Running the sample

1. Launch Cuttlefish

    ```bash
    launch_cvd --noresume -vsock_guest_cid=3 -guest-enforce-security=false
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

#### HVAC Service Output

The HVAC service attempts to register the same configuration twice, and we expect the user profile service to ignore the second request.

```bash
Registering config with ID: HVAC,
Version: 1.0
Settings: [
    SettingDefinition {
        type_: PER_USER,
        setting: MessageField(
            Some(
                Setting {
                    key: "TEMPERATURE",
                    value: Some(
                        Int(
                            16,
                        ),
                    ),
                    special_fields: SpecialFields {
                        unknown_fields: UnknownFields {
                            fields: None,
                        },
                        cached_size: CachedSize {
                            size: 0,
                        },
                    },
                },
            ),
        ),
        special_fields: SpecialFields {
            unknown_fields: UnknownFields {
                fields: None,
            },
            cached_size: CachedSize {
                size: 0,
            },
        },
    },
    SettingDefinition {
        type_: PER_USER,
        setting: MessageField(
            Some(
                Setting {
                    key: "FAN_SPEED",
                    value: Some(
                        Int(
                            1,
                        ),
                    ),
                    special_fields: SpecialFields {
                        unknown_fields: UnknownFields {
                            fields: None,
                        },
                        cached_size: CachedSize {
                            size: 0,
                        },
                    },
                },
            ),
        ),
        special_fields: SpecialFields {
            unknown_fields: UnknownFields {
                fields: None,
            },
            cached_size: CachedSize {
                size: 0,
            },
        },
    },
    SettingDefinition {
        type_: PER_USER,
        setting: MessageField(
            Some(
                Setting {
                    key: "ENABLED",
                    value: Some(
                        Bool(
                            false,
                        ),
                    ),
                    special_fields: SpecialFields {
                        unknown_fields: UnknownFields {
                            fields: None,
                        },
                        cached_size: CachedSize {
                            size: 0,
                        },
                    },
                },
            ),
        ),
        special_fields: SpecialFields {
            unknown_fields: UnknownFields {
                fields: None,
            },
            cached_size: CachedSize {
                size: 0,
            },
        },
    },
    SettingDefinition {
        type_: PER_USER,
        setting: MessageField(
            Some(
                Setting {
                    key: "MODE",
                    value: Some(
                        Str(
                            "COOL",
                        ),
                    ),
                    special_fields: SpecialFields {
                        unknown_fields: UnknownFields {
                            fields: None,
                        },
                        cached_size: CachedSize {
                            size: 0,
                        },
                    },
                },
            ),
        ),
        special_fields: SpecialFields {
            unknown_fields: UnknownFields {
                fields: None,
            },
            cached_size: CachedSize {
                size: 0,
            },
        },
    },
]
Registering the same configuration again
Updating setting 'ENABLED' to 'true'
```

#### User Profile Service output

```bash
UserProfileServer service bundle started.
Registered config HVAC: ConfigMetadata {
    version: "1.0",
    settings_definition: [
        SettingDefinition {
            type_: PER_USER,
            setting: MessageField(
                Some(
                    Setting {
                        key: "TEMPERATURE",
                        value: Some(
                            Int(
                                16,
                            ),
                        ),
                        special_fields: SpecialFields {
                            unknown_fields: UnknownFields {
                                fields: None,
                            },
                            cached_size: CachedSize {
                                size: 0,
                            },
                        },
                    },
                ),
            ),
            special_fields: SpecialFields {
                unknown_fields: UnknownFields {
                    fields: None,
                },
                cached_size: CachedSize {
                    size: 0,
                },
            },
        },
        SettingDefinition {
            type_: PER_USER,
            setting: MessageField(
                Some(
                    Setting {
                        key: "FAN_SPEED",
                        value: Some(
                            Int(
                                1,
                            ),
                        ),
                        special_fields: SpecialFields {
                            unknown_fields: UnknownFields {
                                fields: None,
                            },
                            cached_size: CachedSize {
                                size: 0,
                            },
                        },
                    },
                ),
            ),
            special_fields: SpecialFields {
                unknown_fields: UnknownFields {
                    fields: None,
                },
                cached_size: CachedSize {
                    size: 0,
                },
            },
        },
        SettingDefinition {
            type_: PER_USER,
            setting: MessageField(
                Some(
                    Setting {
                        key: "ENABLED",
                        value: Some(
                            Bool(
                                false,
                            ),
                        ),
                        special_fields: SpecialFields {
                            unknown_fields: UnknownFields {
                                fields: None,
                            },
                            cached_size: CachedSize {
                                size: 0,
                            },
                        },
                    },
                ),
            ),
            special_fields: SpecialFields {
                unknown_fields: UnknownFields {
                    fields: None,
                },
                cached_size: CachedSize {
                    size: 0,
                },
            },
        },
        SettingDefinition {
            type_: PER_USER,
            setting: MessageField(
                Some(
                    Setting {
                        key: "MODE",
                        value: Some(
                            Str(
                                "COOL",
                            ),
                        ),
                        special_fields: SpecialFields {
                            unknown_fields: UnknownFields {
                                fields: None,
                            },
                            cached_size: CachedSize {
                                size: 0,
                            },
                        },
                    },
                ),
            ),
            special_fields: SpecialFields {
                unknown_fields: UnknownFields {
                    fields: None,
                },
                cached_size: CachedSize {
                    size: 0,
                },
            },
        },
    ],
}
Config HVAC with version 1.0 is already registered
Updating config HVAC
Set 'ENABLED' to 'Bool(true)'
Invalid create user request, ID 0 is reserved for guest profile
Created profile with ID: 1
Successfully registered subscriber to config: ConfigId { service_fqin: "", config_name: "HVAC" }
```

#### HMI Service output

The HMI service perform the following steps:
    1. Create a user profile with ID 0, which is expected to fail since 0 is reserved for guest account
    2. Create a profile with a valid ID
    3. Subscribe to changes in the HVAC config

``bash
Attempting to create a user profile with a reserved ID
Encountered error as expected: Error { error_code: INVALID_REQUEST, message: "User ID '0' is reserved for guest profile", special_fields: SpecialFields { unknown_fields: UnknownFields { fields: None }, cached_size: CachedSize { size: 0 } } }
Creating a user profile
Successfully created profile with ID 1
Current HVAC configuration: Configuration {
    config_id: MessageField(
        Some(
            ConfigId {
                service_fqdn: "",
                config_name: "HVAC",
                special_fields: SpecialFields {
                    unknown_fields: UnknownFields {
                        fields: None,
                    },
                    cached_size: CachedSize {
                        size: 0,
                    },
                },
            },
        ),
    ),
    settings: [
        Setting {
            key: "ENABLED",
            value: Some(
                Bool(
                    true,
                ),
            ),
            special_fields: SpecialFields {
                unknown_fields: UnknownFields {
                    fields: None,
                },
                cached_size: CachedSize {
                    size: 0,
                },
            },
        },
        Setting {
            key: "TEMPERATURE",
            value: Some(
                Int(
                    16,
                ),
            ),
            special_fields: SpecialFields {
                unknown_fields: UnknownFields {
                    fields: None,
                },
                cached_size: CachedSize {
                    size: 0,
                },
            },
        },
        Setting {
            key: "FAN_SPEED",
            value: Some(
                Int(
                    1,
                ),
            ),
            special_fields: SpecialFields {
                unknown_fields: UnknownFields {
                    fields: None,
                },
                cached_size: CachedSize {
                    size: 0,
                },
            },
        },
        Setting {
            key: "MODE",
            value: Some(
                Str(
                    "COOL",
                ),
            ),
            special_fields: SpecialFields {
                unknown_fields: UnknownFields {
                    fields: None,
                },
                cached_size: CachedSize {
                    size: 0,
                },
            },
        },
    ],
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
