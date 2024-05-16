# mayastor

Mayastor Helm chart for Kubernetes

![Version: 0.0.0](https://img.shields.io/badge/Version-0.0.0-informational?style=flat-square) ![Type: application](https://img.shields.io/badge/Type-application-informational?style=flat-square) ![AppVersion: 0.0.0](https://img.shields.io/badge/AppVersion-0.0.0-informational?style=flat-square)

## Installation Guide

### Prerequisites

 - Make sure the [system requirement pre-requisites](https://mayastor.gitbook.io/introduction/quickstart/prerequisites) are met.
 - Label the storage nodes same as the mayastor.nodeSelector in values.yaml
 - Create the namespace you want the chart to be installed, or pass the `--create-namespace` flag in the `helm install` command.
   ```sh
   kubectl create ns <mayastor-namespace>
   ```
 - Create secret if downloading the container images from a private repo.
   ```sh
   kubectl create secret docker-registry <same-as-base.imagePullSecrets.secrets>  --docker-server="https://index.docker.io/v1/" --docker-username="<user-name>" --docker-password="<password>" --docker-email="<user-email>" -n <mayastor-namespace>
   ```

### Installing the chart via the git repo

Clone the mayastor charts repo.
Sync the chart dependencies
```console
$ helm dependency update
```
Install the mayastor chart using the command.
```console
$ helm install mayastor . -n <mayastor-namespace>
```

### Installing the Chart via Helm Registry

To install the chart with the release name `mymayastor`:

```console
$ helm repo add mayastor https://openebs.github.io/mayastor-extensions/
$ helm install mymayastor mayastor/mayastor
```

### Uninstall Helm Chart

```console
$ helm uninstall [RELEASE_NAME]
```

This removes all the Kubernetes components associated with the chart and deletes the release.

*See [helm uninstall](https://helm.sh/docs/helm/helm_uninstall/) for command documentation.*

## Chart Dependencies

| Repository | Name | Version |
|------------|------|---------|
|  | crds | 0.0.0 |
| https://charts.bitnami.com/bitnami | etcd | 8.6.0 |
| https://grafana.github.io/helm-charts | loki-stack | 2.9.11 |
| https://jaegertracing.github.io/helm-charts | jaeger-operator | 2.50.1 |
| https://nats-io.github.io/k8s/helm/charts/ | nats | 0.19.14 |
| https://openebs.github.io/dynamic-localpv-provisioner | localpv-provisioner | 4.0.0 |

## Values

| Key | Description | Default |
|:----|:------------|:--------|
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;capacity.&ZeroWidthSpace;thin.&ZeroWidthSpace;poolCommitment | The allowed pool commitment limit when dealing with thin provisioned volumes. Example: If the commitment is 250 and the pool is 10GiB we can overcommit the pool up to 25GiB (create 2 10GiB and 1 5GiB volume) but no further. | `"250%"` |
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;capacity.&ZeroWidthSpace;thin.&ZeroWidthSpace;snapshotCommitment | When creating snapshots for an existing volume, each replica pool must have at least this much free space percentage of the volume size. Example: if this value is 40, the pool has 40GiB free, then the max volume size allowed to be snapped on the pool is 100GiB. | `"40%"` |
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;capacity.&ZeroWidthSpace;thin.&ZeroWidthSpace;volumeCommitment | When creating replicas for an existing volume, each replica pool must have at least this much free space percentage of the volume size. Example: if this value is 40, the pool has 40GiB free, then the max volume size allowed to be created on the pool is 100GiB. | `"40%"` |
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;capacity.&ZeroWidthSpace;thin.&ZeroWidthSpace;volumeCommitmentInitial | Same as the `volumeCommitment` argument, but applicable only when creating replicas for a new volume. | `"40%"` |
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;logLevel | Log level for the core service | `"info"` |
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;priorityClassName | Set PriorityClass, overrides global. If both local and global are not set, the final deployment manifest has a mayastor custom critical priority class assigned to the pod by default. Refer the `templates/_helpers.tpl` and `templates/mayastor/agents/core/agent-core-deployment.yaml` for more details. | `""` |
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;rebuild.&ZeroWidthSpace;maxConcurrent | The maximum number of system-wide rebuilds permitted at any given time. If set to an empty string, there are no limits. | `""` |
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;rebuild.&ZeroWidthSpace;partial.&ZeroWidthSpace;enabled | Partial rebuild uses a log of missed IO to rebuild replicas which have become temporarily faulted, hence a bit faster, depending on the log size. | `true` |
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;rebuild.&ZeroWidthSpace;partial.&ZeroWidthSpace;waitPeriod | If a faulted replica comes back online within this time period then it will be rebuilt using the partial rebuild capability. Otherwise, the replica will be fully rebuilt. A blank value "" means internally derived value will be used. | `""` |
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;cpu | Cpu limits for core agents | `"1000m"` |
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;memory | Memory limits for core agents | `"128Mi"` |
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;cpu | Cpu requests for core agents | `"500m"` |
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;memory | Memory requests for core agents | `"32Mi"` |
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;tolerations | Set tolerations, overrides global | `[]` |
| agents.&ZeroWidthSpace;ha.&ZeroWidthSpace;cluster.&ZeroWidthSpace;logLevel | Log level for the ha cluster service | `"info"` |
| agents.&ZeroWidthSpace;ha.&ZeroWidthSpace;cluster.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;cpu | Cpu limits for ha cluster agent | `"100m"` |
| agents.&ZeroWidthSpace;ha.&ZeroWidthSpace;cluster.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;memory | Memory limits for ha cluster agent | `"64Mi"` |
| agents.&ZeroWidthSpace;ha.&ZeroWidthSpace;cluster.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;cpu | Cpu requests for ha cluster agent | `"100m"` |
| agents.&ZeroWidthSpace;ha.&ZeroWidthSpace;cluster.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;memory | Memory requests for ha cluster agent | `"16Mi"` |
| agents.&ZeroWidthSpace;ha.&ZeroWidthSpace;node.&ZeroWidthSpace;logLevel | Log level for the ha node service | `"info"` |
| agents.&ZeroWidthSpace;ha.&ZeroWidthSpace;node.&ZeroWidthSpace;priorityClassName | Set PriorityClass, overrides global | `""` |
| agents.&ZeroWidthSpace;ha.&ZeroWidthSpace;node.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;cpu | Cpu limits for ha node agent | `"100m"` |
| agents.&ZeroWidthSpace;ha.&ZeroWidthSpace;node.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;memory | Memory limits for ha node agent | `"64Mi"` |
| agents.&ZeroWidthSpace;ha.&ZeroWidthSpace;node.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;cpu | Cpu requests for ha node agent | `"100m"` |
| agents.&ZeroWidthSpace;ha.&ZeroWidthSpace;node.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;memory | Memory requests for ha node agent | `"64Mi"` |
| agents.&ZeroWidthSpace;ha.&ZeroWidthSpace;node.&ZeroWidthSpace;tolerations | Set tolerations, overrides global | `[]` |
| apis.&ZeroWidthSpace;rest.&ZeroWidthSpace;logLevel | Log level for the rest service | `"info"` |
| apis.&ZeroWidthSpace;rest.&ZeroWidthSpace;priorityClassName | Set PriorityClass, overrides global. If both local and global are not set, the final deployment manifest has a mayastor custom critical priority class assigned to the pod by default. Refer the `templates/_helpers.tpl` and `templates/mayastor/apis/rest/api-rest-deployment.yaml` for more details. | `""` |
| apis.&ZeroWidthSpace;rest.&ZeroWidthSpace;replicaCount | Number of replicas of rest | `1` |
| apis.&ZeroWidthSpace;rest.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;cpu | Cpu limits for rest | `"100m"` |
| apis.&ZeroWidthSpace;rest.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;memory | Memory limits for rest | `"64Mi"` |
| apis.&ZeroWidthSpace;rest.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;cpu | Cpu requests for rest | `"50m"` |
| apis.&ZeroWidthSpace;rest.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;memory | Memory requests for rest | `"32Mi"` |
| apis.&ZeroWidthSpace;rest.&ZeroWidthSpace;service.&ZeroWidthSpace;type | Rest K8s service type | `"ClusterIP"` |
| apis.&ZeroWidthSpace;rest.&ZeroWidthSpace;tolerations | Set tolerations, overrides global | `[]` |
| base.&ZeroWidthSpace;cache_poll_period | Cache timeout for core agent & diskpool deployment | `"30s"` |
| base.&ZeroWidthSpace;default_req_timeout | Request timeout for rest & core agents | `"5s"` |
| base.&ZeroWidthSpace;logging.&ZeroWidthSpace;color | Enable ansi color code for Pod StdOut/StdErr | `true` |
| base.&ZeroWidthSpace;logging.&ZeroWidthSpace;format | Valid values for format are pretty, json and compact | `"pretty"` |
| base.&ZeroWidthSpace;logging.&ZeroWidthSpace;silenceLevel | Silence specific module components | `nil` |
| base.&ZeroWidthSpace;metrics.&ZeroWidthSpace;enabled | Enable the metrics exporter | `true` |
| crds.&ZeroWidthSpace;csi.&ZeroWidthSpace;volumeSnapshots.&ZeroWidthSpace;enabled | Install Volume Snapshot CRDs | `true` |
| crds.&ZeroWidthSpace;enabled | Disables the installation of all CRDs if set to false | `true` |
| csi.&ZeroWidthSpace;controller.&ZeroWidthSpace;logLevel | Log level for the csi controller | `"info"` |
| csi.&ZeroWidthSpace;controller.&ZeroWidthSpace;preventVolumeModeConversion | Prevent modifying the volume mode when creating a PVC from an existing VolumeSnapshot | `true` |
| csi.&ZeroWidthSpace;controller.&ZeroWidthSpace;priorityClassName | Set PriorityClass, overrides global | `""` |
| csi.&ZeroWidthSpace;controller.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;cpu | Cpu limits for csi controller | `"32m"` |
| csi.&ZeroWidthSpace;controller.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;memory | Memory limits for csi controller | `"128Mi"` |
| csi.&ZeroWidthSpace;controller.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;cpu | Cpu requests for csi controller | `"16m"` |
| csi.&ZeroWidthSpace;controller.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;memory | Memory requests for csi controller | `"64Mi"` |
| csi.&ZeroWidthSpace;controller.&ZeroWidthSpace;tolerations | Set tolerations, overrides global | `[]` |
| csi.&ZeroWidthSpace;image.&ZeroWidthSpace;attacherTag | csi-attacher image release tag | `"v4.3.0"` |
| csi.&ZeroWidthSpace;image.&ZeroWidthSpace;provisionerTag | csi-provisioner image release tag | `"v3.5.0"` |
| csi.&ZeroWidthSpace;image.&ZeroWidthSpace;pullPolicy | imagePullPolicy for all CSI Sidecar images | `"IfNotPresent"` |
| csi.&ZeroWidthSpace;image.&ZeroWidthSpace;registrarTag | csi-node-driver-registrar image release tag | `"v2.10.0"` |
| csi.&ZeroWidthSpace;image.&ZeroWidthSpace;registry | Image registry to pull all CSI Sidecar images | `"registry.k8s.io"` |
| csi.&ZeroWidthSpace;image.&ZeroWidthSpace;repo | Image registry's namespace | `"sig-storage"` |
| csi.&ZeroWidthSpace;image.&ZeroWidthSpace;resizerTag | csi-resizer image release tag | `"v1.9.3"` |
| csi.&ZeroWidthSpace;image.&ZeroWidthSpace;snapshotControllerTag | csi-snapshot-controller image release tag | `"v6.3.3"` |
| csi.&ZeroWidthSpace;image.&ZeroWidthSpace;snapshotterTag | csi-snapshotter image release tag | `"v6.3.3"` |
| csi.&ZeroWidthSpace;node.&ZeroWidthSpace;kubeletDir | The kubeletDir directory for the csi-node plugin | `"/var/lib/kubelet"` |
| csi.&ZeroWidthSpace;node.&ZeroWidthSpace;nvme.&ZeroWidthSpace;ctrl_loss_tmo | The ctrl_loss_tmo (controller loss timeout) in seconds | `"1980"` |
| csi.&ZeroWidthSpace;node.&ZeroWidthSpace;priorityClassName | Set PriorityClass, overrides global | `""` |
| csi.&ZeroWidthSpace;node.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;cpu | Cpu limits for csi node plugin | `"100m"` |
| csi.&ZeroWidthSpace;node.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;memory | Memory limits for csi node plugin | `"128Mi"` |
| csi.&ZeroWidthSpace;node.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;cpu | Cpu requests for csi node plugin | `"100m"` |
| csi.&ZeroWidthSpace;node.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;memory | Memory requests for csi node plugin | `"64Mi"` |
| csi.&ZeroWidthSpace;node.&ZeroWidthSpace;tolerations | Set tolerations, overrides global | `[]` |
| csi.&ZeroWidthSpace;node.&ZeroWidthSpace;topology.&ZeroWidthSpace;nodeSelector | Add topology segments to the csi-node and agent-ha-node daemonset node selector | `false` |
| etcd.&ZeroWidthSpace;autoCompactionMode | AutoCompaction Since etcd keeps an exact history of its keyspace, this history should be periodically compacted to avoid performance degradation and eventual storage space exhaustion. Auto compaction mode. Valid values: "periodic", "revision". - 'periodic' for duration based retention, defaulting to hours if no time unit is provided (e.g. 5m). - 'revision' for revision number based retention. | `"revision"` |
| etcd.&ZeroWidthSpace;autoCompactionRetention | Auto compaction retention length. 0 means disable auto compaction. | `100` |
| etcd.&ZeroWidthSpace;extraEnvVars[0] | Raise alarms when backend size exceeds the given quota. | <pre>{<br>"name":"ETCD_QUOTA_BACKEND_BYTES",<br>"value":"8589934592"<br>}</pre> |
| etcd.&ZeroWidthSpace;localpvScConfig.&ZeroWidthSpace;basePath | Host path where local etcd data is stored in. | `"/var/local/{{ .Release.Name }}/localpv-hostpath/etcd"` |
| etcd.&ZeroWidthSpace;localpvScConfig.&ZeroWidthSpace;reclaimPolicy | ReclaimPolicy of etcd's localpv hostpath storage class. | `"Delete"` |
| etcd.&ZeroWidthSpace;localpvScConfig.&ZeroWidthSpace;volumeBindingMode | VolumeBindingMode of etcd's localpv hostpath storage class. | `"WaitForFirstConsumer"` |
| etcd.&ZeroWidthSpace;persistence.&ZeroWidthSpace;enabled | If true, use a Persistent Volume Claim. If false, use emptyDir. | `true` |
| etcd.&ZeroWidthSpace;persistence.&ZeroWidthSpace;reclaimPolicy | PVC's reclaimPolicy | `"Delete"` |
| etcd.&ZeroWidthSpace;persistence.&ZeroWidthSpace;size | Volume size | `"2Gi"` |
| etcd.&ZeroWidthSpace;persistence.&ZeroWidthSpace;storageClass | Will define which storageClass to use in etcd's StatefulSets. Options: <p> - `"manual"` - Will provision a hostpath PV on the same node. <br> - `""` (empty) - Will use the default StorageClass on the cluster. </p> | `"mayastor-etcd-localpv"` |
| etcd.&ZeroWidthSpace;podAntiAffinityPreset | Pod anti-affinity preset Ref: https://kubernetes.io/docs/concepts/scheduling-eviction/assign-pod-node/#inter-pod-affinity-and-anti-affinity | `"hard"` |
| etcd.&ZeroWidthSpace;removeMemberOnContainerTermination | Use a PreStop hook to remove the etcd members from the etcd cluster on container termination Ignored if lifecycleHooks is set or replicaCount=1 | `false` |
| etcd.&ZeroWidthSpace;replicaCount | Number of replicas of etcd | `3` |
| image.&ZeroWidthSpace;pullPolicy | ImagePullPolicy for our images | `"Always"` |
| image.&ZeroWidthSpace;pullSecrets.&ZeroWidthSpace;enabled | Enable pullSecrets for pulling our container images | `false` |
| image.&ZeroWidthSpace;pullSecrets.&ZeroWidthSpace;secrets | Name of the pullSecret in the installed namespace | `[{"name":"datacore-image-registry-credentials"}]` |
| image.&ZeroWidthSpace;registry | Image registry to pull our product images | `"docker.io"` |
| image.&ZeroWidthSpace;repo | Image registry's namespace | `"openebs"` |
| image.&ZeroWidthSpace;tag | Release tag for our images | `"develop"` |
| io_engine.&ZeroWidthSpace;coreList | If not empty, overrides the cpuCount and explicitly sets the list of cores. Example: --set='io_engine.coreList={30,31}' | `[]` |
| io_engine.&ZeroWidthSpace;cpuCount | The number of cores that each io-engine instance will bind to. | `"2"` |
| io_engine.&ZeroWidthSpace;envcontext | Pass additional arguments to the Environment Abstraction Layer. Example: --set {product}.envcontext=iova-mode=pa | `""` |
| io_engine.&ZeroWidthSpace;logLevel | Log level for the io-engine service | `"info"` |
| io_engine.&ZeroWidthSpace;nodeSelector | Node selectors to designate storage nodes for diskpool creation Note that if multi-arch images support 'kubernetes.io/arch: amd64' should be removed. | <pre>{<br>"kubernetes.io/arch":"amd64",<br>"openebs.io/engine":"mayastor"<br>}</pre> |
| io_engine.&ZeroWidthSpace;nvme.&ZeroWidthSpace;ioTimeout | Timeout for IOs The default here is exaggerated for local disks, but we've observed that in shared virtual environments having a higher timeout value is beneficial. Please adjust this according to your hardware and needs. | `"110s"` |
| io_engine.&ZeroWidthSpace;nvme.&ZeroWidthSpace;tcp.&ZeroWidthSpace;maxQueueDepth | You may need to increase this for a higher outstanding IOs per volume | `"32"` |
| io_engine.&ZeroWidthSpace;priorityClassName | Set PriorityClass, overrides global | `""` |
| io_engine.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;cpu | Cpu limits for the io-engine | `""` |
| io_engine.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;hugepages2Mi | Hugepage size available on the nodes | `"2Gi"` |
| io_engine.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;memory | Memory limits for the io-engine | `"1Gi"` |
| io_engine.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;cpu | Cpu requests for the io-engine | `""` |
| io_engine.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;hugepages2Mi | Hugepage size available on the nodes | `"2Gi"` |
| io_engine.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;memory | Memory requests for the io-engine | `"1Gi"` |
| io_engine.&ZeroWidthSpace;target.&ZeroWidthSpace;nvmf.&ZeroWidthSpace;iface | NVMF target interface (ip, mac, name or subnet) | `""` |
| io_engine.&ZeroWidthSpace;target.&ZeroWidthSpace;nvmf.&ZeroWidthSpace;ptpl | Reservations Persist Through Power Loss State | `true` |
| io_engine.&ZeroWidthSpace;tolerations | Set tolerations, overrides global | `[]` |
| localpv-provisioner.&ZeroWidthSpace;enabled | Enables the openebs dynamic-localpv-provisioner. If disabled, modify etcd and loki-stack storage class accordingly. | `true` |
| localpv-provisioner.&ZeroWidthSpace;hostpathClass.&ZeroWidthSpace;enabled | Enable default hostpath localpv StorageClass. | `false` |
| loki-stack.&ZeroWidthSpace;enabled | Enable loki log collection for our components | `true` |
| loki-stack.&ZeroWidthSpace;localpvScConfig.&ZeroWidthSpace;basePath | Host path where local etcd data is stored in. | `"/var/local/{{ .Release.Name }}/localpv-hostpath/loki"` |
| loki-stack.&ZeroWidthSpace;localpvScConfig.&ZeroWidthSpace;reclaimPolicy | ReclaimPolicy of loki's localpv hostpath storage class. | `"Delete"` |
| loki-stack.&ZeroWidthSpace;localpvScConfig.&ZeroWidthSpace;volumeBindingMode | VolumeBindingMode of loki's localpv hostpath storage class. | `"WaitForFirstConsumer"` |
| loki-stack.&ZeroWidthSpace;loki.&ZeroWidthSpace;enabled | Enable loki installation as part of loki-stack | `true` |
| loki-stack.&ZeroWidthSpace;loki.&ZeroWidthSpace;persistence.&ZeroWidthSpace;enabled | Enable persistence storage for the logs | `true` |
| loki-stack.&ZeroWidthSpace;loki.&ZeroWidthSpace;persistence.&ZeroWidthSpace;reclaimPolicy | PVC's ReclaimPolicy, can be Delete or Retain | `"Delete"` |
| loki-stack.&ZeroWidthSpace;loki.&ZeroWidthSpace;persistence.&ZeroWidthSpace;size | Size of Loki's persistence storage | `"10Gi"` |
| loki-stack.&ZeroWidthSpace;loki.&ZeroWidthSpace;persistence.&ZeroWidthSpace;storageClassName | StorageClass for Loki's centralised log storage. Options: <p> - `"manual"` - Will provision a hostpath PV on the same node. <br> - `""` (empty) - Will use the default StorageClass on the cluster. </p> | `"mayastor-loki-localpv"` |
| loki-stack.&ZeroWidthSpace;loki.&ZeroWidthSpace;rbac.&ZeroWidthSpace;create | Create rbac roles for loki | `true` |
| loki-stack.&ZeroWidthSpace;promtail.&ZeroWidthSpace;config.&ZeroWidthSpace;clients[0] | The Loki address to post logs to | <pre>{<br>"url":"http://{<br>{<br> .Release.Name <br>}<br>}-loki:3100/loki/api/v1/push"<br>}</pre> |
| loki-stack.&ZeroWidthSpace;promtail.&ZeroWidthSpace;enabled | Enables promtail for scraping logs from nodes | `true` |
| loki-stack.&ZeroWidthSpace;promtail.&ZeroWidthSpace;tolerations | Disallow promtail from running on the master node | `[]` |
| nodeSelector | Node labels for pod assignment ref: https://kubernetes.io/docs/concepts/configuration/assign-pod-node/ Note that if multi-arch images support 'kubernetes.io/arch: amd64' should be removed and set 'nodeSelector' to empty '{}' as default value. | <pre>{<br>"kubernetes.io/arch":"amd64"<br>}</pre> |
| obs.&ZeroWidthSpace;callhome.&ZeroWidthSpace;enabled | Enable callhome | `true` |
| obs.&ZeroWidthSpace;callhome.&ZeroWidthSpace;logLevel | Log level for callhome | `"info"` |
| obs.&ZeroWidthSpace;callhome.&ZeroWidthSpace;priorityClassName | Set PriorityClass, overrides global | `""` |
| obs.&ZeroWidthSpace;callhome.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;cpu | Cpu limits for callhome | `"100m"` |
| obs.&ZeroWidthSpace;callhome.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;memory | Memory limits for callhome | `"32Mi"` |
| obs.&ZeroWidthSpace;callhome.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;cpu | Cpu requests for callhome | `"50m"` |
| obs.&ZeroWidthSpace;callhome.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;memory | Memory requests for callhome | `"16Mi"` |
| obs.&ZeroWidthSpace;callhome.&ZeroWidthSpace;tolerations | Set tolerations, overrides global | `[]` |
| obs.&ZeroWidthSpace;stats.&ZeroWidthSpace;logLevel | Log level for stats | `"info"` |
| obs.&ZeroWidthSpace;stats.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;cpu | Cpu limits for stats | `"100m"` |
| obs.&ZeroWidthSpace;stats.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;memory | Memory limits for stats | `"32Mi"` |
| obs.&ZeroWidthSpace;stats.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;cpu | Cpu requests for stats | `"50m"` |
| obs.&ZeroWidthSpace;stats.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;memory | Memory requests for stats | `"16Mi"` |
| obs.&ZeroWidthSpace;stats.&ZeroWidthSpace;service.&ZeroWidthSpace;type | Rest K8s service type | `"ClusterIP"` |
| operators.&ZeroWidthSpace;pool.&ZeroWidthSpace;logLevel | Log level for diskpool operator service | `"info"` |
| operators.&ZeroWidthSpace;pool.&ZeroWidthSpace;priorityClassName | Set PriorityClass, overrides global | `""` |
| operators.&ZeroWidthSpace;pool.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;cpu | Cpu limits for diskpool operator | `"100m"` |
| operators.&ZeroWidthSpace;pool.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;memory | Memory limits for diskpool operator | `"32Mi"` |
| operators.&ZeroWidthSpace;pool.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;cpu | Cpu requests for diskpool operator | `"50m"` |
| operators.&ZeroWidthSpace;pool.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;memory | Memory requests for diskpool operator | `"16Mi"` |
| operators.&ZeroWidthSpace;pool.&ZeroWidthSpace;tolerations | Set tolerations, overrides global | `[]` |
| priorityClassName | Pod scheduling priority. Setting this value will apply to all components except the external Chart dependencies. If any component has `priorityClassName` set, then this value would be overridden for that component. For external components like etcd, jaeger or loki-stack, PriorityClass can only be set at component level. | `""` |
| storageClass.&ZeroWidthSpace;allowVolumeExpansion | Enable volume expansion for the default StorageClass. | `true` |
| tolerations | Tolerations to be applied to all components except external Chart dependencies. If any component has tolerations set, then it would override this value. For external components like etcd, jaeger and loki-stack, tolerations can only be set at component level. | `[]` |

