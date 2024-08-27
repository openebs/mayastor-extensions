#!/usr/bin/env bash

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../.."

# Imports
source "$ROOT_DIR/scripts/utils/log.sh"

# -o errexit: abort script if one command fails
# -o errtrace: the ERR trap is inherited by shell functions
# -o pipefail: entire command fails if pipe fails
set -o errexit -o errtrace -o pipefail

# Print usage options for this script.
print_help() {
  cat <<EOF
Usage: $(basename "${0}") [OPTIONS]

Options:
  -h, --help                      Display this text.
  --venv-pth <venv-dir> <paths>   Add path configuration files to the specified venv (similar to extending PYTHONPATH)
                                  hint: <paths> is a ':' separated string list.

Examples:
  $(basename "${0}") --venv-pth
EOF
}

# Parse arguments.
parse_args() {
  while test $# -gt 0; do
    arg="$1"
    case "$arg" in
    --venv-pth)
      VENV_DIR="$2"
      shift 2 || log_fatal "Missing argument: <venv-dir>"
      VENV_PTH="$1"
      shift || log_fatal "Missing argument: <paths>"
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
  done
}

VENV_DIR=
VENV_PTH=

# Parse CLI args.
parse_args "$@"

# Setup the python config files (similar to extending the PYTHONPATH env, but within venv)
if [ -n "$VENV_PTH" ]; then
  for python_version in "$ROOT_DIR"/tests/bdd/venv/lib/*; do
    rm "$python_version/site-packages/bdd.pth" 2>/dev/null || true
    for dir in $(echo "$VENV_PTH" | tr ':' '\n'); do
      echo "$dir" >> "$python_version/site-packages/bdd.pth"
    done
  done
fi
