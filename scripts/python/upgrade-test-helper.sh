#!/usr/bin/env bash

# "Fork" the local chart into a separate location and build the container images and the plugin binary
# with a given TAG (or figures out the tag using $SCRIPT_DIR/build-upgrade-images.sh).
# This is used by the upgrade test

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../.."

# Imports
source "$ROOT_DIR"/scripts/utils/log.sh

set -euo pipefail

TAG=
CHART_VNEXT=
CHART="$(realpath -e "$ROOT_DIR/chart")"
CHART_FORK="false"
CHART_TAG="false"
IMAGE_BUILD="false"
IMAGE_LOAD="false"

cleanup_handler() {
  ERROR=$?
  trap - INT QUIT TERM HUP EXIT
  if [ $ERROR != 0 ]; then
    if [ "$CHART_TAG" = "true" ]; then
      if command -v git &>/dev/null; then
        git restore "$CHART_VNEXT" || :
      fi
    fi
    exit $ERROR
  fi
}

# Print usage options for this script.
print_help() {
  cat <<EOF
Usage: $(basename "${0}") [OPTIONS]

Options:
  -h, --help              Display this text.
  --tag        <tag>      The tag for the vnext chart.
                            Default: Automatically figured out.
  --chart      <path>     The path where the chart will be copied to and modified for the vnext tag.
                            Default: \$workspace_root/tests/python/chart_vnext.
  --fork                  Forks the vnext chart from "$CHART" into CHART_VNEXT.
  --chart-tag             Tag the vnext chart.
  --build                 Builds the container images and the plugin binary.
  --load                  Loads the images into the kind cluster.

Examples:
  $(basename "${0}") --fork

The kubectl-mayastor binary will be built at CHART_VNEXT/kubectl-plugin/bin/kubectl-mayastor
EOF
}

# Parse args.
while test $# -gt 0; do
  arg="$1"
  case "$arg" in
  --tag)
    shift
    TAG="$1"
    ;;
  --chart)
    shift
    CHART_VNEXT="$1"
    ;;
  --fork)
    CHART_FORK="true"
    ;;
  --chart-tag)
    CHART_TAG="true"
    ;;
  --build)
    IMAGE_BUILD="true"
    ;;
  --load)
    IMAGE_LOAD="true"
    ;;
  -h* | --help*)
    print_help
    exit 0
    ;;
  *)
    print_help
    log_fatal "unexpected argument '$arg'" 1
    ;;
  esac
  shift
done

if [ "$IMAGE_LOAD" = "true" ]; then
  if [ "$(kubectl config current-context)" != "kind-kind" ]; then
    log_fatal "Only Supported on Kind Clusters!"
  fi
fi

if [ -z "$TAG" ]; then
  TAG="$("$SCRIPT_DIR"/generate-test-tag.sh)"
fi

if [ -z "$CHART_VNEXT" ]; then
  CHART_VNEXT="$ROOT_DIR/tests/bdd/chart-vnext"
fi
CHART_VNEXT="$(realpath -e "$CHART_VNEXT")"
KUBECTL_MAYASTOR="$CHART_VNEXT/kubectl-plugin/bin/kubectl-mayastor"

# Ensure the chart vnext is created, copied from the original
if [ "$CHART_FORK" = "true" ]; then
  if [ "$CHART" = "$CHART_VNEXT" ]; then
    log_fatal "Forked chart is the same as the original => $CHART"
  fi
  mkdir -p "$CHART_VNEXT"
  rm -r "${CHART_VNEXT:?}"/*
  cp -r "$CHART/." "${CHART_VNEXT:?}"
fi

if [ "$CHART_TAG" = "true" ]; then
  # Tag the vnext chart
  CHART_DIR="$CHART_VNEXT" "$SCRIPT_DIR"/tag-chart.sh "$TAG"
  trap cleanup_handler INT QUIT TERM HUP EXIT
fi

# Build the vnext images and kubectl-binary (in debug mode)
if [ "$IMAGE_BUILD" = "true" ]; then
  RUSTFLAGS="-C debuginfo=0 -C strip=debuginfo" "$ROOT_DIR"/scripts/release.sh --tag "$TAG" --build-binary-out "$CHART_VNEXT" --no-static-linking --skip-publish --debug

  # Ensure binary is on the correct version
  PLUGIN_VERSION="$($KUBECTL_MAYASTOR --version)"
  if [[ ! "$PLUGIN_VERSION" =~ ^Kubectl\ Plugin\ \(kubectl-mayastor\).*\($TAG\+0\)$ ]]; then
    log_fatal "The built kubectl-plugin reports version $PLUGIN_VERSION but we want $TAG"
  fi
fi

cleanup_handler

# Load the images into the kind cluster
if [ "$IMAGE_LOAD" = "true" ]; then
  "$ROOT_DIR"/scripts/k8s/load-images-to-kind.sh --tag "$TAG" --trim-debug-suffix
fi
