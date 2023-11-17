#!/usr/bin/env bash

help() {
  cat <<EOF
Usage: $(basename "$0") [COMMAND] [OPTIONS]

Options:
  -h, --help                        Display this text.
  --tag           <tag>             The release tag.
  --workflow      <workflow>        The workflow which builds/archives the artifacts.
  --repo-org      <repo-org>        The repo's owner organization.
  --upload        <repos>           Upload artifacts to the given repos.

Command:
  download                          Download the artifacts.
  upload                            Upload the artifacts.

Examples:
  $(basename "$0") --tag v3.0.0 --workflow "Release Artifacts" --repo-org openebs download
EOF
}

echo_stderr() {
  echo -e "${1}" >&2
}

die() {
  local _return="${2:-1}"
  echo_stderr "$1"
  exit "${_return}"
}

get_artifacts_id() {
  local tag="$1"
  local workflow="$2"
  local repo="$BINARY_ORG_REPO"
  local sha=$(gh api repos/$repo/git/refs/tags/$tag | jq '.object.sha')
  job=$(gh run list --branch "$tag" --workflow "$workflow" --repo "$repo" --json conclusion,headSha,databaseId --jq ".[] | select(.headSha == $sha)")
  if [ -z "$job" ]; then
    echo "Job not found" >&2
    return 1
  fi
  conclusion="$(echo "$job" | jq ".conclusion")"
  id=$(echo "$job" | jq ".databaseId")
  if [ "$conclusion" = "\"\"" ]; then
    echo "Job id=$id is still running" >&2
    return 2
  fi
  if [ "$conclusion" = "\"failed\"" ]; then
    echo "Job id=$id has failed, please retry it..." >&2
    return 3
  fi
  if [ "$conclusion" = "\"success\"" ]; then
    echo "$id"
    return 0
  fi
  echo "Unexpected conclusion=$conclusion for job=$id" >&2
  return 4
}

download_artifacts() {
  local tag="$1"
  local workflow="$2"

  mkdir artifacts 2>/dev/null || rm -rf ./artifacts/* || true
  gh run download $id --dir artifacts --repo "$BINARY_ORG_REPO"
}

upload_artifacts() {
  local tag="$1"
  local repo="$2"

  tars=$(find artifacts/ -type f | xargs)
  gh release upload "$tag" --clobber $tars --repo "$repo"
}

retry() {
  local attempt=1
  local delay="$1"
  local attempts="$2"
  shift
  shift
  while true; do
    "$@" && break || {
      if [[ $attempt -lt $attempts ]]; then
        let "attempt+=1"
        echo "[ failed ] Attempt $attempt/$attempts in $delay seconds.." >&2
        sleep $delay
      else
        local _error=$?
        echo_stderr "The command has failed after $attempt attempts!"
        return $_error
      fi
    }
  done
}

retry_download_artifacts() {
  local tag="$1"
  local workflow="$2"

  echo "Getting artifacts workflow id tag=$tag workflow=$workflow"
  id=$(retry 120 60 get_artifacts_id "$tag" "$workflow")
  if [ "$?" -ne 0 ] || [ -z "$id" ]; then
    die "Failed to get github workflow id"
  fi
  echo "Found workflow id=$id tag=$tag workflow=$workflow"
  echo "Downloading artifacts tag=$tag workflow=$workflow"
  retry 30 10 download_artifacts "$tag" "$workflow"
}

retry_upload_artifacts() {
  local tag="$1"
  local repos="$2"

  tree ./artifacts
  tars=$(find artifacts/ -type f | xargs)
  echo "Uploading artifacts tag=$tag repos=$repos"
  for repo in $repos; do
    echo "Uploading artifacts to $repo"
    retry 30 10 upload_artifacts "$tag" "$repo"
  done
}

RELEASE_TAG=
BINARY_WORFLOW=
COMMAND=
REPO_ORG=
BINARY_ORG_REPO=

while [ "$#" -gt 0 ]; do
  case $1 in
    -h|--help)
      help
      exit 0
      ;;
    --tag)
      shift
      RELEASE_TAG="$1"
      shift
      ;;
    --workflow)
      shift
      BINARY_WORKFLOW="$1"
      shift
      ;;
    --repo-org)
      shift
      REPO_ORG="$1"
      shift
      ;;
    download)
      COMMAND="download"
      shift
      ;;
    --upload)
      shift
      UPLOAD_TO="$UPLOAD_TO $1"
      shift
      ;;
    upload)
      shift
      COMMAND="upload"
      shift
      ;;
    *)
      help
      die "Unknown option: $1"
      ;;
  esac
done

set -u

if [ -z "$COMMAND" ]; then
  die "Command is required!"
fi
if [ -z "$REPO_ORG" ]; then
  die "--repo-org parameter is required!"
fi

BINARY_ORG_REPO="$REPO_ORG/mayastor-extensions"

if [ "$COMMAND" == "download" ]; then
  if [ -z "$BINARY_WORKFLOW" ]; then
    die "--workflow parameter is required!"
  fi
  retry_download_artifacts "$RELEASE_TAG" "$BINARY_WORKFLOW"
fi

if [ "$COMMAND" == "upload" ]; then
  if [ -z "$UPLOAD_TO" ]; then
    die "--upload parameter is required!"
  fi
  org_repo=
  for repo in $UPLOAD_TO; do
    org_repo="$org_repo $REPO_ORG/$repo"
  done
  retry_upload_artifacts "$RELEASE_TAG" "$org_repo"
fi
