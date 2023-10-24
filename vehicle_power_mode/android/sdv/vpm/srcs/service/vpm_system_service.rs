// Copyright 2023 Google LLC

use android_sdv_vpm_aidl::aidl::android::sdv::vpm::PowerState::PowerState;
use log::{info, warn};
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::broadcast;
use tracing::instrument;

pub const NUMBER_OF_MESSAGES_TO_QUEUE: usize = 32;

pub struct VPMSystemService {
    power_state: Mutex<PowerState>,
    update_sender: broadcast::Sender<PowerState>,
    custom_states_map: Mutex<HashMap<String, CustomState>>,
}

struct CustomState {
    value: String,
    sender: broadcast::Sender<String>,
}

impl VPMSystemService {
    pub fn new() -> Self {
        let (update_sender, _) = broadcast::channel(NUMBER_OF_MESSAGES_TO_QUEUE);
        Self {
            // In the current design power and custom states are protected by the individual
            // mutexes. That creates a risk of ABBA deadlocks in case we want lock both of them in the
            // same thread. However we want to control the different state machines independently and don't
            // need to lock both of them for now. If we ever do, we need to either move those
            // resources under the single mutex or enforce ordering of the locks.
            power_state: Mutex::<PowerState>::new(PowerState::WAITING_FOR_SIGNAL),
            update_sender,
            custom_states_map: Mutex::new(HashMap::new()),
        }
    }

    #[instrument(skip(self))]
    pub(super) fn get_power_state(&self) -> PowerState {
        info!("Getting Power state");
        *self.power_state.lock().unwrap()
    }

    #[instrument(skip(self))]
    pub(super) fn set_custom_state(
        &self,
        state_machine_name: String,
        state_machine_value: String,
    ) -> String {
        let mut mut_custom_states_map = self.custom_states_map.lock().unwrap();
        let result = match mut_custom_states_map.get_mut(&state_machine_name) {
            Some(custom_state) => {
                custom_state.value = state_machine_value.to_owned();
                custom_state.sender.send(state_machine_value.to_owned())
            }
            None => {
                // if the custom state has not been broadcasted before, then create a new struct and a new channel.
                let (sender, _) = broadcast::channel(NUMBER_OF_MESSAGES_TO_QUEUE);
                let custom_state =
                    CustomState { value: state_machine_name.to_owned(), sender: sender.clone() };
                mut_custom_states_map.insert(state_machine_name, custom_state);
                sender.send(state_machine_value.to_owned())
            }
        };

        match result {
            Err(e) => {
                warn!(
                    "Message could not be forwarded because there are no active receivers {:?}",
                    e
                );
                state_machine_value
            }
            Ok(_) => state_machine_value,
        }
    }

    #[instrument(skip(self))]
    pub(super) fn set_power_state(&self, in_power_state: PowerState) -> PowerState {
        let mut state = self.power_state.lock().unwrap();
        *state = in_power_state;
        let result = self.update_sender.send(*state);
        match result {
            Err(e) => {
                warn!(
                    "Message could not be forwarded because there are no active receivers {:?}",
                    e
                );
                *state
            }
            Ok(_) => *state,
        }
    }

    #[instrument(skip(self))]
    pub(crate) fn subscribe_to_power_updates(&self) -> broadcast::Receiver<PowerState> {
        self.update_sender.subscribe()
    }

    #[instrument(skip(self))]
    pub(crate) fn subscribe_to_custom_updates(
        &self,
        state_machine_name: &str,
    ) -> Option<broadcast::Receiver<String>> {
        let mut_custom_states_map = self.custom_states_map.lock().unwrap();
        match mut_custom_states_map.get(state_machine_name) {
            Some(custom_state) => Some(custom_state.sender.subscribe()),
            None => {
                warn!("Custom state machine was not found.");
                None
            }
        }
    }
}
