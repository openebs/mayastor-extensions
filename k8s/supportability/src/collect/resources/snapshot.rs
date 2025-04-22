use crate::collect::{
    logs::create_directory_if_not_exist,
    resources::{traits::Topologer, utils, ResourceError, Resourcer},
    rest_wrapper::RestClient,
};
use openapi::models::VolumeSnapshot;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Write, path::PathBuf};

/// Holds topological information of volume snapshot resource.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct VolumeSnapshotTopology {
    snapshot: VolumeSnapshot,
}

/// Implements functionality to inspect topological information of snapshot resource.
impl Topologer for VolumeSnapshotTopology {
    fn get_printable_topology(&self) -> Result<(String, String), ResourceError> {
        let topology_as_pretty = serde_json::to_string_pretty(self)?;
        let file_path = format!(
            "snapshot-{}-topology.json",
            self.snapshot.definition.spec.uuid
        );
        Ok((file_path, topology_as_pretty))
    }

    fn dump_topology_info(&self, dir_path: PathBuf) -> Result<(), ResourceError> {
        create_directory_if_not_exist(dir_path.clone())?;
        let file_path = dir_path.join(format!(
            "snapshot-{}-topology.json",
            self.snapshot.definition.spec.uuid
        ));
        let mut topo_file = File::create(file_path)?;
        let topology_as_pretty = serde_json::to_string_pretty(self)?;
        topo_file.write_all(topology_as_pretty.as_bytes())?;
        topo_file.flush()?;
        Ok(())
    }
}

/// Wrapper around mayastor REST client.
#[derive(Debug)]
pub(crate) struct VolumeSnapshotClientWrapper {
    rest_client: RestClient,
}

impl VolumeSnapshotClientWrapper {
    /// Builds new instance of VolumeSnapshotClientWrapper
    pub(crate) fn new(client: RestClient) -> Self {
        VolumeSnapshotClientWrapper {
            rest_client: client,
        }
    }

    async fn list_snapshots(&self) -> Result<Vec<VolumeSnapshot>, ResourceError> {
        let mut volume_snapshots: Vec<VolumeSnapshot> = Vec::new();
        let mut next_token: Option<isize> = Some(0);
        let max_entries: isize = utils::MAX_RESOURCE_ENTRIES;
        loop {
            let snapshot_api_resp = self
                .rest_client
                .snapshots_api()
                .get_volumes_snapshots(max_entries, None, None, next_token)
                .await?
                .into_body();
            volume_snapshots.extend(snapshot_api_resp.entries);
            if snapshot_api_resp.next_token.is_none() {
                break;
            }
            next_token = snapshot_api_resp.next_token;
        }
        Ok(volume_snapshots)
    }

    async fn get_snapshot(&self, id: openapi::apis::Uuid) -> Result<VolumeSnapshot, ResourceError> {
        let snapshot = self
            .rest_client
            .snapshots_api()
            .get_volumes_snapshot(&id)
            .await?
            .into_body();
        Ok(snapshot)
    }
}

#[async_trait(?Send)]
impl Resourcer for VolumeSnapshotClientWrapper {
    type ID = openapi::apis::Uuid;

    async fn get_topologer(
        &self,
        id: Option<Self::ID>,
    ) -> Result<Box<dyn Topologer>, ResourceError> {
        if let Some(snapshot_id) = id {
            let snapshot = self.get_snapshot(snapshot_id).await?;
            return Ok(Box::new(VolumeSnapshotTopology { snapshot }));
        }
        let snapshots_topology: Vec<VolumeSnapshotTopology> = self
            .list_snapshots()
            .await?
            .into_iter()
            .map(|snapshot| VolumeSnapshotTopology { snapshot })
            .collect();
        Ok(Box::new(snapshots_topology))
    }
}
