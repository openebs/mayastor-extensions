#!/usr/bin/env bash

SCRIPTDIR=$(dirname "$0")
ROOTDIR="$SCRIPTDIR"/../../
TEMPLATE="$ROOTDIR/chart/README.md.tmpl"
README="$ROOTDIR/chart/README.md"

set -euo pipefail

helm-docs --dry-run -t "$TEMPLATE" > "$README"

git diff --exit-code "$README"

