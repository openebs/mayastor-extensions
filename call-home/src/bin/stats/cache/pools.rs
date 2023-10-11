use super::events_cache::StatsCounter;
use events_api::event::EventAction;
use serde::{Deserialize, Serialize};

/// Pool related events.
#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub(crate) struct Pool {
    pub(crate) pool_created: u32,
    pub(crate) pool_deleted: u32,
}

impl StatsCounter for Pool {
    fn update_counter(&mut self, action: EventAction) {
        match action {
            EventAction::Create => {
                self.pool_created += 1;
            }
            EventAction::Delete => {
                self.pool_deleted += 1;
            }
            _ => {}
        }
    }
}
