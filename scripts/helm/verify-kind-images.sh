#!/usr/bin/env bash

set -euo pipefail


SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../.."
CHART_DIR="$ROOT_DIR/chart"
HELM_TPL_IMAGES="$CHART_DIR/images.txt"
HELM_DEP_IMAGES="$CHART_DIR/dependencies-images.txt"
TMP_KIND="/tmp/kind/mayastor"
KIND_IMAGES="kind-images.txt"
DOCKER="docker"

echo_stderr() {
  echo -e "${1}" >&2
}

die() {
  local _return="${2:-1}"
  echo_stderr "$1"
  exit "${_return}"
}

grep_image() {
  local image="$1"
  local file="$2"

  grep -q "^$image$" "$file" || grep -q "^${image##docker.io/}$" "$file"
}

if ! [ -f "$HELM_TPL_IMAGES" ]; then
  die "Missing helm template images, please generate them"
fi

cd "$TMP_KIND"
for worker in kind-worker*; do
  if ! [ -f "$worker/$KIND_IMAGES" ]; then
    die "Missing $KIND_IMAGES file"
  fi

  curr_images=$($DOCKER exec $worker crictl image | tail -n+2 | awk '{ print $1 ":" $2 }')
  for image in ${curr_images[@]}; do
    if grep_image "$image" "$worker/$KIND_IMAGES"; then
      # if it's there before the install, then ignore it.
      continue
    fi
    if ! grep_image "$image" "$HELM_TPL_IMAGES" && ! grep_image "$image" "$HELM_DEP_IMAGES"; then
      echo "$image not found"
    fi
  done
done
