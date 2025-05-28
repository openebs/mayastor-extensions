#!/usr/bin/env bash

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../.."
TEST_ROOT_DIR=${TEST_ROOT_DIR:-"$ROOT_DIR"}

# Imports
source "$ROOT_DIR/scripts/utils/log.sh"

REPORT="$TEST_ROOT_DIR/report.xml"

set -e

# Print usage options for this script.
print_help() {
  cat <<EOF
Usage: $(basename "${0}") [OPTIONS]

Options:
  -h, --help                 Display this text.

Environment Variables:
  BDD_TEST_DIR               The directory from which the pytests would be run. (default: $(realpath "$TEST_ROOT_DIR/tests/bdd"))

Examples:
  BDD_TEST_DIR=./tests/bdd $(basename "${0}")
EOF
}

ARGS=
# Parse args.
while test $# -gt 0; do
  arg="$1"
  case "$arg" in
  -h* | --help*)
    print_help
    exit 0
    ;;
  *)
    if [ -z "$ARGS" ]; then
      ARGS="$1"
    else
      ARGS="$ARGS $1"
    fi
    ;;
  esac
  shift
done

# virtualenv setup.
source "$TEST_ROOT_DIR"/tests/bdd/setup.sh

if [ -z "$ARGS" ]; then
  pytest "${BDD_TEST_DIR:-$TEST_ROOT_DIR/tests/bdd}" --junit-xml="$REPORT" --durations=20
else
  pytest "$ARGS --junit-xml=$REPORT"
fi
