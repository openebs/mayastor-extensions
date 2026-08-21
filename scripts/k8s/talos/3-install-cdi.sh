#!/usr/bin/env bash
# Installs CDI (Containerized Data Importer), used to import disk images
# into PVCs as DataVolumes. Pass --delete to uninstall it instead.
set -euo pipefail

DELETE=false
for arg in "$@"; do
  case "$arg" in
    --delete) DELETE=true ;;
    *) echo "Usage: $0 [--delete]" >&2; exit 1 ;;
  esac
done

VERSION=$(basename "$(curl -sS -o /dev/null -w '%{redirect_url}' https://github.com/kubevirt/containerized-data-importer/releases/latest)")
if [ -z "$VERSION" ]; then
  echo "Failed to resolve latest CDI version from GitHub's redirect." >&2
  exit 1
fi

if $DELETE; then
  echo "Uninstalling CDI ${VERSION}"
  kubectl delete -f "https://github.com/kubevirt/containerized-data-importer/releases/download/${VERSION}/cdi-cr.yaml" --ignore-not-found
  kubectl delete -f "https://github.com/kubevirt/containerized-data-importer/releases/download/${VERSION}/cdi-operator.yaml" --ignore-not-found
  exit 0
fi

echo "Installing CDI ${VERSION}"
kubectl apply -f "https://github.com/kubevirt/containerized-data-importer/releases/download/${VERSION}/cdi-operator.yaml"
kubectl apply -f "https://github.com/kubevirt/containerized-data-importer/releases/download/${VERSION}/cdi-cr.yaml"

echo "Waiting for CDI to report Deployed..."
until [ "$(kubectl get cdi cdi -n cdi -o=jsonpath='{.status.phase}' 2>/dev/null)" = "Deployed" ]; do
  sleep 5
done
echo "CDI Deployed."
