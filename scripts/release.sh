#!/usr/bin/env bash

# Build and upload mayastor extensions docker images to dockerhub repository.
# Use --dry-run to just see what would happen.
# The script assumes that a user is logged on to dockerhub for public images,
# or has insecure registry access setup for CI.

SOURCE_REL=$(dirname "$0")/../dependencies/control-plane/utils/dependencies/scripts/release.sh

if [ ! -f "$SOURCE_REL" ] && [ -z "$CI" ]; then
  git submodule update --init --recursive
fi

IMAGES="metrics.exporter.io-engine obs.callhome stats.aggregator upgrade.job"
HELM_DEPS_IMAGES="upgrade.job"
BUILD_BINARIES="kubectl-plugin"
PROJECT="extensions"
. "$SOURCE_REL"

# Sadly helm ignore does not work on symlinks: https://github.com/helm/helm/issues/13284
# So we must cleanup to ensure the upgrade image is built correctly
CHART_DIR="$(dirname "$0")/../chart"
if [ -L "$CHART_DIR"/kubectl-plugin ]; then
  rm "$CHART_DIR"/kubectl-plugin
fi

common_run $@
