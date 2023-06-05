use crate::common::error;
use mbus_api::message::Action;
use serde::{Deserialize, Serialize};

/// Volume related events.
#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct Volume {
    pub volume_created: u32,
    pub volume_deleted: u32,
}

impl Volume {
    pub fn inc_counter(&mut self, action: Action) -> error::Result<()> {
        match action {
            Action::CreateEvent => {
                self.volume_created += 1;
            }
            Action::DeleteEvent => {
                self.volume_deleted += 1;
            }
        }
        Ok(())
    }
}
