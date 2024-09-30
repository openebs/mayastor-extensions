#!/usr/bin/env bash

set -e

TIMEOUT="5m"
WAIT=
DRY_RUN=""
CHART=
SCRIPT_DIR="$(dirname "$0")"
CHART_DIR="$SCRIPT_DIR"/../../chart
DEP_UPDATE=
RELEASE_NAME="mayastor"
K8S_NAMESPACE="mayastor"
FAIL_IF_INSTALLED=

help() {
  cat <<EOF
Usage: $(basename "$0") [COMMAND] [OPTIONS]

Options:
  -h, --help                            Display this text.
  --timeout         <timeout>           How long to wait for helm to complete install (Default: $TIMEOUT).
  --wait                                Wait for helm to complete install.
  --dry-run                             Install helm with --dry-run.
  --dep-update                          Run helm dependency update.
  --fail-if-installed                   Fail with a status code 1 if the helm release '$RELEASE_NAME' already exists in the $K8S_NAMESPACE namespace.

Examples:
  $(basename "$0")
EOF
}

echo_stderr() {
  echo -e "${1}" >&2
}

die() {
  local _return="${2:-1}"
  echo_stderr "$1"
  exit "${_return}"
}

nvme_ana_check() {
  cat /sys/module/nvme_core/parameters/multipath
}

while [ "$#" -gt 0 ]; do
  case $1 in
    -h|--help)
      help
      exit 0
      shift;;
    --timeout)
      shift
      test $# -lt 1 && die "Missing timeout value"
      TIMEOUT=$1
      shift;;
    --wait)
      WAIT="yes"
      shift;;
    --dry-run)
      DRY_RUN=" --dry-run"
      shift;;
    --dep-update)
      DEP_UPDATE="y"
      shift;;
    --fail-if-installed)
      FAIL_IF_INSTALLED="y"
      shift;;
    *)
      die "Unknown argument $1!"
      shift;;
  esac
done

DEP_UPDATE_ARG=
if [ -n "$DEP_UPDATE" ]; then
  DEP_UPDATE_ARG="--dependency-update"
fi

if [ -n "$WAIT" ]; then
  WAIT_ARG=" --wait --timeout $TIMEOUT"
fi

if [ "$(helm ls -n openebs -o json | jq --arg release_name "$RELEASE_NAME" 'any(.[]; .name == $release_name)')" = "true" ]; then
  already_exists_log= "Helm release $RELEASE_NAME already exists in namespace $K8S_NAMESPACE"
  if [ -n "$FAIL_IF_INSTALLED" ]; then
    die "ERROR: $already_exists_log" 1
  fi
  echo "$already_exists_log"
else
  echo "Installing Mayastor Chart"
  set -x
  helm install "$RELEASE_NAME" "$CHART_DIR" -n "$K8S_NAMESPACE" --create-namespace \
       --set="etcd.livenessProbe.initialDelaySeconds=5,etcd.readinessProbe.initialDelaySeconds=5,etcd.replicaCount=1" \
       --set="obs.callhome.enabled=true,obs.callhome.sendReport=false,localpv-provisioner.analytics.enabled=false" \
       --set="eventing.enabled=false" \
       $DRY_RUN $WAIT_ARG $DEP_UPDATE_ARG
  set +x
fi

kubectl get pods -n "$K8S_NAMESPACE" -o wide
