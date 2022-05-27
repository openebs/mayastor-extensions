# Integrate exporter with kube-prometheus monitoring stack

### Prerequisite

- Mayastor running in the cluster

---

## Step-by-step process:

1. **
   Install [kube-prometheus](https://github.com/prometheus-community/helm-charts/tree/main/charts/kube-prometheus-stack)
   stack**
   ```
   # Helm
   $ helm install [RELEASE_NAME] prometheus-community/kube-prometheus-stack
   ```
2. Install **ServiceMonitor** to select services and their underlying endpoint objects. Refer to
   this [link](https://github.com/prometheus-operator/prometheus-operator/blob/master/Documentation/user-guides/getting-started.md)
   to get a better understanding of how ServiceMonitor works.
   ```
   apiVersion: monitoring.coreos.com/v1
   kind: ServiceMonitor
   metadata:
     name: mayastor-monitoring
     labels:
       app: mayastor
   spec:
     selector:
       matchLabels:
         app: mayastor
     endpoints:
     - port: metrics
   ```

### Verification

- Verify ServiceMonitors are installed
   ```
   kubectl get servicemonitors -n prometheus-stack -l release="monitoring-mayastor"
   NAME                              AGE
   mayastor-monitoring                   33m
   ```