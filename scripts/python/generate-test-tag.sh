#!/usr/bin/env bash

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../.."

set -e

CHART_VERSION=$(yq -eM '.version' "$ROOT_DIR/chart/Chart.yaml" | tr -d '[:space:]')
TAG=

case "$CHART_VERSION" in
0.0.0)
  latest=$(git fetch --tags -q && git describe --tags $(git rev-list --tags --max-count=1) | tr -d '[:space:]')
  TAG=$(semver bump minor $latest)
  ;;
*)
  TAG=$(semver bump patch $CHART_VERSION)
  ;;
esac

echo "v${TAG#v}"
