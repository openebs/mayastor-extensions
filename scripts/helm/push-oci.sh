#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../.."
CHART_DIR="$ROOT_DIR/chart"
CHART="$CHART_DIR/Chart.yaml"
CHART_VALUES="$CHART_DIR/values.yaml"

DRY_RUN="false"
HELM="helm"
ORAS="oras"
DRY_RUN_ARG=""

source "$ROOT_DIR/scripts/utils/yaml.sh"
source "$ROOT_DIR/scripts/utils/log.sh"
source "$ROOT_DIR/scripts/utils/helm.sh"

cleanup() {
  if [ -f "$CHART_PACKAGE" ]; then
    rm "$CHART_PACKAGE"
  fi
}

trap cleanup EXIT
trap 'log_fatal "Error on line $LINENO"' ERR

display_help() {
  cat <<EOF
 Usage: $(basename "$0") [options]

 Options:
    -u, --username        Registry username
    -p, --password        Registry password
    -r, --registry        OCI registry (e.g., docker.io)
    -n, --namespace       Repository namespace
    -k, --kubectl-plugin  Push kubectl plugin
    -j, --upgrade-job     Push upgrade-job container image

    -d, --dry-run       Output actions that would be taken, but don't run them
    -h, --help          Display this help message

Examples:
  $(basename "$0") --registry docker.io --username xyz --password abc --namespace qwerty -d
EOF
}

while [[ "$#" -gt 0 ]]; do
  case $1 in
    -u|--username) REGISTRY_USERNAME="$2"; shift ;;
    -p|--password) REGISTRY_PASSWORD="$2"; shift ;;
    -r|--registry) CHART_REGISTRY="$2"; shift ;;
    -n|--namespace) CHART_NAMESPACE="$2"; shift ;;
    -k|--kubectl-plugin) KUBECTL_PLUGIN="true" ;;
    -j|--upgrade-job) UPGRADE_JOB="true" ;;
    -d|--dry-run) DRY_RUN="true" ;;
    -h|--help) display_help; exit 0 ;;
    *) echo "Unknown parameter passed: $1"; display_help; exit 1 ;;
  esac
  shift
done

if [ "$DRY_RUN" = "true" ]; then
  HELM="echo $HELM"
  ORAS="echo $ORAS"
  DRY_RUN_ARG="--dry-run"
fi

if [[ -z "${CHART_REGISTRY:-}" || -z "${CHART_NAMESPACE:-}" ]]; then
  log_error "Error: --registry and --namespace are required."
  display_help
  exit 1
fi

if [[ -n "${REGISTRY_USERNAME:-}" ]] ; then
  log "Logging in Helm Registry $CHART_REGISTRY..."
  ARGS="registry login "$CHART_REGISTRY" -u "$REGISTRY_USERNAME""
  if [[ -n "${REGISTRY_PASSWORD:-}" ]]; then
    echo "${REGISTRY_PASSWORD:-}" | $HELM $ARGS --password-stdin
  else
    $HELM $ARGS
  fi
else
  log "No registry credentials provided; skipping login."
fi

helm_dep_update

CHART_VERSION="$(yq eval '.version' $CHART_DIR/Chart.yaml)"
APP_VERSION="$(yq eval '.appVersion' $CHART_DIR/Chart.yaml)"
CHART_NAME="$(yq eval '.name' $CHART_DIR/Chart.yaml)"
CHART_PACKAGE="$CHART_NAME-$CHART_VERSION.tgz"

log "Packaging chart with chart version $CHART_VERSION and application version $APP_VERSION..."
$HELM package $CHART_DIR

log "Pushing the chart to $CHART_REGISTRY/$CHART_NAMESPACE..."
# Ensure the pinned version really doesn't exist already
exists=$(helm_oci_chart_exists "$CHART_REGISTRY/$CHART_NAMESPACE" "$CHART_VERSION")
if [[ "$exists" = "true" ]]; then
  log_fatal "Chart $CHART_VERSION already exists!"
fi
$HELM push "$CHART_PACKAGE" "oci://$CHART_REGISTRY/$CHART_NAMESPACE"

log "Helm chart pushed successfully!"

if [[ "${KUBECTL_PLUGIN:-}" = "true" ]] || [[ "${UPGRADE_JOB:-}" = "true" ]]; then
  log "Logging in ORAS Registry $CHART_REGISTRY..."
  ARGS="login "$CHART_REGISTRY" -u "$REGISTRY_USERNAME""
  if [[ -n "${REGISTRY_PASSWORD:-}" ]]; then
    echo "${REGISTRY_PASSWORD:-}" | $ORAS $ARGS --password-stdin
  else
    $ORAS $ARGS
  fi
fi

if [[ "${KUBECTL_PLUGIN:-}" = "true" ]]; then
  cd $ROOT_DIR 2>/dev/null
  PLUGIN="./kubectl-plugin/bin/kubectl-mayastor"

  log "Pushing the plugin binary ($PLUGIN:$CHART_VERSION) to $CHART_REGISTRY/$CHART_NAMESPACE..."
  $ORAS push "$CHART_REGISTRY/$CHART_NAMESPACE/kubectl-mayastor:$CHART_VERSION" "$PLUGIN"

  log "Plugin binary ($PLUGIN:$CHART_VERSION) pushed successfully!"
fi

if [[ "${UPGRADE_JOB:-}" = "true" ]]; then
  log "Pushing the upgrade job container image to $CHART_REGISTRY/$CHART_NAMESPACE..."
  "$ROOT_DIR"/scripts/release.sh --tag "v${CHART_VERSION#v}" --registry "$CHART_REGISTRY/$CHART_NAMESPACE" --skip-bins --skip-build --skip-cargo-deps --image "upgrade.job" "$DRY_RUN_ARG"

  log "Upgrade job container image pushed successfully!"
fi
