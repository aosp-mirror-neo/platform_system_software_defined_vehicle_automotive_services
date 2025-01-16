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
use vsidl_v1::annotations;

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
        let publish = comms
            .create_publish(
                &PublisherAttributes::builder()
                    .publication_name(publication_name)
                    .type_name(
                        TypeName::builder()
                            .package_name("com.sdv.google.diagnostics")
                            .bundle_name("DiagnosticsManager")
                            .type_name("event.Event")
                            .build()?,
                    )
                    .metadata(
                        PublisherMetadata::builder()
                            .message_size(compute_event_message_size(data_size))
                            .message_count(16)
                            .is_multipub(true)
                            .message_metadata(event_message_metadata())
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
            extended_data: data
                .as_ref()
                .map(encode_event_extended_data)
                .transpose()?
                .map(Vec::from),
            ..Default::default()
        })?;

        self.publish.publish_messages(&[event_data])
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
    use diagnostics_v1_api::event::Event;
    use diagnostics_v1_api::event::{OperationCycle as TestProto, Status};
    use futures_util::StreamExt;
    use googletest::assert_that;
    use googletest::prelude::eq;
    use protobuf::reflect::{FileDescriptor, MessageDescriptor};
    use protobuf::EnumOrUnknown;
    use protobuf::MessageDyn;
    use protobuf::{well_known_types, MessageFull};
    use sdv_comms::{
        id::ServiceFqin,
        sd::{ServiceDiscoveryManager, UnitNameDiscoveryArgs},
        ContextRef,
    };
    use sdv_mw_local_comms::LocalComms;
    use sdv_mw_rs_comms::{codec::decode, *};
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
        let _ = messages.iter().inspect(|message| {
            assert_that!(message.status, eq(Status::PRE_FAIL.into()));

            assert!(message.extended_data.is_none());
        });
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
        let _ = messages.iter().inspect(|message| {
            assert_that!(message.status, eq(Status::PRE_FAIL.into()));

            let decoded_data =
                message.extended_data.as_ref().map(|data| decode::<TestProto>(data).unwrap());
            assert_that!(
                decoded_data.unwrap(),
                eq(&TestProto { id: "test".to_string(), ..Default::default() })
            );
        });
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
        // GIVEN event publication
        let context = ContextRef::create(
            ServiceFqin::from_str("vm:com.sdv.oem.sample.diagnostics.SampleBundle/instance")
                .unwrap(),
        );
        let comms = Arc::new(SdvComms { context });
        let _event_publisher =
            EventPublisher::<TestProto>::new(10, comms.clone(), "event".to_string()).await.unwrap();

        let sd = ServiceDiscoveryManager::new(context);
        let discovery_args = UnitNameDiscoveryArgs::new_builder()
            .set_sdv_package_name("com.sdv.oem.sample.diagnostics")
            .set_service_bundle_name("SampleBundle")
            .set_service_unit_name("event")
            .build()
            .expect("Discovery args builder failed.");
        let service_unit = sd.list_units_by_name(&discovery_args).next().await.unwrap();

        // GIVEN parsing event message descriptor from publication metadata
        let parsed_event_descriptor = parse_message_descriptor(
            MessageMetadata::parse_from_bytes(&service_unit.get_application_metadata())
                .expect("parsing message descriptors failed"),
        );

        // THEN parsed descriptor is the same as Event message descriptor
        let event_descriptor = Event { ..Default::default() }.descriptor_dyn();

        assert_that!(parsed_event_descriptor.proto(), eq(event_descriptor.proto()));
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
        let file_descriptors = match FileDescriptor::new_dynamic_fds(
            message_metadata.file_descriptors,
            &wellknown_descriptors,
        ) {
            Err(e) => {
                // this reveals exactly which dependency is missing.
                eprintln!("{}", e);
                panic!("Creating file descriptors failed!")
            }
            Ok(fd) => fd,
        };
        let message_descriptor = file_descriptors
            .iter()
            .find_map(|fd| fd.message_by_full_name(&message_metadata.message_full_name))
            .expect("message descriptor was not found");
        message_descriptor
    }
}
