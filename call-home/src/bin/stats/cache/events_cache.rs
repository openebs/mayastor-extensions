use crate::cache::{pools, volume};
use k8s_openapi::api::core::v1::ConfigMap;
use mbus_api::{
    mbus_nats::BusSubscription,
    message::{Action, Category, EventMessage},
};
use obs::common::{constants::EVENT_STATS_DATA, errors};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use std::{ops::DerefMut, sync::Mutex};

static CACHE: OnceCell<Mutex<Cache>> = OnceCell::new();

/// EventSet captures the type of events.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct EventSet {
    pub pool: pools::Pool,
    pub volume: volume::Volume,
}

impl EventSet {
    pub fn from_event_store(init_data: ConfigMap) -> errors::Result<Self> {
        let data = init_data
            .data
            .ok_or(errors::ReferenceConfigMapNoData.build())?;
        let value = data.get(EVENT_STATS_DATA).ok_or(
            errors::ReferencedKeyNotPresent {
                key: EVENT_STATS_DATA.to_string(),
            }
            .build(),
        )?;

        let event_set = serde_json::from_str(value)
            .context(errors::EventSerdeDeserialization { event: value })?;
        Ok(event_set)
    }

    fn inc_counter(&mut self, category: Category, action: Action) {
        match category {
            Category::Pool => self.pool.update_counter(action),
            Category::Volume => self.volume.update_counter(action),
            Category::Unknown => {}
        }
    }
}

impl From<&mut EventSet> for EventSet {
    fn from(event_set: &mut EventSet) -> Self {
        EventSet {
            pool: event_set.pool.clone(),
            volume: event_set.volume.clone(),
        }
    }
}

/// Cache to store data that has to be exposed though exporter.
pub struct Cache {
    events: EventSet,
}

impl Cache {
    /// Initialize the cache with default value.
    pub(crate) fn initialize(events: EventSet) {
        CACHE.get_or_init(|| Mutex::new(Self { events }));
    }

    /// Returns cache.
    #[warn(clippy::self_named_constructors)]
    pub fn cache_init() -> &'static Mutex<Cache> {
        CACHE.get().expect("Cache is not initialized")
    }

    /// Get data field in cache.
    pub fn data_mut(&mut self) -> &mut EventSet {
        &mut self.events
    }
}

/// To store data in shared variable i.e cache.
pub(crate) async fn store_events(mut sub: BusSubscription<EventMessage>) -> errors::Result<()> {
    while let Some(message) = sub.next().await {
        let mut cache = Cache::cache_init().lock().expect("not poisoned");
        let events_cache = cache.deref_mut();
        events_cache
            .data_mut()
            .inc_counter(message.category, message.action);
    }
    Ok(())
}

/// Trait for updating the counters.
pub(crate) trait StatsCounter {
    fn update_counter(&mut self, action: Action);
}
