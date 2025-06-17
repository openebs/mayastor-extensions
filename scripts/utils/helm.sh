#!/usr/bin/env bash

if ! [ -d "${CHART_DIR:-}" ] || ! [ -f "${CHART:-}" ] || [ -z "${HELM:-}" ]; then
  log_fatal "Please setup helm globals!"
  exit 1
fi

helm_dep_version() {
  $HELM show chart "$CHART_DIR" --kubeconfig "$CHART_DIR/fake" | dep="${1:-}" yq '.dependencies[]|select(.name == strenv(dep)).version'
}
helm_chart_tag() {
  $HELM show values "$CHART_DIR" --kubeconfig "$CHART_DIR/fake" | yq '.image.tag'
}
helm_chart_name() {
  yq eval '.name' "$CHART_DIR/Chart.yaml"
}
helm_chart_version() {
  yq eval '.version' "$CHART_DIR/Chart.yaml"
}

# helm template seems to sometimes use the unpacked dependencies rather than the tar files
# even weirder, if you run helm template a few times, sometimes it does use the tar file!
# given this, it's safer to simply delete the unpacked tars here, ensuring we run helm
# template with a "clean" slate.
helm_dep_clean() {
  local deps

  if ! deps=$($HELM show chart "$CHART_DIR" --kubeconfig "$CHART_DIR/fake" | yq -ojson '.dependencies[]|select(.repository != "")' | jq -c); then
    log_fatal "Can't find the helm dependencies in $CHART_DIR"
  fi

  for chart in ${deps[@]}; do
    name=$(echo "$chart" | jq -r '.name')
    path="$CHART_DIR/charts/$name"
    if [ -d "$path" ]; then
      rm -rf "$path"
    fi
  done
}

# This fetches the dependencies in an exact version from the Chart.yaml
# NOTE: This won't work if we ever modify the Chart.yaml to specify non-pinned versions, ex: 14 vs 14.0.0
# Update can be forced with global var DEP_UPDATE="true".
helm_dep_update() {
  local update="false"

  if [ "${DEP_UPDATE:-false}" = "true" ]; then
    update="true"
  else
    local deps

    if ! deps=$($HELM show chart "$CHART_DIR" --kubeconfig "$CHART_DIR/fake" | yq -ojson '.dependencies[]|select(.repository != "")' | jq -c); then
      log_fatal "Can't find the helm dependencies in $CHART_DIR"
    fi

    for chart in ${deps[@]}; do
      version=$(echo "$chart" | jq -r '.version')
      name=$(echo "$chart" | jq -r '.name')
      if [ "$(semver validate "$version")" != "valid" ]; then
        log_fatal "Found $name with version $version only pinned versions are supported!"
      fi
      if ! [ -f "$CHART_DIR/charts/$name-$version.tgz" ]; then
        update="true"
        break
      fi
    done
  fi

  helm_dep_clean
  if [ "$update" = "true" ]; then
    $HELM dependency update "$CHART_DIR" --kubeconfig "$CHART_DIR/fake"
  fi
}

helm_oci_chart_exists() {
  local repo="$1"
  local version="$2"
  local chart stderr_file error not_found

  stderr_file=$(mktemp)
  _=$(helm show chart "oci://$repo" --version "$version" 2>"$stderr_file")
  error=$?

  if [ $error -eq 0 ]; then
    rm "$stderr_file"
    echo "true"
    return 0
  fi

  if cat "$stderr_file" | grep "not found" >/dev/null; then
    not_found="true"
  fi
  rm "$stderr_file"

  if [ "${not_found:-}" != "true" ]; then
    log_error "Failed to fetch chart oci://$repo $version"
    return $error
  fi

  # ok, there's no chart.. I think?
  echo "false"
  return 0
}
