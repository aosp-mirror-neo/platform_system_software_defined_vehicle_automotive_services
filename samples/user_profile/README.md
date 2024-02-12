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

    adb shell /vendor/bin/sdv_user_profile_service

    ```

3. Launch the HVAC Service

    ```bash

    adb root

    adb shell /vendor/bin/sdv_hvac_service

    ```

### Expected output

#### HVAC Service Output

The HVAC service attempts to register the same configuration twice, and we expect the user profile service to ignore the second request.

```bash
Registering config with ID: hvac,
Version: 1.0
Settings: [
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
]
Registering the same configuration again
```

#### User Profile Service output

```bash
Received request to register configuration 'hvac'
Registered config hvac: ConfigMetadata {
    version: "1.0",
    settings_definition: [
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
    ],
}
Received request to register configuration 'hvac'
Config hvac with version 1.0 is already registered
```
