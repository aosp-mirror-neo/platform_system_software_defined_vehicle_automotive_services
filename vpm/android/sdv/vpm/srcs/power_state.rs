// Copyright 2024 Google LLC
use crate::power_service::PowerRequest;
use async_trait::async_trait;
use log::debug;
use mockall::automock;
use rustutils::system_properties::read;
use sdv_pm::PlatformPower;
use sdv_pm::SuspendMode;
use sdv_vpm_api::power::PowerStateReport;
use std::fmt::Debug;
use tokio::sync::mpsc::Sender;
use tracing::Level;

const PLATFORM_SUSPEND_TIMEOUT_MS_PROP: &str = "sdv.vpm.platform.timeout_ms";
const PLATFORM_SUSPEND_RETRY_INTERVAL_MS_PROP: &str = "sdv.vpm.platform.retry_interval_ms";
const DEFAULT_PLATFORM_TIMEOUT_MS: u64 = 1000;
const DEFAULT_PLATFORM_RETRY_INTERVAL_MS: u64 = 100;
#[cfg(not(test))]
use sdv_pm::PlatformPowerController;
#[cfg(not(test))]
fn get_platform_controller() -> PlatformPowerController {
    PlatformPowerController::default()
}

fn get_platform_timeout_ms() -> u64 {
    let platform_timeout_ms =
        read_property_or_default(PLATFORM_SUSPEND_TIMEOUT_MS_PROP, DEFAULT_PLATFORM_TIMEOUT_MS);
    debug!("VPM platform timeout was set to {:?}", platform_timeout_ms);
    platform_timeout_ms
}

fn get_platform_retry_interval_ms() -> u64 {
    let platform_interval_ms = read_property_or_default(
        PLATFORM_SUSPEND_RETRY_INTERVAL_MS_PROP,
        DEFAULT_PLATFORM_RETRY_INTERVAL_MS,
    );
    debug!("VPM platform retry interval was set to {:?}", platform_interval_ms);
    platform_interval_ms
}

fn read_property_or_default(property: &str, default: u64) -> u64 {
    read(property)
        .ok()
        .flatten()
        .and_then(|timeout_string| timeout_string.parse().ok())
        .unwrap_or(default)
}

fn suspend_to_ram() -> anyhow::Result<()> {
    get_platform_controller().suspend(
        &SuspendMode::RAM,
        get_platform_timeout_ms(),
        get_platform_retry_interval_ms(),
    )?;
    debug!("sdv_vpm_agent has resumed successfully");
    tracing::event!(Level::DEBUG, "sdv_vpm_agent has resumed successfully");
    Ok(())
}

fn suspend_to_disk() -> anyhow::Result<()> {
    get_platform_controller().suspend(
        &SuspendMode::DISK,
        get_platform_timeout_ms(),
        get_platform_retry_interval_ms(),
    )?;
    debug!("sdv_vpm_agent has resumed successfully");
    tracing::event!(Level::DEBUG, "sdv_vpm_agent has resumed successfully");
    Ok(())
}

fn power_off() -> anyhow::Result<()> {
    get_platform_controller().power_off(get_platform_timeout_ms())?;
    Ok(())
}

#[automock]
#[async_trait]
pub trait PowerState: Send + Sync + Debug {
    fn get_report(&self) -> PowerStateReport;

    fn is_valid_next_state(&self, next_state: PowerStateReport) -> bool;

