#!/usr/bin/env bash

# Generate a CycloneDX SBOM for a built kubectl plugin binary.
#
# The binary is scanned rather than its nix closure: cargo vendors the crates
# into a single derivation, so a closure scan lists the C libraries and not one
# of the rust dependencies, which is where the advisories are.
#
# What makes scanning the binary work is cargo-auditable, which records the
# crates it was built from in the binary itself - see auditableBuild in
# nix/lib/rust.nix. Without that syft still writes a perfectly valid SBOM, just
# an empty one, so the crate count is checked rather than assumed.

set -euo pipefail

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$(realpath "$SCRIPT_DIR/../..")"

# shellcheck source-path=SCRIPTDIR source=../utils/log.sh
source "$SCRIPT_DIR/../utils/log.sh"

NIX_SOURCES="$ROOT_DIR/nix/sources.nix"
SYFT=${SYFT:-"syft"}
JQ=${JQ:-"jq"}

BINARY=
OUTPUT=
ALLOW_EMPTY=

nix_experimental() {
  if (nix eval 2>&1 || true) | grep "extra-experimental-features" 1>/dev/null; then
    echo -n " --extra-experimental-features nix-command "
  else
    echo -n " "
  fi
}

# Take the tool from the pinned nixpkgs, so there is nothing to install on the
# runner and every platform scans with the same version. This is what the
# release script's fetch_nix_bin does; it is not reused by sourcing that script
# because it insists on a docker-compatible CLI before it defines anything, and
# the macOS runners this also runs on have none.
fetch_nix_bin() {
  local package="$1"
  local bin="$2"

  [ -f "$NIX_SOURCES" ] || log_fatal "$bin binary missing and no $NIX_SOURCES to fetch it from"
  # shellcheck disable=SC2046 # the flags must word-split
  nix shell --impure $(nix_experimental) \
    --expr "(import (import $NIX_SOURCES).nixpkgs { }).$package" \
    -c bash -c "type -P $bin"
}

binary_check() {
  "$1" "${2:-"--version"}" &>/dev/null
}

help() {
  cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Options:
  -h, --help                Display this text.
  --binary        <path>    The plugin binary to scan.
  --output        <path>    Where to write the CycloneDX SBOM.
  --allow-empty             Warn rather than fail when the SBOM has no crates,
                            for a target which cannot carry the audit data.

Examples:
  $(basename "$0") --binary result/bin/kubectl-mayastor \\
    --output kubectl-mayastor-x86_64-linux-musl.cdx.json
EOF
}

while [ "$#" -gt 0 ]; do
  case $1 in
    -h|--help)
      help
      exit 0
      ;;
    --binary)
      shift
      BINARY=${1:-}
      ;;
    --output)
      shift
      OUTPUT=${1:-}
      ;;
    --allow-empty)
      ALLOW_EMPTY="yes"
      ;;
    *)
      help
      log_fatal "Unknown option: $1"
      ;;
  esac
  shift
done

[ -n "$BINARY" ] || { help; log_fatal "--binary is required"; }
[ -n "$OUTPUT" ] || { help; log_fatal "--output is required"; }
[ -f "$BINARY" ] || log_fatal "No such binary: $BINARY"

NIX_SOURCES=$(realpath "$NIX_SOURCES")
binary_check "$SYFT" || SYFT=$(fetch_nix_bin "syft" "syft")
binary_check "$JQ" || JQ=$(fetch_nix_bin "jq" "jq")

log "Generating the SBOM of $BINARY ..."
$SYFT scan "file:$BINARY" -o cyclonedx-json="$OUTPUT" -q

crates=$($JQ '[ .components[]? | select((.purl // "") | startswith("pkg:cargo/")) ] | length' "$OUTPUT")
log "$OUTPUT: $crates crate(s)"

if [ "$crates" -eq 0 ]; then
  msg="$OUTPUT lists no crates: $BINARY carries no cargo-auditable data"
  [ -n "$ALLOW_EMPTY" ] || log_fatal "$msg"
  log_warn "$msg"
fi
