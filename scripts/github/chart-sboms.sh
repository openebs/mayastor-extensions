#!/usr/bin/env bash

# Collect the SBOMs of everything a Helm chart release is made of: the container
# images listed in the chart's "helm.sh/images" annotation, and the kubectl
# plugin bundles published alongside it.
#
# Kind first, then platform, so that everything for one platform is a pair of
# self-contained trees and neither the kinds nor the platforms are conflated
# when the SBOMs are read. Who published an artefact is a property of the
# artefact, reported in the table, not a place its document belongs:
#
#   <out>/containers/summary.json
#   <out>/containers/linux-amd64/summary.json
#   <out>/containers/linux-amd64/sboms/mayastor-io-engine.cdx.json
#   <out>/containers/linux-amd64/sboms/some-third-party.spdx.json
#   <out>/containers/linux-arm64/summary.json
#   <out>/containers/linux-arm64/sboms/mayastor-io-engine.cdx.json
#   <out>/plugins/summary.json
#   <out>/plugins/linux-amd64/summary.json
#   <out>/plugins/linux-amd64/sboms/kubectl-mayastor.cdx.json
#   <out>/plugins/darwin-arm64/summary.json
#   <out>/plugins/darwin-arm64/sboms/kubectl-mayastor.cdx.json
#
# The documents sit in an sboms/ leaf so that a summary never shares a
# directory with the documents it reports on. A platform's summary has its
# rows; a kind's has every row of that kind.
#
# A container is always one of the platforms it advertises: an image with no
# index has its architecture in its config, and a document that came off an
# index describes every platform of that image, so it is written to each. Every
# directory here is a real platform - nothing is filed under a placeholder.
#
# A plugin bundle is one OCI artifact with no platform of its own, so its
# platform directories come from the bundle payload, which carries one SBOM per
# binary named with its target triple, and it is reported one row per document
# rather than one per bundle: each describes a different binary. A document the
# bundle cannot name is reported without a platform, in the kind's summary only,
# rather than filed somewhere meaningless.
#
# The suffix follows the document: .cdx.json or .spdx.json. We publish
# CycloneDX; buildkit-built dependencies come as SPDX, and both are read.
#
# A table of what was found is printed too - coverage is patchy across
# publishers, so the report matters as much as the files.
#
# --verify gates on attestations that exist. One that does not verify against
# the expected identity fails the run; an artefact with no attestation does not,
# because there is nothing to disagree with. Most images are still unattested
# and a gate that is red from its first run gets ignored - --fail-on-missing is
# the strict form for when that changes. The identity defaults to an openebs
# workflow, so a third-party attestation would not match it: --identity-regexp
# widens it when a dependency starts publishing one.
#
# The cosign and oras calls reuse the helpers in the dependencies' release.sh:
# which flags a given cosign version needs is decided in one place only.

set -euo pipefail

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$(realpath "$SCRIPT_DIR/../..")"
SOURCE_REL="$ROOT_DIR/dependencies/control-plane/utils/dependencies/scripts/release.sh"

# shellcheck source-path=SCRIPTDIR source=../utils/log.sh
source "$SCRIPT_DIR/../utils/log.sh"

CHART=
CHART_VERSION=
# Where to create the output directory, not the output directory itself: what
# gets cleared and rewritten is always a directory named below that we made, so
# "--out /home" writes /home/sboms and can never put /home itself at risk.
OUT_BASE="."
OUT_NAME="sboms"
# The architectures this project builds for. Others an index may advertise -
# ppc64le, s390x, arm/v7, windows - are not deployed by this chart, so scanning
# them only adds rows that will never be anything but missing. --platform
# replaces this list, --all-platforms takes whatever the index has.
PLATFORMS=(linux/amd64 linux/arm64)
PLATFORMS_SET=
ALL_PLATFORMS=
# An inventory with holes in it is not much of an inventory, so anything that
# carries no SBOM gets one scanned from the registry. It is generated here and
# attested by nobody, which the report says plainly.
GENERATE="yes"
# Artefacts run concurrently: this is registry and Rekor latency, not CPU work.
# Measured on the full openebs set with --no-generate: 220s at 1, 60s at 4, and
# at 8 an unauthenticated Docker Hub starts throttling, which costs more in
# retries than the concurrency wins. Scanning what has no SBOM roughly triples
# the run at 4. Registries you are logged in to will take more.
JOBS=4
FILTER=
FORMAT="table"
VERIFY=
FAIL_ON_MISSING=
DRY_RUN=
NO_PLUGINS=
OCI_REGISTRY="ghcr.io"
OCI_NAMESPACE=
OCI_TAG=
PLUGIN_NAME="${PLUGIN:-"mayastor"}"
PLUGIN_REFS=()
COSIGN_KEY="${COSIGN_KEY:-}"
INSECURE_REGISTRY=

# Discovery by default: proves the signature is valid and logged, and reports
# who made it. --verify swaps these for the pinned org identity.
IDENTITY_REGEXP=".*"
ISSUER_REGEXP=".*"
ISSUER=
# The certificate subject is the workflow that did the signing, which is not
# the same one everywhere: images are signed by image.yml, while the kubectl
# bundle is signed from staging.yml or release.yml. So the workflow itself is
# left open and what is asserted is the org and that a workflow signed it.
VERIFY_IDENTITY='^https://github\.com/openebs/[^/]+/\.github/workflows/[^/@]+\.yml@'
VERIFY_ISSUER="https://token.actions.githubusercontent.com"
# The artifact types that carry something "cosign verify-attestation" can read:
# a sigstore bundle, a bare DSSE or in-toto statement, or cosign's own oci-1-1
# attestation type. Deliberately not ".sig" - every signed target has one of
# those and it says nothing about SBOMs - and not ".sbom" either, which is what
# "cosign attach sbom" leaves and verify-attestation cannot read.
#
# Matched against a referrer's manifest, not only its entry in the referrers
# listing: a --new-bundle-format attestation is an OCI image manifest with an
# empty config, and the listing reports the config's media type for it,
# "application/vnd.oci.empty.v1+json", the bundle type being visible only in the
# manifest itself. So those are fetched and looked at, one per referrer.
ATTESTATION_TYPES_RE='sigstore\.bundle|in-toto|dsse|cosign\.artifact\.att'
EMPTY_CONFIG_TYPE='application/vnd.oci.empty.v1+json'

help() {
  cat <<EOF
Usage: $(basename "$0") --chart <chart> [OPTIONS]

Collect and report the SBOMs of a chart's images and of the kubectl plugin
bundles released with it.

Options:
  -h, --help                    Display this text.
  --chart      <chart>          Chart directory, Chart.yaml, packaged .tgz, or a
                                repo reference such as "openebs/mayastor".
  --version    <version>        Chart version, for a repo reference.
  --out        <dir>            Directory in which to create the "$OUT_NAME"
                                output directory, not the output directory
                                itself [default: $OUT_BASE, giving ./$OUT_NAME].
                                It is cleared and rewritten on every run.
  --filter     <glob>           Only consider artefacts matching this.
  --platform   <os/arch>        Only this platform, replacing the default pair.
                                Repeatable. Named without a variant it matches
                                every variant, so linux/arm64 takes
                                linux/arm64/v8 too
                                [default: ${PLATFORMS[*]}].
  --all-platforms               Every platform an index advertises, including
                                the ones this chart never deploys.
  --no-generate                 Do not scan artefacts that carry no SBOM.
                                By default one is generated with syft and
                                reported as "generated", never as verified.
  --jobs       <n>              Artefacts to process at once [default: $JOBS].
                                1 makes the run strictly sequential.
  --registry   <host>           Registry the plugin bundle was pushed to
                                [default: $OCI_REGISTRY].
  --namespace  <namespace>      Namespace of the plugin bundle. With --tag, the
                                bundle reference is derived from these.
  --tag        <tag>            Tag of the plugin bundle.
  --plugin     <ref>            Explicit plugin bundle reference. Repeatable,
                                and overrides the derived one.
  --no-plugins                  Only look at the chart's images.
  --verify                      Require every attestation that exists to verify
                                against the expected identity, by default an
                                openebs workflow. An artefact with no
                                attestation is reported, not failed - use
                                --fail-on-missing for that. Widen the identity
                                with --identity-regexp.
  --key        <path>           Verify with this cosign public key rather than by
                                certificate identity, for artefacts signed with
                                a key instead of keylessly.
  --insecure-registry           Talk to the registry over plain HTTP.
  --identity-regexp <re>        Certificate identity to require.
  --issuer     <url>            Certificate OIDC issuer to require.
  --fail-on-missing             Also fail on anything with no verified SBOM:
                                none at all, an unsigned one, or one this run
                                generated.
  --dry-run                     Report which artefacts and targets would be
                                looked at and where each would be written,
                                then stop. Reads manifests only: no SBOM is
                                fetched, verified or written, and the output
                                directory is left alone.
  --format     table|json       Output format [default: $FORMAT]. With "json"
                                stdout is only the document; progress goes to
                                standard error.

Note that without --verify, and without an explicit --identity-regexp, any
valid signature is accepted whoever made it: the default reports who signed
something, it does not decide whether they should have.

Examples:
  $(basename "$0") --chart ./chart
  $(basename "$0") --chart ./chart --verify --namespace openebs/mayastor/dev/plugin --tag v2.12.0
  $(basename "$0") --chart openebs/mayastor --version 2.12.0 --out ./release-sboms
EOF
}

