#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../.."
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

display_help() {
  cat <<EOF
 Usage: $(basename "$0") [options]

 Options:
    -r, --registry      OCI registry (e.g., $CHART_REGISTRY)
    -n, --namespace     Repository namespace (e.g., $CHART_NAMESPACE)
    -k, --kind          Load upgrade image to kind
    -d, --dry-run       Output actions that would be taken, but don't run them
    -l, --latest        Unpin the helm chart to the latest
    -h, --help          Display this help message

Examples:
  $(basename "$0") --registry $CHART_REGISTRY --namespace $CHART_NAMESPACE -d
EOF
}

oras_latest() {
  local repo="$1"
  local prefix="$2"

  tags=$(oras repo tags "$repo")
  local error=$?
  if [[ $error -ne 0 ]]; then
    local capture
    capture=$(oras repo tags "$repo" 2>&1)
    # todo: can't find a better way to check if repository exists ahead of time
    if echo "$capture" | grep "404" &>/dev/null; then
      return 0
    fi
    return $error
  fi

  echo "$tags" | { grep "^$prefix\." || true; } | sort -rt '.' -k4 -n | head -n 1
}
oci_latest() {
  local repo="$1"
  local prefix="$2"
  local chart latest stderr_file error not_found

  stderr_file=$(mktemp)
  chart=$(helm show chart "oci://$repo" --version "$prefix" 2>"$stderr_file")
  error=$?

  if [[ $error -eq 0 ]]; then
    rm "$stderr_file"

    latest=$(echo "$chart" | yq eval ".annotations.helm-pins/version")
    if ! [[ "${latest#prefix}" = "${latest:-}" ]]; then
      log_fatal "Bad OCI chart latest version annotation: $latest"
    fi

    echo "$latest"
    return 0
  fi

  if cat "$stderr_file" | grep "not found" >/dev/null; then
    not_found="true"
  fi
  rm "$stderr_file"

  if ! [[ "${not_found:-}" = "true" ]]; then
    log_error "Failed to fetch chart oci://$repo $prefix"
    return $error
  fi

  # todo: can't find a better way to check if repository exists ahead of time
  if { oras repo tags "$repo" 2>&1 || :; } | grep "404" >/dev/null; then
    return 0
  fi
  log_error "Failed to fetch tags from $repo"
  return $error
}
oci_next() {
  local repo="$1"
  local prefix="$2"
  local latest

  latest=$(oci_latest "$repo" "$prefix")
  local error=$?
  if [[ $error -ne 0 ]]; then
    return $error
  fi

  if [[ -n "$latest" ]]; then
    semver bump "prerel" "$latest"
  else
    echo "$prefix.1"
  fi
}

helm_pins_version_prefix() {
  local pinned_oci_chart="$1"
  local chart_version="$2"
  local pinned_version_prefix error

  if [[ "$chart_version" = "0.0.0" ]]; then
    local release_branch latest_version bump

    release_branch=$(latest_release_branch "origin")
    error=$?
    if [[ $error -ne 0 ]]; then
      return $error
    fi
    local latest_version="${release_branch#*release/}"
    if [[ "$latest_version" =~ ^[0-9]+$ ]]; then
      bump="major"
      latest_version=${latest_version}.0.0
    elif [[ "$latest_version" =~ ^[0-9]+.[0-9]+$ ]]; then
      bump="minor"
      latest_version=${latest_version}.0
    elif [[ "$latest_version" =~ ^[0-9]+.[0-9]+.[0-9]+$ ]]; then
      bump="patch"
    else
      die "'$release_branch' is not a supported release"
    fi
    latest_version=$(semver bump "$bump" "$latest_version")
    error=$?
    if [[ $error -ne 0 ]]; then
      return $error
    fi
    pinned_version_prefix="$latest_version-pin-dev"
  elif [[ "$(semver get prerel "$chart_version")" = "" ]]; then
    pinned_version_prefix="$chart_version-pin-devrel"
  else
    log_fatal "Version $chart_version is not supported"
  fi
  echo "$pinned_version_prefix"
}
helm_pins_version_latest() {
  local pinned_oci_chart="$1"
  local pinned_version_prefix="$2"
  local pinned_version

  pinned_version=$(oci_latest "$pinned_oci_chart" "$pinned_version_prefix")
  local error=$?
  if [[ $error -ne 0 ]]; then
    return $error
  fi
  echo "$pinned_version"
}
helm_pins_version() {
  local pinned_oci_chart="$1"
  local pinned_version_prefix="$2"
  local pinned_version

  pinned_version=$(oci_next "$pinned_oci_chart" "$pinned_version_prefix")
  local error=$?
  if [[ $error -ne 0 ]]; then
    return $error
  fi
  echo "$pinned_version"
}

