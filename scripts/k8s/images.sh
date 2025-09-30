#!/usr/bin/env bash

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../.."

source "$ROOT_DIR"/scripts/utils/log.sh

set -euo pipefail

TAG=
IMAGE_BUILD="false"
IMAGE_SKIP="false"
BIN_BUILD="false"
IMAGE_LOAD="false"
CHART_DIR="$ROOT_DIR/chart"
BUILD_OUT="${KIND_OUT:-$ROOT_DIR/out}"
HELM=helm

# Print usage options for this script.
print_help() {
  cat <<EOF
Usage: $(basename "${0}") [OPTIONS]

Options:
  -h, --help              Display this text.
  --build-out     [path]  Path where the build output goes to. (Default: $BUILD_OUT)
  --build-all             Builds the mayastor images and binary plugin.
  --build-bin             Builds the mayastor binary plugin.
  --build-img             Builds the mayastor images.
  --load                  Loads the images into the kind cluster.

Examples:
  $(basename "${0}") --build
EOF
}

cleanup() {
  if [ -f "${BUILD_OUT:-}" ]; then
    rm -rf "${BUILD_OUT:-}"
  fi
}

image_tag() {
  $HELM show values "$CHART_DIR" --kubeconfig "$CHART_DIR/fake" | yq '.image.tag'
}

# Parse args.
while test $# -gt 0; do
  arg="$1"
  case "$arg" in
  --tag)
    shift
    TAG="$1"
    ;;
  --build-all)
    IMAGE_BUILD="true"
    BIN_BUILD="true"
    ;;
  --build-bin)
    BIN_BUILD="true"
    ;;
  --build-img)
    IMAGE_BUILD="true"
    ;;
  --build-out)
    shift
    BUILD_OUT="$1"
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

if [ "$IMAGE_LOAD" = "true" ] && [ "$BIN_BUILD" = "true" ] && [ "$IMAGE_BUILD" = "false" ]; then
  log_fatal "Cannot load the bin plugin only!"
fi

trap cleanup EXIT

if [ "$IMAGE_LOAD" = "true" ]; then
  if [ "$(kubectl config current-context)" != "kind-kind" ]; then
    log_fatal "Only Supported on Kind Clusters!"
  fi
fi

if [ -z "$TAG" ]; then
  TAG="$(image_tag)"
fi

if [ -z "$BUILD_OUT" ]; then
  if [ -n "$IMAGE_BUILD" ] && [ -n "$IMAGE_BUILD" ]; then
    BUILD_OUT=$(mktemp -d /tmp/mayastor-images-XXXXXX)
  else
    log_fatal "Build Output Dir is required when using only --build-* or --load"
  fi
fi

if [ "$IMAGE_BUILD" = "true" ] || [ "$BIN_BUILD" = "true" ]; then
  SKIP=""
  if [ "$IMAGE_BUILD" = "false" ]; then
    SKIP="--skip-images"
  elif [ "$BIN_BUILD" = "false" ]; then
    SKIP="--skip-bins"
  fi
  RUSTFLAGS="-C debuginfo=0 -C strip=debuginfo" "$ROOT_DIR"/scripts/release.sh --tag "$TAG" --build-binary-out "$BUILD_OUT" --no-static-linking --skip-publish --debug "$SKIP"
fi

if [ "$IMAGE_LOAD" = "true" ]; then
  "$ROOT_DIR"/scripts/k8s/load-images-to-kind.sh --tag "$TAG" --trim-debug-suffix
fi
