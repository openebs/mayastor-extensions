#!/usr/bin/env bash

# -o errexit: abort script if one command fails
# -o errtrace: the ERR trap is inherited by shell functions
# -o pipefail: entire command fails if pipe fails
# -o history: record shell history
# -o allexport: export all functions and variables to be available to subscripts
set -o errexit -o errtrace -o pipefail -o history -o allexport

# Write output to error output stream.
log_to_stderr() {
  echo -e "${1}" >&2
}

log_error() {
  log_to_stderr "ERROR: $1"
}

log_warn() {
  log_to_stderr "WARNING: $1"
}

# Exit with error status and print error.
log_fatal() {
  local -r _return="${2:-1}"
  log_error "$1"
  exit "${_return}"
}
