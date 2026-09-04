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

BINARY=
OUTPUT=
ALLOW_EMPTY=

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

# syft and jq are pulled from the pinned nixpkgs, the same way the release
# script fetches the tools it needs, so there is nothing to install on the
# runner and every platform gets the same versions.
if [ -z "${IN_NIX_TOOLS_SHELL:-}" ] && ! { command -v syft && command -v jq; } >/dev/null 2>&1; then
  export IN_NIX_TOOLS_SHELL="yes"
  exec nix shell --impure --extra-experimental-features nix-command \
    --expr "let pkgs = import (import $ROOT_DIR/nix/sources.nix).nixpkgs { }; in [ pkgs.syft pkgs.jq ]" \
    --command "$(realpath "${BASH_SOURCE[0]:-"$0"}")" \
    --binary "$BINARY" --output "$OUTPUT" ${ALLOW_EMPTY:+--allow-empty}
fi

log "Generating the SBOM of $BINARY ..."
syft scan "file:$BINARY" -o cyclonedx-json="$OUTPUT" -q

crates=$(jq '[ .components[]? | select((.purl // "") | startswith("pkg:cargo/")) ] | length' "$OUTPUT")
log "$OUTPUT: $crates crate(s)"

if [ "$crates" -eq 0 ]; then
  msg="$OUTPUT lists no crates: $BINARY carries no cargo-auditable data"
  [ -n "$ALLOW_EMPTY" ] || log_fatal "$msg"
  log_warn "$msg"
fi
