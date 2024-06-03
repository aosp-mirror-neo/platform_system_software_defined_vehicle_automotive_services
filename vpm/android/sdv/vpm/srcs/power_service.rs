use anyhow::anyhow;
use mockall::automock;
use sdv_vpm_api::power::PowerStateReport;
use sdv_vpm_api::vpm::PowerStateChangeRequest;
use sdv_vpm_api::vpm::PowerStateRequest;
use sdv_vpm_api::vpm::ShutdownType;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;

#[automock]
pub trait PowerService: Send + Sync {
    /// Requests a state change in the power state machine from the OEM.
    /// returns Ok() if the request passes the state machine validation.
    /// otherwise returns an error.
    fn set_power_state(&mut self, request: PowerStateChangeRequest) -> anyhow::Result<()>;
}

#[derive(Debug, Clone)]
pub struct PowerServiceImpl {
    power_state_report: PowerStateReport,
    sender: Sender<PowerStateReport>,
}

impl PowerServiceImpl {
    /// Creates a new PowerServiceImpl with the default value of the power state being POWER_OFF_EXIST.
    /// The POWER_STATE_EXIST is the only possible state at this step because calling new() means we are coming from cold boot.
    /// If we resume from S2R or S2D new() should not be called again.
    pub fn new(sender: Sender<PowerStateReport>) -> Self {
        Self { power_state_report: PowerStateReport::POWER_OFF_EXIT, sender }
    }
}

