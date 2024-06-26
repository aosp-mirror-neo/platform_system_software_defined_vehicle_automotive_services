// Copyright 2024 Google LLC
//! Vehicle and Power Mode Agent.
//!
use crate::power_state::*;
use crate::PowerStateNotifier;
use crate::PowerStateNotifierImpl;
use crate::POWER_STATE_CHANNEL_QUEUE_SIZE;
use anyhow::anyhow;
use async_trait::async_trait;
use log::{debug, error, info};
use mockall::automock;
use sdv_vpm_api::power::PowerStateReport;
use sdv_vpm_api::vpm::power_state_notification_request::PowerStateChangeReason;
use sdv_vpm_api::vpm::PowerStateChangeRequest;
use sdv_vpm_api::vpm::PowerStateRequest;
use sdv_vpm_api::vpm::ShutdownType;
use sdvgenerated::vpm_system_service_bundle::VpmSystemServiceBundle;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::watch::{
    channel as watch_channel, Receiver as WatchReceiver, Sender as WatchSender,
};
use tokio::task;
use tokio::task::JoinHandle;

#[automock]
#[async_trait]
pub trait PowerService: Send + Sync {
    async fn start(&mut self) -> anyhow::Result<()>;
    fn get_power_state_request_sender(&self) -> Sender<Box<dyn PowerState>>;
    fn get_power_state_watch_receiver(&self) -> WatchReceiver<Arc<dyn PowerState>>;
}

pub struct PowerServiceImpl {
    power_state_notifier: Arc<dyn PowerStateNotifier>,
    power_state_request_sender: Sender<Box<dyn PowerState>>,
    power_state_request_receiver: Receiver<Box<dyn PowerState>>,
    power_state_watch_sender: Arc<WatchSender<Arc<dyn PowerState>>>,
    power_state_watch_receiver: WatchReceiver<Arc<dyn PowerState>>,
}

impl PowerServiceImpl {
    /// Creates a new PowerServiceImpl with the default value of the power state being POWER_OFF_EXIST.
    /// The POWER_OFF_EXIT state is the only possible state at this step because calling new() means we are coming from cold boot.
    /// If we resume from S2R or S2D new() should not be called again.
    pub fn new(vpm_service_bundle: Arc<Mutex<VpmSystemServiceBundle>>) -> Self {
        let power_state_notifier =
            Arc::new(PowerStateNotifierImpl::new(vpm_service_bundle.clone()));
        Self::new_with_notifier(power_state_notifier)
    }

    fn new_with_notifier(power_state_notifier: Arc<dyn PowerStateNotifier>) -> Self {
        //TODO: check which suspend services is provided by the Host.

        // Channel to forward power state requests.
        let (power_state_request_sender, power_state_request_receiver) =
            channel::<Box<dyn PowerState>>(POWER_STATE_CHANNEL_QUEUE_SIZE);

        // Watch channel that holds the value for current power state.
        let (power_state_watch_sender, power_state_watch_receiver) =
            watch_channel::<Arc<dyn PowerState>>(Arc::new(PowerOffExit {}));

        power_state_notifier.create_binder_service();

        Self {
            power_state_notifier,
            power_state_request_sender,
            power_state_request_receiver,
            power_state_watch_sender: power_state_watch_sender.into(),
            power_state_watch_receiver,
        }
    }
}

#[async_trait]
impl PowerService for PowerServiceImpl {
    fn get_power_state_request_sender(&self) -> Sender<Box<dyn PowerState>> {
        self.power_state_request_sender.clone()
    }

    fn get_power_state_watch_receiver(&self) -> WatchReceiver<Arc<dyn PowerState>> {
        self.power_state_watch_receiver.clone()
    }