while [ "$#" -gt 0 ]; do
  case $1 in
    -h|--help)
      help
      exit 0
      ;;
    --chart)
      shift; CHART=${1:-}
      ;;
    --version)
      shift; CHART_VERSION=${1:-}
      ;;
    --out)
      shift; OUT_BASE=${1:-}
      ;;
    --filter)
      shift; FILTER=${1:-}
      ;;
    --platform)
      shift
      # The first --platform replaces the default pair; further ones add to it.
      if [ -z "$PLATFORMS_SET" ]; then
        PLATFORMS=()
        PLATFORMS_SET="yes"
      fi
      PLATFORMS+=("${1:-}")
      ;;
    --registry)
      shift; OCI_REGISTRY=${1:-}
      ;;
    --namespace)
      shift; OCI_NAMESPACE=${1:-}
      ;;
    --tag)
      shift; OCI_TAG=${1:-}
      ;;
    --plugin)
      shift; PLUGIN_REFS+=("${1:-}")
      ;;
    --all-platforms)
      ALL_PLATFORMS="yes"
      ;;
    --no-generate)
      GENERATE=
      ;;
    --jobs)
      shift; JOBS=${1:-}
      ;;
    --no-plugins)
      NO_PLUGINS="yes"
      ;;
    --verify)
      VERIFY="yes"
      ;;
    --identity-regexp)
      shift; IDENTITY_REGEXP=${1:-}
      ;;
    --key)
      shift; COSIGN_KEY=${1:-}
      ;;
    --insecure-registry)
      INSECURE_REGISTRY="yes"
      ;;
    --issuer)
      shift; ISSUER=${1:-}
      ;;
    --fail-on-missing)
      FAIL_ON_MISSING="yes"
      ;;
    --dry-run)
      DRY_RUN="yes"
      ;;
    --format)
      shift; FORMAT=${1:-}
      ;;
    *)
      log_fatal "Unknown option: $1"
      ;;
  esac
  shift
done

[ -n "$CHART" ] || { help; log_fatal "--chart is required"; }
case "$FORMAT" in
  table|json) ;;
  *) log_fatal "Unknown --format '$FORMAT', expected 'table' or 'json'" ;;
esac
case "$JOBS" in
  ""|*[!0-9]*) log_fatal "--jobs takes a number, got '$JOBS'" ;;
  0) log_fatal "--jobs must be at least 1" ;;
esac

if [ -n "$VERIFY" ]; then
  # An explicit identity still wins, so that a single repo can be pinned.
  [ "$IDENTITY_REGEXP" = ".*" ] && IDENTITY_REGEXP="$VERIFY_IDENTITY"
  [ -z "$ISSUER" ] && ISSUER="$VERIFY_ISSUER"
fi

# Resolved before release.sh is sourced, because it cd's to its own parent root.
CHART_ARG="$CHART"
[ -e "$CHART" ] && CHART_ARG="$(realpath "$CHART")"
OUT_DIR="$(realpath -m "$OUT_BASE")/$OUT_NAME"

# A stale tree is worse than no tree: run once for every arch and again with
# --platform or --filter, and last run's files sit there looking current. So the
# output is cleared first - but only when it is recognisably ours, because
# --out is whatever the caller passed and rm -rf on the wrong thing is not
# recoverable. A run that died before writing its summary still leaves the
# marker, so it can be cleared too.
MARKER=".chart-sboms"
if [ -e "$OUT_DIR" ] && [ ! -d "$OUT_DIR" ]; then
  log_fatal "--out $OUT_DIR exists and is not a directory"
fi
CLEAR_NOTE="$OUT_DIR does not exist yet"
if [ -d "$OUT_DIR" ] && [ -n "$(ls -A "$OUT_DIR" 2>/dev/null)" ]; then
  if [ -e "$OUT_DIR/$MARKER" ]; then
    CLEAR_NOTE="$OUT_DIR would be cleared: it was written by a previous run"
    [ -n "$DRY_RUN" ] || rm -rf "$OUT_DIR"
  else
    # Fatal in a dry run too: better to say now that the real run cannot start.
    log_fatal "--out $OUT_DIR is not empty and was not written by this script; remove it or pick another directory"
  fi
fi
if [ -z "$DRY_RUN" ]; then
  mkdir -p "$OUT_DIR"
  : > "$OUT_DIR/$MARKER"
fi

# release.sh checks for the binaries it needs at source time and cd's to the
# project root. Neither is wanted here - this reads registries and touches no
# build - so the check is narrowed to what is actually used and the working
# directory restored afterwards.
CWD="$PWD"
# release.sh reads the git tag and branch at source time, so it is sourced from
# its own directory: this script may be pointed at a chart anywhere.
# release.sh sets its own defaults for several of these variables, so the values
# parsed above are kept and put back once it has been sourced.
# Every name this script parses that release.sh also assigns has to be listed
# here, or sourcing it silently discards what the caller asked for: --dry-run
# did nothing at all until DRY_RUN was added. To check for others:
#   grep -E '^\s*(export +)?(NAME)=' release.sh
OPT_NAMES=(COSIGN_KEY INSECURE_REGISTRY DRY_RUN)
declare -A OPT_SAVED=()
for opt in "${OPT_NAMES[@]}"; do
  OPT_SAVED["$opt"]="${!opt}"
done
cd "$(dirname "$SOURCE_REL")"
# shellcheck disable=SC2034 # read by the sourced release script
IMAGES=""
# shellcheck disable=SC2034 # read by the sourced release script
COMMON_BINS="nix"
# shellcheck source=/dev/null
source "$SOURCE_REL"
cd "$CWD"
for opt in "${OPT_NAMES[@]}"; do
  declare -g "$opt=${OPT_SAVED[$opt]}"
done

# shellcheck disable=SC2034 # read by the sourced release script
# ATTEST is what makes cosign_check resolve the tools; nothing is signed here.
ATTEST="yes"
cosign_check
# Same trick for syft, which sbom_check resolves.
# shellcheck disable=SC2034 # read by the sourced release script
SBOM="$GENERATE"
sbom_check
# crane is not one of release.sh's tools, so it is resolved the same way here.
CRANE=${CRANE:-"crane"}
CRANE_TLS=${INSECURE_REGISTRY:+--insecure}
if ! binary_check "$CRANE" "version"; then
  CRANE=$(fetch_nix_bin "crane" "crane")
fi
# release.sh only resolves yq when it has helm dependencies to update, which is
# never the case here, and the chart annotation is read with it.
if ! binary_check "$YQ"; then
  YQ=$(fetch_nix_bin "yq-go" "yq")
fi

# None of these tools has a timeout of its own, and a registry that accepts the
# connection and then stalls would hang the run indefinitely - in CI until the
# job limit, with no output to say which artefact it died on. Wrapping the
# resolved binaries keeps one stuck request from taking the run with it: the
# call just fails, and the retry in cosign_sboms decides what that means.
# Verification pulls a multi-megabyte document and talks to Rekor, so it gets
# considerably longer than a manifest lookup.
COSIGN="timeout 180 $COSIGN"
CRANE="timeout 60 $CRANE"
ORAS="timeout 60 $ORAS"

