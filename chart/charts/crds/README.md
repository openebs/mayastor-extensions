# crds

A Helm chart that collects CustomResourceDefinitions (CRDs) from Mayastor.

## Values

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| csi.volumeSnapshots.annotations | object | `{}` | Annotations to be added to all CRDs |
| csi.volumeSnapshots.enabled | bool | `true` | Install Volume Snapshot CRDs |
| csi.volumeSnapshots.keep | bool | `true` | Keep CRDs on chart uninstall |
| jaeger.annotations | object | `{}` | Annotations to be added to all CRDs |
| jaeger.enabled | bool | `true` | Install Jaeger CRDs |
| jaeger.keep | bool | `true` | Keep CRDs on chart uninstall |

