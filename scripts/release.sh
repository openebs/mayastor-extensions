#!/usr/bin/env bash

# Build and upload mayastor extensions docker images to dockerhub repository.
# Use --dry-run to just see what would happen.
# The script assumes that a user is logged on to dockerhub for public images,
# or has insecure registry access setup for CI.

# Allow override from caller
if [[ -z "${SOURCE_REL:-}" ]]; then
    SOURCE_REL=$(dirname "$0")/../dependencies/control-plane/utils/dependencies/scripts/release.sh
fi

if [ ! -f "$SOURCE_REL" ] && [ -z "$CI" ]; then
  git submodule update --init --recursive
fi

IMAGES="metrics.exporter.io-engine obs.callhome stats.aggregator upgrade.job events.aggregator"
HELM_DEPS_IMAGES="upgrade.job"

if [[ -z "${HELM_CHART_DIR:-}" ]]; then
    HELM_CHART_DIR="$(dirname "$0")/../chart"
fi

BUILD_BINARIES="kubectl-plugin"
PROJECT="extensions"
. "$SOURCE_REL"

# Sadly helm ignore does not work on symlinks: https://github.com/helm/helm/issues/13284
# So we must cleanup to ensure the upgrade image is built correctly
if [ -L "$HELM_CHART_DIR"/kubectl-plugin ]; then
  rm "$HELM_CHART_DIR"/kubectl-plugin
fi

if [ "${NO_RUN:-}" != "true" ]; then
  common_run "$@"
fi
