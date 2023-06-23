use super::events_cache::StatsCounter;
use mbus_api::message::Action;
use serde::{Deserialize, Serialize};

/// Pool related events.
#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct Pool {
    pub pool_created: u32,
    pub pool_deleted: u32,
}

impl StatsCounter for Pool {
    fn update_counter(&mut self, action: Action) {
        match action {
            Action::CreateEvent => {
                self.pool_created += 1;
            }
            Action::DeleteEvent => {
                self.pool_deleted += 1;
            }
            Action::Unknown => {}
        }
    }
}
