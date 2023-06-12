#!/usr/bin/env bash

# Build and upload docker images to dockerhub repository.
# Use --dry-run to just see what would happen.
# The script assumes that a user is logged on to dockerhub for public images,
# or has insecure registry access setup for CI.

CI=${CI-}

set -euo pipefail

# Test if the image already exists in dockerhub
dockerhub_tag_exists() {
  curl --silent -f -lSL https://hub.docker.com/v2/repositories/$1/tags/$2 1>/dev/null 2>&1
}

# Get the tag at the HEAD
get_tag() {
  vers=`git tag --points-at HEAD`
  echo -n $vers
}
get_hash() {
  vers=`git rev-parse --short=12 HEAD`
  echo -n $vers
}
nix_experimental() {
  if (nix eval 2>&1 || true) | grep "extra-experimental-features" 1>/dev/null; then
      echo -n " --extra-experimental-features nix-command "
  else
      echo -n " "
  fi
}

help() {
  cat <<EOF
Usage: $(basename $0) [OPTIONS]

Options:
  -d, --dry-run              Output actions that would be taken, but don't run them.
  -h, --help                 Display this text.
  --registry <host[:port]>   Push the built images to the provided registry.
  --debug                    Build debug version of images where possible.
  --skip-build               Don't perform nix-build.
  --skip-publish             Don't publish built images.
  --image           <image>  Specify what image to build.
  --alias-tag       <tag>    Explicit alias for short commit hash tag.
  --tag             <tag>    Explicit tag (overrides the git tag).
  --incremental              Builds components in two stages allowing for faster rebuilds during development.
  --build-bins               Builds all the static binaries.
  --build-bin                Specify which binary to build.
  --build-binary-out <path>  Specify the outlink path for the binaries (otherwise it's the current directory).

Examples:
  $(basename $0) --registry 127.0.0.1:5000
EOF
}

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
DOCKER="docker"
NIX_BUILD="nix-build"
NIX_EVAL="nix eval$(nix_experimental)"
NIX_SHELL="nix-shell"
RM="rm"
SCRIPTDIR=$(dirname "$0")
TAG=`get_tag`
HASH=`get_hash`
BRANCH=`git rev-parse --abbrev-ref HEAD`
BRANCH=${BRANCH////-}
IMAGES=
DEFAULT_IMAGES="exporters.metrics.pool  obs.callhome upgrade.job stats.aggregator"
IMAGES_THAT_REQUIRE_HELM_CHART=("upgrade.job")
UPLOAD=
SKIP_PUBLISH=
SKIP_BUILD=
OVERRIDE_COMMIT_HASH=
REGISTRY=
ALIAS=
BUILD_TYPE="release"
ALL_IN_ONE="true"
INCREMENTAL="false"
DEFAULT_BINARIES="kubectl-plugin"
BUILD_BINARIES=
BIN_TARGET_PLAT="linux-musl"
BINARY_OUT_LINK="."

# Check if all needed tools are installed
curl --version >/dev/null
if [ $? -ne 0 ]; then
  echo "Missing curl - install it and put it to your PATH"
  exit 1
fi
$DOCKER --version >/dev/null
if [ $? -ne 0 ]; then
  echo "Missing docker - install it and put it to your PATH"
  exit 1
fi
nix --version >/dev/null
if [ $? -ne 0 ]; then
  echo "Missing nix - install it and put it to your PATH"
  exit 1
fi

# Parse arguments
while [ "$#" -gt 0 ]; do
  case $1 in
    -d|--dry-run)
      DOCKER="echo $DOCKER"
      NIX_BUILD="echo $NIX_BUILD"
      NIX_SHELL="echo $NIX_SHELL"
      RM="echo $RM"
      shift
      ;;
    -h|--help)
      help
      exit 0
      shift
      ;;
    --registry)
      shift
      REGISTRY=$1
      shift
      ;;
    --alias-tag)
      shift
      ALIAS=$1
      shift
      ;;
    --tag)
      shift
      if [ "$TAG" != "" ] && [ "$TAG" != "$1" ]; then
        echo "Overriding $TAG with $1"
      fi
      TAG=$1
      shift
      ;;
    --image)
      shift
      IMAGES="$IMAGES $1"
      shift
      ;;
    --skip-build)
      SKIP_BUILD="yes"
      shift
      ;;
    --skip-publish)
      SKIP_PUBLISH="yes"
      shift
      ;;
    --debug)
      BUILD_TYPE="debug"
      shift
      ;;
    --incremental)
      INCREMENTAL="true"
      shift
      ;;
    --build-bin)
      shift
      BUILD_BINARIES="$BUILD_BINARIES $1"
      shift
      ;;
    --build-bins)
      BUILD_BINARIES="$DEFAULT_BINARIES"
      shift
      ;;
    --build-binary-out)
      shift
      BINARY_OUT_LINK="$1"
      shift
      ;;
    *)
      echo "Unknown option: $1"
      exit 1
      ;;
  esac
