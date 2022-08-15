# Helm Repository for Mayastor

## Installation Guide

### Prerequisites

 - Make sure the [system requirement pre-requisites](https://mayastor.gitbook.io/introduction/quickstart/prerequisites) are met.
 - Label the nodes same as the mayastor.nodeSelector in values.yaml

### Add the repository

```bash
helm repo add mayastor https://openebs.github.io/mayastor-extensions/ 
```

### Install Mayastor

```bash
helm install mayastor mayastor/mayastor -n mayastor --create-namespace
```

#### ⚠ WARNING: non-production playground testing with no default storage class and single-replica etcd
```bash
helm install mayastor mayastor/mayastor -n mayastor --create-namespace --set="etcd.replicaCount=1,etcd.persistence.storageClass=manual,etcd.livenessProbe.initialDelaySeconds=5,etcd.readinessProbe.initialDelaySeconds=5,loki-stack.loki.persistence.storageClassName=manual"
```

For more details on installing Mayastor please see the [chart's README](https://github.com/openebs/mayastor-extensions/blob/develop/chart/README.md).
