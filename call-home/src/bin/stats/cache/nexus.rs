use super::events_cache::StatsCounter;
use events_api::event::EventAction;
use serde::{Deserialize, Serialize};

/// Nexus related events.
#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub(crate) struct Nexus {
    pub(crate) nexus_created: u32,
    pub(crate) nexus_deleted: u32,
    pub(crate) rebuild_started: u32,
    pub(crate) rebuild_ended: u32,
}

impl StatsCounter for Nexus {
    fn update_counter(&mut self, action: EventAction) {
        match action {
            EventAction::Create => {
                self.nexus_created += 1;
            }
            EventAction::Delete => {
                self.nexus_deleted += 1;
            }
            EventAction::RebuildBegin => {
                self.rebuild_started += 1;
            }
            EventAction::RebuildEnd => {
                self.rebuild_ended += 1;
            }
            _ => {}
        }
    }
}
