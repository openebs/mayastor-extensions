#!/usr/bin/env bash

SCRIPTDIR=$(dirname "$0")
ROOTDIR="$SCRIPTDIR"/../../
TEMPLATE="$ROOTDIR/chart/README.md.tmpl"
README="$ROOTDIR/chart/README.md"
SKIP_GIT=${SKIP_GIT:-}

set -euo pipefail

helm-docs --dry-run -t "$TEMPLATE" > "$README"

if [ -z "$SKIP_GIT" ]; then
  git diff --exit-code "$README"
fi
