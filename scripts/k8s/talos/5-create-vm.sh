#!/usr/bin/env bash
# Imports the Ubuntu cloud image onto a Mayastor RWX block volume and boots
# a VM from it. Pass --delete to remove the VM and DataVolume instead.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

DELETE=false
for arg in "$@"; do
  case "$arg" in
    --delete) DELETE=true ;;
    *) echo "Usage: $0 [--delete]" >&2; exit 1 ;;
  esac
done

if $DELETE; then
  kubectl delete -f vm.yaml --ignore-not-found
  kubectl delete -f cdi.yaml --ignore-not-found
  exit 0
fi

kubectl apply -f cdi.yaml
DV_NAME=$(kubectl get -f cdi.yaml -o jsonpath='{.metadata.name}')

echo "Waiting for DataVolume $DV_NAME to succeed (this downloads/imports the image, can take a few minutes)..."
until [ "$(kubectl get dv "$DV_NAME" -o=jsonpath='{.status.phase}' 2>/dev/null)" = "Succeeded" ]; do
  kubectl get dv "$DV_NAME"
  sleep 15
done

kubectl apply -f vm.yaml
VM_NAME=$(kubectl get -f vm.yaml -o jsonpath='{.metadata.name}')

echo "Waiting for VM $VM_NAME to be Ready..."
until [ "$(kubectl get vm "$VM_NAME" -o=jsonpath='{.status.ready}' 2>/dev/null)" = "true" ]; do
  sleep 5
done

kubectl get vm "$VM_NAME"
echo "VM $VM_NAME is running. Try: kubectl virt console $VM_NAME or kubectl virt ssh ubuntu@vm/$VM_NAME"
