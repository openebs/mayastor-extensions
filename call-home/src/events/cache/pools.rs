use crate::common::errors;
use mbus_api::message::Action;
use serde::{Deserialize, Serialize};

/// Pool related events.
#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct Pool {
    pub pool_created: u32,
    pub pool_deleted: u32,
}

impl Pool {
    pub(crate) fn inc_counter(&mut self, action: Action) -> errors::Result<()> {
        match action {
            Action::CreateEvent => {
                self.pool_created += 1;
            }
            Action::DeleteEvent => {
                self.pool_deleted += 1;
            }
            Action::Unknown => {
                return Ok(());
            }
        }
        Ok(())
    }
}
