#!/usr/bin/env bash

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../../.."
UTILS_DIR="$ROOT_DIR/scripts/utils"

# Imports
source "$UTILS_DIR/log.sh"
source "$UTILS_DIR/advisory_lock.sh"

# -o errexit: abort script if one command fails
# -o errtrace: the ERR trap is inherited by shell functions
# -o pipefail: entire command fails if pipe fails
set -o errexit -o errtrace -o pipefail
# ERR trap
trap 'log_fatal "failed minikube cleanup"' ERR
trap 'cleanup_and_exit "$?"' EXIT

cleanup_and_exit() {
  local -r status=${1}

  advisory_lock_remove "$CLEANUP_CONFIG_FILE" "$CLEANUP_CONFIG_FILE_LOCK" "cleanup"
  exit "$status"
}

# Exit because there's nothing to do.
nothing_to_do() {
  echo "Nothing to do."
  exit 0
}

# Print usage options for this script.
print_help() {
  cat <<EOF
Usage: $(basename "${0}") [OPTIONS]

Options:
  -h, --help      Display this text.

Examples:
  $(basename "${0}")
EOF
}

# Parse arguments.
parse_args() {
  while test $# -gt 0; do
    arg="$1"
    case "$arg" in
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
}

# Consts
CLEANUP_CONFIG_FILE="${SCRIPT_DIR}/.cleanup_config.yaml"
CLEANUP_CONFIG_FILE_LOCK="${CLEANUP_CONFIG_FILE}.lock"
CONFIG_YAML_OBJ=".cleanupAble"

# Parse CLI args.
parse_args "$@"

# LOCK cleanup config
advisory_lock_acquire "$CLEANUP_CONFIG_FILE" "$CLEANUP_CONFIG_FILE_LOCK" "cleanup"

if ! [ -f "$CLEANUP_CONFIG_FILE" ]; then
  nothing_to_do
elif ! yq -e=1 "${CONFIG_YAML_OBJ}" "${CLEANUP_CONFIG_FILE}" &>/dev/null; then
  log_fatal "couldn't find $CONFIG_YAML_OBJ in file $CLEANUP_CONFIG_FILE"
elif [[ $(yq -e=1 '.status' "${CLEANUP_CONFIG_FILE}") != "Ready" ]]; then
  nothing_to_do
fi

job_count=$(yq -e=1 "${CONFIG_YAML_OBJ} | length" "${CLEANUP_CONFIG_FILE}")
if [ "$job_count" -eq 0 ]; then
  nothing_to_do
fi

echo "Starting cleanup..."
yq -i '.status="CleaningUp"' "$CLEANUP_CONFIG_FILE"
for ((i=0; i<job_count; i++)); do
  script=$(yq "${CONFIG_YAML_OBJ}[$i]" "$CLEANUP_CONFIG_FILE")
  echo "Running cleanup job $((i+1))/$job_count..."
  eval "$script" || true
done
yq -i '.status="CleanupCompleted"' "$CLEANUP_CONFIG_FILE"
echo "Cleanup complete!"