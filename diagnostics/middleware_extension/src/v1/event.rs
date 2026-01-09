// Copyright 2024 Google LLC

use diagnostics_v1_api::event::{self, Event, Status};
use protobuf::Message;
use sdv_mw_rs_comms::{
    codec, Communicate, Publish, PublisherAttributes, PublisherMetadata, TypeName,
};
use sdv_proto_reflection_rs::message_metadata::MessageMetadata;
use sdv_status_rs::SdvResult;
use sdv_status_rs::{SdvStatus, SdvStatusCode};
use std::marker::PhantomData;
use std::sync::Arc;
use vsidl_v1_stdlib::annotations;

/// Publisher of diagnostics events.
pub struct EventPublisher<M> {
    publish: Box<dyn Publish>,
    phantom: PhantomData<M>,
}

impl<M: Message> EventPublisher<M> {
    /// Create new publisher for diagnostics event.
    pub async fn new(
        data_size: u32,
        comms: Arc<dyn Communicate>,
        publication_name: String,
    ) -> SdvResult<Self> {
        let options = EventPublisherOptions::builder()
            .publication_name(publication_name)
            .data_size(data_size)
            .build()?;

        Self::new_with_options(comms, &options).await
    }

    /// Create new publisher for diagnostics event, with options.
    pub async fn new_with_options(
        comms: Arc<dyn Communicate>,
        options: &EventPublisherOptions,
    ) -> SdvResult<Self> {
        let publish = comms
            .create_publish(
                &PublisherAttributes::builder()
                    .publication_name(options.get_publication_name())
                    .type_name(
                        TypeName::builder()
                            .package_name("com.sdv.google.diagnostics")
                            .bundle_name("DiagnosticsManager")
                            .type_name("event.Event")
                            .build()?,
                    )
                    .metadata(
                        PublisherMetadata::builder()
                            .message_size(compute_event_message_size(options.get_data_size()))
                            .message_count(16)
                            .is_multipub(true)
                            .message_metadata(event_message_metadata())
                            .message_metadata_required(options.register_reflection_metadata())
                            .build()?,
                    )
                    .build()?,
            )
            .await?;
        Ok(Self { publish, phantom: PhantomData })
    }

    /// Publish diagnostics event without extended data.
    pub fn publish(&self, status: Status) -> SdvResult<()> {
        self.publish_internal(status, None)
    }

    /// Publish diagnostics event with extended data.
    pub fn publish_with_data(&self, status: Status, data: M) -> SdvResult<()> {
        self.publish_internal(status, Some(data))
    }

    fn publish_internal(&self, status: Status, data: Option<M>) -> SdvResult<()> {
        let event_data = codec::encode(&Event {
            status: status.into(),
            extended_data: data.as_ref().map(encode_event_extended_data).transpose()?,
            ..Default::default()
        })?;

        self.publish.publish_messages(&[event_data])
    }
}

/// EventPublisherOptions
#[derive(Debug, Default, Clone)]
pub struct EventPublisherOptions {
    /// publication name
    publication_name: String,
    /// event data size
    data_size: u32,
    /// true if metadata for message reflection shall be registered
    register_reflection_metadata: bool,
}

impl EventPublisherOptions {
    /// returns a Builder for EventPublisherOptions
    pub fn builder() -> EventPublisherOptionsBuilder {
        EventPublisherOptionsBuilder::default()
    }

    /// publication name
    pub fn get_publication_name(&self) -> &str {
        &self.publication_name
    }

    /// event data size
    pub fn get_data_size(&self) -> u32 {
        self.data_size
    }

    /// true if metadata for message reflection shall be registered
    pub fn register_reflection_metadata(&self) -> bool {
        self.register_reflection_metadata
    }
}

/// EventPublisherOptionsBuilder
#[derive(Debug, Default, Clone)]
pub struct EventPublisherOptionsBuilder {
    /// publication name
    publication_name: String,
    /// event data size
    data_size: u32,
    /// true if metadata for message reflection shall be registered
    register_reflection_metadata: bool,
}

