#!/usr/bin/env bash

# This uses the existing remote refs for the openebs/mayastor-extensions repo to find the latest 'release/x.y' branch.
# Requires a 'git fetch origin' (origin being the remote entry for openebs/mayastor-extensions) or equivalent, if not
# done already.
latest_release_branch() {
  local -r remote=${1:-"origin"}
  local -r root_dir=${2:-"${ROOTDIR:-"$ROOT_DIR"}"}

  if [ -n "${LATEST_RELEASE_BRANCH:-}" ]; then
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
    | sort -V | tail -n 1)

  if [ "$latest_release_branch" = "" ]; then
    latest_release_branch="$remote/release/0.0"
  fi

  popd > /dev/null

  echo "${latest_release_branch#*$remote/}"
}

# Get the latest tag created against a commit of a specific branch
# Args:
# 1. Branch name
# 2. Git remote name
# 3. Root directory of the repository
latest_tag_at_branch() {
  local -r branch=$1
  local -r remote=${2:-"origin"}
  local -r root_dir=${3:-"$ROOTDIR"}

  pushd "$root_dir" > /dev/null

  git fetch --quiet --tags "$remote"

  local latest_tag=""
  local git_tag_exit_code=0
  local -r git_tag_output=$(git tag --list --merged "$remote"/"$branch" --sort=-creatordate 2> /dev/null) || git_tag_exit_code=$?
  if [ "$git_tag_exit_code" = 0 ]; then
    local head_exit_code=0
    local -r head_output=$(echo $git_tag_output | head -n 1) || head_exit_code=$?
    if [ "$head_exit_code" = 0 ]; then
      latest_tag=$head_output
    fi
  fi

  popd > /dev/null

  echo "$latest_tag"
}
