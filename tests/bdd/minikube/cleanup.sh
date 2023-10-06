#!/usr/bin/env bash

# -o errexit: abort script if one command fails
# -o errtrace: the ERR trap is inherited by shell functions
# -o pipefail: entire command fails if pipe fails
# -o history: record shell history
set -o errexit -o errtrace -o pipefail -o history
# ERR trap
trap 'die "failed minikube cleanup"' ERR
trap 'cleanup_and_exit "$?"' EXIT

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../../.."
UTILS_DIR="$ROOT_DIR/scripts/utils"

# Imports
source "$UTILS_DIR/log.sh"
source "$UTILS_DIR/advisory_lock.sh"

cleanup_and_exit() {
  local -r status=${1}

  advisory_lock_remove "$CLEANUP_CONFIG_FILE" "$CLEANUP_CONFIG_FILE_LOCK" "cleanup"
  exit "$status"
}

# Exit with error status and print error.
die() {
  local -r msg=$1
  local -r return=${2:-1}

  test "${_PRINT_HELP:-no}" = yes && print_help >&2
  log_fatal "$msg" "$return"
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
      _PRINT_HELP=yes die "unexpected argument '$arg'" 1
      ;;
    esac
    shift
  done
}

# Parse CLI args.
parse_args "$@"

# Consts
CLEANUP_CONFIG_FILE="${SCRIPT_DIR}/.cleanup_config.yaml"
CLEANUP_CONFIG_FILE_LOCK="${CLEANUP_CONFIG_FILE}.lock"
CONFIG_YAML_OBJ=".cleanupAble"

# LOCK cleanup config
advisory_lock_acquire "$CLEANUP_CONFIG_FILE" "$CLEANUP_CONFIG_FILE_LOCK" "cleanup"

if ! [ -f "$CLEANUP_CONFIG_FILE" ]; then
  echo "Nothing to do."
  exit 0
elif ! yq -e=1 "${CONFIG_YAML_OBJ}" "${CLEANUP_CONFIG_FILE}" &>/dev/null; then
  die "couldn't find $CONFIG_YAML_OBJ in file $CLEANUP_CONFIG_FILE"
fi

job_count=$(yq -e=1 "${CONFIG_YAML_OBJ} | length" "${CLEANUP_CONFIG_FILE}")
if [ "$job_count" -eq 0 ]; then
  echo "Nothing to do."
  exit 0
fi

echo "Starting cleanup..."
for ((i=0; i<job_count; i++)); do
  script=$(yq "${CONFIG_YAML_OBJ}[$i]" "$CLEANUP_CONFIG_FILE")
  echo "Running cleanup job $((i+1))/$job_count..."
  eval "$script" || true
done
echo "Cleanup complete!"