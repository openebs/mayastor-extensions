#!/usr/bin/env bash

set -e

repo_add() {
  local -r url=$1
  local -r preferred_name=$2

  local repo
  if [ -z "$DRY_RUN" ] && [ "$($HELM repo ls -o yaml | yq "contains([{\"url\": \"$url\"}])")" = "true" ]; then
    repo=$($HELM repo ls -o yaml | yq ".[] | select(.url == \"$url\") | .name")
  else
    $HELM repo add "$preferred_name" "$url" > /dev/null
    repo=$preferred_name
  fi

  $HELM repo update > /dev/null || true

  echo "$repo"
}


TIMEOUT="5m"
WAIT=
DRY_RUN=
HELM_DRY_RUN=""
HELM_ARGS=
SCRIPT_DIR="$(dirname "$0")"
CHART_DIR="$SCRIPT_DIR"/../../chart
CHART_SOURCE=$CHART_DIR
DEP_UPDATE=
RELEASE_NAME="mayastor"
K8S_NAMESPACE="mayastor"
FAIL_IF_INSTALLED=
HOSTED=
VERSION=
PULL_POLICY=
INS_LOKI="true"
DEFAULT_REGISTRY="https://openebs.github.io/mayastor-extensions"
HELM="helm"
KUBECTL="kubectl"

help() {
  cat <<EOF
Usage: $(basename "$0") [COMMAND] [OPTIONS]

Options:
  -h, --help                     Display this text.
  --timeout  <timeout>           How long to wait for helm to complete install (Default: $TIMEOUT).
  --wait                         Wait for helm to complete install.
  --dry-run                      Don't run any commands, output them only.
  --helm-dry-run                 Install helm with --dry-run.
  --dep-update                   Run helm dependency update.
  --fail-if-installed            Fail with a status code 1 if the helm release '$RELEASE_NAME' already exists in the $K8S_NAMESPACE namespace.
  --hosted-chart                 Install a hosted chart instead of the local chart.
  --version   <version>          Set the version/version-range for the chart. Works only when used with the '--hosted' option.
  --registry  <registry-url>     Set the registry URL for the hosted chart. Works only when used with the '--hosted' option. (Default: $DEFAULT_REGISTRY)
  --pull-policy <policy>         Set the image pull policy.
  --no-loki                      Don't deploy Loki.
  --helm      <stringArray>      Pass Helm Args directly to the install/upgrade commands.

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

prefix_args() {
  local prefix="$1"
  if [ -n "$prefix" ]; then
    prefix="$prefix."
  fi
  local sep=""
  for set in $(echo "${@:2}" | awk -F, '{ for(i=1; i<=NF; i++) print $i; }'); do
    echo -n "$sep$prefix$set"
    if [ -z "$sep" ]; then
      sep=","
    fi
  done
}

set_args() {
  echo -n "--set="
  prefix_args "" "$@"
}

loki_args() {
  local replicas=1
  local minio="true"
  local mode="standalone"
  if [ "$replicas" != 1 ]; then
    mode="distributed"
  fi

  if [ "$replicas" = "0" ] || [ "$INS_LOKI" = "false" ]; then
    echo -n "$(set_args "loki.enabled=false,alloy.enabled=false")"
    return 0
  fi

  echo -n "$(set_args "loki.loki.commonConfig.replication_factor=$replicas")" \
          "$(set_args "loki.singleBinary.replicas=$replicas")" \
          "$(set_args "loki.minio.mode=$mode") "

  if [ "$minio" = "true" ]; then
    echo -n "$(set_args "loki.minio.enabled=true,loki.minio.replicas=$replicas")"
  else
    echo -n "$(set_args "loki.loki.storage.type=filesystem")" \
            "$(set_args "loki.singleBinary.persistence.enabled=true")" \
            "$(set_args "loki.minio.enabled=false")"
  fi
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
    --helm-dry-run)
      HELM_DRY_RUN=" --dry-run"
      shift;;
    --dry-run)
      DRY_RUN="yes"
      HELM="echo $HELM"
      KUBECTL="echo $KUBECTL"
      shift;;
    --dep-update)
      DEP_UPDATE="y"
      shift;;
    --fail-if-installed)
      FAIL_IF_INSTALLED="y"
      shift;;
    --hosted-chart)
      HOSTED=1
      shift;;
    --version*)
      if [ "$1" = "--version" ]; then
        test $# -lt 2 && die "Missing value for the optional argument '$1'."
        VERSION="$2"
        shift
      else
        VERSION="${1#*=}"
      fi
      shift;;
    --registry)
      if [ "$1" = "--registry" ]; then
        test $# -lt 2 && die "Missing value for the optional argument '$1'."
        REGISTRY="$2"
        shift
      else
        REGISTRY="${1#*=}"
      fi
      shift;;
    --pull-policy)
      shift
      test $# -lt 1 && die "Missing pull policy"
      PULL_POLICY="$1"
      shift;;
    --no-loki)
      INS_LOKI="false"
      shift;;
    --helm)
      shift
      test $# -lt 1 && die "Missing helm args"
      HELM_ARGS="${HELM_ARGS:-} $1"
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

VERSION_ARG=
if [ -n "$HOSTED" ]; then
  if [ -n "$VERSION" ]; then
    VERSION_ARG="--version $VERSION"
  fi
  if [ -z "$REGISTRY" ]; then
    REGISTRY=$DEFAULT_REGISTRY
  fi
  CHART_SOURCE="$(repo_add "$REGISTRY" "mayastor")/mayastor"
  DEP_UPDATE_ARG=
fi

if [ -n "$PULL_POLICY" ]; then
  PULL_POLICY_ARG="--set image.pullPolicy=$PULL_POLICY"
fi

if [ -z "$DRY_RUN" ] && [ "$($HELM ls -n "$K8S_NAMESPACE" -o yaml | yq "contains([{\"name\": \"$RELEASE_NAME\"}])")" = "true" ]; then
  already_exists_log="Helm release $RELEASE_NAME already exists in namespace $K8S_NAMESPACE"
  if [ -n "$FAIL_IF_INSTALLED" ]; then
    die "ERROR: $already_exists_log" 1
  fi
  echo "$already_exists_log"
else
  echo "Installing Mayastor Chart"

  $HELM install "$RELEASE_NAME" "$CHART_SOURCE" -n "$K8S_NAMESPACE" --create-namespace \
       --set="etcd.livenessProbe.initialDelaySeconds=5,etcd.readinessProbe.initialDelaySeconds=5,etcd.replicaCount=1" \
       --set="obs.callhome.enabled=true,obs.callhome.sendReport=false,localpv-provisioner.analytics.enabled=false" \
       --set="eventing.enabled=true,nats.cluster.enabled=false,nats.cluster.replicas=1" \
       $(loki_args) \
       $HELM_DRY_RUN $WAIT_ARG $PULL_POLICY_ARG $DEP_UPDATE_ARG $VERSION_ARG ${HELM_ARGS:-}
fi

$KUBECTL get pods -n "$K8S_NAMESPACE" -o wide
