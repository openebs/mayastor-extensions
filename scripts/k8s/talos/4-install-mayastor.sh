#!/usr/bin/env bash
# Installs Mayastor, creates a DiskPool on every worker node, and creates
# the RWX/NVMe-oF storage class used by the VM's DataVolume. Pass --delete
# to remove it all instead.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

NAMESPACE="${NAMESPACE:-mayastor}"
DISK="${DISK:-/dev/vdb}"

DELETE=false
for arg in "$@"; do
  case "$arg" in
    --delete) DELETE=true ;;
    *) echo "Usage: $0 [--delete]" >&2; exit 1 ;;
  esac
done

if $DELETE; then
  echo "Removing storage class and disk pools"
  kubectl delete -f storage-class.yaml --ignore-not-found
  kubectl delete diskpools --all -n "$NAMESPACE" --ignore-not-found
  echo "Uninstalling Mayastor"
  helm uninstall mayastor -n "$NAMESPACE"
  exit 0
fi

EXTENSIONS_ROOT="$(cd ../../.. && pwd)"

(cd "$EXTENSIONS_ROOT" && ./scripts/helm/install.sh \
  --no-loki \
  --helm "--set agents.core.allowNonPersistentDevlink=true" \
  --wait \
  --helm "--set io_engine.cpuCount=1")

echo "Discovering worker nodes..."
WORKER_NODES=$(kubectl get nodes --selector='!node-role.kubernetes.io/control-plane' -o jsonpath='{.items[*].metadata.name}')
if [ -z "$WORKER_NODES" ]; then
  echo "No worker nodes found." >&2
  exit 1
fi

for NODE in $WORKER_NODES; do
  echo "Creating DiskPool on $NODE ($DISK)"
  NODE="$NODE" DISK="$DISK" NAMESPACE="$NAMESPACE" envsubst < storage-pool.yaml | kubectl apply -f -
done

kubectl apply -f storage-class.yaml

echo "Disk pools:"
kubectl get diskpools -n "$NAMESPACE"
