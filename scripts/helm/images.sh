#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../.."
CHART_DIR="$ROOT_DIR/chart"
CHART="$CHART_DIR/Chart.yaml"
IMAGES="$CHART_DIR/images.txt"
NAMESPACE="mayastor"
HELM="helm"

source "$ROOT_DIR/scripts/utils/yaml.sh"
source "$ROOT_DIR/scripts/utils/log.sh"
source "$ROOT_DIR/scripts/utils/helm.sh"

EXIT_CODE=
DEP_UPDATE=
ENABLE_ANALYTICS="eventing.enabled=true,obs.callhome.enabled=true,obs.callhome.sendReport=true,localpv-provisioner.analytics.enabled=true"

helm_dep_value() {
  local dep_chart="${1:-}"
  local dep_name="${2:-}"
  local key="${3:-}"
  local value=""

  # First, check if we're setting the value in our values.yaml
  value="$($HELM show values "$CHART_DIR" --kubeconfig "$CHART_DIR/fake" | yq -r ".$dep_name.$key")"
  if [ -n "$value" ] && [ "$value" != "null" ]; then
    echo "$value"
    return 0
  fi

  # Otherwise, get it from the source (or rather, the dependency :))
  if ! value="$($HELM show values "$dep_chart" --kubeconfig "$CHART_DIR/fake" | yq -r ".$key")"; then
    log_fatal "Can't show the helm dependency $dep_name: $dep_chart"
  fi
  if [ -n "$value" ] && [ "$value" != "null" ]; then
    echo "$value"
    return 0
  fi
}

helm_dep_value_required() {
  local dep_chart="${1:-}"
  local dep_name="${2:-}"
  local key="${3:-}"
  local value
  value=$(helm_dep_value "$dep_chart" "$dep_name" "$key")
  [ -z "$value" ] && log_fatal "I can't find $dep_name.$key neither in our chart nor $dep_chart"
  echo "$value"
}

helm_localpv_prov_helper_image() {
  local registry
  local repository
  local tag
  local version
  local name="localpv-provisioner"
  local chart

  version=$(helm_dep_version "$name")
  if [ "$version" = "" ]; then
    log_fatal "Can't find the version of the helm dependency: $name"
  fi
  chart="$CHART_DIR/charts/$name-$version.tgz"
  if ! [ -f "$chart" ]; then
    log_fatal "Can't find the helm dependency: $chart"
  fi

  registry="$(helm_dep_value "$chart" "$name" "helperPod.image.registry")"
  repository="$(helm_dep_value_required "$chart" "$name" "helperPod.image.repository")"
  tag="$(helm_dep_value_required "$chart" "$name" "helperPod.image.tag")"

  [ -n "$registry" ] && registry="${registry%/}/"
  echo "$registry$repository:$tag"
}

helm_on_demand_images() {
  local images="${1:-}"

  helm_localpv_prov_helper_image >> "$images"
}

cleanup() {
  if [ -f "${TEMPLATE_IMAGES:-}" ]; then
    rm "${TEMPLATE_IMAGES:-}"
  fi
  if [ -d "${CHART_VERSIONED:-}" ]; then
    rm -rf "${CHART_VERSIONED:-}"
  fi
  if [ -f "${LIVE_IMAGES:-}" ]; then
    rm "${LIVE_IMAGES:-}"
  fi
}

needs_help() {
  if [ -n "$1" ]; then
    log_error "$1"
    help
    exit 1
  fi
}

help() {
  cat <<EOF
Usage: $(basename "$0") [COMMAND] [OPTIONS]

Command:
  generate                   Generate a list of helm images using the helm chart.
  patch                      Patch the chart/Chart.yaml with the images from the chart/images.txt file.
  verify                     Verify the chart/images.txt file with the pod images by installing on a live cluster.
  build                      Builds the container images which reside on this repository.

Options:
  -h, --help                 Display this text.
  --exit-code                Exit with error code if the chart/images.txt or chart/Chart.yaml was modified.
  --dependency-update        Forces a helm dependency update as the first step.
                             Otherwise this step is only performed if deemed necessary.

Examples:
  $(basename "$0") generate
EOF
}

