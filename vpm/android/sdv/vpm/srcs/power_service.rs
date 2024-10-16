// Copyright 2024 Google LLC
//! Vehicle and Power Mode Agent.
//!
use crate::power_state::*;
use crate::power_state_notifier::PowerStateNotificationError;
use crate::PowerStateNotifier;
use crate::PowerStateNotifierImpl;
use crate::POWER_STATE_CHANNEL_QUEUE_SIZE;
use anyhow::anyhow;
use async_trait::async_trait;
use log::{debug, error, info};
use mockall::automock;
use sdv_mw_rs_sdv_comms::SdvComms;
use sdv_vpm_api::power::PowerStateReport;
use sdv_vpm_api::vpm::PowerStateChangeRequest;
use sdv_vpm_api::vpm::PowerStateRequest;
use sdv_vpm_api::vpm::ShutdownType;
use sdv_vpm_api::vpm_notifications::power_state_notification_request::PowerStateChangeReason;
use std::sync::Arc;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::watch::{
    channel as watch_channel, Receiver as WatchReceiver, Sender as WatchSender,
};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

#[automock]
#[async_trait]
pub trait PowerService: Send + Sync {
    async fn start(&mut self) -> anyhow::Result<()>;
    fn get_power_state_request_sender(&self) -> Sender<PowerRequest>;
    fn get_power_state_watch_receiver(&self) -> WatchReceiver<Arc<dyn PowerState>>;
}

struct PowerStateTracker {
    cancel_token: CancellationToken,
    power_state_handle: Option<JoinHandle<anyhow::Result<()>>>,
    power_report: Option<PowerStateReport>,
}

impl PowerStateTracker {
    fn new() -> Self {
        Self {
            cancel_token: CancellationToken::new(),
            power_state_handle: None,
            power_report: None,
        }
    }

    fn set_power_state_handle(
        &mut self,
        handle: JoinHandle<anyhow::Result<()>>,
        power_report: PowerStateReport,
    ) {
        self.power_state_handle = Some(handle);
        self.power_report = Some(power_report);
    }
}

pub struct PowerServiceImpl {
    power_state_notifier: Arc<dyn PowerStateNotifier>,
    power_state_request_sender: Sender<PowerRequest>,
    power_state_request_receiver: Receiver<PowerRequest>,
    power_state_watch_sender: Arc<WatchSender<Arc<dyn PowerState>>>,
    power_state_watch_receiver: WatchReceiver<Arc<dyn PowerState>>,
    power_state_tracker: PowerStateTracker,
}

impl PowerServiceImpl {
    /// Creates a new PowerServiceImpl with the default value of the power state being POWER_OFF_EXIST.
    /// The POWER_OFF_EXIT state is the only possible state at this step because calling new() means we are coming from cold boot.
    /// If we resume from S2R or S2D new() should not be called again.
    pub fn new(comms: Arc<SdvComms>) -> Self {
        Self::new_with_notifier(Arc::new(PowerStateNotifierImpl::new(comms)))
    }

    fn new_with_notifier(power_state_notifier: Arc<dyn PowerStateNotifier>) -> Self {
        //TODO: check which suspend services is provided by the Host.

        // Channel to forward power state requests.
        let (power_state_request_sender, power_state_request_receiver) =
            channel::<PowerRequest>(POWER_STATE_CHANNEL_QUEUE_SIZE);

        // Watch channel that holds the value for current power state.
        let (power_state_watch_sender, power_state_watch_receiver) =
            watch_channel::<Arc<dyn PowerState>>(Arc::new(PowerOffExit {}));

        power_state_notifier.create_binder_service(power_state_watch_receiver.clone()).unwrap();

        Self {
            power_state_notifier,
            power_state_request_sender,
            power_state_request_receiver,
            power_state_watch_sender: power_state_watch_sender.into(),
            power_state_watch_receiver,
            power_state_tracker: PowerStateTracker::new(),
        }
    }

