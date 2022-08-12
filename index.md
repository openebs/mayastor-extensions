# Helm Repository for Mayastor

## Installation Guide

### Prerequisites

 - Make sure the [system requirement pre-requisites](https://mayastor.gitbook.io/introduction/quickstart/prerequisites) are met.
 - Label the nodes same as the mayastor.nodeSelector in values.yaml
 - Create the namespace you want the chart to be installed, or pass the `--create-namespace` flag in the `helm install` command.
   ```sh
   kubectl create ns <mayastor-namespace>
   ```
 - Create secret to download the images from private docker hub repo.
   ```sh
   kubectl create secret docker-registry <same-as-base.imagePullSecrets.secrets>  --docker-server="https://index.docker.io/v1/" --docker-username="<user-name>" --docker-password="<password>" --docker-email="<user-email>" -n <mayastor-namespace>
   ```

### Add the repository

```bash
helm repo add mayastor https://openebs.github.io/mayastor-extensions/ 
```

### Install Mayastor

```bash
helm install mayastor mayastor/mayastor -n mayastor --create-namespace
```

#### :warning: Non-production Playground testing with no default storage class and single-replica etcd
```bash
helm install mayastor mayastor/mayastor -n mayastor --create-namespace --set="etcd.replicaCount=1,etcd.persistence.storageClass=manual,etcd.livenessProbe.initialDelaySeconds=5,etcd.readinessProbe.initialDelaySeconds=5,loki-stack.loki.persistence.storageClassName=manual"
```

For more details on installing Mayastor please see the [chart's README](https://github.com/openebs/mayastor-extensions/blob/develop/chart/README.md).
