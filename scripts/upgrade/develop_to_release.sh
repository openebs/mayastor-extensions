#!/usr/bin/env bash

set -e

SCRIPT_DIR="$(dirname "$(realpath "${BASH_SOURCE[0]:-"$0"}")")"
ROOT_DIR="$SCRIPT_DIR/../.."

source "$ROOT_DIR/scripts/utils/log.sh"

echo "Updating allowed umbrella-chart version..."
echo "============================================"
UPGRADE_CONSTANTS_FILE_PATH="$ROOT_DIR/k8s/upgrade/src/bin/upgrade-job/common/constants.rs"
test -f "$UPGRADE_CONSTANTS_FILE_PATH" || log_fatal "couldn't find file $UPGRADE_CONSTANTS_FILE_PATH"

# There is templating on the $UPGRADE_CONSTANTS_FILE_PATH file to mark the line where this change needs to happen.
DEVELOP_UMBRELLA_VERSION=$(awk '/\/\* @@@UPGRADE_PREP@@@ \*\// {
    match($0, /"([0-9]+\.[0-9]+\.[0-9]+)"/, ver);
    print ver[1];
}' "$UPGRADE_CONSTANTS_FILE_PATH")
echo "Found current allowed umbrella-chart version: '$DEVELOP_UMBRELLA_VERSION'"

# Typically a release branch is cut to make a new minor release.
# Raise a PR to manually change the allowed umbrella chart version if it's a major/patch release.
RELEASE_UMBRELLA_VERSION=$(semver bump minor "$DEVELOP_UMBRELLA_VERSION")
echo "Bumped allowed umbrella-chart version: '$RELEASE_UMBRELLA_VERSION'"

# This requires GNU sed. If you're on Mac, you'd get BSD sed by default. You'd need GNU coreutils to get GNU sed.
# Check if sed is GNU sed.
sed --version &> /dev/null || log_fatal "couldn't find GNU 'sed'"
# Update constants module in upgrade-job.
sed -i "s/\(\"\)$DEVELOP_UMBRELLA_VERSION\(\";\ \/\*\ @@@UPGRADE_PREP@@@\ \*\/\)/\1$RELEASE_UMBRELLA_VERSION\2/" $UPGRADE_CONSTANTS_FILE_PATH