    // Power state request should be already validated processing the new power state.
    async fn process_power_request(
        &self,
        power_request: PowerRequest,
    ) -> JoinHandle<anyhow::Result<()>> {
        let cloned_power_state_notifier = self.power_state_notifier.clone();
        let cloned_power_state_watch_sender = self.power_state_watch_sender.clone();
        let cloned_power_state_request_sender = self.power_state_request_sender.clone();
        let cloned_cancellation_token: CancellationToken =
            self.power_state_tracker.cancel_token.clone();
        tokio::spawn(async move {
            debug!(
                "Handling new power state {:?} with timeout {:?}!",
                power_request.power_state.get_report(),
                power_request.timeout_ms
            );

            // Setting the power state so that it can be cancelled if necessary.
            let new_state: Arc<dyn PowerState> = Arc::from(power_request.power_state);
            cloned_power_state_watch_sender.send(new_state.clone())?;

            let select_result = tokio::select! {
                 _ = sleep(Duration::from_millis(power_request.timeout_ms)) => {
                     debug!("Timeout reached");
                     Err(PowerStateNotificationError::TimedOut)
                 },
                 _ = cloned_cancellation_token.cancelled() => {
                    debug!("Power state transition was  cancelled");
                    Err(PowerStateNotificationError::Cancelled)
                },
                // Notify SDV agent callbacks.
                result  = cloned_power_state_notifier.notify(new_state.get_report()) => {
                    result
                }
            };

            // Interrupted states which have been cancelled should not be set or notified to OEM.
            // After this point cancellation does not make sense.
            // All callbacks have been already notified.
            if cloned_cancellation_token.is_cancelled() && select_result.as_ref().is_err() {
                debug!("State was interrupted and cancelled {:?}", new_state.get_report());
                return Err(anyhow!("The state was interrupted and cancelled."));
            }

            debug!("Start post state execution of {:?}", new_state.get_report());
            let execution_result = new_state
                .execute(cloned_power_state_request_sender, power_request.timeout_ms)
                .await;

            if execution_result.is_err() {
                error!("Could not execute state.");
            }

            // We don't need to notify OEM about sleep reports, because new_state.execute will suspend the VM including this process or power-off VM.
            // If the VM suspends, the following code will only be executed once the process resumes from suspension, therefore the report will be outdated.
            // The new_state.execute will also request a transition to the new resume state which will be received by the power_state_request_receiver.
            if !new_state.is_sleep_state() {
                notify_oem(new_state, select_result, cloned_power_state_notifier).await;
            }

            Ok(())
        })
    }
}

async fn notify_oem(
    new_state: Arc<dyn PowerState>,
    select_result: anyhow::Result<(), PowerStateNotificationError>,
    cloned_power_state_notifier: Arc<dyn PowerStateNotifier>,
) {
    match select_result {
        Ok(_) => {
            info!(
                "Successfully notified all callbacks, notifying OEM for {:?}!",
                new_state.get_report()
            );
            let result = cloned_power_state_notifier
                .notify_oem(new_state.get_report(), PowerStateChangeReason::HOST_REQUESTED)
                .await;
            debug!("OEM state update result {:?} {:?}", new_state.get_report(), result);
        }
        Err(PowerStateNotificationError::TimedOut) => {
            debug!("Call to notify callbacks has timed-out!");
            let result = cloned_power_state_notifier
                .notify_oem(new_state.get_report(), PowerStateChangeReason::TIMEOUT)
                .await;
            debug!("OEM state update result {:?} {:?}", new_state.get_report(), result);
        }
        Err(e) => {
            debug!("Failed to notify one of the callbacks! error={e}");
            let result = cloned_power_state_notifier
                //TODO: b/353655403 Add more reasons for failing callbacks.
                .notify_oem(new_state.get_report(), PowerStateChangeReason::TIMEOUT)
                .await;
            debug!(
                "OEM was notified about power state update {:?} {:?}",
                new_state.get_report(),
                result
            );
        }
    }
}

#[async_trait]
impl PowerService for PowerServiceImpl {
    fn get_power_state_request_sender(&self) -> Sender<PowerRequest> {
        self.power_state_request_sender.clone()
    }

    fn get_power_state_watch_receiver(&self) -> WatchReceiver<Arc<dyn PowerState>> {
        self.power_state_watch_receiver.clone()
    }

