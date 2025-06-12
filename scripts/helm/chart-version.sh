#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../.."
CHART_DIR="$ROOT_DIR/chart"
CHART="$CHART_DIR/Chart.yaml"
HELM="helm"

source "$ROOT_DIR/scripts/utils/helm.sh"

echo -n "$(helm_chart_version)"