COMMAND=
while test $# -gt 0; do
  arg="$1"
  case "$arg" in
    generate | patch | verify | build)
      [ -n "$COMMAND" ] && needs_help "Can't specify two commands"
      COMMAND="$1"
      ;;
    -h|--help)
      help
      exit 0
      ;;
    --exit-code)
      EXIT_CODE="true"
      ;;
    --dependency-update)
      DEP_UPDATE="true"
      ;;
    *)
      log_fatal "Unrecognized argument $1"
      ;;
  esac
  shift
done

trap cleanup EXIT

case "$COMMAND" in
  build)
    TAG="$(helm_chart_tag)"
    RUSTFLAGS="-C debuginfo=0 -C strip=debuginfo" "$ROOT_DIR"/scripts/release.sh --no-static-linking --skip-bins --skip-publish --debug --alias-tag "$TAG"
    "$ROOT_DIR"/scripts/k8s/load-images-to-kind.sh --tag "$TAG" --trim-debug-suffix
    ;;
  generate)
    TEMPLATE_IMAGES=$(mktemp /tmp/helm-XXXXXX.txt)

    helm_dep_update

    # First let's get all images which are statically deployed by the chart. IOW, any images which are part of
    # a pod or pod-controller which gets deployed by the chart.
    $HELM template "$CHART_DIR" --set "$ENABLE_ANALYTICS" --kubeconfig "$CHART_DIR/fake" | grep -Po "^[ \t]*image: \K(.*:.*)$" | tr -d \" | LC_ALL=C sort | uniq > "$TEMPLATE_IMAGES"
    $HELM template "$CHART_DIR" --set "$ENABLE_ANALYTICS" --kubeconfig "$CHART_DIR/fake" --is-upgrade | grep -Po "^[ \t]*image: \K(.*:.*)$" | tr -d \" | LC_ALL=C sort | uniq >> "$TEMPLATE_IMAGES"

    # Second, we handle images which are deployed on-demand by the product and its dependencies.
    # An example of this is the init pod's deployed by the localpv-provisioner in order to prepare the filesystem for
    # a PVC.
    helm_on_demand_images "$TEMPLATE_IMAGES"

    cat "$TEMPLATE_IMAGES" | LC_ALL=C sort | uniq > "$IMAGES"

    if [ "$EXIT_CODE" = "true" ]; then
      git diff --exit-code "$IMAGES"
    fi

    echo "Finished generating the images:"
    cat "$IMAGES"
    ;;
  patch)
    LIST=""
    while IFS= read -r image; do
      name=$(echo "$image" | awk -F '[/:]' '{print $(NF-1)}')
      if [ -z "$LIST" ]; then
        LIST=$(echo -n "{ \"name\": \"$name\", \"image\": \"$image\" }")
      else
        LIST=$(echo -n "$LIST,{ \"name\": \"$name\", \"image\": \"$image\" }")
      fi
    done < "$IMAGES"

    yq_ibl ".annotations.\"helm.sh/images\" |= [$LIST]" "$CHART"
    sed -i 's/  helm.sh\/images:/  helm.sh\/images: |/' "$CHART"

    if [ "$EXIT_CODE" = "true" ]; then
      git diff --exit-code "$CHART"
    fi

    echo "Finished patching $CHART"
    cat "$CHART"
    ;;
  verify)
    LIVE_IMAGES=$(mktemp helm-XXXXXX.txt)
    MISSING_IMAGES=""

    kubectl -n "$NAMESPACE" get pods -o json | jq -r '.items[].spec.containers[]?.image, .items[].spec.initContainers[]?.image' | LC_ALL=C sort | uniq > "$LIVE_IMAGES"

    while IFS= read -r image; do
      if ! grep -xq "$image" "$IMAGES"; then
        if [ -z "$MISSING_IMAGES" ]; then
          MISSING_IMAGES="$image"
        else
          MISSING_IMAGES="$MISSING_IMAGES, $image"
        fi
      fi
    done < "$LIVE_IMAGES"

    if [ -n "$MISSING_IMAGES" ]; then
      log_fatal "Missing images: $MISSING_IMAGES"
    fi

    echo "Finished verifying the images"
    ;;
  *)
    log_fatal "Missing Command"
    ;;
esac