# Progress and totals are for a person to read. With --format json the only
# thing on stdout is the document, so they go to standard error instead - the
# shared log() writes to stdout, which would otherwise corrupt the output.
note() {
  if [ "$FORMAT" = "json" ]; then
    log_to_stderr "$1"
  else
    log "$1"
  fi
}

# Resolve the chart argument to a Chart.yaml on disk. One extracted or fetched
# here is a temporary file, removed on exit - alongside release.sh's own cleanup,
# whose trap this replaces.
CHART_TMP=
trap 'cleanup; [ -z "$CHART_TMP" ] || rm -f "$CHART_TMP"' EXIT
chart_yaml() {
  local chart="$1" tmp
  if [ -d "$chart" ]; then
    echo -n "$chart/Chart.yaml"
  elif [ -f "$chart" ] && [[ "$chart" == *.tgz || "$chart" == *.tar.gz ]]; then
    tmp=$(mktemp -t chart-XXXXXX.yaml)
    CHART_TMP="$tmp"
    $TAR -xzOf "$chart" --wildcards '*/Chart.yaml' > "$tmp" 2>/dev/null \
      || log_fatal "No Chart.yaml inside $chart"
    echo -n "$tmp"
  elif [ -f "$chart" ]; then
    echo -n "$chart"
  else
    # Not a path, so a repo reference: ask helm for it.
    binary_check "$HELM" "version" || HELM=$(fetch_nix_bin "kubernetes-helm-wrapped" "helm")
    tmp=$(mktemp -t chart-XXXXXX.yaml)
    CHART_TMP="$tmp"
    local args=(show chart "$chart")
    [ -n "$CHART_VERSION" ] && args+=(--version "$CHART_VERSION")
    $HELM "${args[@]}" > "$tmp" || log_fatal "Failed to fetch the chart $chart"
    echo -n "$tmp"
  fi
}

# The images a chart deploys, as "<name> <reference>" lines. The annotation is a
# YAML document inside a YAML string, hence the second parse.
chart_images() {
  local yaml="$1"
  $YQ -r '.annotations."helm.sh/images"' "$yaml" 2>/dev/null \
    | $YQ -r '.[] | [.name, .image] | @tsv' - 2>/dev/null \
    || log_fatal "Could not read the helm.sh/images annotation from $yaml"
}

