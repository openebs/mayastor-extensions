#!/usr/bin/env bash

# Sign the kubectl plugin bundle pushed by kubectl-oci.sh and attach the
# per-target SBOMs to it as in-toto attestations.
#
# Both are attached in the registry as OCI 1.1 referrers of the bundle's
# manifest, addressed by digest rather than by tag, exactly as done for the
# container images. Signing is keyless, so this needs a workflow with
# "id-token: write" and a registry login cosign can pick up.
#
# The cosign calls themselves come from the dependencies' release.sh rather than
# being made here: which flags are needed depends on the cosign version, and one
# repo has no business holding two answers to that. Only its helpers are used -
# sourcing it defines variables and functions, the work is driven by common_run,
# which is not called.

set -euo pipefail

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$(realpath "$SCRIPT_DIR/../..")"
SOURCE_REL="$ROOT_DIR/dependencies/control-plane/utils/dependencies/scripts/release.sh"

# shellcheck source-path=SCRIPTDIR source=../utils/log.sh
source "$SCRIPT_DIR/../utils/log.sh"

PLUGIN="${PLUGIN:-"mayastor"}"
OCI_TAG=
NAMESPACE=
OCI_REGISTRY="ghcr.io"
SBOM_DIR="artifacts"

help() {
  cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Options:
  -h, --help                    Display this text.
  --tag           <tag>         The tag the bundle was pushed with (required).
  --namespace     <namespace>   Namespace path of the bundle (required).
  --registry      <registry>    The registry the bundle was pushed to [default: $OCI_REGISTRY].
  --sbom-dir      <dir>         Where the per-target SBOMs are [default: $SBOM_DIR].
  --dry-run                     Output the cosign calls which would be made.

Examples:
  $(basename "$0") --tag v2.10.0 --namespace openebs/mayastor/dev/plugin
EOF
}

DRY_RUN_ARG=
while [ "$#" -gt 0 ]; do
  case $1 in
    -h|--help)
      help
      exit 0
      ;;
    --tag)
      shift
      OCI_TAG=${1:-}
      ;;
    --namespace)
      shift
      NAMESPACE=${1:-}
      ;;
    --registry)
      shift
      OCI_REGISTRY=${1:-}
      ;;
    --sbom-dir)
      shift
      SBOM_DIR=${1:-}
      ;;
    --dry-run)
      DRY_RUN_ARG="yes"
      ;;
    *)
      help
      log_fatal "Unknown option: $1"
      ;;
  esac
  shift
done

[ -n "$OCI_TAG" ] || { help; log_fatal "--tag is required"; }
[ -n "$NAMESPACE" ] || { help; log_fatal "--namespace is required"; }
[ -d "$SBOM_DIR" ] || log_fatal "No such SBOM directory: $SBOM_DIR"

# Resolved before sourcing, which changes directory to the repo root.
SBOM_DIR="$(realpath "$SBOM_DIR")"
REPOSITORY="$OCI_REGISTRY/$NAMESPACE/kubectl-$PLUGIN"

sboms=("$SBOM_DIR"/*.cdx.json)
[ -e "${sboms[0]}" ] || log_fatal "No SBOMs (*.cdx.json) in $SBOM_DIR"

# Nothing is built here, so the tools the release script wants for that are not
# needed; the ones it does need are fetched from the pinned nixpkgs by its own
# checks below.
# shellcheck disable=SC2034 # read by the sourced release script
IMAGES=""
# shellcheck disable=SC2034 # read by the sourced release script
COMMON_BINS="nix"
# shellcheck source=/dev/null
. "$SOURCE_REL"

# ATTEST is what enables the cosign check, and with it the helpers used here.
# shellcheck disable=SC2034 # read by the sourced release script
ATTEST="yes"
# shellcheck disable=SC2034 # read by the sourced release script
DRY_RUN="$DRY_RUN_ARG"
cosign_check

# The digest is resolved with oras rather than with the release script's
# image_digest: the bundle is not an image, and skopeo refuses to inspect an
# artifact whose type it does not know. Signing a digest rather than the tag is
# the point of it - a tag can be moved after the fact.
digest=$("$ORAS" resolve "$REPOSITORY:$OCI_TAG") || log_fatal "Failed to resolve the digest of $REPOSITORY:$OCI_TAG"
[ -n "$digest" ] || log_fatal "Empty digest for $REPOSITORY:$OCI_TAG"

if already_signed "$REPOSITORY@$digest"; then
  log "Skipping $REPOSITORY@$digest which is already signed"
  exit 0
fi

sign_ref "$REPOSITORY@$digest"

# One attestation per target: each SBOM describes a single binary, so a single
# one of them cannot stand for the bundle as a whole.
for sbom in "${sboms[@]}"; do
  attest_ref "$REPOSITORY@$digest" "$sbom"
done
