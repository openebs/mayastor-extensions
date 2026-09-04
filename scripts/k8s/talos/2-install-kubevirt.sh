#!/usr/bin/env bash
# Installs KubeVirt and right-sizes it for a local dev cluster. Pass
# --delete to uninstall it instead.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

DELETE=false
for arg in "$@"; do
  case "$arg" in
    --delete) DELETE=true ;;
    *) echo "Usage: $0 [--delete]" >&2; exit 1 ;;
  esac
done

VERSION=$(curl -sSL https://storage.googleapis.com/kubevirt-prow/release/kubevirt/kubevirt/stable.txt)

if $DELETE; then
  echo "Uninstalling KubeVirt ${VERSION}"
  kubectl delete -f "https://github.com/kubevirt/kubevirt/releases/download/${VERSION}/kubevirt-cr.yaml" --ignore-not-found
  kubectl delete -f "https://github.com/kubevirt/kubevirt/releases/download/${VERSION}/kubevirt-operator.yaml" --ignore-not-found
  exit 0
fi

echo "Installing KubeVirt ${VERSION}"
kubectl apply -f "https://github.com/kubevirt/kubevirt/releases/download/${VERSION}/kubevirt-operator.yaml"
kubectl apply -f "https://github.com/kubevirt/kubevirt/releases/download/${VERSION}/kubevirt-cr.yaml"

# Lower memory requests (and drop limits) on virt-api/virt-controller/virt-handler.
kubectl apply -f kubevirt.yaml

echo "Waiting for KubeVirt to report Deployed..."
until [ "$(kubectl get kubevirt.kubevirt.io/kubevirt -n kubevirt -o=jsonpath='{.status.phase}' 2>/dev/null)" = "Deployed" ]; do
  sleep 5
done
echo "KubeVirt Deployed."