    async fn start(&mut self) -> anyhow::Result<()> {
        // The `power_state_request` channel has a size of 1 message which means that only one power request can be received and can be processed at any point in time.

        // While a power state is processing we can receive new power state requests to cancel/interrupt the previous state
        // as long as they are valid in the power state machine flow. This is guaranteed by power state machine validation. New requests will be validated against the current executed state of the watch channel.
        // For cancellation we need to stop processing of the previous state. The cancellation tokens are cancelled and the task of the already running power state is aborted.
        while let Some(power_request) = self.power_state_request_receiver.recv().await {
            let new_power_report = power_request.power_state.get_report();
            if let Some(power_state_process) = self.power_state_tracker.power_state_handle.take() {
                if power_request.power_state.get_report() == PowerStateReport::SHUTDOWN_CANCELLED {
                    // Cancel the cancellation token to stop the progress of ongoing request.
                    // No need to abort the already executing task `power_state_process` because cancel token should handle the cancellation cases.
                    self.power_state_tracker.cancel_token.cancel();

                    // The power state process could still finish successfully if its already passed cancellation points.
                    let result = power_state_process.await;
                    debug!(
                        "Aborted power state {:?} because a cancel was received {:?}",
                        self.power_state_tracker.power_report, result
                    );
                } else {
                    debug!("Waiting for power state to complete before executing the next one!");
                    let result = power_state_process.await;
                    debug!(
                        "Executed power state {:?} result {:?}",
                        self.power_state_tracker.power_report, result
                    );
                }
            }
            // We need to create a new tracker before setting the task because it needs a new cancellation token set to self before we can process the request.
            // Otherwise process_power_request will use the old token
            self.power_state_tracker = PowerStateTracker::new();
            self.power_state_tracker.set_power_state_handle(
                self.process_power_request(power_request).await,
                new_power_report,
            );
        }
        Ok(())
    }
}

pub struct PowerRequest {
    power_state: Box<dyn PowerState>,
    timeout_ms: u64,
}
impl PowerRequest {
    pub fn new(power_state: Box<dyn PowerState>, timeout_ms: u64) -> Self {
        Self { power_state, timeout_ms }
    }
}

#[automock]
#[async_trait]
pub trait PowerStateRequestHandler: Send + Sync {
    /// Requests a state change in the power state machine from the VM owner.
    /// returns Ok() if the request passes the state machine validation.
    /// otherwise returns an error.
    async fn handle_set_power_state(&self, request: PowerStateChangeRequest) -> anyhow::Result<()>;

    /// Gets the current power state
    async fn get_current_power_state(&self) -> PowerStateReport;
}

pub struct PowerStateRequestHandlerImpl {
    power_state_request_sender: Sender<PowerRequest>,
    power_state_watch_receiver: WatchReceiver<Arc<dyn PowerState>>,
}

impl PowerStateRequestHandlerImpl {
    pub fn new(
        power_state_request_sender: Sender<PowerRequest>,
        power_state_watch_receiver: WatchReceiver<Arc<dyn PowerState>>,
    ) -> Self {
        Self { power_state_request_sender, power_state_watch_receiver }
    }
}

#[async_trait]
impl PowerStateRequestHandler for PowerStateRequestHandlerImpl {
    async fn handle_set_power_state(&self, request: PowerStateChangeRequest) -> anyhow::Result<()> {
        let power_request = validate_power_state_request(request)?;

        let current_state = self.power_state_watch_receiver.borrow().clone();
        if !current_state.is_valid_next_state(power_request.power_state.get_report()) {
            error!(
                "Power state transition is invalid, current={:?} requested {:?}",
                current_state.get_report(),
                power_request.power_state.get_report()
            );
            return Err(anyhow!("Power request was not specified!"));
        }

        debug!(
            "Request for power state change {:?} approved!",
            power_request.power_state.get_report()
        );

        self.power_state_request_sender.send(power_request).await?;
        Ok(())
    }

    async fn get_current_power_state(&self) -> PowerStateReport {
        self.power_state_watch_receiver.borrow().get_report()
    }
}

fn validate_power_state_request(request: PowerStateChangeRequest) -> anyhow::Result<PowerRequest> {
    let power_state: Box<dyn PowerState> = match request.power_request.unwrap() {
        PowerStateRequest::TURN_ON => Box::new(On {}),
        PowerStateRequest::CANCEL_SHUTDOWN => Box::new(ShutdownCancelled {}),
        PowerStateRequest::PREPARE_SHUTDOWN => match request.shutdown_type.unwrap() {
            ShutdownType::POWER_OFF => Box::new(PowerOffEnter {}),
            ShutdownType::SUSPEND_TO_RAM => Box::new(SuspendToRamEnter {}),
            ShutdownType::SUSPEND_TO_DISK => Box::new(SuspendToDiskEnter {}),
            ShutdownType::SHUTDOWN_TYPE_UNSPECIFIED => {
                return Err(anyhow!("Shutdown type was not specified!"))
            }
        },
        PowerStateRequest::FINISH_SHUTDOWN => match request.shutdown_type.unwrap() {
            ShutdownType::POWER_OFF => Box::new(PowerOffPostFinish {}),
            ShutdownType::SUSPEND_TO_RAM => Box::new(SuspendToRamPostFinish {}),
            ShutdownType::SUSPEND_TO_DISK => Box::new(SuspendToDiskPostFinish {}),
            ShutdownType::SHUTDOWN_TYPE_UNSPECIFIED => {
                return Err(anyhow!("Shutdown type was not specified!"))
            }
        },
        PowerStateRequest::REQUEST_UNSPECIFIED => {
            return Err(anyhow!("Power request was not specified!"))
        }
    };

    if request.timeout_ms == 0 {
        return Err(anyhow!("Timeout cannot be Zero!"));
    }

    Ok(PowerRequest::new(power_state, request.timeout_ms))
}

