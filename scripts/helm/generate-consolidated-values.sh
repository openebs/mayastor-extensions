#!/usr/bin/env bash

set -o errexit

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
DEFAULT_CHART_DIR="$SCRIPT_DIR/../../chart"
CHART_DIR="$DEFAULT_CHART_DIR"

# Imports
source "$SCRIPT_DIR/../utils/log.sh"

# Print usage options for this script.
print_help() {
  cat <<EOF
Usage: $(basename "${0}") [OPTIONS]

Options:
  -h, --help                    Display this text
  -d, --chart-dir <DIRECTORY>   Specify the helm chart directory (default "$DEFAULT_CHART_DIR")

Examples:
  $(basename "${0}") --chart-dir "./chart"
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
}

# Generate in-place consolidated values YAMLs throughout the
# helm chart hierarchy (root chart and sub-charts).
# Ignore if values file doesn't exist. Ex: alloy's crd chart dependency doesn't have values.yaml
consolidate() {
  local -r chart_dir="$1"
  local -r chart_name="${chart_dir##*/}"

  if stat "$chart_dir"/charts &> /dev/null; then
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

for dir in "$CHART_DIR"/charts/*; do
  consolidate "$dir"
done
