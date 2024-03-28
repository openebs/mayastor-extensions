# crds

A Helm chart that collects CustomResourceDefinitions (CRDs) from Mayastor.

## Values

| Key | Description | Default |
|:----|:------------|:--------|
| csi.&ZeroWidthSpace;volumeSnapshots.&ZeroWidthSpace;annotations | Annotations to be added to all CRDs | <pre>{<br><br>}</pre> |
| csi.&ZeroWidthSpace;volumeSnapshots.&ZeroWidthSpace;enabled | Install Volume Snapshot CRDs | `true` |
| csi.&ZeroWidthSpace;volumeSnapshots.&ZeroWidthSpace;keep | Keep CRDs on chart uninstall | `true` |
| jaeger.&ZeroWidthSpace;annotations | Annotations to be added to all CRDs | <pre>{<br><br>}</pre> |
| jaeger.&ZeroWidthSpace;enabled | Install Jaeger CRDs | `true` |
| jaeger.&ZeroWidthSpace;keep | Keep CRDs on chart uninstall | `true` |

