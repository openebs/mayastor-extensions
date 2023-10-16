# Overview

An exporter is a piece of software that collects data from a service or application and exposes it via HTTP in the
Prometheus format.

Mayastor-exporter runs as a sidecar container with mayastor container and collects data through gRPC calls and exposes the
metrics via HTTP endpoint in the Prometheus format. Metrics are exposed via cached data which are fetched at an interval
of `5 minutes`.

The metrics are exported on the HTTP endpoint `/metrics` on the listening port (default 9052). They are served as
plaintext. They are designed to be consumed either by Prometheus itself or by a scraper that is compatible with scraping
a Prometheus client endpoint. You can also open `/metrics` in a browser to see the raw metrics.

# Metrics Documentation

See the [docs](../docs) directory for more information on the exposed metrics.

# Usage

Simply build and run mayastor-exporter as a sidecar container with mayastor daemonset.
To integrate exporter with `kube-prometheus` stack, please refer this [doc](../docs/Integrate-exporter-prometheus.md) for more information.

#### Command line arguments

Exporter can be configured through command line arguments.

```
spec:
  template:
    spec:
      containers:
        - args:
          - '--metrics-endpoint=9052'
```

## Examples

```
# HELP disk_pool_status mayastor name status
# TYPE disk_pool_status gauge
disk_pool_status{node="worker-0",name="mayastor-disk-pool"} 1
# HELP disk_pool_total_size_bytes mayastor name total size in bytes
# TYPE disk_pool_total_size_bytes gauge
disk_pool_total_size_bytes{node="worker-0",name="mayastor-disk-pool"} 5.360320512e+09
# HELP disk_pool_used_size_bytes mayastor name used size in bytes
# TYPE disk_pool_used_size_bytes gauge
disk_pool_used_size_bytes{node="worker-0",name="mayastor-disk-pool"} 2.147483648e+09
```