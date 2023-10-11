use super::events_cache::StatsCounter;
use events_api::event::EventAction;
use serde::{Deserialize, Serialize};

/// Volume related events.
#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub(crate) struct Volume {
    pub(crate) volume_created: u32,
    pub(crate) volume_deleted: u32,
}

impl StatsCounter for Volume {
    fn update_counter(&mut self, action: EventAction) {
        match action {
            EventAction::Create => {
                self.volume_created += 1;
            }
            EventAction::Delete => {
                self.volume_deleted += 1;
            }
            _ => {}
        }
    }
}
