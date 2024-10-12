#!/usr/bin/env bash

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../.."
# Imports
source "$ROOT_DIR/scripts/utils/log.sh"

set -e

_pytest() {
  pytest "$1" || pytest_return=$?
  # Exit code 5 denotes no tests were run, which is something we're ok with.
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

Environment Variables:
  BDD_TEST_DIR               The directory from which the pytests would be run. (default: $(realpath "$ROOT_DIR/tests/bdd"))

Examples:
  BDD_TEST_DIR=./tests/bdd $(basename "${0}")
EOF
}

# Parse args.
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

# virtualenv setup.
source $ROOT_DIR/tests/bdd/setup.sh

if [ $# -eq 0 ]; then
  _pytest "${BDD_TEST_DIR:-$ROOT_DIR/tests/bdd}" --durations=20
else
  _pytest "$@"
fi