update_required() {
  local pinned_oci_chart="$1"
  local pinned_version_prefix="$2"
  local pinned_version="$3"
  local extensions="$4"
  local controller="$5"
  local data="$6"
  local chart_values

  # todo: validate metadata?
  chart=$($HELM show chart "oci://$pinned_oci_chart" --version "$pinned_version_prefix" 2>/dev/null)

  oci_extensions=$(echo "$chart" | yq eval ".annotations.$PIN/commits/extensions")
  if [[ "${#oci_extensions}" != "12" ]]; then
    local commit
    commit=$(echo "$chart" | yq eval ".annotations.$PIN/commit")
    if [[ "${#commit}" = "12" ]]; then
      echo "true"
      return 0
    fi
    log_fatal "Bad OCI extensions hash: $oci_extensions"
  fi
  oci_controller=$(echo "$chart" | yq eval ".annotations.$PIN/commits/control-plane")
  if [[ "${#oci_controller}" != "12" ]]; then
    log_fatal "Bad OCI control-plane hash: $oci_controller"
  fi
  oci_data=$(echo "$chart" | yq eval ".annotations.$PIN/commits/data-plane")
  if [[ "${#oci_data}" != "12" ]]; then
    log_fatal "Bad OCI data-plane hash: $oci_data"
  fi

  if [[ "$oci_extensions" != "$extensions" ]] || [[ "$oci_controller" != "$controller" ]] || [[ "$oci_data" != "$data" ]]; then
    echo "true"
  else
    echo "false"
  fi
}

PIN="helm-pins"
CHART_REGISTRY="ghcr.io"
CHART_NAMESPACE="openebs/$PIN"
DRY_RUN="no"
DRY_RUN_ARG=""
CLEAN_TREE=

while [[ "$#" -gt 0 ]]; do
  case $1 in
    -r|--registry) CHART_REGISTRY="$2"; shift ;;
    -n|--namespace) CHART_NAMESPACE="$2"; shift ;;
    -c|--clean) CLEAN_TREE="true" ;;
    -d|--dry-run) DRY_RUN="true" ;;
    -k|--kind) KIND_LD="true" ;;
    -l|--latest) UNPIN_CHART="true" ;;
    -h|--help) display_help; exit 0 ;;
    *) echo "Unknown parameter passed: $1"; display_help; exit 1 ;;
  esac
  shift
done

TAG="$(helm_chart_tag)"
if [[ -z "$TAG" ]]; then
  log_fatal "No Helm CHART Tag"
fi

 if [[ "${DRY_RUN:-}" = "true" ]] && [[ "${CLEAN_TREE:-}" = "true" ]]; then
  log_fatal "-c|--clean and -d|--dry-run cannot be combined!"
fi

if [ "${UNPIN_CHART:-}" = "true" ]; then
  if [[ "${CLEAN_TREE:-}" = "true" ]]; then
    log_fatal "-c|--clean and -l|--latest cannot be combined!"
  fi
  CHART_VERSION="$(helm_chart_version)"
  if [[ ! "$CHART_VERSION" =~ ^(v?[0-9]+\.[0-9]+\.[0-9]+-pin-(dev|devrel).([0-9]+))$ ]]; then
    log_fatal "Chart is not pinned!"
  fi
  PINNED_VERSION="$(echo "$CHART_VERSION" | sed 's/\.[0-9]\+$//')"
  EXT_HASH=$(yq eval ".image.repoTags.extensions" "$CHART_VALUES")
  CTRL_HASH=$(yq eval ".image.repoTags.controlPlane" "$CHART_VALUES")
  DATA_HASH=$(yq eval ".image.repoTags.dataPlane" "$CHART_VALUES")
  DATE_TIME=$(yq eval ".annotations.$PIN/timestamp" "$CHART")
fi

if git diff --quiet "$CHART_DIR" && git diff --cached --quiet "$CHART_DIR"; then
  if [ "${UNPIN_CHART:-}" = "true" ]; then
    # Assumes we never commit changes (true for now)
    log_fatal "Can't unpin if the chart is not pinned!"
  fi
elif [[ "${CLEAN_TREE:-}" = "true" ]]; then
  echo "Dirty Chart at $CHART_DIR"
  git restore "$CHART_DIR"
elif [[ ! "${UNPIN_CHART:-}" = "true" ]]; then
  log_fatal "Chart $CHART_DIR is dirty. Please restore changes or run with --clean"
fi

# We could use get_hash but this wouldn't work on "local" commits
if [[ -z "${EXT_HASH:-}" ]]; then
  DATA_REMOTE=$(git remote get-url origin)
  EXT_HASH=$(git ls-remote "$DATA_REMOTE" "refs/heads/$TAG" | awk '{print substr($1, 1, 12)}')
fi
# We could use the submodule dep, but I've noticed that it may be pointing to a commit part of a
# merge commit, and in this case there's no equivalent docker image
if [[ -z "${CTRL_HASH:-}" ]]; then
  DATA_REMOTE=$(git remote get-url origin | sed 's/mayastor-extensions/mayastor-control-plane/')
  CTRL_HASH=$(git ls-remote "$DATA_REMOTE" "refs/heads/$TAG" | awk '{print substr($1, 1, 12)}')
fi
if [[ -z "${DATA_HASH:-}" ]]; then
  DATA_REMOTE=$(git remote get-url origin | sed 's/mayastor-extensions/mayastor/')
  DATA_HASH=$(git ls-remote "$DATA_REMOTE" "refs/heads/$TAG" | awk '{print substr($1, 1, 12)}')
