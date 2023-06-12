use crate::{
    common::{
        constants::{EVENT_STATS_DATA, EVENT_STORE, PATCH_PARAM_FILED_MANAGER},
        errors,
        utils::get_release_name,
    },
    events::cache::events_cache::Cache,
};
use k8s_openapi::api::core::v1::ConfigMap;
use kube::{
    api::{Api, Patch, PatchParams},
    core::ObjectMeta,
    Client,
};
use snafu::ResultExt;
use std::{collections::BTreeMap, ops::DerefMut, time::Duration};
use tracing::info;

/// Initialize a config map for storing events.
pub(crate) async fn initialize(namespace: &str) -> errors::Result<ConfigMap> {
    let client = Client::try_default().await.context(errors::K8sClient)?;
    let release_name = get_release_name(namespace, client.clone()).await?;
    let api: Api<ConfigMap> = Api::namespaced(client.clone(), namespace);
    let config_map_name = format!("{release_name}-{EVENT_STORE}");

    let cm_maybe =
        api.get_opt(config_map_name.as_str())
            .await
            .context(errors::GetEventStoreConfigMap {
                name: config_map_name.clone(),
            })?;
    let cm = cm_maybe.ok_or(
        errors::ConfigMapNotPresent {
            name: config_map_name,
        }
        .build(),
    )?;
    Ok(cm)
}

/// Function to update the config map data.
pub(crate) async fn update_config_map_data(namespace: &str) -> errors::Result<()> {
    let client = Client::try_default().await.unwrap();
    let release_name = get_release_name(namespace, client.clone()).await?;
    let config_map_name = format!("{release_name}-{EVENT_STORE}");
    let api: Api<ConfigMap> = Api::namespaced(client.clone(), namespace);
    loop {
        let map = update_config_map()?;
        let meta = ObjectMeta {
            name: Some(config_map_name.clone()),
            ..Default::default()
        };
        let cm = ConfigMap {
            data: Some(map),
            metadata: meta,
            ..Default::default()
        };
        let ssapply = PatchParams::apply(PATCH_PARAM_FILED_MANAGER).force();
        api.patch(config_map_name.as_str(), &ssapply, &Patch::Apply(&cm))
            .await
            .unwrap();
        info!(
            "Config map {} for events store updated successfully.",
            config_map_name
        );
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

/// Function to update config map.
pub(crate) fn update_config_map() -> errors::Result<BTreeMap<String, String>> {
    let mut c = Cache::get_cache().lock().unwrap();
    let mut binding = c.deref_mut().data_mut();
    let cp = binding.deref_mut();
    let value = serde_json::to_string(&cp).context(errors::SerializeEvent)?;
    let mut data = BTreeMap::new();
    data.insert(EVENT_STATS_DATA.to_string(), value);
    Ok(data)
}
