#!/usr/bin/env bash

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../.."
# Imports
source "$ROOT_DIR/scripts/utils/log.sh"
source "$ROOT_DIR/scripts/utils/yaml.sh"

set -e

# No tag specified.
if [ -z "$1" ]; then
  log_fatal "no tag specified"
fi

CHART_VERSION=${1#v}
IMAGE_TAG="v$CHART_VERSION"
CHART_DIR="$ROOT_DIR/chart"
# TODO: tests should copy the chart and work with its own copy of the chart. Shouldn't modify the chart.
# chart/Chart.yaml
yq_ibl "
  .version = \"$CHART_VERSION\" |
  .appVersion = \"$CHART_VERSION\" | .appVersion style=\"double\" |
  (.dependencies[] | select(.name == \"crds\").version) = \"$CHART_VERSION\"
" "$CHART_DIR/Chart.yaml"
# chart/charts/crds/Chart.yaml
yq_ibl ".version = \"$CHART_VERSION\"" "$CHART_DIR/charts/crds/Chart.yaml"
# chart/doc.yaml
yq_ibl ".chart.version = \"$CHART_VERSION\"" "$CHART_DIR/doc.yaml"
# chart/values.yaml
yq_ibl ".image.repoTags.extensions = \"$IMAGE_TAG\" | .image.repoTags.extensions style=\"double\" |
  .image.repoTags.controlPlane = .image.tag | .image.repoTags.controlPlane style=\"double\" |
  .image.repoTags.dataPlane = .image.tag | .image.repoTags.dataPlane style=\"double\" |
  .image.pullPolicy = \"IfNotPresent\"
" "$CHART_DIR/values.yaml"