impl EventPublisherOptionsBuilder {
    /// set publication name
    pub fn publication_name(mut self, publication_name: impl Into<String>) -> Self {
        self.publication_name = publication_name.into();
        self
    }

    ///  set event data size
    pub fn data_size(mut self, data_size: u32) -> Self {
        self.data_size = data_size;
        self
    }

    /// set true if metadata for message reflection shall be registered
    pub fn register_reflection_metadata(mut self, register_reflection_metadata: bool) -> Self {
        self.register_reflection_metadata = register_reflection_metadata;
        self
    }

    /// Validates EventPublisherOptions before returning an instance of it.
    pub fn build(self) -> SdvResult<EventPublisherOptions> {
        if self.publication_name.is_empty() {
            return Err(SdvStatus::with_message(
                SdvStatusCode::InvalidArgument,
                "publication_name must not be empty.",
            ));
        }
        Ok(EventPublisherOptions {
            publication_name: self.publication_name,
            data_size: self.data_size,
            register_reflection_metadata: self.register_reflection_metadata,
        })
    }
}

fn event_message_metadata() -> MessageMetadata {
    MessageMetadata {
        message_full_name: ".com.sdv.google.diagnostics.event.Event".to_string(),
        file_descriptors: vec![
            event::file_descriptor().proto().clone(),
            annotations::file_descriptor().proto().clone(),
        ],
        ..Default::default()
    }
}

fn encode_event_extended_data<M: Message>(data: &M) -> SdvResult<Vec<u8>> {
    data.write_to_bytes().map_err(|_| SdvStatus::new(SdvStatusCode::InvalidArgument))
}

// Compute size of com.sdv.google.diagnostics.event.Event size, based on exteded data max size.
fn compute_event_message_size(extended_data_max_size: u32) -> u32 {
    let mut result = 0;

    // 'status' field tag
    result += compute_tag_size(1);
    // 'status' field max size (Type.ENUM)
    result += 10;
    // 'extended_data' field tag
    result += compute_tag_size(2);
    // max size of 'extended_data' length encoding
    result += compute_length_size(extended_data_max_size);
    // max size of 'extended_data' field
    result += extended_data_max_size;

    result
}

/// Compute size of the tag segment in protobuf wire format
fn compute_tag_size(tag: u32) -> u32 {
    if tag < (1 << 4) {
        1
    } else if tag < (1 << 11) {
        2
    } else {
        3
    }
}

