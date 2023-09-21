#!/usr/bin/env bash

set -eE
trap 'die "failed minikube setup"' ERR
trap 'cleanup_workspace' EXIT

CURL="curl -fSsL"
JQ="jq -r"

# Write output to error output stream.
echo_stderr() {
  echo -e "${1}" >&2
}

# Removes workspace temporary directory.
cleanup_workspace() {
  if [ -n "${workspace}" ]; then
    echo "Cleaning up workspace directory ${workspace}..."
    rm -rf "${workspace}"
    echo "Removed directory ${workspace}."
    workspace=""
  fi
}

# Exit with error status and print error.
die() {
  local _return="${2:-1}"
  test "${_PRINT_HELP:-no}" = yes && print_help >&2
  echo_stderr "ERROR: $1"
  exit "${_return}"
}

# Print usage options for this script.
print_help() {
  cat <<EOF
Usage: $(basename "${0}") [OPTIONS]

Options:
  -h, --help                      Display this text.
  --kubernetes-version <version>  Specify the version of kubernetes.
  -i, --install-prerequisites     Install cri-dockerd and containernetworking plugins before creating minikube cluster.
  -y, --assume-yes                Assume the answer 'Y' for all interactive questions.
  -k, --skip-kube-context-switch  Skip switching kubectl cluster to the created minikube cluster's context.

Examples:
  $(basename "${0}") --kubernetes-version v1.25.11
EOF
}

# Parse arguments.
parse_args() {
  while test $# -gt 0; do
    arg="$1"
    case "$arg" in
    --kubernetes-version)
      test $# -lt 2 && die "missing value for the optional argument '$arg'."
      KUBERNETES_VERSION="${2}"
      shift
      ;;
    --kubernetes-version=*)
      KUBERNETES_VERSION="${arg#*=}"
      ;;
    -i | --install-prerequisites)
      INSTALL_PREREQUISITES="true"
      ;;
    -y | --assume-yes)
      ASSUME_YES="true"
      ;;
    -k | --skip-kube-context-switch)
      SKIP_KUBE_CONTEXT_SWITCH="true"
      ;;
    -h | --help)
      print_help
      exit 0
      ;;
    -h*)
      print_help
      exit 0
      ;;
    *)
      _PRINT_HELP=yes die "unexpected argument '$arg'" 1
      ;;
    esac
    shift
  done
}

# Run command and expect success, or else exit with error.
must_succeed_command() {
  local error="${2:-command \'${1}\' failed}"
  ${1} || die "${error}"
}

# Check for command in PATH, else exit with error.
must_exists_in_path() {
  local error="${2:-command ${1} not present in PATH}"
  must_succeed_command "command -v ${1}" "${error}"
}