#[cfg(test)]
mod tests {
    use crate::power_service::PowerRequest;
    use crate::power_service::PowerService;
    use crate::power_service::PowerStateRequestHandler;
    use crate::power_state::MockPowerState;
    use crate::power_state::*;
    use crate::power_state_notifier::MockPowerStateNotifier;
    use crate::power_state_notifier::PowerStateNotificationError;
    use crate::PowerServiceImpl;
    use crate::PowerStateRequestHandlerImpl;
    use googletest::fail;
    use sdv_vpm_api::power::PowerStateReport;
    use sdv_vpm_api::vpm::PowerStateChangeRequest;
    use sdv_vpm_api::vpm::PowerStateRequest;
    use sdv_vpm_api::vpm::ShutdownType;
    use sdv_vpm_api::vpm_notifications::power_state_notification_request::PowerStateChangeReason;
    use std::sync::Arc;
    use tokio::sync::mpsc::channel;
    use tokio::sync::watch::channel as WatchChannel;
    use tokio::time::timeout;
    use tokio::time::Duration;

    #[tokio::test]
    async fn set_power_state_succeeds() {
        // Given: the power state request handler.
        let new_report = PowerStateReport::ON;
        let mut new_state = MockPowerState::default();
        let mut power_state_notifier = MockPowerStateNotifier::default();

        power_state_notifier.expect_create_binder_service().times(1).returning(|_| Ok(()));
        // expect callback notifications to be executed.
        power_state_notifier
            .expect_notify()
            .times(1)
            .withf(move |report| *report == new_report)
            .returning(|_| Ok(()));

        // expect OEM to be notified.
        power_state_notifier
            .expect_notify_oem()
            .times(1)
            .withf(move |report, reason| {
                *report == new_report && *reason == PowerStateChangeReason::HOST_REQUESTED
            })
            .returning(|_, _| Ok(()));

        // expect report to be ON.
        new_state.expect_get_report().returning(move || new_report);
        // epect sleep state to be a non-sleep state.
        new_state.expect_is_sleep_state().returning(|| false);
        // expect new state to be executed.
        new_state.expect_execute().times(1).returning(|_, _| Ok(()));

        let mut power_service = PowerServiceImpl::new_with_notifier(Arc::new(power_state_notifier));
        let power_state_request_sender = power_service.get_power_state_request_sender();
        let mut power_state_watch_receiver = power_service.get_power_state_watch_receiver();
        let _handle = tokio::spawn(async move { power_service.start().await });

        // When: a power state request is requested
        let _ = power_state_request_sender.send(PowerRequest::new(Box::new(new_state), 1)).await;

        // Then: power state is set.
        if (timeout(
            Duration::from_secs(5),
            power_state_watch_receiver.wait_for(|state| state.get_report() == new_report),
        )
        .await)
            .is_err()
        {
            fail!("Test timed out").unwrap();
        }
        assert_eq!(power_state_watch_receiver.borrow().get_report(), new_report);
    }

