#!/usr/bin/env bash

# Print usage options for this script.
print_help() {
  cat <<EOF
Usage: $(basename "${0}") --tag <TAG>

Options:
  -h, --help        Display this text.
  --tag <TAG>       Input the container image tag.

Examples:
  $(basename "${0}") --tag "ribbit"
EOF
}

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../.."
# Imports
source "$ROOT_DIR/scripts/utils/log.sh"

set -e

TAG=

while test $# -gt 0; do
  arg="$1"
  case "$arg" in
  --tag)
    test $# -lt 2 && log_fatal "missing value for the argument '$arg'"
    TAG=$2
    shift
    ;;
  --tag=*)
    TAG=${arg#*=}
    ;;
  -h* | --help*)
    print_help
    exit 0
    ;;
  *)
    print_help
    log_fatal "unexpected argument '$arg'" 1
    ;;
  esac
  shift
done

if [ -z "$TAG" ]; then
  log_fatal "requires an image tag"
fi

IMAGE_TAG="v${TAG#v}"
images=("upgrade-job" "obs-callhome" "obs-callhome-stats" "metrics-exporter-io-engine")
load_cmd="kind load docker-image"
for image in "${images[@]}"; do
  load_cmd+=" openebs/mayastor-"$image:$IMAGE_TAG
done
eval $load_cmd
