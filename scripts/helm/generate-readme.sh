#!/usr/bin/env bash

SCRIPTDIR=$(dirname "$0")
ROOTDIR="$SCRIPTDIR"/../..
CHART_DIR_NAME="chart"
CHART_DIR="$ROOTDIR/$CHART_DIR_NAME"
TEMPLATE="$CHART_DIR/README.md.tmpl"
README="$CHART_DIR/README.md"
SKIP_GIT=${SKIP_GIT:-}

set -euo pipefail

helm-docs --dry-run -g "$CHART_DIR_NAME" -t "$TEMPLATE" > "$README"

if [ -z "$SKIP_GIT" ]; then
  git diff --exit-code "$README"
fi
