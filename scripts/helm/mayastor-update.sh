#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$(realpath "$SCRIPT_DIR/../..")"
CHART_DIR="$ROOT_DIR/chart"
CHART="$CHART_DIR/Chart.yaml"
CHART_VALUES="$CHART_DIR/values.yaml"
HELM="helm"
BINARY="./kubectl-plugin/bin/kubectl-mayastor"
KIND="kind"

source "$ROOT_DIR/scripts/utils/yaml.sh"
source "$ROOT_DIR/scripts/utils/log.sh"
source "$ROOT_DIR/scripts/utils/helm.sh"
source "$ROOT_DIR/scripts/utils/repo.sh"

get_hash() {
  vers=$(git rev-parse --short=12 HEAD)
  echo -n "$vers"
}

yq_ibl_() {
  if [ "${DRY_RUN:-}" = "true" ]; then
    echo "yq_ibl $*"
  else
    yq_ibl "$1" "$2"
  fi
}

chart_clean() {
  git diff --quiet "$CHART_DIR" && git diff --cached --quiet "$CHART_DIR"
}

display_help() {
  cat <<EOF
 Usage: $(basename "$0") [options]

 Options:
    -k, --kind          Load upgrade image to kind
    -d, --dry-run       Output actions that would be taken, but don't run them
    -l, --latest        Unpin the helm chart to the latest
    -h, --help          Display this help message

Examples:
  $(basename "$0")
EOF
}

PIN="helm-pins"
DRY_RUN="no"
DRY_RUN_ARG=""
CLEAN_TREE=

while [[ "$#" -gt 0 ]]; do
  case $1 in
    -c|--clean) CLEAN_TREE="true" ;;
    -d|--dry-run) DRY_RUN="true" ;;
    -k|--kind) KIND_LD="true" ;;
    -h|--help) display_help; exit 0 ;;
    *) echo "Unknown parameter passed: $1"; display_help; exit 1 ;;
  esac
  shift
done

TAG="$(helm_chart_tag)"
if [[ -z "$TAG" ]]; then
  log_fatal "No Helm CHART Tag"
fi

if ! chart_clean && [[ "${CLEAN_TREE:-}" = "true" ]]; then
  echo "Dirty Chart at $CHART_DIR"
  git restore "$CHART_DIR"
fi

CURR_EXT_HASH=$(yq eval ".image.repoTags.extensions" "$CHART_VALUES")
CURR_CTRL_HASH=$(yq eval ".image.repoTags.controlPlane" "$CHART_VALUES")
CURR_DATA_HASH=$(yq eval ".image.repoTags.dataPlane" "$CHART_VALUES")

$ROOT_DIR/scripts/git/set-submodule-branches.sh -u

EXT_HASH="$(get_hash)"
CTRL_HASH="$(cd "$ROOT_DIR/dependencies/control-plane"; get_hash)"

if [[ -z "${DATA_HASH:-}" ]]; then
  DATA_REMOTE=$(git remote get-url origin | sed 's/mayastor-extensions/mayastor/')
  DATA_HASH=$(git ls-remote "$DATA_REMOTE" "refs/heads/$TAG" | awk '{print substr($1, 1, 12)}')
fi
if [[ -z "${DATE_TIME:-}" ]]; then
  DATE_TIME=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
fi

CHART_NAME="$(helm_chart_name)"
CHART_VERSION="$(helm_chart_version)"

echo "Tag                : $TAG"
echo "Chart              : $CHART_NAME"
echo "Chart Version      : $CHART_VERSION"
echo "Extensions hash    : $CURR_EXT_HASH => $EXT_HASH"
echo "Control-Plane hash : $CURR_CTRL_HASH => $CTRL_HASH"
echo "Data-Plane hash    : $CURR_DATA_HASH => $DATA_HASH"
echo "Date               : $DATE_TIME"

if [[ "${DRY_RUN:-}" = "true" ]]; then
  HELM="echo $HELM"
  DRY_RUN_ARG="--dry-run"
  KIND="echo $KIND"
fi

# Then we pin the floating docker tags, ensuring that we always get the same image
yq_ibl_ ".image.repoTags.controlPlane |= \"$CTRL_HASH\"" "$CHART_VALUES"
yq_ibl_ ".image.repoTags.dataPlane |= \"$DATA_HASH\"" "$CHART_VALUES"
yq_ibl_ ".image.repoTags.extensions |= \"$EXT_HASH\"" "$CHART_VALUES"

# Since the images are now pinned, we can set the pull policy to IfNotPresent
yq_ibl_ ".image.pullPolicy |= \"IfNotPresent\"" "$CHART_VALUES"

## Update the helm annotation images for completeness
"$ROOT_DIR"/scripts/helm/images.sh generate >/dev/null
"$ROOT_DIR"/scripts/helm/images.sh patch >/dev/null

# Add the pin packaging timestamp and commit hashes
yq_ibl_ ".annotations.$PIN/commit |= \"$EXT_HASH\"" "$CHART"
yq_ibl_ ".annotations.$PIN/commits/control-plane |= \"$CTRL_HASH\"" "$CHART"
yq_ibl_ ".annotations.$PIN/commits/data-plane |= \"$DATA_HASH\"" "$CHART"
yq_ibl_ ".annotations.$PIN/commits/extensions |= \"$EXT_HASH\"" "$CHART"

if chart_clean; then
  echo "Modified           : false"
else
  echo "Modified           : true"
fi