/// Compute size of the length segment in protobuf wire format
fn compute_length_size(content_size: u32) -> u32 {
    if content_size < (1 << 7) {
        1
    } else if content_size < (1 << 14) {
        2
    } else if content_size < (1 << 21) {
        3
    } else if content_size < (1 << 28) {
        4
    } else {
        5
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::test_messages::UnserializableMessage;
    use diagnostics_v1_api::event::Event;
    use diagnostics_v1_api::event::{OperationCycle as TestProto, Status};
    use futures_util::StreamExt;
    use googletest::assert_that;
    use googletest::prelude::eq;
    use protobuf::reflect::{FileDescriptor, MessageDescriptor};
    use protobuf::EnumOrUnknown;
    use protobuf::MessageDyn;
    use protobuf::MessageFull;
    use protobuf::{well_known_types, Message};
    use sdv_comms::{
        id::ServiceFqin,
        sd::{ServiceDiscoveryManager, UnitNameDiscoveryArgs},
        ContextRef,
    };
    use sdv_mw_local_comms::LocalComms;
    use sdv_mw_rs_comms::*;
    use sdv_mw_rs_sdv_comms::SdvComms;
    use std::str::FromStr;

    #[tokio::test]
    async fn test_compute_length_size() {
        assert_that!(compute_length_size(0), eq(1));
        assert_that!(compute_length_size(0b1000000), eq(1));
        assert_that!(compute_length_size(0b10000000), eq(2));
        assert_that!(compute_length_size(0x2000), eq(2));
        assert_that!(compute_length_size(0x4000), eq(3));
        assert_that!(compute_length_size(0x100000), eq(3));
        assert_that!(compute_length_size(0x200000), eq(4));
        assert_that!(compute_length_size(0xF000000), eq(4));
        assert_that!(compute_length_size(0x10000000), eq(5));
    }

    #[tokio::test]
    async fn test_compute_tag_size() {
        assert_that!(compute_tag_size(0), eq(1));
        assert_that!(compute_tag_size(0b1000), eq(1));
        assert_that!(compute_tag_size(0b10000), eq(2));
        assert_that!(compute_tag_size(0x400), eq(2));
        assert_that!(compute_tag_size(0x800), eq(3));
    }

    #[tokio::test]
    async fn test_new() {
        let comms = LocalComms::default();
        let event_publisher =
            EventPublisher::<TestProto>::new(1, Arc::new(comms), "event1".to_string()).await;

        assert!(event_publisher.is_ok());
    }

    #[tokio::test]
    async fn test_publish() {
        // GIVEN event publication and subscription
        let comms = Arc::new(LocalComms::default());
        let event_publisher =
            EventPublisher::<TestProto>::new(10, comms.clone(), "event1".to_string())
                .await
                .unwrap();

        let event_publication = comms
            .find_publication(&UnitQuery::Name(
                UnitName::builder()
                    .vm_name("")
                    .package_name("com.sdv.oem.sample.diagnostics")
                    .bundle_name("DiagnosticsProvider1")
                    .service_unit_name("event1")
                    .build()
                    .unwrap(),
            ))
            .await
            .unwrap();

        let event_subscriber = Subscriber::<Event>::new(
            event_publication.subscribe(&SubscribeOptions::default()).await.unwrap(),
        );

        // WHEN publisher publishes and subscriber reads
        event_publisher.publish(Status::PRE_FAIL).unwrap();
        let messages = event_subscriber.read_next_messages(1).unwrap();

        // THEN event message is read and decoded
        assert_that!(messages.len(), eq(1));
        let message = &messages[0];

        assert_that!(message.status, eq(Status::PRE_FAIL.into()));
        assert!(message.extended_data.is_none());
    }

    #[tokio::test]
    async fn test_publish_with_data() {
        // GIVEN event publication and subscription
        let comms = Arc::new(LocalComms::default());
        let event_publisher = EventPublisher::<TestProto>::new(
            10,
            comms.clone(),
            "sdv-diagnostics-event-event1".to_string(),
        )
        .await
        .unwrap();

        let event_publication = comms
            .find_publication(&UnitQuery::Name(
                UnitName::builder()
                    .vm_name("")
                    .package_name("com.sdv.oem.sample.diagnostics")
                    .bundle_name("DiagnosticsProvider1")
                    .service_unit_name("sdv-diagnostics-event-event1")
                    .build()
                    .unwrap(),
            ))
            .await
            .unwrap();

        let event_subscriber = Subscriber::<Event>::new(
            event_publication.subscribe(&SubscribeOptions::default()).await.unwrap(),
        );

        // WHEN publisher publishes and subscriber reads
        event_publisher
            .publish_with_data(
                Status::PRE_FAIL,
                TestProto { id: "test".to_string(), ..Default::default() },
            )
            .unwrap();
        let messages = event_subscriber.read_next_messages(1).unwrap();

        // THEN event message is read and decoded
        assert_that!(messages.len(), eq(1));
        let message = &messages[0];

        assert_that!(message.status, eq(Status::PRE_FAIL.into()));

        let extended_data = message.extended_data.clone().unwrap();
        let decoded_data = TestProto::parse_from_bytes(&extended_data).unwrap();
        assert_that!(decoded_data, eq(&TestProto { id: "test".to_string(), ..Default::default() }));
    }

    #[tokio::test]
    async fn test_publish_with_malformed_data() {
        // GIVEN event publication and subscription
        let comms = Arc::new(LocalComms::default());
        let event_publisher = EventPublisher::<UnserializableMessage>::new(
            1,
            comms.clone(),
            "sdv-diagnostics-event-event1".to_string(),
        )
        .await
        .unwrap();

        // WHEN publisher publishes and subscriber reads
        let publication_result =
            event_publisher.publish_with_data(Status::PRE_FAIL, UnserializableMessage::default());

        // THEN fails to publish
        assert!(publication_result.is_err());
    }

    #[test]
    fn test_compute_event_message_size() {
        // Assert the number of fields hasn't changed since this test was written.
        // Adding new fields will require adjusting this test.
        let descriptor = Event::descriptor();
        assert_that!(descriptor.fields().count(), eq(2));

        // Verify compute function for different values of max_size
        for max_size in (0..29).map(|i| 2u32.pow(i)).collect::<Vec<_>>() {
            // Create the largest possible message with max_size of extended data and get its size.
            let message = Event {
                status: EnumOrUnknown::<Status>::from_i32(i32::MIN + 1),
                extended_data: vec![0; max_size.try_into().unwrap()].into(),
                ..Default::default()
            };
            let expected_computed_message_size = message.compute_size();

            let actual_computed_message_size = compute_event_message_size(max_size) as u64;

            assert_that!(actual_computed_message_size, eq(expected_computed_message_size));
        }
    }

    #[test]
    fn test_event_message_metadata() {
        let parsed_event_descriptor = parse_message_descriptor(event_message_metadata());

        let event_descriptor = Event { ..Default::default() }.descriptor_dyn();

        assert_that!(parsed_event_descriptor.proto(), eq(event_descriptor.proto()));
    }

    #[tokio::test]
    async fn test_event_message_metadata_is_set() {
        // Note: ACLs for this test are in the respective acl textproto file of the com.sdv.oem.sample.diagnostics apex
        // GIVEN event publication where register_reflection_metadata is true
        const REGISTER_REFLECTION_METADATA: bool = true;
        const USE_NEW_WITH_OPTIONS: bool = true;
        let service_unit = create_event_publisher_service_unit(
            "with-metadata",
            REGISTER_REFLECTION_METADATA,
            USE_NEW_WITH_OPTIONS,
        )
        .await;

        // GIVEN parsing event message descriptor from publication metadata
        let parsed_event_descriptor = parse_message_descriptor(
            MessageMetadata::parse_from_bytes(&service_unit.get_application_metadata())
                .expect("parsing message descriptors failed"),
        );

        // THEN parsed descriptor is the same as Event message descriptor
        let event_descriptor = Event { ..Default::default() }.descriptor_dyn();

        assert_that!(parsed_event_descriptor.proto(), eq(event_descriptor.proto()));
    }

    #[tokio::test]
    async fn test_event_message_metadata_is_not_set() {
        // Note: ACLs for this test are in the respective acl textproto file of the com.sdv.oem.sample.diagnostics apex
        // GIVEN event publication where register_reflection_metadata is false
        const REGISTER_REFLECTION_METADATA: bool = false;
        const USE_NEW_WITH_OPTIONS: bool = true;
        let service_unit = create_event_publisher_service_unit(
            "without-metadata",
            REGISTER_REFLECTION_METADATA,
            USE_NEW_WITH_OPTIONS,
        )
        .await;

        // THEN the application metadata is empty
        assert_that!(service_unit.get_application_metadata().is_empty(), eq(true));
    }

    #[tokio::test]
    async fn test_event_message_metadata_is_not_set_for_legacy_ctor() {
        // Note: ACLs for this test are in the respective acl textproto file of the com.sdv.oem.sample.diagnostics apex
        // GIVEN event publication created with legacy 'new()' ctor
        const REGISTER_REFLECTION_METADATA: bool = true;
        const USE_NEW_WITH_OPTIONS: bool = false;
        let service_unit = create_event_publisher_service_unit(
            "legacy",
            REGISTER_REFLECTION_METADATA,
            USE_NEW_WITH_OPTIONS,
        )
        .await;

        // THEN the application metadata is empty
        assert_that!(service_unit.get_application_metadata().is_empty(), eq(true));
    }

    // Ensure that all calls to this method pass a different instance name within the same test target.
    async fn create_event_publisher_service_unit(
        instance_name: &str,
        register_reflection_metadata: bool,
        use_ctor_new_with_options: bool,
    ) -> sdv_comms::sd::ServiceUnitDefinition {
        // create event publisher
        let context = ContextRef::create(
            ServiceFqin::from_str(&format!(
                "vm:com.sdv.oem.sample.diagnostics.SampleBundle/{instance_name}",
            ))
            .expect("ContextRef creation failed."),
        );
        let comms = Arc::new(SdvComms { context });

        // ctor: new_with_options vs. new(legacy)
        let _event_publisher = if use_ctor_new_with_options {
            EventPublisher::<TestProto>::new_with_options(
                comms.clone(),
                &EventPublisherOptions::builder()
                    .publication_name("event")
                    .data_size(10)
                    .register_reflection_metadata(register_reflection_metadata)
                    .build()
                    .unwrap(),
            )
            .await
            .unwrap()
        } else {
            EventPublisher::<TestProto>::new(10, comms.clone(), "event".to_string()).await.unwrap()
        };

        // discover event publisher service unit
        let sd = ServiceDiscoveryManager::new(context);
        let discovery_args = UnitNameDiscoveryArgs::new_builder()
            .set_sdv_package_name("com.sdv.oem.sample.diagnostics")
            .set_service_bundle_name("SampleBundle")
            .set_service_unit_name("event")
            .build()
            .expect("Discovery args builder failed.");

        let service_units = sd.list_units_by_name(&discovery_args).collect::<Vec<_>>().await;
        service_units
            .iter()
            .find(|service_unit| {
                service_unit.get_service_bundle_identity().get_fqin().get_service_instance_name()
                    == instance_name
            })
            .expect("Instance not discovered")
            .clone()
    }

    fn parse_message_descriptor(message_metadata: MessageMetadata) -> MessageDescriptor {
        // file descriptors for proto well_known_types are not passed over the wire.
        let wellknown_descriptors: Vec<FileDescriptor> = vec![
            well_known_types::any::file_descriptor().clone(),
            well_known_types::api::file_descriptor().clone(),
            well_known_types::duration::file_descriptor().clone(),
            well_known_types::empty::file_descriptor().clone(),
            well_known_types::field_mask::file_descriptor().clone(),
            well_known_types::source_context::file_descriptor().clone(),
            well_known_types::struct_::file_descriptor().clone(),
            well_known_types::timestamp::file_descriptor().clone(),
            well_known_types::type_::file_descriptor().clone(),
            well_known_types::wrappers::file_descriptor().clone(),
            protobuf::descriptor::file_descriptor().clone(),
        ];

        // message-specific file descriptor.
        let file_descriptors = FileDescriptor::new_dynamic_fds(
            message_metadata.file_descriptors,
            &wellknown_descriptors,
        )
        .expect("Creating file descriptors failed!");

        let message_descriptor = file_descriptors
            .iter()
            .find_map(|fd| fd.message_by_full_name(&message_metadata.message_full_name))
            .expect("message descriptor was not found");
        message_descriptor
    }

    #[test]
    fn test_event_publisher_options_builder() {
        let publication_name = "some-name";
        let data_size = 18;
        let register_reflection_metadata = true;

        // GIVEN publication_name is not set.
        let result = EventPublisherOptions::builder()
            .data_size(data_size)
            .register_reflection_metadata(register_reflection_metadata)
            .build();

        // THEN build returns an error.
        assert_that!(result.is_err(), eq(true));

        // GIVEN all values are valid
        let result = EventPublisherOptions::builder()
            .publication_name(publication_name)
            .data_size(data_size)
            .register_reflection_metadata(register_reflection_metadata)
            .build();
        assert_that!(result.is_ok(), eq(true));
        // THEN the correcct attributes are set
        let event_publisher_options = result.unwrap();
        assert_that!(event_publisher_options.get_publication_name(), eq(publication_name));
        assert_that!(event_publisher_options.get_data_size(), eq(data_size));
        assert_that!(
            event_publisher_options.register_reflection_metadata(),
            eq(register_reflection_metadata)
        );
    }
}