impl PowerService for PowerServiceImpl {
    fn set_power_state(&mut self, request: PowerStateChangeRequest) -> anyhow::Result<()> {
        //TODO validate the request
        let report = match request.power_request.unwrap() {
            PowerStateRequest::TURN_ON => PowerStateReport::ON,
            PowerStateRequest::CANCEL_SHUTDOWN => PowerStateReport::SHUTDOWN_CANCELLED,
            PowerStateRequest::PREPARE_SHUTDOWN => match request.shutdown_type.unwrap() {
                ShutdownType::POWER_OFF => PowerStateReport::POWER_OFF_ENTER,
                ShutdownType::SUSPEND_TO_RAM => PowerStateReport::SUSPEND_TO_RAM_ENTER,
                ShutdownType::SUSPEND_TO_DISK => PowerStateReport::SUSPEND_TO_DISK_ENTER,
                ShutdownType::SHUTDOWN_TYPE_UNSPECIFIED => {
                    return Err(anyhow!("Shutdown type was not specified!"))
                }
            },
            PowerStateRequest::FINISH_SHUTDOWN => match request.shutdown_type.unwrap() {
                ShutdownType::POWER_OFF => PowerStateReport::POWER_OFF_POST_FINISH,
                ShutdownType::SUSPEND_TO_RAM => PowerStateReport::SUSPEND_TO_RAM_POST_FINISH,
                ShutdownType::SUSPEND_TO_DISK => PowerStateReport::SUSPEND_TO_DISK_POST_FINISH,
                ShutdownType::SHUTDOWN_TYPE_UNSPECIFIED => {
                    return Err(anyhow!("Shutdown type was not specified!"))
                }
            },
            PowerStateRequest::REQUEST_UNSPECIFIED => {
                return Err(anyhow!("Power request was not specified!"))
            }
        };

        self.sender.blocking_send(report);
        self.power_state_report = report;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::power_service::PowerService;
    use crate::PowerServiceImpl;
    use sdv_vpm_api::power::PowerStateReport;
    use sdv_vpm_api::vpm::PowerStateChangeRequest;
    use sdv_vpm_api::vpm::PowerStateRequest;
    use sdv_vpm_api::vpm::ShutdownType;
    use tokio::sync::mpsc::channel;

    fn get_power_service() -> PowerServiceImpl {
        let (tx, _) = channel::<PowerStateReport>(1);
        PowerServiceImpl::new(tx)
    }

    #[test]
    fn set_power_state_fails_unspecified_power_state() {
        // Given: the power service.
        let mut power_service = get_power_service();

        // When: a power state request is sent without power state.
        let request = PowerStateChangeRequest { ..Default::default() };

        // Then: power state is set.
        let result = power_service.set_power_state(request);
        assert!(result.is_err());
    }

    #[test]
    fn set_power_state_fails_prepare_no_shutdown_type() {
        // Given: the power service.
        let mut power_service = get_power_service();

        // When: a prepare power state request is sent without shutdown type.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_service.set_power_state(request);
        assert!(result.is_err());
    }

    #[test]
    fn set_power_state_fails_turn_off_no_shutdown_type() {
        // Given: the power service.
        let mut power_service = get_power_service();

        // When: a power off state request is sent without shutdown type.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_service.set_power_state(request);
        assert!(result.is_err());
    }

    #[test]
    fn set_power_state_on() {
        // Given: the power service.
        let mut power_service = get_power_service();

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::TURN_ON.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_service.set_power_state(request);
        assert_eq!(PowerStateReport::ON, power_service.power_state_report);
        assert!(result.is_ok());
    }

    #[test]
    fn set_power_state_prepare_s2r() {
        // Given: the power service.
        let mut power_service = get_power_service();

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
            shutdown_type: ShutdownType::SUSPEND_TO_RAM.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_service.set_power_state(request);
        assert_eq!(PowerStateReport::SUSPEND_TO_RAM_ENTER, power_service.power_state_report);
        assert!(result.is_ok());
    }

    #[test]
    fn set_power_state_prepare_s2d() {
        // Given: the power service.
        let mut power_service = get_power_service();

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
            shutdown_type: ShutdownType::SUSPEND_TO_DISK.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_service.set_power_state(request);
        assert_eq!(PowerStateReport::SUSPEND_TO_DISK_ENTER, power_service.power_state_report);
        assert!(result.is_ok());
    }

    #[test]
    fn set_power_state_prepare_off() {
        // Given: the power service.
        let mut power_service = get_power_service();

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::PREPARE_SHUTDOWN.into(),
            shutdown_type: ShutdownType::POWER_OFF.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_service.set_power_state(request);
        assert_eq!(PowerStateReport::POWER_OFF_ENTER, power_service.power_state_report);
        assert!(result.is_ok());
    }

    #[test]
    fn set_power_state_cancel_shutdown() {
        // Given: the power service.
        let mut power_service = get_power_service();

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::CANCEL_SHUTDOWN.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_service.set_power_state(request);
        assert_eq!(PowerStateReport::SHUTDOWN_CANCELLED, power_service.power_state_report);
        assert!(result.is_ok());
    }

    #[test]
    fn set_power_state_s2r() {
        // Given: the power service.
        let mut power_service = get_power_service();

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
            shutdown_type: ShutdownType::SUSPEND_TO_RAM.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_service.set_power_state(request);
        assert_eq!(PowerStateReport::SUSPEND_TO_RAM_POST_FINISH, power_service.power_state_report);
        assert!(result.is_ok());
    }

    #[test]
    fn set_power_state_s2d() {
        // Given: the power service.
        let mut power_service = get_power_service();

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
            shutdown_type: ShutdownType::SUSPEND_TO_DISK.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_service.set_power_state(request);
        assert_eq!(PowerStateReport::SUSPEND_TO_DISK_POST_FINISH, power_service.power_state_report);
        assert!(result.is_ok());
    }

    #[test]
    fn set_power_state_off() {
        // Given: the power service.
        let mut power_service = get_power_service();

        // When: a power state request is sent.
        let request = PowerStateChangeRequest {
            power_request: PowerStateRequest::FINISH_SHUTDOWN.into(),
            shutdown_type: ShutdownType::POWER_OFF.into(),
            ..Default::default()
        };

        // Then: power state is set.
        let result = power_service.set_power_state(request);
        assert_eq!(PowerStateReport::POWER_OFF_POST_FINISH, power_service.power_state_report);
        assert!(result.is_ok());
    }
}
