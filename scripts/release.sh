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

common_run $@
