#!/usr/bin/env bash

set -euo errexit

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
DEFAULT_CHART_DIR="$(realpath "$SCRIPT_DIR/../../chart")"
CHART_DIR="$DEFAULT_CHART_DIR"
CHART_OUT_DIR=

# Imports
source "$SCRIPT_DIR/../utils/log.sh"

# Print usage options for this script.
print_help() {
  cat <<EOF
Usage: $(basename "${0}") [OPTIONS]

Options:
  -h, --help                    Display this text
  -d, --chart-dir <DIRECTORY>   Specify the helm chart directory (default "$DEFAULT_CHART_DIR")
  -o, --out-dir   <DIRECTORY>   Specify the consolidated helm chart out directory

Examples:
  $(basename "${0}") --chart-dir "./chart" --out-dir "./chart-box"
EOF
}

# Parse arguments.
parse_args() {
  while test $# -gt 0; do
    arg="$1"
    case "$arg" in
    -d | --chart-dir)
      test $# -lt 2 && log_fatal "missing value for the optional argument '$arg'"
      CHART_DIR="${2%/}"
      shift
      ;;
    -d=* | --chart-dir=*)
      CHART_DIR="${arg#*=}"
      ;;
    -o | --out-dir)
      test $# -lt 2 && log_fatal "missing value for the optional argument '$arg'"
      CHART_OUT_DIR="${2%/}"
      shift
      ;;
    -o=* | --out-dir=*)
      CHART_OUT_DIR="${arg#*=}"
      ;;
    -h* | --help*)
      print_help
      exit 0
      ;;
    *)
      print_help
      log_fatal "unexpected argument '$arg'"
      ;;
    esac
    shift
  done

  if [ -z "${CHART_OUT_DIR:-}" ]; then
    log_fatal "Please specify --out-dir"
  fi
}

# Gets a list of top-level dependencies based on the Chart.yaml files
chart_deps() {
  local -r chart_dir="$1"

  if [ ! -f "$chart_dir/Chart.yaml" ]; then
    log_fatal "No $chart_dir/Chart.yaml !"
  fi

  if ! deps=$(helm show chart "$chart_dir" --kubeconfig "$CHART_DIR/fake" | yq -ojson '.dependencies[]' | jq -c); then
    log_fatal "Can't find the helm dependencies in $chart_dir"
  fi

  for chart in ${deps[@]}; do
    repository=$(echo "$chart" | jq -r '.repository')
    name=$(echo "$chart" | jq -r '.name')
    version=$(echo "$chart" | jq -r '.version')

    local name_rel="charts/$name"
    if [ -n "${repository:-}" ]; then
      echo "$chart_dir/charts/$name-$version.tgz"
    else
      echo "$chart_dir/charts/$name"
    fi
  done
}
# Clean the charts directory, ensuring it contains only valid dependencies
clean_chart_deps() {
  local -r chart_dir="$1"
  local chart_deps

  chart_deps=$(chart_deps "$chart_dir")

  for file in "$chart_dir"/charts/*; do
    if grep -Fxq "$file" <<< "$chart_deps"; then
      log_to_stderr "Found dependency: $file"
    else
      log_warn "Removing unknown dependency $file"
      rm -rf "$file"
    fi
  done
}

# For some reason, if the order of the consolation is different the end result is slightly different
# This keeps the previous generated consolidated value without change
chart_deps_compat() {
  local -r chart_dir="$1"

  for file in "$chart_dir"/charts/*; do
    echo "$file"
  done
}

consolidate() {
  local -r deps="$1"

  while IFS= read -r dep; do
    if [ -d "$dep" ]; then
      consolidate_dir "$dep"
    elif [ -f "$dep" ]; then
      consolidate_pkg "$dep"
    else
      log_fatal "Invalid dependency: $dep"
    fi
  done <<< "$deps"
}

# turns x/localpv-provisioner-2.50.1.tgz into localpv-provisioner
pkg_name() {
  local -r pkg=$(basename "${1%.tgz}")
  echo "${pkg%-*}"
}

consolidate_pkg() {
  local -r chart_pkg="$1"
  local chart_name extracted_pkg

  chart_name="$(pkg_name "$chart_pkg")"
  extracted_pkg="$(dirname "$chart_pkg")"

  log_to_stderr "Extracting $chart_name to $extracted_pkg"

  tar -xf "$chart_pkg" -C "$extracted_pkg"

  consolidate_dir "$extracted_pkg/$chart_name"
}

# Generate in-place consolidated values YAMLs throughout the
# helm chart hierarchy (root chart and sub-charts).
# Ignore if values file doesn't exist. Ex: alloy's crd chart dependency doesn't have values.yaml
consolidate_dir() {
  local -r chart_dir="$1"
  local -r chart_name="${chart_dir##*/}"

  if [ -d "$chart_dir"/charts ]; then
    for dir in "$chart_dir"/charts/*; do
      consolidate "$dir"
    done
  fi

  local -r values_file="$chart_dir/values.yaml"
  local -r values_yaml="$chart_dir/../../values.yaml"

  if [[ $(yq ".$chart_name" "$values_yaml") == null ]]; then
    yq -i ".$chart_name = {}" "$values_yaml"
  fi

  if [[ -f "$values_file" ]]; then
    yq -i ".$chart_name |= (load(\"$values_file\") * .)" "$values_yaml"
  fi
}


# Parse CLI args.
parse_args "$@"

if ! stat "$CHART_DIR"/charts &> /dev/null; then
  exit 0
fi

# Clean up any stales entries
# TODO: move this to the release.sh!
clean_chart_deps "$CHART_DIR"

# Avoid touching the original (mostly useful for debugging since the actual use of the script is inside the nix jail)
CHART_BOX="$CHART_OUT_DIR/consolidated"
mkdir -p "$CHART_OUT_DIR"

[ -d "$CHART_BOX" ] && rm -rf "$CHART_BOX"
cp -r "$CHART_DIR" "$CHART_BOX"

consolidate "$(chart_deps_compat "$CHART_BOX")"
