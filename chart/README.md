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

To install the chart with the release name `my-release`:

```console
$ helm repo add openebs https://openebs.github.io/mayastor-extensions/
$ helm install my-release openebs/mayastor
```

## Chart Dependencies

| Repository | Name | Version |
|------------|------|---------|
| https://charts.bitnami.com/bitnami | etcd | 8.6.0 |
| https://grafana.github.io/helm-charts | loki-stack | 2.6.4 |
| https://jaegertracing.github.io/helm-charts | jaeger-operator | 2.25.0 |
| https://nats-io.github.io/k8s/helm/charts/ | nats | 0.19.14 |

## Values

| Key | Description | Default |
|-----|-------------|:-------:|
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;capacity.&ZeroWidthSpace;thin.&ZeroWidthSpace;poolCommitment | The allowed pool commitment limit when dealing with thin provisioned volumes. Example: If the commitment is 250 and the pool is 10GiB we can overcommit the pool up to 25GiB (create 2 10GiB and 1 5GiB volume) but no further. | `"250%"` |
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;capacity.&ZeroWidthSpace;thin.&ZeroWidthSpace;volumeCommitment | When creating replicas for an existing volume, each replica pool must have at least this much free space percentage of the volume size. Example: if this value is 40, the pool has 40GiB free, then the max volume size allowed to be created on the pool is 100GiB. | `"40%"` |
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;capacity.&ZeroWidthSpace;thin.&ZeroWidthSpace;volumeCommitmentInitial | Same as the `volumeCommitment` argument, but applicable only when creating replicas for a new volume. | `"40%"` |
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;logLevel | Log level for the core service | `"info"` |
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;partialRebuildWaitPeriod | If a faulted replica comes back online within this time period then it will be rebuilt using the partial rebuild capability (using a log of missed IO), hence a bit faster depending on the log size. Otherwise, the replica will be fully rebuilt. A blank value "" means internally derived value will be used. | `""` |
| agents.&ZeroWidthSpace;core.&ZeroWidthSpace;priorityClassName | Set PriorityClass, overrides global. If both local and global are not set, the final deployment manifest has a mayastor custom critical priority class assigned to the pod by default. Refer the `templates/_helpers.tpl` and `templates/mayastor/agents/core/agent-core-deployment.yaml` for more details. | `""` |
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
| base.&ZeroWidthSpace;imagePullSecrets.&ZeroWidthSpace;enabled | Enable imagePullSecrets for pulling our container images | `false` |
| base.&ZeroWidthSpace;jaeger.&ZeroWidthSpace;enabled | Enable jaeger tracing | `false` |
| base.&ZeroWidthSpace;logSilenceLevel | Silence specific module components | `nil` |
| base.&ZeroWidthSpace;metrics.&ZeroWidthSpace;enabled | Enable the metrics exporter | `true` |
| csi.&ZeroWidthSpace;controller.&ZeroWidthSpace;logLevel | Log level for the csi controller | `"info"` |
| csi.&ZeroWidthSpace;controller.&ZeroWidthSpace;priorityClassName | Set PriorityClass, overrides global | `""` |
| csi.&ZeroWidthSpace;controller.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;cpu | Cpu limits for csi controller | `"32m"` |
| csi.&ZeroWidthSpace;controller.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;memory | Memory limits for csi controller | `"128Mi"` |
| csi.&ZeroWidthSpace;controller.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;cpu | Cpu requests for csi controller | `"16m"` |
| csi.&ZeroWidthSpace;controller.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;memory | Memory requests for csi controller | `"64Mi"` |
| csi.&ZeroWidthSpace;controller.&ZeroWidthSpace;tolerations | Set tolerations, overrides global | `[]` |
| csi.&ZeroWidthSpace;image.&ZeroWidthSpace;attacherTag | csi-attacher image release tag | `"v3.2.1"` |
| csi.&ZeroWidthSpace;image.&ZeroWidthSpace;provisionerTag | csi-provisioner image release tag | `"v2.2.1"` |
| csi.&ZeroWidthSpace;image.&ZeroWidthSpace;pullPolicy | imagePullPolicy for all CSI Sidecar images | `"IfNotPresent"` |
| csi.&ZeroWidthSpace;image.&ZeroWidthSpace;registrarTag | csi-node-driver-registrar image release tag | `"v2.1.0"` |
| csi.&ZeroWidthSpace;image.&ZeroWidthSpace;registry | Image registry to pull all CSI Sidecar images | `"registry.k8s.io"` |
| csi.&ZeroWidthSpace;image.&ZeroWidthSpace;repo | Image registry's namespace | `"sig-storage"` |
| csi.&ZeroWidthSpace;image.&ZeroWidthSpace;snapshotControllerTag | csi-snapshot-controller image release tag | `"v6.2.1"` |
| csi.&ZeroWidthSpace;image.&ZeroWidthSpace;snapshotterTag | csi-snapshotter image release tag | `"v6.2.1"` |
| csi.&ZeroWidthSpace;node.&ZeroWidthSpace;kubeletDir | The kubeletDir directory for the csi-node plugin | `"/var/lib/kubelet"` |
| csi.&ZeroWidthSpace;node.&ZeroWidthSpace;nvme.&ZeroWidthSpace;ctrl_loss_tmo | The ctrl_loss_tmo (controller loss timeout) in seconds | `"1980"` |
| csi.&ZeroWidthSpace;node.&ZeroWidthSpace;nvme.&ZeroWidthSpace;io_timeout | The nvme_core module io timeout in seconds | `"30"` |
| csi.&ZeroWidthSpace;node.&ZeroWidthSpace;priorityClassName | Set PriorityClass, overrides global | `""` |
| csi.&ZeroWidthSpace;node.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;cpu | Cpu limits for csi node plugin | `"100m"` |
| csi.&ZeroWidthSpace;node.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;memory | Memory limits for csi node plugin | `"128Mi"` |
| csi.&ZeroWidthSpace;node.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;cpu | Cpu requests for csi node plugin | `"100m"` |
| csi.&ZeroWidthSpace;node.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;memory | Memory requests for csi node plugin | `"64Mi"` |
| csi.&ZeroWidthSpace;node.&ZeroWidthSpace;tolerations | Set tolerations, overrides global | `[]` |
| csi.&ZeroWidthSpace;node.&ZeroWidthSpace;topology.&ZeroWidthSpace;nodeSelector | Add topology segments to the csi-node daemonset node selector | `false` |
| etcd.&ZeroWidthSpace;autoCompactionMode | AutoCompaction Since etcd keeps an exact history of its keyspace, this history should be periodically compacted to avoid performance degradation and eventual storage space exhaustion. Auto compaction mode. Valid values: "periodic", "revision". - 'periodic' for duration based retention, defaulting to hours if no time unit is provided (e.g. 5m). - 'revision' for revision number based retention. | `"revision"` |
| etcd.&ZeroWidthSpace;autoCompactionRetention | Auto compaction retention length. 0 means disable auto compaction. | `100` |
| etcd.&ZeroWidthSpace;extraEnvVars[0] | Raise alarms when backend size exceeds the given quota. | <pre>{<br>"name":"ETCD_QUOTA_BACKEND_BYTES",<br>"value":"8589934592"<br>}</pre> |
| etcd.&ZeroWidthSpace;persistence.&ZeroWidthSpace;enabled | If true, use a Persistent Volume Claim. If false, use emptyDir. | `true` |
| etcd.&ZeroWidthSpace;persistence.&ZeroWidthSpace;reclaimPolicy | PVC's reclaimPolicy | `"Delete"` |
| etcd.&ZeroWidthSpace;persistence.&ZeroWidthSpace;size | Volume size | `"2Gi"` |
| etcd.&ZeroWidthSpace;persistence.&ZeroWidthSpace;storageClass | Will define which storageClass to use in etcd's StatefulSets a `manual` storageClass will provision a hostpath PV on the same node an empty storageClass will use the default StorageClass on the cluster | `""` |
| etcd.&ZeroWidthSpace;podAntiAffinityPreset | Pod anti-affinity preset Ref: https://kubernetes.io/docs/concepts/scheduling-eviction/assign-pod-node/#inter-pod-affinity-and-anti-affinity | `"hard"` |
| etcd.&ZeroWidthSpace;removeMemberOnContainerTermination | Use a PreStop hook to remove the etcd members from the etcd cluster on container termination Ignored if lifecycleHooks is set or replicaCount=1 | `false` |
| etcd.&ZeroWidthSpace;replicaCount | Number of replicas of etcd | `3` |
| image.&ZeroWidthSpace;pullPolicy | ImagePullPolicy for our images | `"Always"` |
| image.&ZeroWidthSpace;registry | Image registry to pull our product images | `"docker.io"` |
| image.&ZeroWidthSpace;repo | Image registry's namespace | `"openebs"` |
| image.&ZeroWidthSpace;tag | Release tag for our images | `"develop"` |
| io_engine.&ZeroWidthSpace;coreList | If not empty, overrides the cpuCount and explicitly sets the list of cores. Example: --set='io_engine.coreList={30,31}' | `[]` |
| io_engine.&ZeroWidthSpace;cpuCount | The number of cores that each io-engine instance will bind to. | `"2"` |
| io_engine.&ZeroWidthSpace;envcontext | Pass additional arguments to the Environment Abstraction Layer. Example: --set {product}.envcontext=iova-mode=pa | `""` |
| io_engine.&ZeroWidthSpace;logLevel | Log level for the io-engine service | `"info"` |
| io_engine.&ZeroWidthSpace;nodeSelector | Node selectors to designate storage nodes for diskpool creation Note that if multi-arch images support 'kubernetes.io/arch: amd64' should be removed. | <pre>{<br>"kubernetes.io/arch":"amd64",<br>"openebs.io/engine":"mayastor"<br>}</pre> |
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
| loki-stack.&ZeroWidthSpace;enabled | Enable loki log collection for our components | `true` |
| loki-stack.&ZeroWidthSpace;loki.&ZeroWidthSpace;enabled | Enable loki installation as part of loki-stack | `true` |
| loki-stack.&ZeroWidthSpace;loki.&ZeroWidthSpace;persistence.&ZeroWidthSpace;enabled | Enable persistence storage for the logs | `true` |
| loki-stack.&ZeroWidthSpace;loki.&ZeroWidthSpace;persistence.&ZeroWidthSpace;reclaimPolicy | PVC's ReclaimPolicy, can be Delete or Retain | `"Delete"` |
| loki-stack.&ZeroWidthSpace;loki.&ZeroWidthSpace;persistence.&ZeroWidthSpace;size | Size of Loki's persistence storage | `"10Gi"` |
| loki-stack.&ZeroWidthSpace;loki.&ZeroWidthSpace;persistence.&ZeroWidthSpace;storageClassName | StorageClass for Loki's centralised log storage empty storageClass implies cluster default storageClass & `manual` creates a static hostpath PV | `""` |
| loki-stack.&ZeroWidthSpace;loki.&ZeroWidthSpace;rbac.&ZeroWidthSpace;create | Create rbac roles for loki | `true` |
| loki-stack.&ZeroWidthSpace;promtail.&ZeroWidthSpace;config.&ZeroWidthSpace;lokiAddress | The Loki address to post logs to | `"http://{{ .Release.Name }}-loki:3100/loki/api/v1/push"` |
| loki-stack.&ZeroWidthSpace;promtail.&ZeroWidthSpace;enabled | Enables promtail for scraping logs from nodes | `true` |
| loki-stack.&ZeroWidthSpace;promtail.&ZeroWidthSpace;tolerations | Disallow promtail from running on the master node | `[]` |
| nats.&ZeroWidthSpace;enabled | Enable nats for transmitting events to subscribers | `true` |
| nodeSelector | Node labels for pod assignment ref: https://kubernetes.io/docs/concepts/configuration/assign-pod-node/ Note that if multi-arch images support 'kubernetes.io/arch: amd64' should be removed and set 'nodeSelector' to empty '{}' as default value. | <pre>{<br>"kubernetes.io/arch":"amd64"<br>}</pre> |
| obs.&ZeroWidthSpace;callhome.&ZeroWidthSpace;enabled | Enable callhome | `true` |
| obs.&ZeroWidthSpace;callhome.&ZeroWidthSpace;logLevel | Log level for callhome | `"info"` |
| obs.&ZeroWidthSpace;callhome.&ZeroWidthSpace;priorityClassName | Set PriorityClass, overrides global | `""` |
| obs.&ZeroWidthSpace;callhome.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;cpu | Cpu limits for callhome | `"100m"` |
| obs.&ZeroWidthSpace;callhome.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;memory | Memory limits for callhome | `"32Mi"` |
| obs.&ZeroWidthSpace;callhome.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;cpu | Cpu requests for callhome | `"50m"` |
| obs.&ZeroWidthSpace;callhome.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;memory | Memory requests for callhome | `"16Mi"` |
| obs.&ZeroWidthSpace;callhome.&ZeroWidthSpace;tolerations | Set tolerations, overrides global | `[]` |
| operators.&ZeroWidthSpace;pool.&ZeroWidthSpace;logLevel | Log level for diskpool operator service | `"info"` |
| operators.&ZeroWidthSpace;pool.&ZeroWidthSpace;priorityClassName | Set PriorityClass, overrides global | `""` |
| operators.&ZeroWidthSpace;pool.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;cpu | Cpu limits for diskpool operator | `"100m"` |
| operators.&ZeroWidthSpace;pool.&ZeroWidthSpace;resources.&ZeroWidthSpace;limits.&ZeroWidthSpace;memory | Memory limits for diskpool operator | `"32Mi"` |
| operators.&ZeroWidthSpace;pool.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;cpu | Cpu requests for diskpool operator | `"50m"` |
| operators.&ZeroWidthSpace;pool.&ZeroWidthSpace;resources.&ZeroWidthSpace;requests.&ZeroWidthSpace;memory | Memory requests for diskpool operator | `"16Mi"` |
| operators.&ZeroWidthSpace;pool.&ZeroWidthSpace;tolerations | Set tolerations, overrides global | `[]` |
| priorityClassName | Pod scheduling priority. Setting this value will apply to all components except the external Chart dependencies. If any component has `priorityClassName` set, then this value would be overridden for that component. For external components like etcd, jaeger or loki-stack, PriorityClass can only be set at component level. | `""` |
| tolerations | Tolerations to be applied to all components except external Chart dependencies. If any component has tolerations set, then it would override this value. For external components like etcd, jaeger and loki-stack, tolerations can only be set at component level. | `[]` |

