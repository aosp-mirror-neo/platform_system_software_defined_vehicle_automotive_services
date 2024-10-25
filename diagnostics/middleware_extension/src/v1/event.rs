// Copyright 2024 Google LLC

use diagnostics_v1_api::event::{Event, Status};
use protobuf::Message;
use sdv_mw_rs_comms::{
    codec, Communicate, Publish, PublisherAttributes, PublisherMetadata, TypeName,
};
use sdv_status_rs::SdvResult;
use std::marker::PhantomData;
use std::sync::Arc;

/// Publisher of diagnostics events.
pub struct EventPublisher<M> {
    publish: Box<dyn Publish>,
    phantom: PhantomData<M>,
}

impl<M: Message> EventPublisher<M> {
    /// Create new publisher for diagnostics event.
    pub async fn new(
        event_name: &'static str,
        data_size: u32,
        comms: Arc<dyn Communicate>,
    ) -> SdvResult<Self> {
        let publish = comms
            .create_publish(&PublisherAttributes {
                publication_name: format!("sdv-diagnostics-event-{}", event_name.replace('.', "-")),
                type_name: TypeName {
                    package: "sdv.diagnostics".to_string(),
                    bundle: "DiagnosticsManager".to_string(),
                    name: "event.Event".to_string(),
                },
                metadata: PublisherMetadata {
                    message_size: compute_event_message_size(data_size),
                    message_count: 16,
                    multipub: true,
                    ..Default::default()
                },
            })
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
                .map(|data| codec::encode(data))
                .transpose()?
                .map(Vec::from),
            ..Default::default()
        })?;

        self.publish.publish_messages(&[event_data])
    }
}

// Compute size of sdv.diagnostics.event.Event size, based on exteded data max size.
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
    use googletest::assert_that;
    use googletest::prelude::eq;
    use sdv_mw_local_comms::LocalComms;
    use sdv_mw_rs_comms::{codec::decode, *};

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
        let event_publisher = EventPublisher::<TestProto>::new("event1", 1, Arc::new(comms)).await;

        assert!(event_publisher.is_ok());
    }

    #[tokio::test]
    async fn test_publish() {
        // GIVEN event publication and subscription
        let comms = Arc::new(LocalComms::default());
        let event_publisher =
            EventPublisher::<TestProto>::new("event1", 10, comms.clone()).await.unwrap();

        let event_publication = comms
            .find_publication(&UnitQuery::Name(UnitName {
                vm: "".to_string(),
                package: "sdv.diagnostics.sample".to_string(),
                bundle: "DiagnosticsProvider1".to_string(),
                name: "sdv-diagnostics-event-event1".to_string(),
            }))
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
        let event_publisher =
            EventPublisher::<TestProto>::new("event1", 10, comms.clone()).await.unwrap();

        let event_publication = comms
            .find_publication(&UnitQuery::Name(UnitName {
                vm: "".to_string(),
                package: "sdv.diagnostics.sample".to_string(),
                bundle: "DiagnosticsProvider1".to_string(),
                name: "sdv-diagnostics-event-event1".to_string(),
            }))
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
                eq(TestProto { id: "test".to_string(), ..Default::default() })
            );
        });
    }
}
