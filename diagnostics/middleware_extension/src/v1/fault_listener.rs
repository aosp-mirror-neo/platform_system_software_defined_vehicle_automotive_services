/*
 * Copyright (C) 2026 The Android Open Source Project
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use diagnostics_v1_api::fault_listener_service::OnFaultStatusChangeResponse;
use protobuf::Message;

/// Typed trait that corresponds to FaultListener server
///
/// See com.android.sdv.diagnostics.fault.FaultListener service.
#[async_trait::async_trait]
pub trait FaultListenerHandler<T: Message>: Send + Sync {
    /// See com.android.sdv.diagnostics.fault.FaultListener::OnFaultStatusChange.
    async fn on_fault_status_change(&self, status: u8, extended_data: Option<T>);

    /// Untyped version of `on_fault_status_change`.
    async fn on_fault_status_change_untyped(
        &self,
        status: u8,
        extended_data_bytes: Option<&[u8]>,
    ) -> OnFaultStatusChangeResponse {
        let parsed_exteded_data = extended_data_bytes.map(T::parse_from_bytes).transpose();

        if let Ok(extended_data) = parsed_exteded_data {
            self.on_fault_status_change(status, extended_data).await;
        }

        OnFaultStatusChangeResponse::default()
    }
}

#[cfg(test)]
mod tests {
    use crate::fault_listener::FaultListenerHandler;
    use diagnostics_v1_api::event::Event as TestProto;
    use diagnostics_v1_api::event::Status;
    use diagnostics_v1_api::fault_listener_service::OnFaultStatusChangeResponse;
    use protobuf::Message;

    const STATUS: u8 = 1;

    struct TestFaultListenerHandler {}

    #[async_trait::async_trait]
    impl FaultListenerHandler<TestProto> for TestFaultListenerHandler {
        async fn on_fault_status_change(&self, _: u8, _: Option<TestProto>) {}
    }

    #[tokio::test]
    async fn test_on_fault_status_change_untyped() {
        let message_bytes = TestProto { status: Status::PASS.into(), ..Default::default() }
            .write_to_bytes()
            .unwrap();

        let handler = TestFaultListenerHandler {};

        assert_eq!(
            handler.on_fault_status_change_untyped(STATUS, Some(&message_bytes)).await,
            OnFaultStatusChangeResponse::default(),
        );
    }

    #[tokio::test]
    async fn test_on_fault_status_change_untyped_format_err() {
        let invalid_message_bytes = vec![2, 1];

        let handler = TestFaultListenerHandler {};

        assert_eq!(
            handler.on_fault_status_change_untyped(STATUS, Some(&invalid_message_bytes)).await,
            OnFaultStatusChangeResponse::default()
        );
    }

    #[tokio::test]
    async fn test_on_fault_status_change_untyped_empty_extended_data() {
        let handler = TestFaultListenerHandler {};

        assert_eq!(
            handler.on_fault_status_change_untyped(STATUS, None).await,
            OnFaultStatusChangeResponse::default()
        );
    }
}
