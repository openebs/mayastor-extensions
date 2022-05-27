# Monitoring Disk pools

## Metrics supported by exporter as of the current release are

| Metric name                | Metric type | Labels/tags | Metric unit | Description                                                                    |
|----------------------------| ----------- | ----------- | ----------- |--------------------------------------------------------------------------------|
| disk_pool_total_size_bytes | Gauge | `name`=&lt; pool_id&gt; <br> `node`=&lt;pool_node&gt; | Integer | Total size of the pool                                                         |
| disk_pool_used_size_bytes  | Gauge | `name`=&lt; pool_id&gt; <br> `node`=&lt;pool_node&gt; | Integer | Used size of the pool                                                          |
| disk_pool_status           | Gauge | `name`=&lt; pool_id&gt; <br> `node`=&lt;pool_node&gt; | Integer | Status of the pool (0, 1, 2, 3) = {"Unknown", "Online", "Degraded", "Faulted"} |

### Example of the above-mentioned metrics:

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