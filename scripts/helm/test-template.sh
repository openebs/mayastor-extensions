#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../.."
CHART_DIR="$ROOT_DIR/chart"

pushd "$CHART_DIR"
helm dependency update
helm template .