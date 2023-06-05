use crate::{
    common::{
        constants::{
            EVENT_STATS_DATA, EVENT_STORE, EVENT_STORE_LABLE_KEY, PATCH_PARAM_FILED_MANAGER,
        },
        error,
        utils::get_release_name,
    },
    Cache, EventSet,
};
use k8s_openapi::api::core::v1::ConfigMap;
use kube::{
    api::{Api, Patch, PatchParams, PostParams},
    core::ObjectMeta,
    Client,
};
use snafu::ResultExt;
use std::{collections::BTreeMap, ops::DerefMut, time::Duration};
use tracing::info;

/// Initialize a config map for storing events.
pub async fn initialize(namespace: &str) -> error::Result<ConfigMap> {
    let client = Client::try_default().await.context(error::K8sClient)?;
    let release_name = get_release_name(namespace, client.clone()).await?;
    let api: Api<ConfigMap> = Api::namespaced(client.clone(), namespace);
    let config_map_name = format!("{release_name}-{EVENT_STORE}");

    if let Some(cm) =
        api.get_opt(config_map_name.as_str())
            .await
            .context(error::GetEventStoreConfigMap {
                name: config_map_name.clone(),
            })?
    {
        info!(
            "Config map {} for events store already exists.",
            config_map_name.clone()
        );
        return Ok(cm);
    }

    info!(
        "Creating Config map {} for events store!!!",
        config_map_name.clone()
    );
    let cm = create_configmap(namespace, client.clone(), config_map_name.as_str()).await?;
    Ok(cm)
}

/// Create a config map for storing events.
async fn create_configmap(
    ns: &str,
    client: Client,
    config_map_name: &str,
) -> error::Result<ConfigMap> {
    let api: Api<ConfigMap> = Api::namespaced(client.clone(), ns);
    let mut labels = BTreeMap::new();
    labels.insert(EVENT_STORE_LABLE_KEY.to_string(), EVENT_STORE.to_string());
    let metadata = ObjectMeta {
        name: Some(config_map_name.to_string()),
        labels: Some(labels),
        ..Default::default()
    };

    let data = init_config_map_data()?;
    let config_map = ConfigMap {
        data: Some(data),
        metadata,
        ..Default::default()
    };

    let cm = api
        .create(&PostParams::default(), &config_map)
        .await
        .context(error::ServiceAccountCreate {
            name: config_map_name,
        })?;

    // Waiting for the api-server to accept the cm
    tokio::time::sleep(Duration::from_secs(5)).await;
    info!(
        "Config map {} for events store created successfully.",
        config_map_name
    );
    Ok(cm)
}

// Function to initialize the confige map data
fn init_config_map_data() -> error::Result<BTreeMap<String, String>> {
    let cp = EventSet::default();
    let value = serde_json::to_string(&cp).context(error::SerializeEvent { note: cp })?;

    let mut data = BTreeMap::new();
    data.insert(EVENT_STATS_DATA.to_string(), value);
    Ok(data)
}

/// Function to update the config map data.
pub async fn update_config_map_data(namespace: &str) -> error::Result<()> {
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
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

fn update_config_map() -> error::Result<BTreeMap<String, String>> {
    let mut c = Cache::get_cache().lock().unwrap();
    let mut binding = c.deref_mut().data_mut();
    let cp = binding.deref_mut();
    let value = serde_json::to_string(&cp).context(error::SerializeEvent { note: cp })?;
    let mut data = BTreeMap::new();
    data.insert(EVENT_STATS_DATA.to_string(), value);
    Ok(data)
}
