#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../.."
CHART_DIR="$ROOT_DIR/chart"
IMAGES="$CHART_DIR/images.txt"

EXIT_CODE=
DRY_RUN=
DEP_UPDATE=
HELM="helm"
ENABLE_ANALYTICS="eventing.enabled=true,obs.callhome.enabled=true,obs.callhome.sendReport=true,localpv-provisioner.analytics.enabled=true"

help() {
  cat <<EOF
Usage: $(basename "$0") [COMMAND] [OPTIONS]

Options:
  -h, --help                 Display this text.
  --exit-code                Exit with error code if the images file changed ($IMAGES).
  --dry-run                  Show which commands we'd run, but don't run them.
  --dependency-update        Run helm dependency update as the first step.
Examples:
  $(basename "$0")
EOF
}

echo_stderr() {
  echo -e "${1}" >&2
}

die() {
  local _return="${2:-1}"
  echo_stderr "$1"
  exit "${_return}"
}

while [ "$#" -gt 0 ]; do
  case $1 in
    -h|--help)
      help
      exit 0
      shift;;
    --exit-code)
      EXIT_CODE="true"
      shift;;
    --dry-run)
      DRY_RUN="true"
      HELM="echo $HELM"
      shift;;
    --dependency-update)
      DEP_UPDATE="true"
      shift;;
    *)
      die "Unknown argument $1!"
      shift;;
  esac
done

cd "$CHART_DIR"

if [ "$DEP_UPDATE" = "true" ]; then
    $HELM dependency update
fi

$HELM template . --set "$ENABLE_ANALYTICS" | grep "image:" | awk '{ print $2 }' | tr -d \" > "$IMAGES"

if [ "$EXIT_CODE" = "true" ]; then
  git diff --exit-code "$IMAGES"
fi
