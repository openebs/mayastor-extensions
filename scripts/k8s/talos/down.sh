#!/usr/bin/env bash
# Tears down. By default destroys the whole Talos/QEMU cluster (which takes
# KubeVirt/CDI/Mayastor with it, since they run inside it). Pass --kubevirt
# and/or --mayastor to remove just those layers and keep the Talos cluster
# running, or --talos to destroy the cluster explicitly. Each step script
# also accepts --delete directly if you want to tear down just that one.
# Pass --cluster-name if you created the cluster with a non-default name.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

export CLUSTER_NAME="${CLUSTER_NAME:-talos-default}"

TALOS=false
KUBEVIRT=false
MAYASTOR=false
while [ $# -gt 0 ]; do
  case "$1" in
    --talos) TALOS=true; shift ;;
    --kubevirt) KUBEVIRT=true; shift ;;
    --mayastor) MAYASTOR=true; shift ;;
    --cluster-name) export CLUSTER_NAME="$2"; shift 2 ;;
    *)
      echo "Usage: $0 [--talos] [--kubevirt] [--mayastor] [--cluster-name NAME]  (no flags = --talos)" >&2
      exit 1
      ;;
  esac
done
if ! $TALOS && ! $KUBEVIRT && ! $MAYASTOR; then
  TALOS=true
fi

# The VM/DataVolume consume both KubeVirt and Mayastor resources, so clear
# them first whenever either of those layers is coming down.
if $KUBEVIRT || $MAYASTOR; then
  echo "==> 5-create-vm.sh --delete"
  ./5-create-vm.sh --delete
fi

if $MAYASTOR; then
  echo "==> 4-install-mayastor.sh --delete"
  ./4-install-mayastor.sh --delete
fi

if $KUBEVIRT; then
  echo "==> 3-install-cdi.sh --delete"
  ./3-install-cdi.sh --delete
  echo "==> 2-install-kubevirt.sh --delete"
  ./2-install-kubevirt.sh --delete
fi

if $TALOS; then
  echo "==> 1-create-cluster.sh --delete"
  ./1-create-cluster.sh --delete
  echo "==> 0-setup-ovmf.sh --delete"
  ./0-setup-ovmf.sh --delete
fi
