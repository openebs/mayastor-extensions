#!/usr/bin/env bash

SCRIPTDIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOTDIR="$SCRIPTDIR/../.."

source "$ROOTDIR/scripts/utils/log.sh"

set -euo pipefail

# Check if the given branch matches the tag
# Example:
# release/2.7 matches tag v2.7.2, v2.7.2-rc.4, etc..
# release/2.7 does not match tag v2.6.0, etc...
tag_matches_branch() {
  local tag="${1#v}"
  local release_branch="$2"

  branch_version="${release_branch#release/}"
  if ! [[ "$branch_version" =~ ^[0-9]+.[0-9]+$ ]]; then
    return 1
  fi

  if ! [[ "$tag" = "$branch_version"* ]]; then
    return 1
  fi
}

# For the given tag, find the branch which is compatible
# See tag_matches_branch for more information.
find_released_branch() {
  local tag="$1"
  local branches=$(git branch -r --contains "$TAG" --points-at "$TAG" --format "%(refname:short)" 2>/dev/null)
  local branch=""

  for release_branch in $branches; do
    release_branch=${release_branch#origin/}
    if tag_matches_branch "$TAG" "$release_branch"; then
      if [ -n "$branch" ]; then
        log_fatal "Multiple branches matched!"
      fi
      branch="$release_branch"
    fi
  done

  echo "$branch"
}

TAG="$1"
BRANCH="$(find_released_branch "$TAG")"

if [ -z "$BRANCH" ]; then
  log_fatal "Failed to find matching released branch for tag '$TAG'"
fi

echo "$BRANCH"
