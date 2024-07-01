#!/usr/bin/env bash

set -e

TIMEOUT="5m"
WAIT=
DRY_RUN=""
CHART=
SCRIPT_DIR="$(dirname "$0")"
CHART_DIR="$SCRIPT_DIR"/../../chart
DEP_UPDATE=
help() {
  cat <<EOF
Usage: $(basename "$0") [COMMAND] [OPTIONS]

Options:
  -h, --help                            Display this text.
  --timeout         <timeout>           How long to wait for helm to complete install (Default: $TIMEOUT).
  --wait                                Wait for helm to complete install.
  --dry-run                             Install helm with --dry-run.
  --dep-update                          Run helm dependency update.

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
    *)
      die "Unknown argument $1!"
      shift;;
  esac
done

if [ -n "$DEP_UPDATE" ]; then
  helm dependency update "$CHART_DIR"
fi

if [ -n "$WAIT" ]; then
  WAIT_ARG=" --wait --timeout $TIMEOUT"
fi

echo "Installing Mayastor Chart"

set -x
helm install mayastor "$CHART_DIR" -n mayastor --create-namespace \
     --set="etcd.livenessProbe.initialDelaySeconds=5,etcd.readinessProbe.initialDelaySeconds=5,etcd.replicaCount=1" \
     --set="obs.callhome.enabled=true,obs.callhome.sendReport=false,eventing.enabled=false" \
     $DRY_RUN $WAIT_ARG
set +x

kubectl get pods -n mayastor -o wide