    #[tokio::test]
    async fn set_power_state_callbacks_fail_send_timeout() {
        // Given: the power state request handler.
        let new_report = PowerStateReport::ON;
        let mut new_state = MockPowerState::default();
        let mut power_state_notifier = MockPowerStateNotifier::default();

        power_state_notifier.expect_create_binder_service().times(1).returning(|_| Ok(()));
        // expect callback notifications to be executed.
        power_state_notifier
            .expect_notify()
            .times(1)
            .withf(move |report| *report == new_report)
            .returning(|_| Err(PowerStateNotificationError::Failed));

        // expect OEM to be notified.
        power_state_notifier
            .expect_notify_oem()
            .times(1)
            .withf(move |report, reason| {
                *report == new_report && *reason == PowerStateChangeReason::TIMEOUT
            })
            .returning(|_, _| Ok(()));

        // expect report to be ON.
        new_state.expect_get_report().returning(move || new_report);
        // epect sleep state to be a non-sleep state.
        new_state.expect_is_sleep_state().returning(|| false);
        // expect new state to be executed.
        new_state.expect_execute().times(1).returning(|_, _| Ok(()));

        let mut power_service = PowerServiceImpl::new_with_notifier(Arc::new(power_state_notifier));
        let power_state_request_sender = power_service.get_power_state_request_sender();
        let mut power_state_watch_receiver = power_service.get_power_state_watch_receiver();
        let _handle = tokio::spawn(async move { power_service.start().await });

        // When: a power state request is requested
        let _ = power_state_request_sender.send(PowerRequest::new(Box::new(new_state), 1)).await;

        // Then: power state is set.
        if (timeout(
            Duration::from_secs(5),
            power_state_watch_receiver.wait_for(|state| state.get_report() == new_report),
        )
        .await)
            .is_err()
        {
            fail!("Test timed out").unwrap();
        }
        assert_eq!(power_state_watch_receiver.borrow().get_report(), new_report);
    }

    #[tokio::test]
    async fn set_sleep_state_no_oem_notification_sent() {
        // Given: the power state request handler.
        let new_report = PowerStateReport::SUSPEND_TO_RAM_POST_FINISH;
        let mut new_state = MockPowerState::default();
        let mut power_state_notifier = MockPowerStateNotifier::default();

        // expect callback notifications to be executed.
        power_state_notifier.expect_create_binder_service().times(1).returning(|_| Ok(()));
        power_state_notifier
            .expect_notify()
            .times(1)
            .withf(move |report| *report == new_report)
            .returning(|_| Ok(()));

        // expect report to be SuspendToRam.
        new_state.expect_get_report().returning(move || new_report);
        // epect sleep state to be a sleep state.
        new_state.expect_is_sleep_state().returning(|| true);
        // expect new state to be executed.
        new_state.expect_execute().times(1).returning(|_, _| Ok(()));

        let mut power_service = PowerServiceImpl::new_with_notifier(Arc::new(power_state_notifier));
        let power_state_request_sender = power_service.get_power_state_request_sender();
        let mut power_state_watch_receiver = power_service.get_power_state_watch_receiver();
        let _handle = tokio::spawn(async move { power_service.start().await });

        // When: a power state request is requested
        let _ = power_state_request_sender.send(PowerRequest::new(Box::new(new_state), 1)).await;

        // Then: power state is set.
        if (timeout(
            Duration::from_secs(5),
            power_state_watch_receiver.wait_for(|state| state.get_report() == new_report),
        )
        .await)
            .is_err()
        {
            fail!("Test timed out").unwrap();
        }
        assert_eq!(power_state_watch_receiver.borrow().get_report(), new_report);
    }

    #[tokio::test]
    async fn handle_power_state_fails_invalid_timeout() {
        // Given: the power state request handler.
        let (tx, _) = channel::<PowerRequest>(1);
        let (_, rx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(PowerOffExit {}));
        let power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, rx);

        // When: a prepare power state request without a timeout is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
            shutdown_type: ShutdownType::SUSPEND_TO_DISK.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn handle_power_state_fails_unspecified_power_state() {
        // Given: the power state request handler.
        let (tx, _) = channel::<PowerRequest>(1);
        let (_, rx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(PowerOffExit {}));
        let power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, rx);

        // When: a power state request is sent without power state.
        let request = PowerStateChangeRequest { ..Default::default() };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn handle_power_state_fails_prepare_no_shutdown_type() {
        // Given: the power state request handler.
        let (tx, _) = channel::<PowerRequest>(1);
        let (_, rx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(PowerOffExit {}));
        let power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, rx);

        // When: a prepare power state request is sent without shutdown type.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
            timeout_ms: 1,
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn handle_power_state_fails_turn_off_no_shutdown_type() {
        // Given: the power state request handler.
        let (tx, _) = channel::<PowerRequest>(1);
        let (_, rx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(PowerOffExit {}));
        let power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, rx);

