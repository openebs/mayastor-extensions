#!/usr/bin/env bash

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../.."
# Imports
source "$ROOT_DIR/scripts/utils/log.sh"

set -e

_pytest() {
  pytest "$1" || pytest_return=$?
  if [ "$pytest_return" = 5 ]; then
    exit 0
  fi
  exit $pytest_return
}

# Print usage options for this script.
print_help() {
  cat <<EOF
Usage: $(basename "${0}") [OPTIONS]

Options:
  -h, --help                 Display this text.
  -p, --skip-venv-setup      Skip setting up python environment
  --pytest-args <ARGS>       Use custom arguments when executing pytest.

Examples:
  $(basename "${0}")
EOF
}

PYTEST_ARGS=
PYTHON_VENV_SETUP=1

# Parse args.
while test $# -gt 0; do
  arg="$1"
  case "$arg" in
  -p | --skip-venv-setup)
    PYTHON_VENV_SETUP=
    ;;
  --pytest-args)
    test $# -lt 2 && log_fatal "missing value for the optional argument '$arg'"
    PYTEST_ARGS=$2
    shift
    ;;
  --pytest-args=*)
    PYTEST_ARGS=${arg#*=}
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

if [ "$PYTHON_VENV_SETUP" = 1 ]; then
  source $ROOT_DIR/tests/bdd/setup.sh
fi

if [ -z "$PYTEST_ARGS" ]; then
  PYTEST_ARGS="$ROOT_DIR"/tests/bdd
fi
_pytest "$PYTEST_ARGS"