# Get latest release tag (not un-released) for a GitHub repo using GitHub's api.
github_latest_version_tag() {
  local github_org=${1}
  local github_repo=${2}

  local tag_name=$(${CURL} \
    -H "Accept: application/vnd.github+json" \
    https://api.github.com/repos/"${github_org}"/"${github_repo}"/releases/latest | ${JQ} '.tag_name' | tr -d " \t\r\n")

  echo -n "${tag_name}"
}

# Get asset url for a GitHub release asset for a GitHub repo's latest release, using GitHub's api.
github_asset_url_from_latest_release() {
  local github_org=${1}
  local github_repo=${2}
  local release_asset_regex=${3}

  local url=$(${CURL} \
    -H "Accept: application/vnd.github+json" \
    https://api.github.com/repos/"${github_org}"/"${github_repo}"/releases/latest | ${JQ} ".assets[] | select(.name? | match(\"${release_asset_regex}\")).url" | tr -d " \t\r\n")

  echo -n "${url}"
}

# Get GitHub asset binary for a GitHub release asset for a GitHub repo, using GitHub's api.
github_asset_binary() {
  local github_asset_url=${1}
  local workspace=${2}
  local output_filepath=${3}

  cd "${workspace}"
  ${CURL} \
    -H "Accept: application/octet-stream" \
    "${github_asset_url}" \
    -o "${output_filepath}"
  cd -
}

# TODO: Use nix derivations for all prerequisites.
# Install prerequisites for minikube which aren't directly available from nixpkgs.
install_prerequisites() {
  local os=${1}
  local arch=${2}
  local workspace=${3}

  case "$os" in
  "GNU/Linux")
    case "$arch" in
    "x86_64")
      # Check if nix-shell prerequisites conntrack, minikube, crictl
      # are present in PATH.
      nix_shell_prerequisites=("conntrack" "minikube" "crictl" "curl" "jq" "awk" "systemctl" "docker")
      for bin in "${nix_shell_prerequisites[@]}"; do
        must_exists_in_path "${bin}" >/dev/null
      done

      echo "Installing prerequisites for $os-$arch..."

      # Install cri-dockerd, if not installed.
      cri_dockerd_error=""
      command -v cri-dockerd >/dev/null || cri_dockerd_error=$?
      if [ -n "${cri_dockerd_error}" ]; then
        echo "Downloading latest version of cri-dockerd..."
        url=$(github_asset_url_from_latest_release "Mirantis" "cri-dockerd" "^(cri-dockerd-[0-9]+.[0-9]+.[0-9]+.amd64.tgz)$")
        github_asset_binary "${url}" "${workspace}" "cri-dockerd.tgz"
        tar -xf "${workspace}"/cri-dockerd.tgz -C "${workspace}"
        mkdir -p /usr/local/bin
        install -o root -g root -m 0755 "${workspace}"/cri-dockerd/cri-dockerd /usr/local/bin/cri-dockerd
        command -v cri-dockerd >/dev/null || export PATH=$PATH:/usr/local/bin
        must_exists_in_path "cri-dockerd" "failed to install cri-dockerd" >/dev/null
        echo "Downloaded latest version of cri-dockerd."
      fi
      # Check if cri-docker.socket systemd service is active. While this is extremely
      # unlikely if cri-dockerd wasn't already installed, it is possible.
      cri_dockerd_service_error=""
      systemctl is-active --quiet cri-docker.socket || cri_dockerd_service_error=$?
      if [ -n "${cri_dockerd_service_error}" ]; then
        # Download systemd service files
        cri_dockerd_version="v$(cri-dockerd --version 2>&1 | awk '{print $2}')"
        systemd_service_files_url="https://raw.githubusercontent.com/Mirantis/cri-dockerd/${cri_dockerd_version}/packaging/systemd"
        cd ${workspace}
        ${CURL} "${systemd_service_files_url}"/cri-docker.service -o cri-docker.service
        ${CURL} "${systemd_service_files_url}"/cri-docker.socket -o cri-docker.socket
        cd -
        install "${workspace}"/cri-docker.{service,socket} /etc/systemd/system
        sed -i -e 's,/usr/bin/cri-dockerd,/usr/local/bin/cri-dockerd,' /etc/systemd/system/cri-docker.service
        systemctl daemon-reload
        systemctl enable --now cri-docker.socket
        systemctl is-active --quiet cri-dockerd.socket || die "failed to set up cri-dockerd systemd service" 1
        echo "Enabled cri-dockerd.socket systemd service."
      fi

      # Install container-network-plugins.
      echo "Installing container-network-plugins..."
      containernetworking_tag=$(github_latest_version_tag "containernetworking" "plugins")
      cni_plugin_version="${containernetworking_tag}"
      cni_plugin_tar="cni-plugins-linux-amd64-${cni_plugin_version}.tgz"
      cni_plugin_install_dir="/opt/cni/bin"
      cd "${workspace}"
      ${CURL} \
        -O "https://github.com/containernetworking/plugins/releases/download/${cni_plugin_version}/${cni_plugin_tar}"
      mkdir -p "${cni_plugin_install_dir}"
      tar -xf "${cni_plugin_tar}" -C "${cni_plugin_install_dir}"
      cd - >/dev/null
      echo "Installed container-network-plugins."

      echo "Installed prerequisites for $os-$arch."
      ;;
      # TODO: Needs implementation.
    "arm64" | "aarch64")
      die "the 'install_prerequisites' option is not implemented for ${os}-${arch}" 1
      ;;
    *)
      die "the 'install_prerequisites' option does not support the arch ${arch} for OS ${os}" 1
      ;;
    esac
    ;;
  "Darwin")
    case "$arch" in
    # TODO: Needs implementation.
    "x86_64")
      die "the 'install_prerequisites' option is not implemented for ${os}-${arch}" 1
      ;;
      # TODO: Needs implementation.
    "arm64")
      die "the 'install_prerequisites' option is not implemented for ${os}-${arch}" 1
      ;;
    *)
      die "the 'install_prerequisites' option does not support the arch ${arch} for OS ${os}" 1
      ;;
    esac
    ;;
  *)
    die "the 'install_prerequisites' option does not support the OS ${os}" 1
    ;;
  esac
}

