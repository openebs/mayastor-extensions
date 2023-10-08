#!/usr/bin/env bash

# -o errexit: abort script if one command fails
# -o errtrace: the ERR trap is inherited by shell functions
# -o pipefail: entire command fails if pipe fails
set -o errexit -o errtrace -o pipefail

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
export ROOT_DIR="$SCRIPT_DIR/../.."
BDD_DIR="$ROOT_DIR/tests/bdd"
VENV_DIR="$BDD_DIR/venv"
VENV_PTH="$BDD_DIR"

virtualenv --no-setuptools "$VENV_DIR"

# Set up virtual env.
"$ROOT_DIR"/scripts/python/venv-setup-prep.sh --venv-pth "$VENV_DIR" "$VENV_PTH"

# Because we source this script, and the tests may fail when writing them,
# having `set -e` forces the shell to exit on error. We'd not want to repeatedly
# set up the virtual env, so going with a set +e.
set +e

source "$VENV_DIR/bin/activate"

pip install -r "$BDD_DIR/requirements.txt"
