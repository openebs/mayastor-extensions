#!/usr/bin/env bash

# This uses the existing remote refs for the openebs/mayastor-extensions repo to find the latest 'release/x.y' branch.
# Requires a 'git fetch origin' (origin being the remote entry for openebs/mayastor-extensions) or equivalent, if not
# done already.
latest_release_branch() {
  local -r remote=${1:-"origin"}
  local -r root_dir=${2:-"$ROOTDIR"}

  if [ -n "$LATEST_RELEASE_BRANCH" ]; then
    echo "$LATEST_RELEASE_BRANCH"
    return 0
  fi

  pushd "$root_dir" > /dev/null

  # The latest release branch name is required for generating the helm chart version/appVersion
  # for the 'main' branch only.
  # The 'git branch' command in the below lines checks remote refs for release/x.y branch entries.
  # Because the 'main' branch is not a significant branch for a user/contributor, this approach towards
  # finding the latest release branch assumes that this script is used when the 'openebs/mayastor-extensions'
  # repo is present amongst git remote refs. This happens automatically when the 'openebs/mayastor-extensions'
  # repo is cloned, and not a user/contributor's fork.
  local latest_release_branch=$(git branch \
    --all \
    --list "$remote/release/*.*" \
    --format '%(refname:short)' \
    --sort 'refname' \
    | tail -n 1)

  if [ "$latest_release_branch" = "" ]; then
    latest_release_branch="$remote/release/0.0"
  fi

  popd > /dev/null

  echo "${latest_release_branch#*$remote/}"
}
