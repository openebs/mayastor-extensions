#!/usr/bin/env bash

SCRIPTDIR=$(dirname "$0")
ROOTDIR="$(realpath $SCRIPTDIR/../..)"
CHART_DIR="$ROOTDIR/chart"
CRDS_CHART_DIR="$CHART_DIR/charts/crds"
TEMPLATE="README.md.tmpl"
README="README.md"
SKIP_GIT=${SKIP_GIT:-}

set -euo pipefail

helm-docs -c "$ROOTDIR" -g "$CHART_DIR,$CRDS_CHART_DIR" -t "$TEMPLATE" -o $README

if [ -z "$SKIP_GIT" ]; then
  git diff --exit-code "$CHART_DIR/$README" "$CRDS_CHART_DIR/$README"
fi
