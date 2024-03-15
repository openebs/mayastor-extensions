# crds

A Helm chart that collects CustomResourceDefinitions (CRDs) from Mayastor.

## Values

| Key | Description | Default |
|:----|:------------|:--------|
| jaeger.&ZeroWidthSpace;enabled | Install Jaeger CRDs | `true` |
| volumeSnapshots.&ZeroWidthSpace;enabled | Install Volume Snapshot CRDs | `true` |
| volumeSnapshots.&ZeroWidthSpace;snapshotClassesEnabled | Install volumesnapshotclasses.snapshot.storage.k8s.io CRD | `true` |
| volumeSnapshots.&ZeroWidthSpace;snapshotContentsEnabled | Install volumesnapshotcontents.snapshot.storage.k8s.io CRD | `true` |
| volumeSnapshots.&ZeroWidthSpace;snapshotsEnabled | Install volumesnapshots.snapshot.storage.k8s.io CRD | `true` |