    async fn start(&mut self) -> anyhow::Result<()> {
        while let Some(next_state) = self.power_state_request_receiver.recv().await {
            let cloned_power_state_notifier = self.power_state_notifier.clone();
            let cloned_power_state_watch_sender = self.power_state_watch_sender.clone();
            let cloned_power_state_request_sender = self.power_state_request_sender.clone();
            tokio::spawn(async move {
                debug!("Starting a task for callback notifications!");
                // TODO: Cancel shutdown implementation, check if there is a prepare request in progress
                // if so cancel it and run cancel shutdown.
                // TODO: Run this notify request with timeout from the request.
                let notification_result =
                    cloned_power_state_notifier.notify(next_state.get_report()).await;
                let new_state: Arc<dyn PowerState> = Arc::from(next_state);
                debug!(
                    "callbacks finished, setting the current state to {:?}!",
                    new_state.get_report()
                );

                cloned_power_state_watch_sender.send(new_state.clone());
                debug!("Executing state {:?}", new_state.get_report());
                let execution_result = new_state.execute(cloned_power_state_request_sender).await;
                if (execution_result.is_err()) {
                    error!("Could not execute state.");
                }

                // We don't need to notify OEM about sleep reports, because new_state.execute will suspend the VM including this process or power-off VM.
                // If the VM suspends, the following code will only be executed once the process resumes from suspension, therefore the report will be outdated.
                // The new_state.execute will also request a transition to the new resume state which will be received by the power_state_request_receiver.
                if (!new_state.is_sleep_state()) {
                    if (notification_result.is_ok()) {
                        cloned_power_state_notifier.notify_oem(
                            new_state.get_report(),
                            PowerStateChangeReason::HOST_REQUESTED,
                        );
                    } else {
                        cloned_power_state_notifier
                            .notify_oem(new_state.get_report(), PowerStateChangeReason::TIMEOUT);
                    }
                }
            })
            .await;
        }
        Ok(())
    }
}

#[automock]
pub trait PowerStateRequestHandler: Send + Sync {
    /// Requests a state change in the power state machine from the VM owner.
    /// returns Ok() if the request passes the state machine validation.
    /// otherwise returns an error.
    fn handle_set_power_state(&self, request: PowerStateChangeRequest) -> anyhow::Result<()>;
}

pub struct PowerStateRequestHandlerImpl {
    power_state_request_sender: Sender<Box<dyn PowerState>>,
    power_state_watch_receiver: WatchReceiver<Arc<dyn PowerState>>,
}

impl PowerStateRequestHandlerImpl {
    pub fn new(
        power_state_request_sender: Sender<Box<dyn PowerState>>,
        power_state_watch_receiver: WatchReceiver<Arc<dyn PowerState>>,
    ) -> Self {
        Self { power_state_request_sender, power_state_watch_receiver }
    }
}

impl PowerStateRequestHandler for PowerStateRequestHandlerImpl {
    fn handle_set_power_state(&self, request: PowerStateChangeRequest) -> anyhow::Result<()> {
        let next_power_state = get_power_state_from_power_state_request(request)?;
        let current_state = self.power_state_watch_receiver.borrow();
        if (!current_state.is_valid_next_state(next_power_state.get_report())) {
            error!(
                "Power state transition is invalid, current={:?} requested {:?}",
                current_state.get_report(),
                next_power_state.get_report()
            );
            return Err(anyhow!("Power request was not specified!"));
        }

        debug!("Request for power state change {:?} approved!", next_power_state.get_report());
        self.power_state_request_sender.blocking_send(next_power_state);
        Ok(())
    }
}

