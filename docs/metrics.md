# Monitoring Disk pools

## Metrics supported by exporter as of the current release are

| Metric name                | Metric type | Labels/tags | Metric unit | Description                                                                    |
|----------------------------| ----------- | ----------- | ----------- |--------------------------------------------------------------------------------|
| diskpool_total_size_bytes | Gauge | `name`=&lt; pool_id&gt; <br> `node`=&lt;pool_node&gt; | Integer | Total size of the pool                                                         |
| diskpool_used_size_bytes  | Gauge | `name`=&lt; pool_id&gt; <br> `node`=&lt;pool_node&gt; | Integer | Used size of the pool                                                          |
| diskpool_status           | Gauge | `name`=&lt; pool_id&gt; <br> `node`=&lt;pool_node&gt; | Integer | Status of the pool (0, 1, 2, 3, 4) = {"Unknown", "Online", "Degraded", "Faulted", "Suspected"} |

### Example of the above-mentioned metrics:

```
# HELP diskpool_status Status of the pool
# TYPE diskpool_status gauge
diskpool_status{name="pool-on-node-2-477115",node="node-2-477115"} 1
# HELP diskpool_total_size_bytes Total size of the pool in bytes
# TYPE diskpool_total_size_bytes gauge
diskpool_total_size_bytes{name="pool-on-node-2-477115",node="node-2-477115"} 10724835328
# HELP diskpool_used_size_bytes Used size of the pool in bytes
# TYPE diskpool_used_size_bytes gauge
diskpool_used_size_bytes{name="pool-on-node-2-477115",node="node-2-477115"} 1073741824
```