        // When: a power off state request is sent without shutdown type.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
            timeout_ms: 1,
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn handle_power_state_on() {
        // Given: the power state request handler.
        let (tx, mut rx) = channel::<PowerRequest>(1);
        let (_, wrx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(PowerOffExit {}));
        let power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, wrx);

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::TURN_ON.into(),
            timeout_ms: 1,
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request).await;
        assert_eq!(
            PowerStateReport::ON,
            rx.recv().await.expect("Nothing was received!").power_state.get_report()
        );
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn handle_power_state_prepare_s2r() {
        // Given: the power state request handler.
        let (tx, mut rx) = channel::<PowerRequest>(1);
        let (_, wrx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(On {}));
        let power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, wrx);

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
            shutdown_type: ShutdownType::SUSPEND_TO_RAM.into(),
            timeout_ms: 1,
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request).await;
        assert_eq!(
            PowerStateReport::SUSPEND_TO_RAM_ENTER,
            rx.recv().await.expect("Nothing was received!").power_state.get_report()
        );
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn handle_power_state_prepare_s2d() {
        // Given: the power state request handler.
        let (tx, mut rx) = channel::<PowerRequest>(1);
        let (_, wrx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(On {}));
        let power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, wrx);

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
            shutdown_type: ShutdownType::SUSPEND_TO_DISK.into(),
            timeout_ms: 1,
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request).await;
        assert_eq!(
            PowerStateReport::SUSPEND_TO_DISK_ENTER,
            rx.recv().await.expect("Nothing was received!").power_state.get_report()
        );
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn handle_power_state_prepare_off() {
        // Given: the power state request handler.
        let (tx, mut rx) = channel::<PowerRequest>(1);
        let (_, wrx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(On {}));
        let power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, wrx);

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
            shutdown_type: ShutdownType::POWER_OFF.into(),
            timeout_ms: 1,
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request).await;
        assert_eq!(
            PowerStateReport::POWER_OFF_ENTER,
            rx.recv().await.expect("Nothing was received!").power_state.get_report()
        );
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn handle_power_state_cancel_shutdown() {
        // Given: the power state request handler.
        let (tx, mut rx) = channel::<PowerRequest>(1);
        let (_, wrx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(PowerOffEnter {}));
        let power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, wrx);

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::CANCEL_SHUTDOWN.into(),
            timeout_ms: 1,
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request).await;
        assert_eq!(
            PowerStateReport::SHUTDOWN_CANCELLED,
            rx.recv().await.expect("Nothing was received!").power_state.get_report()
        );
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn handle_power_state_s2r() {
        // Given: the power state request handler.
        let (tx, mut rx) = channel::<PowerRequest>(1);
        let (_, wrx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(WaitForFinishSuspendToRam {}));
        let power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, wrx);

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
            shutdown_type: ShutdownType::SUSPEND_TO_RAM.into(),
            timeout_ms: 1,
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request).await;
        assert_eq!(
            PowerStateReport::SUSPEND_TO_RAM_POST_FINISH,
            rx.recv().await.expect("Nothing was received!").power_state.get_report()
        );
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn handle_power_state_s2d() {
        // Given: the power state request handler.
        let (tx, mut rx) = channel::<PowerRequest>(1);
        let (_, wrx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(WaitForFinishSuspendToDisk {}));
        let power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, wrx);

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
            shutdown_type: ShutdownType::SUSPEND_TO_DISK.into(),
            timeout_ms: 1,
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request).await;
        assert_eq!(
            PowerStateReport::SUSPEND_TO_DISK_POST_FINISH,
            rx.recv().await.expect("Nothing was received!").power_state.get_report()
        );
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn handle_power_state_off() {
        // Given: the power state request handler.
        let (tx, mut rx) = channel::<PowerRequest>(1);
        let (_, wrx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(WaitForFinishPowerOff {}));
        let power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, wrx);

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
            shutdown_type: ShutdownType::POWER_OFF.into(),
            timeout_ms: 1,
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request).await;
        assert_eq!(
            PowerStateReport::POWER_OFF_POST_FINISH,
            rx.recv().await.expect("Nothing was received!").power_state.get_report()
        );
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn handle_get_power_state() {
        // Given: the power state request handler.
        let (tx, _) = channel::<PowerRequest>(1);
        let (_, wrx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(On {}));
        let power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, wrx);

        //When we get the current power state request.
        let result = power_state_request_handler.get_current_power_state().await;

        // Then: power state is returned.
        assert_eq!(PowerStateReport::ON, result);
    }
}
