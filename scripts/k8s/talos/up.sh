#!/usr/bin/env bash
# Runs the setup steps. By default runs everything (Talos/QEMU cluster,
# KubeVirt+CDI, Mayastor, then a demo VM); pass --talos, --kubevirt and/or
# --mayastor to run just those parts on their own. Pass --cluster-name to
# use something other than the default "talos-default" cluster.
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
      echo "Usage: $0 [--talos] [--kubevirt] [--mayastor] [--cluster-name NAME]  (no flags = all)" >&2
      exit 1
      ;;
  esac
done
if ! $TALOS && ! $KUBEVIRT && ! $MAYASTOR; then
  TALOS=true
  KUBEVIRT=true
  MAYASTOR=true
fi

if $TALOS; then
  for step in 0-setup-ovmf.sh 1-create-cluster.sh; do
    echo "==> $step"
    ./"$step"
  done
fi

if $KUBEVIRT; then
  for step in 2-install-kubevirt.sh 3-install-cdi.sh; do
    echo "==> $step"
    ./"$step"
  done
fi

if $MAYASTOR; then
  echo "==> 4-install-mayastor.sh"
  ./4-install-mayastor.sh
fi

# The demo VM needs both KubeVirt/CDI and Mayastor's storage class in place.
if $KUBEVIRT && $MAYASTOR; then
  echo "==> 5-create-vm.sh"
  ./5-create-vm.sh
fi