# Pull in kubectl from hosted binary as mixing nix-shell and nix-env is not idiomatic.
pull_and_install_kubectl_binary() {
  local os=${1}
  local arch=${2}
  local workspace=${3}
  local kubernetes_version=${4}
  local kubectl_dir=${5}

  dl_link_os_path=""
  dl_link_arch_path=""
  case "$os" in
  "GNU/Linux")
    dl_link_os_path="linux"
    ;;
  "Darwin")
    dl_link_os_path="darwin"
    ;;
  *)
    die "the 'install_kubectl' option does not support the OS ${os}" 1
    ;;
  esac
  case "$arch" in
  "x86_64")
    dl_link_arch_path="amd64"
    ;;
  "arm64" | "aarch64")
    dl_link_arch_path="arm64"
    ;;
  *)
    die "the 'install_kubectl' option does not support the arch ${arch}" 1
    ;;
  esac

  # Pulling kubectl binary.
  cd "${workspace}"
  ${CURL} -O "https://dl.k8s.io/release/${kubernetes_version}/bin/${dl_link_os_path}/${dl_link_arch_path}/kubectl"
  cd - >/dev/null

  install -o root -g root -m 0755 "${workspace}"/kubectl "${kubectl_dir}"
  command -v kubectl >/dev/null || export PATH=$PATH:"$kubectl_dir"
  must_exists_in_path "kubectl" "failed to install kubectl" >/dev/null
}

# Installing kubectl if not already present in PATH, and if not the correct version.
install_kubectl() {
  local os=${1}
  local arch=${2}
  local workspace=${3}
  local kubernetes_version=${4}
  local kubectl_dir="/usr/local/bin"

  local kubectl_path=""
  local kubectl_path=$(command -v kubectl)
  if [ -n "${kubectl_path}" ]; then
    local kubectl_version=$(kubectl version --client -o json | ${JQ} '.clientVersion.gitVersion' | tr -d " \r\t\n") || die "failed to get kubectl version from existing binary"
    if [ "$kubectl_version" != "$kubernetes_version" ]; then
      kubectl_dir=${kubectl_path%"kubectl"}
      echo "kubectl already exists in PATH, replacing existing binary with kubectl ${kubernetes_version}..."
      pull_and_install_kubectl_binary "$os" "$arch" "$workspace" "$kubernetes_version" "$kubectl_dir"
    else
      echo "kubectl ${kubernetes_version} already exists at $kubectl_path."
    fi
  else
    pull_and_install_kubectl_binary "$os" "$arch" "$workspace" "$kubernetes_version" "$kubectl_dir"
  fi
}

KUBERNETES_VERSION="v1.25.11"
INSTALL_PREREQUISITES="false"
ASSUME_YES="false"
SKIP_KUBE_CONTEXT_SWITCH="false"

parse_args "$@"

# Gather platform info.
os=$(must_succeed_command "uname -o" | tr -d " \t\r\n")
arch=$(must_succeed_command "uname -m" | tr -d " \t\r\n")

# Directory to store downloaded files.
workspace=$(must_succeed_command "mktemp -d --suffix=-mayastor-extensions" "failed to create temporary directory" | tr -d " \t\r\n")
echo "Created workspace directory ${workspace}."

# Install cri-dockerd and containernetworking-plugins.
test "${INSTALL_PREREQUISITES}" = "true" && install_prerequisites "$os" "$arch" "$workspace"

# Set up minikube.
if [ "${ASSUME_YES}" != "true" ]; then
  echo_stderr "======================"
  echo_stderr "WARNING: Starting minikube. This may add a new kubernetes cluster context to your kubeconfig file at ${HOME}/.kube/config."
  read -p "Do you want to proceed? (Y/N): " confirm_minikube && [[ $confirm_minikube == [yY] ]] || exit 1
fi
echo "Starting minikube cluster with Kubernetes ${KUBERNETES_VERSION}..."
minikube start \
  --kubernetes-version=${KUBERNETES_VERSION} \
  --cni=calico \
  --driver=none \
  --install-addons=false \
  --keep-context=true \
  --force || die "failed to start minikube cluster" 1
echo "Started minikube cluster!"

# Set up kubectl.
if [ "${ASSUME_YES}" != "true" ]; then
  echo_stderr "======================"
  echo_stderr "WARNING: Setting up kubectl. This may replace your existing kubectl binary."
  read -p "Do you want to proceed? (Y/N): " confirm_kubectl && [[ $confirm_kubectl == [yY] ]] || exit 1
fi
echo "Setting up kubectl ${KUBERNETES_VERSION}..."
install_kubectl "$os" "$arch" "$workspace" "$KUBERNETES_VERSION"
echo "Set up kubectl ${KUBERNETES_VERSION}."

# Switch kubectl cluster context to the minikube cluster.
if [ "${SKIP_KUBE_CONTEXT_SWITCH}" = "true" ]; then
  echo "Skipped kubectl cluster context switch to 'minikube'."
else
  must_succeed_command "kubectl config use-context minikube" "failed to switch cluster context to 'minikube'"
fi