    fn is_sleep_state(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        _power_state_request_sender: Sender<PowerRequest>,
        _timeout_ms: u64,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SuspendToRamExit {}

#[async_trait]
impl PowerState for SuspendToRamExit {
    fn is_valid_next_state(&self, next_state: PowerStateReport) -> bool {
        PowerStateReport::ON == next_state
    }
    fn get_report(&self) -> PowerStateReport {
        PowerStateReport::SUSPEND_TO_RAM_EXIT
    }
}

#[derive(Debug, Clone)]
pub struct SuspendToDiskExit {}

#[async_trait]
impl PowerState for SuspendToDiskExit {
    fn is_valid_next_state(&self, next_state: PowerStateReport) -> bool {
        PowerStateReport::ON == next_state
    }
    fn get_report(&self) -> PowerStateReport {
        PowerStateReport::SUSPEND_TO_DISK_EXIT
    }
}

#[derive(Debug, Clone)]
pub struct PowerOffExit {}

impl PowerState for PowerOffExit {
    fn is_valid_next_state(&self, next_state: PowerStateReport) -> bool {
        PowerStateReport::ON == next_state
    }
    fn get_report(&self) -> PowerStateReport {
        PowerStateReport::POWER_OFF_EXIT
    }
}

#[derive(Debug, Clone)]
pub struct On {}

#[async_trait]
impl PowerState for On {
    fn is_valid_next_state(&self, next_state: PowerStateReport) -> bool {
        [
            PowerStateReport::SUSPEND_TO_RAM_ENTER,
            PowerStateReport::SUSPEND_TO_DISK_ENTER,
            PowerStateReport::POWER_OFF_ENTER,
        ]
        .contains(&next_state)
    }
    fn get_report(&self) -> PowerStateReport {
        PowerStateReport::ON
    }
}

#[derive(Debug, Clone)]
pub struct SuspendToRamEnter {}

#[async_trait]
impl PowerState for SuspendToRamEnter {
    fn is_valid_next_state(&self, next_state: PowerStateReport) -> bool {
        [PowerStateReport::WAIT_FOR_FINISH, PowerStateReport::SHUTDOWN_CANCELLED]
            .contains(&next_state)
    }
    fn get_report(&self) -> PowerStateReport {
        PowerStateReport::SUSPEND_TO_RAM_ENTER
    }
    async fn execute(
        &self,
        power_state_request_sender: Sender<PowerRequest>,
        timeout_ms: u64,
    ) -> anyhow::Result<()> {
        power_state_request_sender
            .send(PowerRequest::new(Box::new(WaitForFinishSuspendToRam {}), timeout_ms))
            .await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SuspendToDiskEnter {}

#[async_trait]
impl PowerState for SuspendToDiskEnter {
    fn is_valid_next_state(&self, next_state: PowerStateReport) -> bool {
        [PowerStateReport::WAIT_FOR_FINISH, PowerStateReport::SHUTDOWN_CANCELLED]
            .contains(&next_state)
    }
    fn get_report(&self) -> PowerStateReport {
        PowerStateReport::SUSPEND_TO_DISK_ENTER
    }

    async fn execute(
        &self,
        power_state_request_sender: Sender<PowerRequest>,
        timeout_ms: u64,
    ) -> anyhow::Result<()> {
        power_state_request_sender
            .send(PowerRequest::new(Box::new(WaitForFinishSuspendToDisk {}), timeout_ms))
            .await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PowerOffEnter {}

#[async_trait]
impl PowerState for PowerOffEnter {
    fn is_valid_next_state(&self, next_state: PowerStateReport) -> bool {
        [PowerStateReport::WAIT_FOR_FINISH, PowerStateReport::SHUTDOWN_CANCELLED]
            .contains(&next_state)
    }
    fn get_report(&self) -> PowerStateReport {
        PowerStateReport::POWER_OFF_ENTER
    }
    async fn execute(
        &self,
        power_state_request_sender: Sender<PowerRequest>,
        timeout_ms: u64,
    ) -> anyhow::Result<()> {
        power_state_request_sender
            .send(PowerRequest::new(Box::new(WaitForFinishPowerOff {}), timeout_ms))
            .await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct WaitForFinishSuspendToRam {}

#[async_trait]
impl PowerState for WaitForFinishSuspendToRam {
    fn is_valid_next_state(&self, next_state: PowerStateReport) -> bool {
        next_state == PowerStateReport::SHUTDOWN_CANCELLED
            || next_state == PowerStateReport::SUSPEND_TO_RAM_POST_FINISH
    }
    fn get_report(&self) -> PowerStateReport {
        PowerStateReport::WAIT_FOR_FINISH
    }
}

#[derive(Debug, Clone)]
pub struct WaitForFinishSuspendToDisk {}

#[async_trait]
impl PowerState for WaitForFinishSuspendToDisk {
    fn is_valid_next_state(&self, next_state: PowerStateReport) -> bool {
        next_state == PowerStateReport::SHUTDOWN_CANCELLED
            || next_state == PowerStateReport::SUSPEND_TO_DISK_POST_FINISH
    }
    fn get_report(&self) -> PowerStateReport {
        PowerStateReport::WAIT_FOR_FINISH
    }
}

#[derive(Debug, Clone)]
pub struct WaitForFinishPowerOff {}

#[async_trait]
impl PowerState for WaitForFinishPowerOff {
    fn is_valid_next_state(&self, next_state: PowerStateReport) -> bool {
        next_state == PowerStateReport::SHUTDOWN_CANCELLED
            || next_state == PowerStateReport::POWER_OFF_POST_FINISH
    }
    fn get_report(&self) -> PowerStateReport {
        PowerStateReport::WAIT_FOR_FINISH
    }
}

#[derive(Debug, Clone)]
pub struct ShutdownCancelled {}

impl PowerState for ShutdownCancelled {
    fn is_valid_next_state(&self, next_state: PowerStateReport) -> bool {
        PowerStateReport::ON == next_state
    }
    fn get_report(&self) -> PowerStateReport {
        PowerStateReport::SHUTDOWN_CANCELLED
    }
}

#[derive(Debug, Clone)]
pub struct SuspendToRamPostFinish {}

#[async_trait]

impl PowerState for SuspendToRamPostFinish {
    fn is_valid_next_state(&self, next_state: PowerStateReport) -> bool {
        PowerStateReport::SUSPEND_TO_RAM_EXIT == next_state
    }
    fn get_report(&self) -> PowerStateReport {
        PowerStateReport::SUSPEND_TO_RAM_POST_FINISH
    }
    fn is_sleep_state(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        power_state_request_sender: Sender<PowerRequest>,
        timeout_ms: u64,
    ) -> anyhow::Result<()> {
        suspend_to_ram()?;
        power_state_request_sender
            .send(PowerRequest::new(Box::new(SuspendToRamExit {}), timeout_ms))
            .await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SuspendToDiskPostFinish {}

#[async_trait]
impl PowerState for SuspendToDiskPostFinish {
    fn is_valid_next_state(&self, next_state: PowerStateReport) -> bool {
        PowerStateReport::SUSPEND_TO_DISK_EXIT == next_state
    }
    fn get_report(&self) -> PowerStateReport {
        PowerStateReport::SUSPEND_TO_DISK_POST_FINISH
    }
    fn is_sleep_state(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        power_state_request_sender: Sender<PowerRequest>,
        timeout_ms: u64,
    ) -> anyhow::Result<()> {
        suspend_to_disk()?;
        power_state_request_sender
            .send(PowerRequest::new(Box::new(SuspendToDiskExit {}), timeout_ms))
            .await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PowerOffPostFinish {}

#[async_trait]
impl PowerState for PowerOffPostFinish {
    fn is_valid_next_state(&self, _next_state: PowerStateReport) -> bool {
        // Terminal state, nothing should come after it.
        false
    }
    fn get_report(&self) -> PowerStateReport {
        PowerStateReport::POWER_OFF_POST_FINISH
    }
    fn is_sleep_state(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        _power_state_request_sender: Sender<PowerRequest>,
        _timeout_ms: u64,
    ) -> anyhow::Result<()> {
        power_off()
    }
}

#[cfg(test)]
use once_cell::sync::OnceCell;
#[cfg(test)]
use sdv_pm::MockPlatformPower;
#[cfg(test)]
static PLATFORM_MOCK_INSTANCE: OnceCell<MockPlatformPower> = OnceCell::new();

#[cfg(test)]
fn get_platform_controller() -> &'static MockPlatformPower {
    PLATFORM_MOCK_INSTANCE.get().expect("Couldn't get mock")
}

#[cfg(test)]
mod tests {
    use crate::power_state::*;

    #[test]
    fn verify_allowed_transitions_to_power_off_exit() {
        let power_off_exit = PowerStateReport::POWER_OFF_EXIT;

        // Denied states
        for state in get_all_states() {
            assert!(!state.is_valid_next_state(power_off_exit));
        }
    }

    #[test]
    fn verify_allowed_transitions_to_suspend_to_ram_exit() {
        let suspend_to_ram_exit = PowerStateReport::SUSPEND_TO_RAM_EXIT;

        // Allowed source states
        let allowed_states: Vec<Box<dyn PowerState>> = vec![Box::new(SuspendToRamPostFinish {})];
        for state in &allowed_states {
            assert!(state.is_valid_next_state(suspend_to_ram_exit));
        }

        // Denied states
        for state in filter_from_all_states(&allowed_states) {
            assert!(!state.is_valid_next_state(suspend_to_ram_exit));
        }
    }

    #[test]
    fn verify_allowed_transitions_to_suspend_to_disk_exit() {
        let suspend_to_disk_exit = PowerStateReport::SUSPEND_TO_DISK_EXIT;

        // Allowed source states
        let allowed_states: Vec<Box<dyn PowerState>> = vec![Box::new(SuspendToDiskPostFinish {})];
        for state in &allowed_states {
            assert!(state.is_valid_next_state(suspend_to_disk_exit));
        }

        // Denied states
        for state in filter_from_all_states(&allowed_states) {
            assert!(!state.is_valid_next_state(suspend_to_disk_exit));
        }
    }

    #[test]
    fn verify_allowed_transitions_to_on() {
        let on = PowerStateReport::ON;

        // Allowed source states
        let allowed_states: Vec<Box<dyn PowerState>> = vec![
            Box::new(PowerOffExit {}),
            Box::new(SuspendToRamExit {}),
            Box::new(SuspendToDiskExit {}),
            Box::new(ShutdownCancelled {}),
        ];
        for state in &allowed_states {
            assert!(state.is_valid_next_state(on));
        }

        // Denied states
        for state in filter_from_all_states(&allowed_states) {
            assert!(!state.is_valid_next_state(on));
        }
    }

    #[test]
    fn verify_allowed_transitions_to_power_off_enter() {
        let power_off_enter = PowerStateReport::POWER_OFF_ENTER;

        // Allowed source states
        let allowed_states: Vec<Box<dyn PowerState>> = vec![Box::new(On {})];
        for state in &allowed_states {
            assert!(state.is_valid_next_state(power_off_enter));
        }

        // Denied states
        for state in filter_from_all_states(&allowed_states) {
            assert!(!state.is_valid_next_state(power_off_enter));
        }
    }

    #[test]
    fn verify_allowed_transitions_to_suspend_to_ram_enter() {
        let suspend_to_ram_enter = PowerStateReport::SUSPEND_TO_RAM_ENTER;

        // Allowed source states
        let allowed_states: Vec<Box<dyn PowerState>> = vec![Box::new(On {})];
        for state in &allowed_states {
            assert!(state.is_valid_next_state(suspend_to_ram_enter));
        }

        // Denied states
        for state in filter_from_all_states(&allowed_states) {
            assert!(!state.is_valid_next_state(suspend_to_ram_enter));
        }
    }

    #[test]
    fn verify_allowed_transitions_to_suspend_to_disk_enter() {
        let suspend_to_disk_enter = PowerStateReport::SUSPEND_TO_DISK_ENTER;

        // Allowed source states
        let allowed_states: Vec<Box<dyn PowerState>> = vec![Box::new(On {})];
        for state in &allowed_states {
            assert!(state.is_valid_next_state(suspend_to_disk_enter));
        }

        // Denied states
        for state in filter_from_all_states(&allowed_states) {
            assert!(!state.is_valid_next_state(suspend_to_disk_enter));
        }
    }

    #[test]
    fn verify_allowed_transitions_to_shutdown_cancelled() {
        let shutdown_cancelled = PowerStateReport::SHUTDOWN_CANCELLED;

        // Allowed source states
        let allowed_states: Vec<Box<dyn PowerState>> = vec![
            Box::new(SuspendToRamEnter {}),
            Box::new(SuspendToDiskEnter {}),
            Box::new(PowerOffEnter {}),
            Box::new(WaitForFinishSuspendToDisk {}),
            Box::new(WaitForFinishSuspendToRam {}),
            Box::new(WaitForFinishPowerOff {}),
        ];
        for state in &allowed_states {
            assert!(state.is_valid_next_state(shutdown_cancelled));
        }

        // Denied states
        for state in filter_from_all_states(&allowed_states) {
            assert!(!state.is_valid_next_state(shutdown_cancelled));
        }
    }

    #[test]
    fn verify_allowed_transitions_to_wait_for_finish() {
        let wait_for_finish = PowerStateReport::WAIT_FOR_FINISH;

        // Allowed source states
        let allowed_states: Vec<Box<dyn PowerState>> = vec![
            Box::new(SuspendToRamEnter {}),
            Box::new(SuspendToDiskEnter {}),
            Box::new(PowerOffEnter {}),
        ];
        for state in &allowed_states {
            assert!(state.is_valid_next_state(wait_for_finish));
        }

        // Denied states
        for state in filter_from_all_states(&allowed_states) {
            assert!(!state.is_valid_next_state(wait_for_finish));
        }
    }

    #[test]
    fn verify_allowed_transitions_to_power_off_post_finish() {
        let power_off_post_finish = PowerStateReport::POWER_OFF_POST_FINISH;

        // Allowed source states
        let allowed_states: Vec<Box<dyn PowerState>> = vec![Box::new(WaitForFinishPowerOff {})];
        for state in &allowed_states {
            assert!(state.is_valid_next_state(power_off_post_finish));
        }

        // Denied states
        for state in filter_from_all_states(&allowed_states) {
            assert!(!state.is_valid_next_state(power_off_post_finish));
        }
    }

    #[test]
    fn verify_allowed_transitions_to_suspend_to_ram_post_finish() {
        let suspend_to_ram_post_finish = PowerStateReport::SUSPEND_TO_RAM_POST_FINISH;

        // Allowed source states
        let allowed_states: Vec<Box<dyn PowerState>> = vec![Box::new(WaitForFinishSuspendToRam {})];
        for state in &allowed_states {
            assert!(state.is_valid_next_state(suspend_to_ram_post_finish));
        }

        // Denied states
        for state in filter_from_all_states(&allowed_states) {
            assert!(!state.is_valid_next_state(suspend_to_ram_post_finish));
        }
    }

    #[test]
    fn verify_allowed_transitions_to_suspend_to_disk_post_finish() {
        let suspend_to_disk_post_finish = PowerStateReport::SUSPEND_TO_DISK_POST_FINISH;

        // Allowed source states
        let allowed_states: Vec<Box<dyn PowerState>> =
            vec![Box::new(WaitForFinishSuspendToDisk {})];
        for state in &allowed_states {
            assert!(state.is_valid_next_state(suspend_to_disk_post_finish));
        }

        // Denied states
        for state in filter_from_all_states(&allowed_states) {
            assert!(!state.is_valid_next_state(suspend_to_disk_post_finish));
        }
    }

    #[tokio::test]
    async fn states_without_execute_return_ok() {
        let (sender, _) = tokio::sync::mpsc::channel(1);
        // Allowed states and remove states with suspend/shutdown
        let states_with_execute: Vec<Box<dyn PowerState>> = vec![
            Box::new(SuspendToRamPostFinish {}),
            Box::new(SuspendToDiskPostFinish {}),
            Box::new(PowerOffPostFinish {}),
            Box::new(SuspendToRamEnter {}),
            Box::new(SuspendToDiskEnter {}),
            Box::new(PowerOffEnter {}),
        ];
        let states_without_platform = filter_from_all_states(&states_with_execute);

        // Executing returns ok
        for state in &states_without_platform {
            assert!(state.execute(sender.clone(), /*timeout=*/ 100).await.is_ok());
        }
    }

    #[tokio::test]
    async fn states_with_execute_no_platform_request_next_state() {
        let (sender, mut receiver) = tokio::sync::mpsc::channel::<PowerRequest>(1);
        // List of pairs (state, next state after execute).
        // Given states that request the next state.
        let states_with_execute_no_platform: Vec<(Box<dyn PowerState>, PowerStateReport)> = vec![
            (Box::new(SuspendToRamEnter {}), PowerStateReport::SUSPEND_TO_RAM_POST_FINISH),
            (Box::new(SuspendToDiskEnter {}), PowerStateReport::SUSPEND_TO_DISK_POST_FINISH),
            (Box::new(PowerOffEnter {}), PowerStateReport::POWER_OFF_POST_FINISH),
        ];

        for (state, expected_next_state) in &states_with_execute_no_platform {
            // When state is executed.
            assert!(state.execute(sender.clone(), /*timeout=*/ 100).await.is_ok());
            let power_request = receiver.recv().await.unwrap();

            // Then next state is requested.
            assert_eq!(power_request.power_state.get_report(), PowerStateReport::WAIT_FOR_FINISH);
            assert!(power_request.power_state.is_valid_next_state(*expected_next_state));
        }
    }

    #[tokio::test]
    async fn suspend_to_ram_calls_suspend_and_requests_next_state() {
        let (sender, mut receiver) = tokio::sync::mpsc::channel::<PowerRequest>(1);
        // Given state suspend to ram.
        let suspend_to_ram = Box::new(SuspendToRamPostFinish {});
        let mut mock = MockPlatformPower::default();
        // Expect to call platform with S2R.
        mock.expect_suspend()
            .times(1)
            .withf(move |mode, platform_timeout, retry_interval_ms| {
                mode == &SuspendMode::RAM
                    && *platform_timeout == DEFAULT_PLATFORM_TIMEOUT_MS
                    && *retry_interval_ms == DEFAULT_PLATFORM_RETRY_INTERVAL_MS
            })
            .returning(|_, _, _| Ok(()));
        PLATFORM_MOCK_INSTANCE.set(mock).unwrap();

        // When state is executed.
        assert!(suspend_to_ram.execute(sender.clone(), /*timeout=*/ 100).await.is_ok());

        // Then next state is requested
        let power_request = receiver.recv().await.unwrap();
        assert_eq!(power_request.power_state.get_report(), PowerStateReport::SUSPEND_TO_RAM_EXIT);
    }

    #[tokio::test]
    async fn suspend_to_disk_calls_suspend_and_requests_next_state() {
        let (sender, mut receiver) = tokio::sync::mpsc::channel::<PowerRequest>(1);
        // Given state suspend to disk.
        let suspend_to_disk = Box::new(SuspendToDiskPostFinish {});
        let mut mock = MockPlatformPower::default();
        // Expect to call platform with S2D.
        mock.expect_suspend()
            .times(1)
            .withf(move |mode, platform_timeout, retry_interval_ms| {
                mode == &SuspendMode::DISK
                    && *platform_timeout == DEFAULT_PLATFORM_TIMEOUT_MS
                    && *retry_interval_ms == DEFAULT_PLATFORM_RETRY_INTERVAL_MS
            })
            .returning(|_, _, _| Ok(()));
        PLATFORM_MOCK_INSTANCE.set(mock).unwrap();

        // When state is executed.
        assert!(suspend_to_disk.execute(sender.clone(), /*timeout=*/ 100).await.is_ok());

        // Then next state is requested
        let power_request = receiver.recv().await.unwrap();
        assert_eq!(power_request.power_state.get_report(), PowerStateReport::SUSPEND_TO_DISK_EXIT);
    }

    #[tokio::test]
    async fn power_off_calls_platform_power_off() {
        let (sender, _) = tokio::sync::mpsc::channel::<PowerRequest>(1);
        // Given state suspend to ram
        let power_off = Box::new(PowerOffPostFinish {});
        let mut mock = MockPlatformPower::default();
        // Expect to call platform with power off.
        mock.expect_power_off()
            .times(1)
            .withf(move |platform_timeout| *platform_timeout == DEFAULT_PLATFORM_TIMEOUT_MS)
            .returning(|_| Ok(()));
        PLATFORM_MOCK_INSTANCE.set(mock).unwrap();

        // When state is executed, then power off is run.
        assert!(power_off.execute(sender.clone(), /*timeout=*/ 100).await.is_ok());
    }

    #[test]
    fn verify_sleep_states() {
        // Allowed sleep states
        let allowed_states: Vec<Box<dyn PowerState>> = vec![
            Box::new(PowerOffPostFinish {}),
            Box::new(SuspendToDiskPostFinish {}),
            Box::new(SuspendToRamPostFinish {}),
        ];

        for state in &allowed_states {
            assert!(state.is_sleep_state());
        }

        // non sleep states
        for state in filter_from_all_states(&allowed_states) {
            assert!(!state.is_sleep_state());
        }
    }

    fn get_all_states() -> Vec<Box<dyn PowerState>> {
        vec![
            Box::new(PowerOffExit {}),
            Box::new(SuspendToRamExit {}),
            Box::new(SuspendToDiskExit {}),
            Box::new(ShutdownCancelled {}),
            Box::new(On {}),
            Box::new(PowerOffEnter {}),
            Box::new(SuspendToRamEnter {}),
            Box::new(SuspendToDiskEnter {}),
            Box::new(WaitForFinishSuspendToRam {}),
            Box::new(WaitForFinishSuspendToDisk {}),
            Box::new(WaitForFinishPowerOff {}),
            Box::new(SuspendToRamPostFinish {}),
            Box::new(SuspendToDiskPostFinish {}),
            Box::new(PowerOffPostFinish {}),
        ]
    }

    fn filter_from_all_states(
        states_to_filter: &Vec<Box<dyn PowerState>>,
    ) -> Vec<Box<dyn PowerState>> {
        let mut filtered_states = Vec::new();
        for state in get_all_states() {
            let mut found = false;
            for to_filter in states_to_filter {
                if state.get_report() == to_filter.get_report() {
                    found = true;
                    break;
                }
            }
            if !found {
                filtered_states.push(state);
            }
        }
        filtered_states
    }
}
