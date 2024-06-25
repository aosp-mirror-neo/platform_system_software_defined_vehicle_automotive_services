use anyhow::anyhow;
use async_trait::async_trait;
use mockall::automock;
use sdv_pm::PlatformPowerController;
use sdv_pm::SuspendMode;
use sdv_vpm_api::power::PowerStateReport;
use tokio::sync::mpsc::Sender;

fn suspend_to_ram() -> anyhow::Result<()> {
    let platform = PlatformPowerController::default();
    platform.suspend(&SuspendMode::RAM, 100, 10)?;
    Ok(())
}

fn suspend_to_disk() -> anyhow::Result<()> {
    let platform = PlatformPowerController::default();
    platform.suspend(&SuspendMode::DISK, 100, 10)?;
    Ok(())
}

fn power_off() -> anyhow::Result<()> {
    let platform = PlatformPowerController::default();
    platform.power_off(100)?;
    Ok(())
}

#[automock]
#[async_trait]
pub trait PowerState: Send + Sync {
    fn get_report(&self) -> PowerStateReport;

    fn is_valid_next_state(&self, next_state: PowerStateReport) -> bool;

    fn is_sleep_state(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        power_state_request_sender: Sender<Box<dyn PowerState>>,
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
        power_state_request_sender: Sender<Box<dyn PowerState>>,
    ) -> anyhow::Result<()> {
        power_state_request_sender.send(Box::new(WaitForFinish {})).await;
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
        power_state_request_sender: Sender<Box<dyn PowerState>>,
    ) -> anyhow::Result<()> {
        power_state_request_sender.send(Box::new(WaitForFinish {})).await;
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
        power_state_request_sender: Sender<Box<dyn PowerState>>,
    ) -> anyhow::Result<()> {
        power_state_request_sender.send(Box::new(WaitForFinish {})).await;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct WaitForFinish {}

#[async_trait]
impl PowerState for WaitForFinish {
    //TODO: This needs to be optimized, otherwise someone can call suspend to ram
    // Then wait until wait_for_finish and call suspend to disk or power off
    fn is_valid_next_state(&self, next_state: PowerStateReport) -> bool {
        [
            PowerStateReport::SUSPEND_TO_RAM_POST_FINISH,
            PowerStateReport::SUSPEND_TO_DISK_POST_FINISH,
            PowerStateReport::POWER_OFF_POST_FINISH,
        ]
        .contains(&next_state)
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
        power_state_request_sender: Sender<Box<dyn PowerState>>,
    ) -> anyhow::Result<()> {
        suspend_to_ram()?;
        power_state_request_sender.send(Box::new(SuspendToRamExit {})).await;
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
        power_state_request_sender: Sender<Box<dyn PowerState>>,
    ) -> anyhow::Result<()> {
        suspend_to_disk()?;
        power_state_request_sender.send(Box::new(SuspendToDiskExit {})).await;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PowerOffPostFinish {}

#[async_trait]
impl PowerState for PowerOffPostFinish {
    fn is_valid_next_state(&self, next_state: PowerStateReport) -> bool {
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
        power_state_request_sender: Sender<Box<dyn PowerState>>,
    ) -> anyhow::Result<()> {
        power_off()
    }
}

#[cfg(test)]
mod tests {
    use crate::power_state::*;
    use log::info;

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
        let allowed_states: Vec<Box<dyn PowerState>> = vec![Box::new(WaitForFinish {})];
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
        let allowed_states: Vec<Box<dyn PowerState>> = vec![Box::new(WaitForFinish {})];
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
        let allowed_states: Vec<Box<dyn PowerState>> = vec![Box::new(WaitForFinish {})];
        for state in &allowed_states {
            assert!(state.is_valid_next_state(suspend_to_disk_post_finish));
        }

        // Denied states
        for state in filter_from_all_states(&allowed_states) {
            assert!(!state.is_valid_next_state(suspend_to_disk_post_finish));
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
            Box::new(WaitForFinish {}),
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
