#!/usr/bin/env bash
# Creates the Talos-on-QEMU cluster with the controlplane/worker config
# patches from this directory applied. Pass --delete to destroy it instead.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

DELETE=false
CLUSTER_NAME="${CLUSTER_NAME:-talos-default}"
while [ $# -gt 0 ]; do
  case "$1" in
    --delete) DELETE=true; shift ;;
    --cluster-name) CLUSTER_NAME="$2"; shift 2 ;;
    *) echo "Usage: $0 [--delete] [--cluster-name NAME]" >&2; exit 1 ;;
  esac
done

if $DELETE; then
  if [ -d ~/.talos/clusters/"$CLUSTER_NAME" ]; then
    echo "==> Destroying Talos cluster $CLUSTER_NAME..."
    sudo -E talosctl cluster destroy --name "$CLUSTER_NAME"
  else
    echo "==> No Talos cluster named $CLUSTER_NAME found, skipping destroy"
  fi
  exit 0
fi

WORKERS="${WORKERS:-2}"
CPUS_WORKERS="${CPUS_WORKERS:-4}"
MEMORY_WORKERS="${MEMORY_WORKERS:-8GiB}"
MEMORY_CONTROLPLANES="${MEMORY_CONTROLPLANES:-4GiB}"

# talosctl writes the final generated machine config back to whatever path
# is given to --config-patch-controlplanes/--config-patch-workers. Feed it
# disposable copies so the checked-in *.patch.yaml files (and their git
# history) never get clobbered with a live config full of cluster secrets.
GENDIR=".generated"
mkdir -p "$GENDIR"
cp controlplane.patch.yaml "$GENDIR/controlplane.yaml"
cp worker.patch.yaml "$GENDIR/worker.yaml"

sudo -E talosctl cluster create \
  --name "$CLUSTER_NAME" \
  --workers "$WORKERS" \
  --cpus-workers "$CPUS_WORKERS" \
  --memory-workers "$MEMORY_WORKERS" \
  --memory-controlplanes "$MEMORY_CONTROLPLANES" \
  --config-patch-controlplanes "$GENDIR/controlplane.yaml" \
  --config-patch-workers "$GENDIR/worker.yaml" \
  qemu

echo "Waiting for nodes to become Ready..."
kubectl wait --for=condition=Ready nodes --all --timeout=300s
kubectl get nodes -o wide