fn get_power_state_from_power_state_request(
    request: PowerStateChangeRequest,
) -> anyhow::Result<Box<dyn PowerState>> {
    let power_state: Box<dyn PowerState> = match (request.power_request.unwrap()) {
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
    Ok(power_state)
}

#[cfg(test)]
mod tests {
    use crate::power_service::PowerService;
    use crate::power_service::PowerStateRequestHandler;
    use crate::power_state::MockPowerState;
    use crate::power_state::*;
    use crate::power_state_notifier::MockPowerStateNotifier;
    use crate::power_state_notifier::PowerStateNotifier;
    use crate::PowerServiceImpl;
    use crate::PowerStateRequestHandlerImpl;
    use anyhow::anyhow;
    use googletest::fail;
    use sdv_vpm_api::power::PowerStateReport;
    use sdv_vpm_api::vpm::power_state_notification_request::PowerStateChangeReason;
    use sdv_vpm_api::vpm::PowerStateChangeRequest;
    use sdv_vpm_api::vpm::PowerStateRequest;
    use sdv_vpm_api::vpm::ShutdownType;
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

        power_state_notifier.expect_create_binder_service().times(1).returning(|| Ok(()));
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
        new_state.expect_execute().times(1).returning(|_| Ok(()));

        let mut power_service = PowerServiceImpl::new_with_notifier(Arc::new(power_state_notifier));
        let power_state_request_sender = power_service.get_power_state_request_sender();
        let mut power_state_watch_receiver = power_service.get_power_state_watch_receiver();
        let _handle = tokio::spawn(async move { power_service.start().await });

        // When: a power state request is requested
        power_state_request_sender.send(Box::new(new_state)).await;

        // Then: power state is set.
        if (timeout(
            Duration::from_secs(5),
            power_state_watch_receiver.wait_for(|state| state.get_report() == new_report),
        )
        .await)
            .is_err()
        {
            fail!("Test timed out");
        }
        assert_eq!(power_state_watch_receiver.borrow().get_report(), new_report);
    }

    #[tokio::test]
    async fn set_power_state_callbacks_fail_send_timeout() {
        // Given: the power state request handler.
        let new_report = PowerStateReport::ON;
        let mut new_state = MockPowerState::default();
        let mut power_state_notifier = MockPowerStateNotifier::default();

        power_state_notifier.expect_create_binder_service().times(1).returning(|| Ok(()));
        // expect callback notifications to be executed.
        power_state_notifier
            .expect_notify()
            .times(1)
            .withf(move |report| *report == new_report)
            .returning(|_| Err(anyhow!("Error")));

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
        new_state.expect_execute().times(1).returning(|_| Ok(()));

        let mut power_service = PowerServiceImpl::new_with_notifier(Arc::new(power_state_notifier));

        let power_state_request_sender = power_service.get_power_state_request_sender();
        let mut power_state_watch_receiver = power_service.get_power_state_watch_receiver();
        let _handle = tokio::spawn(async move { power_service.start().await });

        // When: a power state request is requested
        power_state_request_sender.send(Box::new(new_state)).await;

        // Then: power state is set.
        if (timeout(
            Duration::from_secs(5),
            power_state_watch_receiver.wait_for(|state| state.get_report() == new_report),
        )
        .await)
            .is_err()
        {
            fail!("Test timed out");
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
        power_state_notifier.expect_create_binder_service().times(1).returning(|| Ok(()));
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
        new_state.expect_execute().times(1).returning(|_| Ok(()));

        let mut power_service = PowerServiceImpl::new_with_notifier(Arc::new(power_state_notifier));
        let power_state_request_sender = power_service.get_power_state_request_sender();
        let mut power_state_watch_receiver = power_service.get_power_state_watch_receiver();
        let _handle = tokio::spawn(async move { power_service.start().await });

        // When: a power state request is requested
        power_state_request_sender.send(Box::new(new_state)).await;

        // Then: power state is set.
        if (timeout(
            Duration::from_secs(5),
            power_state_watch_receiver.wait_for(|state| state.get_report() == new_report),
        )
        .await)
            .is_err()
        {
            fail!("Test timed out");
        }
        assert_eq!(power_state_watch_receiver.borrow().get_report(), new_report);
    }

    #[test]
    fn handle_power_state_fails_unspecified_power_state() {
        // Given: the power state request handler.
        let (tx, _) = channel::<Box<dyn PowerState>>(1);
        let (_, rx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(PowerOffExit {}));
        let mut power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, rx);

        // When: a power state request is sent without power state.
        let request = PowerStateChangeRequest { ..Default::default() };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request);
        assert!(result.is_err());
    }

    #[test]
    fn handle_power_state_fails_prepare_no_shutdown_type() {
        // Given: the power state request handler.
        let (tx, _) = channel::<Box<dyn PowerState>>(1);
        let (_, rx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(PowerOffExit {}));
        let mut power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, rx);

        // When: a prepare power state request is sent without shutdown type.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request);
        assert!(result.is_err());
    }

    #[test]
    fn handle_power_state_fails_turn_off_no_shutdown_type() {
        // Given: the power state request handler.
        let (tx, _) = channel::<Box<dyn PowerState>>(1);
        let (_, rx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(PowerOffExit {}));
        let mut power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, rx);

        // When: a power off state request is sent without shutdown type.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request);
        assert!(result.is_err());
    }

    #[test]
    fn handle_power_state_on() {
        // Given: the power state request handler.
        let (tx, mut rx) = channel::<Box<dyn PowerState>>(1);
        let (_, wrx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(PowerOffExit {}));
        let mut power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, wrx);

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::TURN_ON.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request);
        assert_eq!(
            PowerStateReport::ON,
            rx.blocking_recv().expect("Nothing was received!").get_report()
        );
        assert!(result.is_ok());
    }

    #[test]
    fn handle_power_state_prepare_s2r() {
        // Given: the power state request handler.
        let (tx, mut rx) = channel::<Box<dyn PowerState>>(1);
        let (_, wrx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(On {}));
        let mut power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, wrx);

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
            shutdown_type: ShutdownType::SUSPEND_TO_RAM.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request);
        assert_eq!(
            PowerStateReport::SUSPEND_TO_RAM_ENTER,
            rx.blocking_recv().expect("Nothing was received!").get_report()
        );
        assert!(result.is_ok());
    }

    #[test]
    fn handle_power_state_prepare_s2d() {
        // Given: the power state request handler.
        let (tx, mut rx) = channel::<Box<dyn PowerState>>(1);
        let (_, wrx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(On {}));
        let mut power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, wrx);

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
            shutdown_type: ShutdownType::SUSPEND_TO_DISK.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request);
        assert_eq!(
            PowerStateReport::SUSPEND_TO_DISK_ENTER,
            rx.blocking_recv().expect("Nothing was received!").get_report()
        );
        assert!(result.is_ok());
    }

    #[test]
    fn handle_power_state_prepare_off() {
        // Given: the power state request handler.
        let (tx, mut rx) = channel::<Box<dyn PowerState>>(1);
        let (_, wrx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(On {}));
        let mut power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, wrx);

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
            shutdown_type: ShutdownType::POWER_OFF.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request);
        assert_eq!(
            PowerStateReport::POWER_OFF_ENTER,
            rx.blocking_recv().expect("Nothing was received!").get_report()
        );
        assert!(result.is_ok());
    }

    #[test]
    fn handle_power_state_cancel_shutdown() {
        // Given: the power state request handler.
        let (tx, mut rx) = channel::<Box<dyn PowerState>>(1);
        let (_, wrx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(PowerOffEnter {}));
        let mut power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, wrx);

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::CANCEL_SHUTDOWN.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request);
        assert_eq!(
            PowerStateReport::SHUTDOWN_CANCELLED,
            rx.blocking_recv().expect("Nothing was received!").get_report()
        );
        assert!(result.is_ok());
    }

    #[test]
    fn handle_power_state_s2r() {
        // Given: the power state request handler.
        let (tx, mut rx) = channel::<Box<dyn PowerState>>(1);
        let (_, wrx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(WaitForFinish {}));
        let mut power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, wrx);

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
            shutdown_type: ShutdownType::SUSPEND_TO_RAM.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request);
        assert_eq!(
            PowerStateReport::SUSPEND_TO_RAM_POST_FINISH,
            rx.blocking_recv().expect("Nothing was received!").get_report()
        );
        assert!(result.is_ok());
    }

    #[test]
    fn handle_power_state_s2d() {
        // Given: the power state request handler.
        let (tx, mut rx) = channel::<Box<dyn PowerState>>(1);
        let (_, wrx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(WaitForFinish {}));
        let mut power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, wrx);

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
            shutdown_type: ShutdownType::SUSPEND_TO_DISK.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request);
        assert_eq!(
            PowerStateReport::SUSPEND_TO_DISK_POST_FINISH,
            rx.blocking_recv().expect("Nothing was received!").get_report()
        );
        assert!(result.is_ok());
    }

    #[test]
    fn handle_power_state_off() {
        // Given: the power state request handler.
        let (tx, mut rx) = channel::<Box<dyn PowerState>>(1);
        let (_, wrx) = WatchChannel::<Arc<dyn PowerState>>(Arc::new(WaitForFinish {}));
        let mut power_state_request_handler = PowerStateRequestHandlerImpl::new(tx, wrx);

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
            shutdown_type: ShutdownType::POWER_OFF.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_state_request_handler.handle_set_power_state(request);
        assert_eq!(
            PowerStateReport::POWER_OFF_POST_FINISH,
            rx.blocking_recv().expect("Nothing was received!").get_report()
        );
        assert!(result.is_ok());
    }
}
