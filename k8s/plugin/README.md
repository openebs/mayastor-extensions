# Kubectl Plugin

## Overview
The kubectl plugin has been created in accordance with the instructions outlined in the [official documentation](https://kubernetes.io/docs/tasks/extend-kubectl/kubectl-plugins/).


The name of the plugin binary dictates how it is used. From the documentation:
> For example, a plugin named `kubectl-foo` provides a command `kubectl foo`.

In our case the name of the binary is specified in the Cargo.toml file as `kubectl-mayastor`, therefore the command is `kubectl mayastor`.

## Usage
**The plugin must be placed in your `PATH` in order for it to be used.**

To make the plugin as intuitive as possible, every attempt has been made to make the usage as similar to that of the standard `kubectl` command line utility as possible.

The general command structure is `kubectl mayastor <operation> <resource>` where the operation defines what should be performed (i.e. `get`, `scale`) and the resource defines what the operation should be performed on (i.e. `volumes`, `pools`).

Here is the top-level help which includes global options:
```
The types of operations that are supported

Usage: kubectl-mayastor [OPTIONS] <COMMAND>

Commands:
  drain      'Drain' resources
  get        'Get' resources
  scale      'Scale' resources
  cordon     'Cordon' resources
  uncordon   'Uncordon' resources
  dump       'Dump' resources
  upgrade    'Upgrade' the deployment
  delete     'Delete' the upgrade resources
  help        Print this message or the help of the given subcommand(s)

Options:
  -r, --rest <REST>
          The rest endpoint to connect to
  -k, --kube-config-path <KUBE_CONFIG_PATH>
          Path to kubeconfig file
  -o, --output <OUTPUT>
          The Output, viz yaml, json [default: none]
  -j, --jaeger <JAEGER>
          Trace rest requests to the Jaeger endpoint agent
  -t, --timeout <TIMEOUT>
          Timeout for the REST operations [default: 10s]
  -n, --namespace <NAMESPACE>
          Kubernetes namespace of mayastor service
  -h, --help
          Print help
  -V, --version
          Print version
```

### Examples and Outputs


<details>
<summary> General Resources operations </summary>

1. Get Volumes
```
❯ kubectl mayastor get volumes
 ID                                    REPLICAS TARGET-NODE  ACCESSIBILITY STATUS  SIZE     THIN-PROVISIONED ALLOCATED
 18e30e83-b106-4e0d-9fb6-2b04e761e18a  4        kworker1     nvmf          Online  1GiB     true             8MiB
 0c08667c-8b59-4d11-9192-b54e27e0ce0f  4        kworker2     <none>        Online  381.6MiB false            384MiB

```
2. Get Volume by ID
```
❯ kubectl mayastor get volume 18e30e83-b106-4e0d-9fb6-2b04e761e18a
 ID                                    REPLICAS TARGET-NODE  ACCESSIBILITY STATUS  SIZE     THIN-PROVISIONED ALLOCATED
 18e30e83-b106-4e0d-9fb6-2b04e761e18a  4        kworker1     nvmf          Online  1GiB     true             8MiB

```
3. Get Pools
```
❯ kubectl mayastor get pools
 ID               DISKS                                                     MANAGED  NODE      STATUS  CAPACITY  ALLOCATED  AVAILABLE
 pool-1-kworker1  aio:///dev/vdb?uuid=d8a36b4b-0435-4fee-bf76-f2aef980b833  true     kworker1  Online  500GiB    100GiB     400GiB
 pool-1-kworker2  aio:///dev/vdc?uuid=bb12ec7d-8fc3-4644-82cd-dee5b63fc8c5  true     kworker1  Online  500GiB    100GiB     400GiB
 pool-1-kworker2  aio:///dev/vdb?uuid=f324edb7-1aca-41ec-954a-9614527f77e1  true     kworker2  Online  500GiB    100GiB     400GiB
```
4. Get Pool by ID
```
❯ kubectl mayastor get pool pool-1-kworker1
 ID               DISKS                                                     MANAGED  NODE      STATUS  CAPACITY  ALLOCATED  AVAILABLE
 pool-1-kworker1  aio:///dev/vdb?uuid=d8a36b4b-0435-4fee-bf76-f2aef980b833  true     kworker1  Online  500GiB    100GiB     400GiB
```
5. Get Nodes
```
❯ kubectl mayastor get nodes
 ID          GRPC ENDPOINT   STATUS   CORDONED
 kworker1    10.1.0.7:10124  Online   false
 kworker2    10.1.0.6:10124  Online   false
 kworker3    10.1.0.8:10124  Online   false
```
6. Get Node by ID
```
❯ kubectl mayastor get node mayastor-2
 ID          GRPC ENDPOINT   STATUS  CORDONED
 mayastor-2  10.1.0.7:10124  Online  false
```

7. Get Volume(s)/Pool(s)/Node(s) to a specific Output Format
```
❯ kubectl mayastor -ojson get volumes
[{"spec":{"num_replicas":2,"size":67108864,"status":"Created","target":{"node":"ksnode-2","protocol":"nvmf"},"uuid":"5703e66a-e5e5-4c84-9dbe-e5a9a5c805db","topology":{"explicit":{"allowed_nodes":["ksnode-1","ksnode-3","ksnode-2"],"preferred_nodes":["ksnode-2","ksnode-3","ksnode-1"]}},"policy":{"self_heal":true}},"state":{"target":{"children":[{"state":"Online","uri":"bdev:///ac02cf9e-8f25-45f0-ab51-d2e80bd462f1?uuid=ac02cf9e-8f25-45f0-ab51-d2e80bd462f1"},{"state":"Online","uri":"nvmf://192.168.122.6:8420/nqn.2019-05.io.openebs:7b0519cb-8864-4017-85b6-edd45f6172d8?uuid=7b0519cb-8864-4017-85b6-edd45f6172d8"}],"deviceUri":"nvmf://192.168.122.234:8420/nqn.2019-05.io.openebs:nexus-140a1eb1-62b5-43c1-acef-9cc9ebb29425","node":"ksnode-2","rebuilds":0,"protocol":"nvmf","size":67108864,"state":"Online","uuid":"140a1eb1-62b5-43c1-acef-9cc9ebb29425"},"size":67108864,"status":"Online","uuid":"5703e66a-e5e5-4c84-9dbe-e5a9a5c805db"}}]

```

```
❯ kubectl mayastor -oyaml get pools
---
- id: pool-1-kworker1
  state:
    capacity: 5360320512
    disks:
      - "aio:///dev/vdb?uuid=d8a36b4b-0435-4fee-bf76-f2aef980b833"
    id: pool-1-kworker1
    node: kworker1
    status: Online
    used: 1111490560
- id: pool-1-kworker2
  state:
    capacity: 5360320512
    disks:
      - "aio:///dev/vdc?uuid=bb12ec7d-8fc3-4644-82cd-dee5b63fc8c5"
    id: pool-1-kworker-2
    node: kworker1
    status: Online
    used: 2185232384
- id: pool-1-kworker3
  state:
    capacity: 5360320512
    disks:
      - "aio:///dev/vdb?uuid=f324edb7-1aca-41ec-954a-9614527f77e1"
    id: pool-1-kworker-3
    node: kworker2
    status: Online
    used: 3258974208
```
8. Replica topology for a specific volume
```
❯ kubectl mayastor get volume-replica-topology ec4e66fd-3b33-4439-b504-d49aba53da26
 ID                                    NODE      POOL             STATUS  CAPACITY  ALLOCATED SNAPSHOTS  CHILD-STATUS  REASON  REBUILD
 b32769b8-e5b3-4e1c-9db0-89867470f6eb  kworker1  pool-1-kworker1  Online  384MiB    8MiB      12MiB      Degraded      <none>  75 %
 d3856829-22b3-414d-a01b-4b6467db14fb  kworker2  pool-1-kworker2  Online  384MiB    8MiB      64MiB      Online        <none>  <none>
```

9. Replica topology for all volumes
```
❯ kubectl mayastor get volume-replica-topologies
VOLUME-ID                              ID                                    NODE      POOL             STATUS  CAPACITY  ALLOCATED SNAPSHOTS CHILD-STATUS  REASON      REBUILD
 c05ef923-a320-468c-b426-a260c1d84107  b58e1975-633f-4b34-9611-b648babf76a8  kworker1  pool-1-kworker1  Online  60MiB     36MiB     0MiB      Degraded      OutOfSpace  <none>
 ├─                                    67a6ec31-5923-490f-84b7-0be1df3bfb53  kworker2  pool-1-kworker2  Online  60MiB     60MiB     0MiB      Online        <none>      <none>
 └─                                    553aeb7c-4be4-4391-a403-ad241d96711f  kworker3  pool-1-kworker3  Online  60MiB     60MiB     0MiB      Online        <none>      <none>
 83241cc8-5dca-4bf1-b55a-c427c3e9b4a1  adde358f-70cd-4a2d-9dfb-f40d6663ecbc  kworker1  pool-1-kworker1  Online  20MiB     16MiB     0MiB      Degraded      <none>      51%
 ├─                                    b5ff41b8-1a0a-4bc7-84bb-5bfdfe72a71e  kworker2  pool-1-kworker2  Online  60MiB     60MiB     0MiB      Online        <none>      <none>
 └─                                    39431c11-0eea-48e7-970f-a2359ebbb9d1  kworker3  pool-1-kworker3  Online  60MiB     60MiB     0MiB      Online        <none>      <none>
```

10. Volume Snapshots by volumeID
```
❯ kubectl mayastor get volume-snapshots --volume dc4e66fd-3b33-4439-b504-d49aba53da26
 ID                                    TIMESTAMP                        SIZE  SOURCE-VOL
 11823425-41fa-434a-9efd-a356b70b5d7c  2023-06-06T05:49:13.987Z         0     dc4e66fd-3b33-4439-b504-d49aba53da26
 22823425-41fa-434a-9efd-a356b70b5d7c  2023-06-06T05:50:14.980Z         0     dc4e66fd-3b33-4439-b504-d49aba53da26

```

11. Volume Rebuild History by volumeID
```
❯ kubectl mayastor get rebuild-history e898106d-e735-4edf-aba2-932d42c3c58d
DST                                   SRC                                   STATE      TOTAL  RECOVERED  TRANSFERRED  IS-PARTIAL  START-TIME            END-TIME
b5de71a6-055d-433a-a1c5-2b39ade05d86  0dafa450-7a19-4e21-a919-89c6f9bd2a97  Completed  7MiB   7MiB       0 B          true        2023-07-04T05:45:47Z  2023-07-04T05:45:47Z
b5de71a6-055d-433a-a1c5-2b39ade05d86  0dafa450-7a19-4e21-a919-89c6f9bd2a97  Completed  7MiB   7MiB       0 B          true        2023-07-04T05:45:46Z  2023-07-04T05:45:46Z

❯ kubectl mayastor get rebuild-history e898106d-e735-4edf-aba2-932d42c3c58d -ojson
{"targetUuid":"c9eb4172-e90c-40ca-9db0-26b2ae372b28","records":[{"childUri":"nvmf://10.1.0.9:8420/nqn.2019-05.io.openebs:b5de71a6-055d-433a-a1c5-2b39ade05d86?uuid=b5de71a6-055d-433a-a1c5-2b39ade05d86","srcUri":"bdev:///0dafa450-7a19-4e21-a919-89c6f9bd2a97?uuid=0dafa450-7a19-4e21-a919-89c6f9bd2a97","rebuildJobState":"Completed","blocksTotal":14302,"blocksRecovered":14302,"blocksTransferred":0,"blocksRemaining":0,"blockSize":512,"isPartial":true,"startTime":"2023-07-04T05:45:47.765932276Z","endTime":"2023-07-04T05:45:47.766825274Z"},{"childUri":"nvmf://10.1.0.9:8420/nqn.2019-05.io.openebs:b5de71a6-055d-433a-a1c5-2b39ade05d86?uuid=b5de71a6-055d-433a-a1c5-2b39ade05d86","srcUri":"bdev:///0dafa450-7a19-4e21-a919-89c6f9bd2a97?uuid=0dafa450-7a19-4e21-a919-89c6f9bd2a97","rebuildJobState":"Completed","blocksTotal":14302,"blocksRecovered":14302,"blocksTransferred":0,"blocksRemaining":0,"blockSize":512,"isPartial":true,"startTime":"2023-07-04T05:45:46.242015389Z","endTime":"2023-07-04T05:45:46.242927463Z"}]}

```

**NOTE: The above command lists volume snapshots for all volumes if `--volume` or `--snapshot` or a combination of both flags is not used.**

12. Get BlockDevices by NodeID
```
❯ kubectl mayastor get block-devices kworker1 --all
 DEVNAME          DEVTYPE    SIZE       AVAILABLE  MODEL                       DEVPATH                                                         FSTYPE  FSUUID  MOUNTPOINT  PARTTYPE                              MAJOR            MINOR                                     DEVLINKS
 /dev/nvme1n1     disk       400GiB     no         Amazon Elastic Block Store  /devices/pci0000:00/0000:00:1f.0/nvme/nvme1/nvme1n1             259     4       ext4        4616cd08-7a7d-49fe-ae6d-908f9e864fc7                                                             "/dev/disk/by-uuid/4616cd08-7a7d-49fe-ae6d-908f9e864fc7", "/dev/disk/by-id/nvme-Amazon_Elastic_Block_Store_vol04bfba0a58c4ffdae", "/dev/disk/by-id/nvme-nvme.1d0f-766f6c303462666261306135386334666664
 /dev/nvme4n1     disk       2TiB       yes        Amazon Elastic Block Store  /devices/pci0000:00/0000:00:1d.0/nvme/nvme4/nvme4n1             259     12                                                                                                                   "/dev/disk/by-id/nvme-Amazon_Elastic_Block_Store_vol06eb486c9593587a9", "/dev/disk/by-id/nvme-nvme.1d0f-766f6c3036656234383663393539333538376139-416d617a6f6e20456c617374696320426c6f636b2053746f7265-00000001", "/dev/disk/by-path/pci-0000:00:1d.0-nvme-1"
 /dev/nvme2n1     disk       1TiB       no         Amazon Elastic Block Store  /devices/pci0000:00/0000:00:1e.0/nvme/nvme2/nvme2n1             259     5                                                                                                                    "/dev/disk/by-id/nvme-nvme.1d0f-766f6c3033623636623930363535636636656465-416d617a6f6e20456c617374696320426c6f636b2053746f7265-00000001", "/dev/disk/by-path/pci-0000:00:1e.0-nvme-1", "/dev/disk/by-id/nvme-Amazon_Elastic_Block_Store_vol03b66b90655cf6ede"
```
```
❯ kubectl mayastor get block-devices kworker1
 DEVNAME       DEVTYPE  SIZE      AVAILABLE  MODEL                       DEVPATH                                              MAJOR  MINOR  DEVLINKS
 /dev/nvme4n1  disk     2TiB      yes        Amazon Elastic Block Store  /devices/pci0000:00/0000:00:1d.0/nvme/nvme4/nvme4n1  259    12     "/dev/disk/by-id/nvme-Amazon_Elastic_Block_Store_vol06eb486c9593587a9", "/dev/disk/by-id/nvme-nvme.1d0f-766f6c3036656234383663393539333538376139-416d617a6f6e20456c617374696320426c6f636b2053746f7265-00000001", "/dev/disk/by-path/pci-0000:00:1d.0-nvme-1"
```
**NOTE: The above command lists usable blockdevices if `--all` flag is not used, but currently since there isn't a way to identify whether the `disk` has a blobstore pool, `disks` not used by `pools` created by `control-plane` are shown as usable if they lack any filesystem uuid.**

</details>

<details>
<summary> Node Cordon And Drain Operations </summary>

1. Node Cordoning
```
❯ kubectl mayastor cordon node kworker1 my_cordon_1
Node node-1-14048 cordoned successfully
```
2. Node Uncordoning
```
❯ kubectl mayastor uncordon node kworker1 my_cordon_1
Node node-1-14048 successfully uncordoned
```
3. Get Cordon
```
❯ kubectl mayastor get cordon node node-1-14048
 ID            GRPC ENDPOINT        STATUS  CORDONED  CORDON LABELS
 node-1-14048  95.217.158.66:10124  Online  true      my_cordon_1

❯ kubectl mayastor get cordon nodes
 ID            GRPC ENDPOINT        STATUS  CORDONED  CORDON LABELS
 node-2-14048  95.217.152.7:10124   Online  true      my_cordon_2
 node-1-14048  95.217.158.66:10124  Online  true      my_cordon_1
```
4. Node Draining
```
❯ kubectl mayastor drain node io-engine-1 my-drain-label
Node node-1-14048 successfully drained

❯ kubectl mayastor drain node node-1-14048 my-drain-label --drain-timeout 10s
Node node-1-14048 drain command timed out
```
5. Cancel Node Drain (via uncordon)
```
❯ kubectl mayastor uncordon node io-engine-1 my-drain-label
Node io-engine-1 successfully uncordoned
```
6. Get Drain
```
❯ kubectl mayastor get drain node node-2-14048
 ID            GRPC ENDPOINT       STATUS  CORDONED  DRAIN STATE  DRAIN LABELS
 node-2-14048  95.217.152.7:10124  Online  true      Draining     my_drain_2

❯ kubectl-plugin/bin/kubectl-mayastor get drain node node-0-14048
 ID            GRPC ENDPOINT          STATUS  CORDONED  DRAIN STATE   DRAIN LABELS
 node-0-14048  135.181.206.173:10124  Online  false     Not draining

❯ kubectl mayastor get drain nodes
 ID            GRPC ENDPOINT        STATUS  CORDONED  DRAIN STATE  DRAIN LABELS
 node-2-14048  95.217.152.7:10124   Online  true      Draining     my_drain_2
 node-1-14048  95.217.158.66:10124  Online  true      Drained      my_drain_1

```
</details>

<details>
<summary> Scale Resources operations </summary>

1. Scale Volume by ID
```
❯ kubectl mayastor scale volume 0c08667c-8b59-4d11-9192-b54e27e0ce0f 5
Volume 0c08667c-8b59-4d11-9192-b54e27e0ce0f Scaled Successfully 🚀

```
</details>

<details>
<summary> Support operations </summary>

```sh
kubectl mayastor dump
Usage: kubectl-mayastor dump [OPTIONS] <COMMAND>

Commands:
  system   Collects entire system information
  volumes  Collects information about all volumes and its descendants (replicas/pools/nodes)
  volume   Collects information about particular volume and its descendants matching to given volume ID
  pools    Collects information about all pools and its descendants (nodes)
  pool     Collects information about particular pool and its descendants matching to given pool ID
  nodes    Collects information about all nodes
  node     Collects information about particular node matching to given node ID
  etcd     Collects information from etcd
  help     Print this message or the help of the given subcommand(s)

Options:
  -r, --rest <REST>
          The rest endpoint to connect to
  -t, --timeout <TIMEOUT>
          Specifies the timeout value to interact with other modules of system [default: 10s]
  -k, --kube-config-path <KUBE_CONFIG_PATH>
          Path to kubeconfig file
  -s, --since <SINCE>
          Period states to collect all logs from last specified duration [default: 24h]
  -l, --loki-endpoint <LOKI_ENDPOINT>
          Endpoint of LOKI service, if left empty then it will try to parse endpoint from Loki service(K8s service resource), if the tool is unable to parse from service then logs will be collected using Kube-apiserver
  -e, --etcd-endpoint <ETCD_ENDPOINT>
          Endpoint of ETCD service, if left empty then will be parsed from the internal service name
  -d, --output-directory-path <OUTPUT_DIRECTORY_PATH>
          Output directory path to store archive file [default: ./]
  -n, --namespace <NAMESPACE>
          Kubernetes namespace of mayastor service[default: mayastor]
  -o, --output <OUTPUT>
          The Output, viz yaml, json [default: none]
  -j, --jaeger <JAEGER>
          Trace rest requests to the Jaeger endpoint agent
  -h, --help
          Print help

Supportability - collects state & log information of services and dumps it to a tar file.
```

**Note**: Each subcommand supports `--help` option to know various other options.


**Examples**:

1. To collect entire mayastor system information into an archive file
   ```sh
   ## Command
   kubectl mayastor dump system -d <output_directory> -n <mayastor_namespace>
   ```

    - Example command while running inside Kubernetes cluster nodes / system which
      has access to cluster node ports
      ```sh
      kubectl mayastor dump system -d /mayastor-dump -n mayastor
      ```
    - Example command while running outside of Kubernetes cluster nodes where
      nodes exist in private network (or) node ports are not exposed for outside cluster
      ```sh
      kubectl mayastor dump system -d /mayastor-dump -r http://127.0.0.1:30011 -l http://127.0.0.1:3100 -e http://127.0.0.1:2379 -n mayastor
      ```

2. To collect information about all mayastor volumes into an archive file
   ```sh
   ## Command
   kubectl mayastor dump volumes -d <output_directory> -n <mayastor_namespace>
   ```

    - Example command while running inside Kubernetes cluster nodes / system which
      has access to cluster node ports
      ```sh
      kubectl mayastor dump volumes -d /mayastor-dump -n mayastor
      ```
    - Example command while running outside of Kubernetes cluster nodes where
      nodes exist in private network (or) node ports are not exposed for outside cluster
      ```sh
      kubectl mayastor dump volumes -d /mayastor-dump -r http://127.0.0.1:30011 -l http://127.0.0.1:3100 -e http://127.0.0.1:2379 -n mayastor
      ```

   **Note**: similarly to dump pools/nodes information then replace `volumes` with an associated resource type(`pools/nodes`).

3. To collect information about particular volume into an archive file
   ```sh
   ## Command
   kubectl mayastor dump volume <volume_name> -d <output_directory> -n <mayastor_namespace>
   ```

    - Example command while running inside Kubernetes cluster nodes / system which
      has access to cluster node ports
      ```sh
      kubectl mayastor dump volume volume-1 -d /mayastor-dump -n mayastor
      ```
    - Example command while running outside of Kubernetes cluster nodes where
      nodes exist in private network (or) node ports are not exposed for outside cluster
      ```sh
      kubectl mayastor dump volume volume-1 -d /mayastor-dump -r http://127.0.0.1:30011 -l http://127.0.0.1:3100 -e http://127.0.0.1:2379 -n mayastor
      ```

</details>
<details>
<summary> Upgrade operations </summary>

**Examples**:

1. Upgrade deployment
```
  ## Command
  kubectl mayastor upgrade
  `Upgrade` the deployment

  Usage: kubectl-mayastor upgrade [OPTIONS]

  Options:
  -d, --dry-run
        Display all the validations output but will not execute upgrade
  -r, --rest <REST>
        The rest endpoint to connect to
  -D, --skip-data-plane-restart
        If set then upgrade will skip the io-engine pods restart
  -k, --kube-config-path <KUBE_CONFIG_PATH>
        Path to kubeconfig file
  -S, --skip-single-replica-volume-validation
        If set then it will continue with upgrade without validating singla replica volume
  -R, --skip-replica-rebuild
        If set then upgrade will skip the repilca rebuild in progress validation
  -C, --skip-cordoned-node-validation
        If set then upgrade will skip the cordoned node validation
  -o, --output <OUTPUT>
        The Output, viz yaml, json [default: none]
  -j, --jaeger <JAEGER>
        Trace rest requests to the Jaeger endpoint agent
  -n, --namespace <NAMESPACE>
        Kubernetes namespace of mayastor service [default: mayastor]
  -h, --help
        Print help
```

2. Get the upgrade status
```
   ## Command
   kubectl mayastor get upgrade-status
   `Get` the upgrade status

   Usage: kubectl-mayastor get upgrade-status [OPTIONS]

   Options:
   -r, --rest <REST>
        The rest endpoint to connect to
   -k, --kube-config-path <KUBE_CONFIG_PATH>
        Path to kubeconfig file
   -o, --output <OUTPUT>
        The Output, viz yaml, json [default: none]
   -j, --jaeger <JAEGER>
        Trace rest requests to the Jaeger endpoint agent
   -n, --namespace <NAMESPACE>
        Kubernetes namespace of mayastor service [default: mayastor]
   -h, --help
        Print help
   ```

3. Delete upgrade resources
```
   ## Command
   kubectl mayastor delete upgrade
  `Delete` the upgrade resources

  Usage: kubectl-mayastor delete upgrade [OPTIONS]

  Options:
  -f, --force
        If true, immediately remove upgrade resources bypass graceful deletion
  -r, --rest <REST>
        The rest endpoint to connect to
  -k, --kube-config-path <KUBE_CONFIG_PATH>
        Path to kubeconfig file
  -o, --output <OUTPUT>
        The Output, viz yaml, json [default: none]
  -j, --jaeger <JAEGER>
        Trace rest requests to the Jaeger endpoint agent
  -n, --namespace <NAMESPACE>
        Kubernetes namespace of mayastor service [default: mayastor]
  -h, --help
          Print help

```
</details>