# The plugin bundle references: explicit ones if given, otherwise derived the
# same way kubectl-oci.sh builds its repository.
plugin_refs() {
  if [ -n "$NO_PLUGINS" ]; then
    return 0
  fi
  if (( ${#PLUGIN_REFS[*]} )); then
    printf '%s\n' "${PLUGIN_REFS[@]}"
    return 0
  fi
  if [ -n "$OCI_NAMESPACE" ] && [ -n "$OCI_TAG" ]; then
    echo "$OCI_REGISTRY/$OCI_NAMESPACE/kubectl-$PLUGIN_NAME:$OCI_TAG"
  fi
}

CHART_YAML="$(chart_yaml "$CHART_ARG")"
[ -f "$CHART_YAML" ] || log_fatal "No chart metadata at $CHART_YAML"

note "Reading $CHART_YAML"
ARTEFACTS=()
while IFS=$'\t' read -r aname ref; do
  [ -n "${ref:-}" ] || continue
  # shellcheck disable=SC2053 # --filter is a glob, so glob matching is the point
  if [ -n "$FILTER" ] && [[ "$ref" != $FILTER ]]; then
    continue
  fi
  ARTEFACTS+=("$aname"$'\t'"$ref")
done < <(chart_images "$CHART_YAML")

while read -r ref; do
  [ -n "${ref:-}" ] || continue
  # shellcheck disable=SC2053 # --filter is a glob, so glob matching is the point
  if [ -n "$FILTER" ] && [[ "$ref" != $FILTER ]]; then
    continue
  fi
  # The repository name without its tag: a colon in a filename is legal but
  # awkward, and the tag is the same for every plugin here anyway.
  pname="${ref%%@*}"; pname="${pname##*/}"; pname="${pname%%:*}"
  ARTEFACTS+=("$pname"$'\t'"$ref")
done < <(plugin_refs)

(( ${#ARTEFACTS[*]} )) || log_fatal "No artefacts to look at"
note "Found ${#ARTEFACTS[*]} artefact(s)"

# The targets of a reference, as "<platform> <digest>" lines.
#
# For an image index that is each child matching $PLATFORMS, because an SBOM
# describes one architecture and is attached to the manifest for it, not to the
# list. Plus the index itself, for the publishers that attach to that instead.
# For a single manifest, or for an OCI artifact such as a plugin bundle, it is
# the reference's own digest and the platform is reported as "-".
#
# "unknown/unknown" children are skipped: that is how buildx attestation
# manifests appear in an index, and they are not something to scan.
targets_of() {
  local ref="$1" manifest kids digest plat err
  err=$(mktemp -t crane-XXXXXX)

  # Report why rather than just that it failed. A stale registry credential is
  # by far the most likely reason and gives a 401 that is indistinguishable
  # from a missing image unless the message is passed on.
  if ! manifest=$($CRANE $CRANE_TLS manifest "$ref" 2>"$err"); then
    [ -s "$err" ] && log_warn "$(head -1 "$err")"
    rm -f "$err"
    return 1
  fi
  rm -f "$err"
  # The variant is part of the platform: linux/arm/v6 and linux/arm/v7 are two
  # different children, and without it they collide on one row and one filename.
  #
  # arm64/v8 is the exception. v8 is the baseline for arm64 rather than a
  # separate architecture, and publishers spell the same thing both ways - the
  # chart has images of each kind. Dropping it here, at the one place platforms
  # are named, keeps a single spelling everywhere downstream: one row per arch
  # in the table, one key in the summary, one directory on disk.
  kids=$(echo "$manifest" | $JQ -r '
    [ .manifests[]? | select(.platform.os != null and .platform.architecture != "unknown") ]
    | .[] | "\(.platform.os)/\(.platform.architecture)\(
        if .platform.variant and (.platform.architecture + "/" + .platform.variant) != "arm64/v8"
        then "/\(.platform.variant)" else "" end) \(.digest)"' 2>/dev/null || true)

  if [ -n "$kids" ]; then
    if [ -n "$ALL_PLATFORMS" ]; then
      echo "$kids"
    else
      # A platform named without a variant matches every variant of it, so
      # linux/arm64 takes linux/arm64/v8 as well.
      for want in "${PLATFORMS[@]}"; do
        grep -E "^${want}(/[^ ]+)? " <<< "$kids" || true
      done
    fi
    # Always, whatever the platforms asked for: the index is not specific to any
    # of them. Kept only if it turns up something the children did not have.
    echo "index $($CRANE $CRANE_TLS digest "$ref" 2>/dev/null)"
    return 0
  fi

  digest=$($CRANE $CRANE_TLS digest "$ref" 2>/dev/null) || return 1
  # No index, but a container still has an architecture - it is just in the
  # config rather than advertised in a list. Reading it there is what keeps
  # single-manifest images out of a platformless bucket. An OCI artifact such as
  # a plugin bundle has no config to read, and reports "-".
  plat=$($CRANE $CRANE_TLS config "$ref" 2>/dev/null | $JQ -r '
    if (.os // "") != "" and (.architecture // "") != "" then
      "\(.os)/\(.architecture)" +
      (if .variant and (.architecture + "/" + .variant) != "arm64/v8" then "/\(.variant)" else "" end)
    else "-" end' 2>/dev/null || true)
  echo "${plat:--} $digest"
}

# Write every CycloneDX or SPDX attestation cosign can verify on the reference
# into $1, one file per attestation.
#
# A plugin bundle carries one attestation per platform binary, so several
# payloads can come back for a single digest; images carry one per target.
cosign_sboms() {
  local dir="$1" ref="$2" known="${3:-}" kind n=0
  local verify_args=()

  if [ -n "$INSECURE_REGISTRY" ]; then
    verify_args+=(--allow-insecure-registry --insecure-ignore-tlog)
  fi
  if [ -n "$COSIGN_KEY" ]; then
    # A key says who signed it directly, so the certificate identity does not
    # apply - there is no certificate.
    verify_args+=(--key "$COSIGN_KEY")
  else
    if [ -n "$ISSUER" ]; then
      verify_args+=(--certificate-oidc-issuer "$ISSUER")
    else
      verify_args+=(--certificate-oidc-issuer-regexp "$ISSUER_REGEXP")
    fi
    verify_args+=(--certificate-identity-regexp "$IDENTITY_REGEXP")
  fi

  local -a bundle=()
  bundle_args bundle
  # An image SBOM runs to megabytes, and none of it goes through a shell
  # variable. nix-shell exports $out and $name, a local of either name inherits
  # that export, and a payload that large in the environment makes every command
  # after it fail to exec with E2BIG - which looks exactly like an image having
  # no SBOM at all. So both names are avoided here, and the payloads are
  # streamed: one DSSE envelope per line in, one compact document per line out,
  # then split into a file each.
  local raw="$dir/envelopes" flat="$dir/payloads" err="$dir/stderr" attempt
  local transient definitive unanswered=""
  # CycloneDX first: that is what we publish, so the common case costs one
  # call. SPDX is still read - buildkit emits it, and we may move to it later.
  for kind in cyclonedx spdxjson; do
    # Per kind: whether *this* question got an answer. One kind timing out and
    # the other answering "nothing here" is still a target that could not be
    # fully asked, so the outcome is remembered across kinds below.
    transient=""; definitive=""
    for attempt in 1 2 3; do
      $COSIGN verify-attestation --type "$kind" "${bundle[@]}" "${verify_args[@]}" "$ref" > "$raw" 2>"$err" && break
      $COSIGN verify-attestation --type "$kind" "${verify_args[@]}" "$ref" > "$raw" 2>"$err" && break
      : > "$raw"
      # "No matching attestations" is an answer, and so is a registry saying the
      # tag does not exist. A timeout, a 429 or a 5xx is a failure to *ask*,
      # which is a different thing: the caller treats an attestation that would
      # not verify as a gate failure, and an infrastructure hiccup must not be
      # reported as one. So anything that is not a plain answer is retried with
      # a little backoff, and if it still will not answer the caller is told
      # that rather than being left to guess.
      if grep -qiE 'no matching attestations|no matching signatures|no signatures found|no attestations found|MANIFEST_UNKNOWN' "$err"; then
        definitive="yes"
        # Except when the registry has already listed an attestation referrer.
        # Then cosign and the registry disagree, and a throttled response wears
        # the same words as "your identity does not match" - so ask again rather
        # than take the first one at its word.
        [ -n "$known" ] || break
      else
        definitive=""
      fi
      transient="yes"
      [ "$attempt" -eq 3 ] && break
      sleep $((attempt * 3))
    done
    if [ -n "$transient" ] && [ -z "$definitive" ]; then
      unanswered="yes"
    fi
    [ -s "$raw" ] || continue
    $JQ -r '.payload | @base64d | fromjson | tojson' "$raw" > "$flat" 2>/dev/null || continue
    [ -s "$flat" ] || continue
    split -l 1 -d -a 3 --additional-suffix=.json "$flat" "$dir/doc" 2>/dev/null || continue
    n=$(find "$dir" -maxdepth 1 -name 'doc*.json' | wc -l)
    if (( n )); then
      rm -f "$raw" "$flat" "$err"
      return 0
    fi
  done
  rm -f "$raw" "$flat" "$err"
  # 1 says "asked, and there is nothing here that matches"; 2 says "could not
  # be asked". A definitive answer, even after retrying, is still an answer -
  # the caller can act on it. But it takes one for each kind: if either never
  # got past the registry or Rekor, there may be an attestation of that kind we
  # simply did not hear about, and saying "nothing here" would be a guess.
  [ -n "$unanswered" ] && return 2
  return 1
}

# Fall back to the older tag layout: cosign used to attach an attestation as a
# "sha256-<digest>.att" tag rather than as a referrer. Unverified - the point of
# reading it is that we cannot verify it - so callers must report it as such.
legacy_sboms() {
  local dir="$1" repo="$2" digest="$3" tag layer n=0

  tag="${digest/:/-}.att"
  $CRANE $CRANE_TLS manifest "$repo:$tag" >/dev/null 2>&1 || return 1
  while read -r layer; do
    [ -n "$layer" ] || continue
    $CRANE $CRANE_TLS blob "$repo@$layer" 2>/dev/null \
      | $JQ -r '.payload' 2>/dev/null \
      | base64 -d > "$dir/att-$n.json" 2>/dev/null || continue
    n=$((n + 1))
  done < <($CRANE $CRANE_TLS manifest "$repo:$tag" 2>/dev/null | $JQ -r '.layers[]?.digest' 2>/dev/null)
  (( n )) || return 1
}

# And the buildx layout: attestations live inside the index as manifests
# annotated "vnd.docker.reference.type=attestation-manifest", whose layers carry
# an "in-toto.io/predicate-type". Provenance is ignored - it says how the image
# was built, not what is in it - and these are unsigned either way.
#
# The attestation is not attached to the child it describes: it is a separate
# entry in the index, tied back to the child by vnd.docker.reference.digest. So
# the index is what has to be read, and the entry for this child picked out of
# it. An empty child means the target is the index itself, so every entry counts.
buildx_sboms() {
  local dir="$1" repo="$2" index="$3" child="$4" am layer n=0

  while read -r am; do
    [ -n "$am" ] || continue
    while read -r layer; do
      [ -n "$layer" ] || continue
      $CRANE $CRANE_TLS blob "$repo@$layer" 2>/dev/null > "$dir/bx-$n.json" || continue
      n=$((n + 1))
    done < <($CRANE $CRANE_TLS manifest "$repo@$am" 2>/dev/null \
      | $JQ -r '.layers[]? | select((.annotations."in-toto.io/predicate-type" // "") | test("spdx|cyclonedx"; "i")) | .digest' 2>/dev/null)
  done < <($CRANE $CRANE_TLS manifest "$index" 2>/dev/null \
    | $JQ -r --arg child "$child" '.manifests[]?
        | select(.annotations."vnd.docker.reference.type" == "attestation-manifest")
        | select($child == "" or .annotations."vnd.docker.reference.digest" == $child)
        | .digest' 2>/dev/null)
  (( n )) || return 1
}

# One referrers query per target, into $2, as {"manifests": [...]} whichever
# way oras spells it - 1.2 says .manifests, 1.3 says .referrers. DISCOVER_OK says
# whether the registry actually answered: registry.k8s.io serves no referrers API
# at all, and "no answer" must never be read as "nothing attached".
discover_refs() {
  local ref="$1" refs="$2" raw

  DISCOVER_OK=""
  echo '{}' > "$refs"
  raw=$($ORAS discover ${INSECURE_REGISTRY:+--plain-http} --format json "$ref" 2>/dev/null) || return 0
  $JQ '{manifests: (.referrers // .manifests)}' <<< "$raw" > "$refs" 2>/dev/null || { echo '{}' > "$refs"; return 0; }
  $JQ -e '.manifests' "$refs" >/dev/null 2>&1 || return 0
  DISCOVER_OK="yes"
}

# Whether cosign is worth asking about this target. Establishing that nothing is
# attached costs four cosign invocations - two predicate types, each tried with
# and without --new-bundle-format, every one of them re-initialising the Sigstore
# trust root - and most of a chart's targets have nothing attached. The referrers
# list already fetched answers it for free.
#
# Returns 0 whenever cosign should be asked, and that includes every case where
# the answer is not knowable: if the registry did not answer, or served no
# referrers API, skipping would report an attested image as having no SBOM. Only
# a positive "the registry listed its referrers and none of them is an
# attestation, and there is no legacy tag either" skips the calls.
# Whether the referrers list shows something cosign would treat as an
# attestation. Used both to decide whether to ask cosign at all, and to know
# when cosign disagreeing with the registry is worth asking again.
#
# First from the listing alone. Then, for every referrer the listing types as an
# empty config - which is all it says about a --new-bundle-format attestation -
# from the referrer's own manifest: its artifactType, its layer types and the
# predicate type cosign annotates it with. A manifest that cannot be fetched is
# taken to be one, since the cost of asking cosign needlessly is a few calls and
# the cost of not asking is an attested image reported as having no SBOM.
has_attestation_referrer() {
  local repo="$1" refs="$2" d

  $JQ -e --arg re "$ATTESTATION_TYPES_RE" '[.manifests[]?.artifactType // ""]
      | any(test($re))' "$refs" >/dev/null 2>&1 && return 0
  while read -r d; do
    [ -n "$d" ] || continue
    $CRANE $CRANE_TLS manifest "$repo@$d" 2>/dev/null \
      | $JQ -e --arg re "$ATTESTATION_TYPES_RE" '
          [ .artifactType // "",
            (.layers[]?.mediaType // ""),
            (.annotations."dev.sigstore.bundle.predicateType" // "") ]
          | any(test($re))' >/dev/null 2>&1 && return 0
    # Nothing readable came back: not knowable, so ask.
    [ "${PIPESTATUS[0]}" -eq 0 ] || return 0
  done < <($JQ -r --arg t "$EMPTY_CONFIG_TYPE" '.manifests[]?
      | select((.artifactType // "") == $t) | .digest' "$refs" 2>/dev/null)
  return 1
}

# $3 is has_attestation_referrer's answer for this target.
worth_asking_cosign() {
  local repo="$1" digest="$2" known="$3"

  [ -n "$DISCOVER_OK" ] || return 0
  [ -n "$known" ] && return 0
  # No referrer cosign would use, so the only thing left is the older tag layout.
  $CRANE $CRANE_TLS manifest "$repo:${digest/:/-}.att" >/dev/null 2>&1 && return 0
  return 1
}

# Whether anything at all has a verifiable attestation here, with the identity
# left wide open. This separates the two very different things behind cosign
# saying "no matching attestations": either something is genuinely attested by an
# identity we do not accept, which is a gate failure worth naming, or the
# registry would not answer, which is not a verification result at all and must
# not be reported as one. Under load the two are indistinguishable by message
# alone, so the question gets asked directly. Only reached on the failure path,
# so it costs nothing in the normal case.
attested_by_anyone() {
  local ref="$1"
  local -a args=(--certificate-identity-regexp '.*' --certificate-oidc-issuer-regexp '.*')
  local -a bundle=()

  # With a key there is no identity to be lenient about: if the key did not
  # verify it, the signature really does not hold.
  [ -z "$COSIGN_KEY" ] || return 0
  [ -n "$INSECURE_REGISTRY" ] && args+=(--allow-insecure-registry --insecure-ignore-tlog)
  bundle_args bundle
  $COSIGN verify-attestation --type cyclonedx "${bundle[@]}" "${args[@]}" "$ref" >/dev/null 2>&1 && return 0
  $COSIGN verify-attestation --type spdxjson "${bundle[@]}" "${args[@]}" "$ref" >/dev/null 2>&1 && return 0
  return 1
}

# Scan the image straight from the registry when nothing is attached to it. No
# daemon and no pull: syft reads the layers over the wire, a couple of seconds
# for a small image and under ten for a large one.
#
# What comes out is ours, made now, and vouched for by nobody - it says what is
# in the image, not who stands behind it. The caller reports it as "generated"
# so it is never counted as verified.
syft_sbom() {
  local dir="$1" ref="$2"

  [ -n "$GENERATE" ] || return 1
  if [ -n "$INSECURE_REGISTRY" ]; then
    export SYFT_REGISTRY_INSECURE_USE_HTTP=true
    export SYFT_REGISTRY_INSECURE_SKIP_TLS_VERIFY=true
  fi
  $SYFT scan "registry:$ref" -o cyclonedx-json="$dir/generated.json" -q 2>/dev/null || return 1
  [ -s "$dir/generated.json" ] || return 1
}

# One probe for a target that yielded no SBOM, answering both questions at once:
# what is attached, and is it an attestation that simply did not verify?
#
# The distinction is what a gate exists for - something attached that cosign
# would not accept is a failure, whereas nothing attached is an absence, and
# reporting the first as the second would hide it. Everything we attach through
# referrers is an SBOM, so a bundle referrer counts as the former.
#
# Writes the note for the row to stdout, and returns 0 only for the "attached
# but unusable" case. One discover call is shared by every check below.
probe_target() {
  local repo="$1" digest="$2" index="$3" child="$4" refs="$5"

  # The same question worth_asking_cosign asked, so that the two never disagree
  # about what counts as an attestation.
  if $CRANE $CRANE_TLS manifest "$repo:${digest/:/-}.att" >/dev/null 2>&1 \
    || has_attestation_referrer "$repo" "$refs"; then
    echo -n "attestation did not verify"
    return 0
  fi
  if $JQ -e '[.manifests[]?.artifactType // ""] | any(test("cosign|sigstore"))' "$refs" >/dev/null 2>&1 \
    || $CRANE $CRANE_TLS manifest "$repo:${digest/:/-}.sig" >/dev/null 2>&1; then
    echo -n "signed, no SBOM"
    return 1
  fi
  if $CRANE $CRANE_TLS manifest "$index" 2>/dev/null \
    | $JQ -e --arg child "$child" '[.manifests[]?
        | select(.annotations."vnd.docker.reference.type" == "attestation-manifest")
        | select($child == "" or .annotations."vnd.docker.reference.digest" == $child)]
        | length > 0' >/dev/null 2>&1; then
    echo -n "unsigned (buildx provenance)"
    return 1
  fi
  echo -n "-"
  return 1
}

# How many components a CycloneDX or SPDX document describes.
# The directory a platform's documents belong in. Empty for a target with no
# platform - an index, or an artefact that would not resolve - because nothing
# is filed under one: an index document is written to each platform it covers,
# and an unresolved artefact has no document to write. arm64/v8 is already
# normalised to arm64 where platforms are named, so there is nothing to fold
# here; arm/v6 and arm/v7 really are different and stay apart.
arch_dir() {
  case "$1" in
    -|index) ;;
    *)       echo -n "${1//\//-}" ;;
  esac
}

# The platform a plugin document describes, as an <os>-<arch> directory to match
# the container trees. A bundle is one OCI artifact with no platform of its own,
# so this comes from the rust target triple in the name the bundle gave the
# document - x86_64-linux-musl, aarch64-apple-darwin, x86_64-windows-gnu.
#
# Empty when it cannot be read both ways. There is no bucket for that: the
# bundle and the attestations are written from one directory in one job, so a
# document that cannot be placed means something is wrong upstream, and a
# directory of documents that describe nothing in particular helps nobody.
plugin_arch() {
  local name="$1" os="" arch=""

  case "$name" in
    *x86_64*|*amd64*)  arch="amd64" ;;
    *aarch64*|*arm64*) arch="arm64" ;;
  esac
  case "$name" in
    *linux*)           os="linux" ;;
    *darwin*|*apple*)  os="darwin" ;;
    *windows*|*mingw*) os="windows" ;;
  esac
  [ -n "$os" ] && [ -n "$arch" ] && echo -n "$os-$arch"
}

# The name of each document a plugin bundle carries, taken from the bundle
# itself. Every attestation hangs off the one bundle digest, and syft named each
# document after the binary it scanned, so nothing inside a document says which
# target it describes. The bundle's payload does: it holds one SBOM file per
# binary, named with the target triple. A CycloneDX serial number, or an SPDX
# document namespace, is unique per generation, so it joins the attested copy
# back to the file it was made from.
#
# Writes "<serial>\t<name>" lines to $2, empty when the bundle carries no SBOM
# files - bundles built before those were generated do not.
plugin_doc_names() {
  local ref="$1" out="$2" layer dir blob f serial name

  : > "$out"
  dir=$(mktemp -d -t bundle-XXXXXX)
  blob="$dir/layer"
  while read -r layer; do
    [ -n "$layer" ] || continue
    $ORAS blob fetch ${INSECURE_REGISTRY:+--plain-http} --output "$blob" "${ref%@*}@$layer" 2>/dev/null || continue
    # The payload is a gzipped tar, but do not insist on it.
    $TAR -xzf "$blob" -C "$dir" 2>/dev/null || $TAR -xf "$blob" -C "$dir" 2>/dev/null || true
    rm -f "$blob"
  done < <($ORAS manifest fetch ${INSECURE_REGISTRY:+--plain-http} "$ref" 2>/dev/null \
    | $JQ -r '.layers[]?.digest' 2>/dev/null)

  for f in "$dir"/*.cdx.json "$dir"/*.spdx.json; do
    [ -f "$f" ] || continue
    serial=$($JQ -r '.serialNumber // .documentNamespace // empty' "$f" 2>/dev/null || true)
    [ -n "$serial" ] || continue
    name="${f##*/}"
    name="${name%.cdx.json}"
    name="${name%.spdx.json}"
    printf '%s\t%s\n' "$serial" "$name" >> "$out"
  done
  rm -rf "$dir"
}

# Whether a reference is one of the kubectl plugin bundles rather than a
# container image.
is_plugin_ref() {
  local ref="$1" p

  for p in "${PLUGIN_REFS[@]:-}"; do
    [ "$ref" = "$p" ] && return 0
  done
  case "$ref" in
    *"/kubectl-$PLUGIN_NAME:"*) return 0 ;;
  esac
  return 1
}

component_count() {
  $JQ -r 'if .components then (.components|length)
          elif .predicate.components then (.predicate.components|length)
          elif .packages then (.packages|length)
          else 0 end' "$1" 2>/dev/null || echo 0
}

# The name to file a document under: what the SBOM says it describes, so that
# the several attestations of a plugin bundle do not collide.
document_name() {
  local fallback="$2" dname
  dname=$($JQ -r '(.metadata.component.name // .predicate.metadata.component.name // .name // "")
                  | split("/") | last' "$1" 2>/dev/null || true)
  if [ -z "$dname" ] || [ "$dname" = "null" ]; then
    dname="$fallback"
  fi
  echo -n "${dname//[^a-zA-Z0-9._-]/_}"
}

# The repository of a reference, without its tag or digest. Not "${ref%%:*}":
# a registry host may carry a port, and that colon comes first.
repo_of() {
  local ref="${1%%@*}" last prefix
  last="${ref##*/}"
  if [ "$last" = "$ref" ]; then
    echo -n "${ref%%:*}"
    return 0
  fi
  prefix="${ref%/*}"
  echo -n "$prefix/${last%%:*}"
}

# Who signed the reference, taken from the signature rather than from the
# attestation: verify-attestation emits DSSE envelopes and the certificate is
# not in them. "-" when there is nothing to report, key-signed included.
cosign_signer() {
  local ref="$1" sig
  local args=(--experimental-oci11)

  if [ -n "$INSECURE_REGISTRY" ]; then
    args+=(--allow-insecure-registry --insecure-ignore-tlog)
  fi
  if [ -n "$COSIGN_KEY" ]; then
    args+=(--key "$COSIGN_KEY")
  else
    if [ -n "$ISSUER" ]; then
      args+=(--certificate-oidc-issuer "$ISSUER")
    else
      args+=(--certificate-oidc-issuer-regexp "$ISSUER_REGEXP")
    fi
    args+=(--certificate-identity-regexp "$IDENTITY_REGEXP")
  fi

  sig=$($COSIGN verify "${args[@]}" "$ref" 2>/dev/null) || { echo -n "-"; return 0; }
  $JQ -r 'map(.optional.Subject // empty) | first // "-"' <<< "$sig" 2>/dev/null || echo -n "-"
}

# A dry run stops here, having resolved which artefacts and targets a real run
# would look at and where each would land. Manifests are read to enumerate the
# targets - that is what decides the platforms - but nothing is fetched,
# verified or written. Useful for checking that --chart, --filter and the
# derived plugin reference select what was meant before paying for a full run.
if [ -n "$DRY_RUN" ]; then
  {
    printf 'ARTEFACT\tPLATFORM\tWOULD WRITE\n'
    for entry in "${ARTEFACTS[@]}"; do
      aname="${entry%%$'\t'*}"
      ref="${entry#*$'\t'}"
      targets_out=$(targets_of "$ref" || true)
      if [ -z "$targets_out" ]; then
        printf '%s\t-\t(could not resolve)\n' "$ref"
        continue
      fi
      while read -r target; do
        [ -n "${target:-}" ] || continue
        platform="${target%% *}"
        arch="$(arch_dir "$platform")"
        maybe=""
        # The index is examined, but a real run drops the row unless the index
        # itself carries something its children did not, so promising a file
        # here would overstate it.
        [ "$platform" = "index" ] && maybe=" (only if attached to the index)"
        if is_plugin_ref "$ref"; then
          # One directory and one row per binary the bundle carries, and which
          # those are is only known once the documents are read.
          printf '%s\t%s\t%s\n' "$ref" "$platform" \
            "${OUT_NAME}/plugins/<platform>/sboms/$aname.*.json$maybe"
        else
          # An index document is written to each platform of the image, so there
          # is no single directory to name for it.
          [ -n "$arch" ] || arch="<platform>"
          printf '%s\t%s\t%s\n' "$ref" "$platform" \
            "${OUT_NAME}/containers/$arch/sboms/$aname.*.json$maybe"
        fi
      done <<< "$targets_out"
    done
  } | column -t -s $'\t'
  note ""
  note "$CLEAR_NOTE"
  note "Dry run: nothing was fetched, verified or written."
  exit 0
fi

ROWS=()
FAILED=0
MISSING=0
ERRORS=0

# One artefact's worth of work, with its rows appended to $2. Nothing in here is
# shared with another artefact, which is what makes it safe to run several at
# once: the rows go to a file of their own, and the counters are derived from all
# of them afterwards rather than incremented in flight.
process_artefact() {
  local entry="$1" rowsfile="$2"
  local aname ref repo targets_out target platform digest is_index child_arg
  local tmp kind signer result ask known cosign_rc arch dest doc suffix base docname fallback total count dup
  local is_plugin parch pplat cand serial mapped sib d
  local -a dests
  local -a targets docs
  local notes=""

  aname="${entry%%$'\t'*}"
  ref="${entry#*$'\t'}"
  log_to_stderr "  start   $ref"
  is_plugin=""
  is_plugin_ref "$ref" && is_plugin="yes"
  repo="$(repo_of "$ref")"

  targets_out=$(targets_of "$ref" || true)
  if [ -z "$targets_out" ]; then
    # An error, not a gap: a reference that will not resolve has not been
    # checked, so --verify has nothing to stand on. The warning above says why.
    printf '%s\t%s\t-\tunresolved\terror\tcould not resolve the reference\t-\n' \
      "$aname" "$ref" >> "$rowsfile"
    log_to_stderr "  done    $ref  unresolved"
    return 0
  fi
  mapfile -t targets <<< "$targets_out"

  for target in "${targets[@]}"; do
    [ -n "${target:-}" ] || continue
    platform="${target%% *}"
    digest="${target##* }"
    # The index target has no platform of its own, and its row is dropped below
    # if it turns up nothing the children did not already have.
    is_index=""
    if [ "$platform" = "index" ]; then
      is_index="yes"; platform="-"
    fi
    [ -n "$digest" ] || continue
    # buildx entries are matched to a child by digest; for the index target
    # there is no child and every entry counts.
    child_arg="$digest"
    [ -n "$is_index" ] && child_arg=""
    tmp=$(mktemp -d -t sbom-XXXXXX)
    kind="none"; signer="-"; result="-"

    # One referrers query per target, shared by the gate below and by the probe
    # at the end, so the registry is asked this once rather than three times.
    # Likewise one look for an attestation among them, shared by the gate and by
    # the retry decision in cosign_sboms.
    discover_refs "$repo@$digest" "$tmp/refs"
    known=""
    has_attestation_referrer "$repo" "$tmp/refs" && known="yes"
    ask="yes"
    worth_asking_cosign "$repo" "$digest" "$known" || ask=""

    cosign_rc=1
    if [ -n "$ask" ]; then
      cosign_sboms "$tmp" "$repo@$digest" "$known" && cosign_rc=0 || cosign_rc=$?
    fi

    if [ "$cosign_rc" -eq 0 ]; then
      kind="cosign"
      signer="$(cosign_signer "$repo@$digest")"
    elif [ "$cosign_rc" -eq 2 ]; then
      # The registry or Rekor would not answer after retries. Falling through to
      # the unverified paths would label this as though we had checked and found
      # something wrong, which is not what happened.
      kind="error"; signer="not checked: registry or Rekor error"
    elif [ -n "$ask" ] && legacy_sboms "$tmp" "$repo" "$digest"; then
      kind="legacy"; signer="unverified (legacy tag)"
    elif [ -z "$is_index" ] && buildx_sboms "$tmp" "$repo" "$ref" "$child_arg"; then
      # Not for the index: a buildx attestation always describes one child, so
      # collecting them all against the index would just write the children's
      # documents a second time.
      kind="buildx"; signer="unsigned (buildx)"
    elif [ -z "$is_index" ] && syft_sbom "$tmp" "$repo@$digest"; then
      # Nothing was attached, so scan it. Not for the index: its children are
      # what get scanned, and they cover the same ground.
      kind="syft"; signer="generated by this run"
    fi

    if [ "$kind" = "error" ]; then
      rm -rf "$tmp"
      # The index is looked at opportunistically - our SBOMs live on the
      # children - so failing to read it says nothing about coverage and must
      # not gate the run when the children were readable.
      if [ -n "$is_index" ] && (( ${#targets[@]} > 1 )); then
        log_to_stderr "  note    $ref index not readable, ignoring: $signer"
        continue
      fi
      result="error"
      printf '%s\t%s\t%s\terror\t%s\t%s\t-\n' \
        "$aname" "$ref" "$platform" "$result" "$signer" >> "$rowsfile"
      notes+=" $platform=error"
      continue
    fi

    if [ "$kind" = "none" ]; then
      # Nothing on the index that the children did not have: not worth a row.
      if [ -n "$is_index" ] && (( ${#targets[@]} > 1 )); then
        rm -rf "$tmp"
        continue
      fi
      kind="none"
      if signer="$(probe_target "$repo" "$digest" "$ref" "$child_arg" "$tmp/refs")"; then
        # Something is attached that our identity rejected. Before calling that a
        # verification failure, check it is really signed by someone: cosign
        # gives the same "no matching attestations" for an identity mismatch and
        # for a registry that would not answer, and only the first is a failure.
        if attested_by_anyone "$repo@$digest"; then
          result="FAIL"
          signer="attested, but not by an accepted identity"
        else
          kind="error"; result="error"
          signer="not checked: registry or Rekor error"
        fi
      else
        result="missing"
      fi
      rm -rf "$tmp"
      printf '%s\t%s\t%s\t%s\t%s\t%s\t-\n' \
        "$aname" "$ref" "$platform" "$kind" "$result" "$signer" >> "$rowsfile"
      notes+=" $platform=$kind"
      continue
    fi

    # How the document was come by, and only one of these is a failure:
    #
    #   cosign     an attestation that verifies - the good case.
    #   buildx     a document with nothing vouching for it. A coverage gap of
    #              the same kind as having none at all, not a broken signature,
    #              so it is written and reported but counted with the missing.
    #   syft       no SBOM was published, so this run scanned the image. Says
    #              what is in it, nothing about who stands behind it, and is
    #              counted with the missing for the same reason.
    #   legacy     an attestation that exists and does not verify. Something
    #              claims to be signed and the claim does not hold: the failure.
    #
    # Judged the same whoever published it. An artefact with no attestation is
    # never a failure - there is nothing to disagree with - so only the last of
    # these gates the run.
    case "$kind" in
      cosign) result="ok" ;;
      buildx) result="unsigned" ;;
      syft)   result="generated" ;;
      *)      result="FAIL" ;;
    esac
    # Kind first, then the platform, so that "everything for linux/amd64" is
    # containers/linux-amd64 alongside plugins/linux-amd64, and each is a
    # self-contained tree that can be archived or diffed on its own. A plugin
    # bundle's documents each describe a different binary, so the platform for
    # those is per document and is worked out inside the loop below.
    arch="$(arch_dir "$platform")"
    dests=()
    if [ -n "$is_plugin" ]; then
      dests=("$OUT_DIR/plugins")
    elif [ "$platform" = "-" ]; then
      # A container is one of the platforms it advertises. Getting
      # here means the document came off the index, so it describes the image as
      # a whole: it belongs in each of that image's platforms rather than in a
      # bucket of its own.
      for sib in "${targets[@]}"; do
        case "${sib%% *}" in
          index|-) continue ;;
          *) dests+=("$OUT_DIR/containers/$(arch_dir "${sib%% *}")/sboms") ;;
        esac
      done
      # Nothing else to go on, so name it for what the run was asked to cover.
      (( ${#dests[@]} )) || dests=("$OUT_DIR/containers/$(arch_dir "${PLATFORMS[0]}")/sboms")
    else
      dests=("$OUT_DIR/containers/$arch/sboms")
    fi
    dest="${dests[0]}"
    # Concurrent artefacts can be creating the same directory.
    for d in "${dests[@]}"; do
      mkdir -p "$d" 2>/dev/null || true
    done

    # One read of the bundle per plugin target, shared by every document.
    : > "$tmp/docnames"
    [ -z "$is_plugin" ] || plugin_doc_names "$repo@$digest" "$tmp/docnames"

    docs=("$tmp"/*.json)
    total=0
    for doc in "${docs[@]}"; do
      [ -f "$doc" ] || continue
      suffix="cdx"
      $JQ -e '.bomFormat // .predicate.bomFormat' "$doc" >/dev/null 2>&1 || suffix="spdx"
      base="$aname"
      # Several documents on one target - a plugin bundle has one per binary -
      # so the document says which is which. Its own name usually already
      # carries the artefact name, in which case it is used on its own rather
      # than repeated.
      if [ -n "$is_plugin" ] || (( ${#docs[@]} > 1 )); then
        fallback="${doc##*/}"
        docname="$(document_name "$doc" "${fallback%.json}")"
        case "$docname" in
          "$base"*) base="$docname" ;;
          *)        base="$base-$docname" ;;
        esac
      fi
      if [ -n "$is_plugin" ]; then
        # Which binary this document describes comes from the bundle's own file
        # list, joined on the document's serial number, because the document's
        # name is just the binary and is the same for every target. Falling back
        # to the document's name covers a bundle that carries no SBOM files.
        serial=$($JQ -r '.predicate.serialNumber // .serialNumber
                         // .predicate.documentNamespace // .documentNamespace
                         // empty' "$doc" 2>/dev/null || true)
        if [ -n "$serial" ]; then
          mapped=$(awk -F"\t" -v s="$serial" '$1 == s { print $2; exit }' "$tmp/docnames" 2>/dev/null || true)
          [ -n "$mapped" ] && docname="$mapped"
        fi
        # The document goes to the platform it describes, and nowhere if that
        # cannot be worked out: the bundle and its attestations are written from
        # one directory in one job, so a document the bundle cannot name means
        # something is wrong upstream. Say so rather than filing it under a name
        # that claims nothing.
        parch="$(plugin_arch "${docname:-$base}")"
        if [ -z "$parch" ]; then
          log_warn "$ref: no platform for the SBOM named '${docname:-$base}', skipped"
          # Still a row, or the table would claim fewer attestations than the
          # bundle carries. It has no platform to be reported under, so it goes
          # to the summary above them.
          printf '%s\t%s\t-\t%s\t%s\t%s\t-\n' \
            "$aname" "$ref" "$kind" "$result" "$signer" >> "$rowsfile"
          notes+=" -=$kind(unplaced)"
          continue
        fi
        dest="$OUT_DIR/plugins/$parch/sboms"
        dests=("$dest")
        mkdir -p "$dest" 2>/dev/null || true
        base="$aname"
        pplat="${parch/-//}"
      fi
      # Never overwrite: a document that cannot be told apart from one already
      # written still has to survive, or the count in the table is a lie about
      # what is on disk. Two cases need it. An index can list the same platform
      # twice, csi-provisioner ships two linux/arm/v7 children, and there the
      # digest distinguishes them. Several documents on one digest cannot use
      # the digest - it is the same for all of them - so they are numbered.
      # Reached only when the bundle could not name its own documents, since
      # otherwise each has its own platform directory by then.
      if [ -e "$dest/$base.$suffix.json" ]; then
        if (( ${#docs[@]} > 1 )); then
          # Several documents on one target all share its digest, so the digest
          # cannot tell them apart and only a number can.
          dup=2
          while [ -e "$dest/$base-$dup.$suffix.json" ]; do
            dup=$((dup + 1))
          done
          base="$base-$dup"
        else
          # One document colliding with another target's file is the other case,
          # and there the digest does distinguish them.
          base="$base-${digest:7:12}"
          cand="$base"
          dup=2
          while [ -e "$dest/$cand.$suffix.json" ]; do
            cand="$base-$dup"
            dup=$((dup + 1))
          done
          base="$cand"
        fi
      fi
      # Unwrap the in-toto envelope: what is wanted is the SBOM, not the
      # statement wrapping it.
      if $JQ -e '.predicate' "$doc" >/dev/null 2>&1; then
        $JQ '.predicate' "$doc" > "$dest/$base.$suffix.json"
      else
        cp "$doc" "$dest/$base.$suffix.json"
      fi
      # An index-level document has more than one home: it describes every
      # platform of the image, and each platform tree has to stand alone.
      for d in "${dests[@]:1}"; do
        cp "$dest/$base.$suffix.json" "$d/$base.$suffix.json"
      done
      count=$(component_count "$dest/$base.$suffix.json")
      if [ -n "$is_plugin" ]; then
        # One row per document for a bundle: each describes a different binary
        # on a different platform, and summing their components would describe
        # nothing that exists. The row is the platform's, so that
        # plugins/<platform>/summary.json says what is in plugins/<platform>/,
        # exactly as the container trees do.
        printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
          "$aname" "$ref" "$pplat" "$kind" "$result" "$signer" "$count" >> "$rowsfile"
        notes+=" $pplat=$kind($count)"
      else
        total=$((total + count))
      fi
    done
    rm -rf "$tmp"
    if [ -z "$is_plugin" ]; then
      printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$aname" "$ref" "$platform" "$kind" "$result" "$signer" "$total" >> "$rowsfile"
      notes+=" $platform=$kind($total)"
    fi
  done
  log_to_stderr "  done    $ref $notes"
}

# Artefacts are independent of one another and almost all of the time here is
# spent waiting on registries and Rekor rather than on the CPU, so they run
# concurrently. Each writes to its own rows file and those are joined in chart
# order afterwards, so the table never depends on which job finished first.
WORK=$(mktemp -d -t chart-sboms-XXXXXX)
artefact_n=0
for entry in "${ARTEFACTS[@]}"; do
  artefact_n=$((artefact_n + 1))
  rowsfile="$WORK/rows.$(printf '%04d' "$artefact_n")"
  : > "$rowsfile"
  if (( JOBS > 1 )); then
    # Throttle to JOBS in flight. A job that fails must not take the run with
    # it: its artefact is reported from whatever rows it managed to write.
    while (( $(jobs -rp | wc -l) >= JOBS )); do
      wait -n || true
    done
    process_artefact "$entry" "$rowsfile" &
  else
    process_artefact "$entry" "$rowsfile" || true
  fi
done
wait || true
mapfile -t ROWS < <(cat "$WORK"/rows.* 2>/dev/null || true)
rm -rf "$WORK"

# Counted from the rows rather than tallied as they were produced, because with
# several artefacts in flight there is no single place to tally them.
for row in "${ROWS[@]:-}"; do
  [ -n "$row" ] || continue
  IFS=$'\t' read -r _ _ _ kind result _rest <<< "$row"
  [ "$result" = "FAIL" ] && FAILED=$((FAILED + 1))
  [ "$result" = "error" ] && ERRORS=$((ERRORS + 1))
  case "$result" in
    # None of these is a verified SBOM, whatever else it is.
    missing|unsigned|generated) MISSING=$((MISSING + 1)) ;;
  esac
  [ "$kind" = "unresolved" ] && MISSING=$((MISSING + 1))
done

# The rows as JSON, machine readable alongside the table so that CI can act on
# them. With a directory named, only the rows belonging to it.
# The directories a row is reported in: its kind, containers or plugins, which
# collects every row of that kind whatever the platform, and its platform below
# that. One with no platform means something could not be placed: a container
# image that could not be resolved at all goes to every platform the run
# covered rather than to a bucket of its own, since there is no such thing as a
# container of no architecture; a plugin bundle with no SBOM, or a document it
# could not name, has only the kind to be reported under.
row_groups() {
  local ref="$1" platform="$2" p

  if is_plugin_ref "$ref"; then
    echo "plugins"
    [ "$platform" != "-" ] && echo "plugins/$(arch_dir "$platform")"
    return 0
  fi
  echo "containers"
  if [ "$platform" != "-" ] && [ "$platform" != "index" ]; then
    echo "containers/$(arch_dir "$platform")"
    return 0
  fi
  for p in "${CONTAINER_ARCHES[@]}"; do
    echo "containers/$p"
  done
}

rows_json() {
  local only="${1:-}" row aname ref platform kind result signer count
  local arch group first=1

  echo "["
  for row in "${ROWS[@]}"; do
    IFS=$'\t' read -r aname ref platform kind result signer count <<< "$row"
    if [ -n "$only" ] && ! row_groups "$ref" "$platform" | grep -qxF "$only"; then
      continue
    fi
    [ $first -eq 1 ] || echo ","
    first=0
    printf '  {"name":"%s","artefact":"%s","platform":"%s","sbom":"%s","result":"%s","signer":"%s","components":"%s"}' \
      "$aname" "$ref" "$platform" "$kind" "$result" "$signer" "$count"
  done
  echo
  echo "]"
}

# One summary per directory rather than one for the lot, so each tree is
# complete on its own: the SBOMs found there and the report of what was looked
# at to find them. A platform where nothing was found still gets a summary -
# that it was looked at and came up empty is the useful part. Each kind has one
# too, above its platforms, with every row of that kind: the whole of what the
# chart deploys as containers, or the whole of the plugin bundle, in one read.
# The container platforms this run covered, which is what a row with no
# platform of its own is reported against. Taken from the rows where possible,
# so it reflects what the images actually advertised, and from the platforms
# asked for when not a single one resolved.
CONTAINER_ARCHES=()
for row in "${ROWS[@]}"; do
  IFS=$'\t' read -r _ ref platform _rest <<< "$row"
  is_plugin_ref "$ref" && continue
  case "$platform" in -|index) continue ;; esac
  arch="$(arch_dir "$platform")"
  [[ " ${CONTAINER_ARCHES[*]} " == *" $arch "* ]] || CONTAINER_ARCHES+=("$arch")
done
if ! (( ${#CONTAINER_ARCHES[@]} )); then
  for platform in "${PLATFORMS[@]}"; do
    CONTAINER_ARCHES+=("$(arch_dir "$platform")")
  done
fi

# Not "GROUPS": bash keeps that as the current user's group ids, an indexed
# array, and declaring it associative fails.
declare -A SUMMARY_DIRS=()
for row in "${ROWS[@]}"; do
  IFS=$'\t' read -r _ ref platform _rest <<< "$row"
  while read -r group; do
    [ -n "$group" ] && SUMMARY_DIRS["$group"]=1
  done < <(row_groups "$ref" "$platform")
done
for group in "${!SUMMARY_DIRS[@]}"; do
  mkdir -p "$OUT_DIR/$group"
  rows_json "$group" > "$OUT_DIR/$group/summary.json"
done

if [ "$FORMAT" = "json" ]; then
  # Every row, so that one run can be consumed whole without walking the tree.
  rows_json
else
  {
    printf 'ARTEFACT\tPLATFORM\tSBOM\tRESULT\tSIGNER\tCOMPONENTS\n'
    for row in "${ROWS[@]}"; do
      IFS=$'\t' read -r aname ref platform kind result signer count <<< "$row"
      # The identity is a workflow URL and far too long for a column; the repo
      # and ref are the parts anyone reads.
      case "$signer" in
        https://github.com/*) signer="${signer#https://github.com/}" ;;
      esac
      printf '%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$ref" "$platform" "$kind" "$result" "$signer" "$count"
    done
  } | column -t -s $'\t'
fi

note ""
note "${#ROWS[*]} target(s) across ${#ARTEFACTS[*]} artefact(s)"
note "Written to $OUT_DIR/{containers,plugins}/<platform>/sboms"

# "Could not check" is not "passed", so it fails the gate too - but with its
# own wording, because an infrastructure hiccup and a signature that does not
# verify call for completely different responses.
if [ -n "$VERIFY" ] && (( FAILED + ERRORS )); then
  gate=""
  (( FAILED )) && gate="$FAILED failed verification"
  (( ERRORS )) && gate="${gate:+$gate, }$ERRORS could not be checked (unresolved reference, or registry or Rekor error)"
  log_fatal "target(s): $gate"
fi
if [ -n "$FAIL_ON_MISSING" ] && (( MISSING )); then
  log_fatal "$MISSING target(s) have no verified SBOM"
fi
exit 0