fi
if [[ -z "${DATE_TIME:-}" ]]; then
  DATE_TIME=$(date +"%Y-%m-%d-%H-%M-%S")
fi

CHART_NAME="$(helm_chart_name)"
CHART_VERSION="$(helm_chart_version)"
PINNED_OCI_CHART="$CHART_REGISTRY/$CHART_NAMESPACE/$CHART_NAME"
if [[ -z "${PINNED_VERSION:-}" ]]; then
  PINNED_VERSION_PREFIX=$(helm_pins_version_prefix "$PINNED_OCI_CHART" "$CHART_VERSION")
  PINNED_VERSION=$(helm_pins_version "$PINNED_OCI_CHART" "$PINNED_VERSION_PREFIX")
fi

echo "Tag                : $TAG"
echo "Chart              : $CHART_NAME"
echo "Chart Version      : $CHART_VERSION"
echo "OCI                : $PINNED_OCI_CHART"
echo "Pins Chart Version : $PINNED_VERSION"
echo "Extensions hash    : $EXT_HASH"
echo "Control-Plane hash : $CTRL_HASH"
echo "Data-Plane hash    : $DATA_HASH"
echo "Chart Timestamp    : $DATE_TIME"

if [ ! "${UNPIN_CHART:-}" = "true" ] && [[ "$PINNED_VERSION" != "$PINNED_VERSION_PREFIX.1" ]]; then
  UPDATE=$(update_required "$PINNED_OCI_CHART" "$PINNED_VERSION_PREFIX" "$PINNED_VERSION" "$EXT_HASH" "$CTRL_HASH" "$DATA_HASH")
  if [[ "$UPDATE" = "false" ]]; then
    log "Latest pinned version is up to date, nothing left to do"
    exit 0
  fi
fi
# Ensure the pinned version really doesn't exist already
exists=$(helm_oci_chart_exists "$PINNED_OCI_CHART" "$PINNED_VERSION")
if [[ "$exists" = "true" ]]; then
  log_fatal "Pinned chart $PINNED_VERSION already exists!"
fi

if [[ "${DRY_RUN:-}" = "true" ]]; then
  HELM="echo $HELM"
  DRY_RUN_ARG="--dry-run"
  KIND="echo $KIND"
fi

# Then we set the pinned version
yq_ibl_ ".version |= \"$PINNED_VERSION\"" "$CHART"
yq_ibl_ ".appVersion |= \"$PINNED_VERSION\"" "$CHART"

if [ "${UNPIN_CHART:-}" = "true" ]; then
  # we've "un-pinned", nothing else to do since we're keeping all attributes the same
  exit 0
fi

# Then we pin the floating docker tags, ensuring that we always get the same image
yq_ibl_ ".image.repoTags.controlPlane |= \"$CTRL_HASH\"" "$CHART_VALUES"
yq_ibl_ ".image.repoTags.dataPlane |= \"$DATA_HASH\"" "$CHART_VALUES"
yq_ibl_ ".image.repoTags.extensions |= \"$EXT_HASH\"" "$CHART_VALUES"

# Since the images are now pinned, we can set the pull policy to IfNotPresent
yq_ibl_ ".image.pullPolicy |= \"IfNotPresent\"" "$CHART_VALUES"

# Update the helm annotation images for completeness
"$ROOT_DIR"/scripts/helm/images.sh generate >/dev/null
"$ROOT_DIR"/scripts/helm/images.sh patch >/dev/null

# Add the pin packaging timestamp and commit hashes
yq_ibl_ ".annotations.$PIN/timestamp |= \"$DATE_TIME\"" "$CHART"
yq_ibl_ ".annotations.$PIN/version |= \"$PINNED_VERSION\"" "$CHART"
yq_ibl_ ".annotations.$PIN/commit |= \"$EXT_HASH\"" "$CHART"
yq_ibl_ ".annotations.$PIN/commits/control-plane |= \"$CTRL_HASH\"" "$CHART"
yq_ibl_ ".annotations.$PIN/commits/data-plane |= \"$DATA_HASH\"" "$CHART"
yq_ibl_ ".annotations.$PIN/commits/extensions |= \"$EXT_HASH\"" "$CHART"

# Build the upgrade image for our $TAG and the plugin
# TODO: multi-arch plugin build
"$ROOT_DIR"/scripts/release.sh --tag "v${PINNED_VERSION#v}" --registry "$CHART_REGISTRY/$CHART_NAMESPACE" --skip-publish --image "upgrade.job" "$DRY_RUN_ARG"
if [ ! "${DRY_RUN:-}" = "true" ]; then
  if [ ! -f "$BINARY" ]; then
    log_fatal "kubectl plugin binary ($BINARY) not found!"
  fi
  ls -lh "$BINARY"
fi

if [ "${KIND_LD:-}" = "true" ]; then
  $KIND load docker-image "$CHART_REGISTRY/$CHART_NAMESPACE/mayastor-upgrade-job:v${PINNED_VERSION#v}"
fi

# TODO: skopeo copy the other images to the registry?

# The pinned chart is now prepared, please run smoke tests before pushing to OCI
