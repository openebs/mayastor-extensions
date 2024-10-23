#!/usr/bin/env bash

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../.."

source "$ROOT_DIR"/scripts/utils/log.sh
source "$ROOT_DIR"/scripts/utils/repo.sh

set -e

CHART_VERSION=$(yq '.version' "$ROOT_DIR/chart/Chart.yaml")
GIT_REMOTE="origin"
TAG=

# Print usage options for this script.
print_help() {
  cat <<EOF
Usage: $(basename "${0}") [OPTIONS]

Options:
  -h, --help                   Display this text.
  -r, --remote <remote-name>   Set the name of the git remote target. (default: "origin")

Examples:
  $(basename "${0}") -r upstream
EOF
}

# Parse args.
while test $# -gt 0; do
  arg="$1"
  case "$arg" in
  -r | --remote)
      test $# -lt 2 && log_fatal "Missing value for the optional argument '$arg'."
      GIT_REMOTE="$2"
      shift
      ;;
  -r=* | --remote=*)
      GIT_REMOTE="${arg#*=}"
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

case "$CHART_VERSION" in
0.0.0)
  latest_branch=$(git fetch -q && latest_release_branch $GIT_REMOTE $ROOT_DIR)
  latest=${latest_branch#release/}
  if [[ "$latest" =~ ^([0-9]+\.[0-9]+)$ ]]; then
    latest="$latest.0"
  fi
  test "$(semver validate $latest)" = "valid"
  TAG=$(semver bump minor $latest)
  ;;
*)
  test "$(semver validate $CHART_VERSION)" = "valid"
  TAG=$(semver bump patch $CHART_VERSION)
  ;;
esac

echo "v${TAG#v}"