done

cd $SCRIPTDIR/..

if [ -z "$IMAGES" ]; then
  IMAGES="$DEFAULT_IMAGES"
elif [ $(echo "$IMAGES" | wc -w) == "1" ]; then
  image=$(echo "$IMAGES" | xargs)
  if $NIX_EVAL -f . "images.debug.$image.imageName" 1>/dev/null 2>/dev/null; then
    if [ "$INCREMENTAL" == "true" ]; then
      # if we're building a single image incrementally, then build only that image
      ALL_IN_ONE="false"
    fi
  fi
fi

# Create alias
alias_tag=
if [ -n "$ALIAS" ]; then
  alias_tag=$ALIAS
  # when alias is created from branch-name we want to keep the hash and have it pushed to CI because
  # the alias will change daily.
  OVERRIDE_COMMIT_HASH="true"
elif [ "$BRANCH" == "develop" ]; then
  alias_tag="$BRANCH"
elif [ "${BRANCH#release-}" != "${BRANCH}" ]; then
  alias_tag="${BRANCH}"
fi

if [ -n "$TAG" ] && [ "$TAG" != "$(get_tag)" ]; then
  # Set the TAG which basically allows building the binaries as if it were a git tag
  NIX_TAG_ARGS="--argstr tag $TAG"
  NIX_BUILD="$NIX_BUILD $NIX_TAG_ARGS"
  alias_tag=
fi

TAG=${TAG:-$HASH}
if [ -n "$OVERRIDE_COMMIT_HASH" ] && [ -n "$alias_tag" ]; then
  # Set the TAG to the alias and remove the alias
  NIX_TAG_ARGS="--argstr img_tag $alias_tag"
  NIX_BUILD="$NIX_BUILD $NIX_TAG_ARGS"
  TAG="$alias_tag"
  alias_tag=
fi

if [ -n "$BUILD_BINARIES" ]; then
  mkdir -p $BINARY_OUT_LINK
  for name in $BUILD_BINARIES; do
    echo "Building static $name ..."
    $NIX_BUILD --out-link $BINARY_OUT_LINK/$name -A utils.$BUILD_TYPE.$BIN_TARGET_PLAT.$name
  done
fi

# This variable will be used to flag if the helm chart dependencies have been
# been updated.
_helm_dependencies_updated=false
for name in $IMAGES; do
  image_basename=$($NIX_EVAL -f . images.$BUILD_TYPE.$name.imageName | xargs)
  image=$image_basename
  if [ -n "$REGISTRY" ]; then
    image="${REGISTRY}/${image}"
  fi

  if [ "$_helm_dependencies_updated" = false ]; then
    for helm_chart_user in ${IMAGES_THAT_REQUIRE_HELM_CHART[@]}; do
      if [ "$name" = "$helm_chart_user" ]; then
        echo "Updating helm chart dependencies..."
        # Helm chart directory path -- /scripts --> /chart
        CHART_DIR="${SCRIPT_DIR}/../chart"

        $NIX_SHELL --run "helm dependency update ${CHART_DIR}"

        # Set flag to true
        _helm_dependencies_updated=true
        break
      fi
    done
  fi

  # If we're skipping the build, then we just want to upload
  # the images we already have locally.
  if [ -z $SKIP_BUILD ]; then
    archive=${name}
    echo "Building $image:$TAG ..."
    $NIX_BUILD --out-link $archive-image -A images.$BUILD_TYPE.$archive --arg allInOne "$ALL_IN_ONE" --arg incremental "$INCREMENTAL"
    $DOCKER load -i $archive-image
    $RM $archive-image
    if [ "$image" != "$image_basename" ]; then
      echo "Renaming $image_basename:$TAG to $image:$TAG"
      $DOCKER tag "${image_basename}:$TAG" "$image:$TAG"
      $DOCKER image rm "${image_basename}:$TAG"
    fi
  fi
  UPLOAD="$UPLOAD $image"
done

if [ -n "$UPLOAD" ] && [ -z "$SKIP_PUBLISH" ]; then
  # Upload them
  for img in $UPLOAD; do
    if [ -z "$REGISTRY" ] && [ -n "$CI" ] && dockerhub_tag_exists $image $TAG; then
      echo "Skipping $image:$TAG that already exists"
      continue
    fi
    echo "Uploading $img:$TAG to registry ..."
    $DOCKER push $img:$TAG
  done

  if [ -n "$alias_tag" ]; then
    for img in $UPLOAD; do
      echo "Uploading $img:$alias_tag to registry ..."
      $DOCKER tag $img:$TAG $img:$alias_tag
      $DOCKER push $img:$alias_tag
    done
  fi
fi

$DOCKER image prune -f
