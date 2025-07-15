#!/usr/bin/env bash

set -e

SCRIPT_DIR="$(dirname "$0")"
TMP_KIND="/tmp/kind/mayastor"
TMP_KIND_CONFIG="$TMP_KIND/config.yaml"
WORKERS=2
DELAY="false"
CORES=1
POOL_SIZE="200MiB"
DRY_RUN=
KIND="kind"
FALLOCATE="fallocate"
KUBECTL="kubectl"
DOCKER="docker"
HUGE_PAGES=1800
LABEL=
CLEANUP="false"
IPFAM="ipv4"
SUDO=${SUDO:-"sudo"}
USE_DOCKER_CFG="true"

help() {
  cat <<EOF
Usage: $(basename "$0") [COMMAND] [OPTIONS]

Options:
  -h, --help                        Display this text.
  --workers       <num>             The number of worker nodes (Default: $WORKERS).
  --cores         <num>             How many cores to set to each io-engine (Default: $CORES).
  --delay                           Enable developer delayed mode on the io-engine (Default: false).
  --disk          <size>            Add disk of this size to each worker node.
  --dry-run                         Don't do anything, just output steps.
  --hugepages     <num>             Add <num> 2MiB hugepages (Default: $HUGE_PAGES).
  --label                           Label worker nodes with the io-engine selector.
  --cleanup                         Prior to starting, stops the running instance of the deployer.
  --ip-family     <str>             KIND has support for IPv4, IPv6 and dual-stack clusters, with the default being ipv4.
  --no-login                        Don't use the docker config json file.

Command:
  start                             Start the k8s cluster.
  stop                              Stop the k8s cluster.

Examples:
  $(basename "$0") start --workers 2 --disk 1GiB
EOF
}

echo_stderr() {
  echo -e "${1}" >&2
}

die() {
  local _return="${2:-1}"
  echo_stderr "$1"
  exit "${_return}"
}

COMMAND=
DO_ARGS=
while [ "$#" -gt 0 ]; do
  case $1 in
    -h|--help)
      help
      exit 0
      ;;
    start)
      COMMAND="start"
      DO_ARGS="y"
      shift;;
    stop)
      COMMAND="stop"
      DO_ARGS="y"
      shift;;
    *)
      [ -z "$DO_ARGS" ] && die "Must specify command before args"
      case $1 in
        --workers)
          shift
          test $# -lt 1 && die "Missing Number of Workers"
          WORKERS=$1
          shift;;
        --cores)
          shift
          CORES=$1
          shift;;
        --disk)
          shift
          test $# -lt 1 && die "Missing Disk Size"
          POOL_SIZE=$1
          shift;;
        --delay)
          DELAY="true"
          shift;;
        --label)
          LABEL="true"
          shift;;
        --cleanup)
          CLEANUP="true"
          shift;;
        --hugepages)
          shift
          test $# -lt 1 && die "Missing hugepage number"
          HUGE_PAGES=$1
          shift;;
        --ip-family)
          shift
          test $# -lt 1 && die "Missing Ip Family"
          case $1 in
            ipv4 | IPv4 | ip4 | 4)
              IPFAM="ipv4"
              shift;;
            ipv6 | IPv6 | ip6 | 6)
              IPFAM="ipv6"
              shift;;
            dual | dual-stack)
              IPFAM="dual"
              shift;;
            *)
              die "Invalid ip family: $1"
              ;;
          esac
          ;;
        --no-login)
          USE_DOCKER_CFG="false"
          shift;;
        --dry-run)
          if [ -z "$DRY_RUN" ]; then
            DRY_RUN="--dry-run"
            KIND="echo $KIND"
            FALLOCATE="echo $FALLOCATE"
            KUBECTL="echo $KUBECTL"
            DOCKER="echo $DOCKER"
            SUDO="echo"
          fi
          shift;;
        *)
          die "Unknown cli argument: $1"
          ;;
      esac
  esac
done

if [ -z "$COMMAND" ]; then
  die "No command specified!"
fi

if [ "$COMMAND" = "stop" ] || [ "$CLEANUP" = "true" ]; then
  if command -v nvme &>/dev/null; then
    $SUDO nvme disconnect-all
  fi
  $KIND delete cluster
  if [ "$COMMAND" = "stop" ]; then
    exit 0
  fi
fi

"$SCRIPT_DIR"/setup-io-prereq.sh --hugepages "$HUGE_PAGES" --nvme-tcp $DRY_RUN

# Create and cleanup the tmp folder
# Note: this is static in case you want to restart the worker node
mkdir -p "$TMP_KIND"
$SUDO rm -rf "$TMP_KIND"/*

# Adds the control-plane/master node
cat <<EOF > "$TMP_KIND_CONFIG"
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
networking:
  ipFamily: $IPFAM
nodes:
- role: control-plane
EOF

start_core=1
nodes=()
for node_index in $(seq 1 "$WORKERS"); do
  if [ "$node_index" == 1 ]; then
    node="kind-worker"
  else
    node="kind-worker$node_index"
  fi
  nodes+=("$node")

  host_path="$TMP_KIND/$node"
  cat <<EOF >> "$TMP_KIND_CONFIG"
- role: worker
  extraMounts:
    - hostPath: /dev
      containerPath: /dev
      propagation: HostToContainer
    - hostPath: /run/udev
      containerPath: /run/udev
      propagation: HostToContainer
    - hostPath: $TMP_KIND/$node
      containerPath: /var/local/mayastor
      propagation: HostToContainer
EOF
  DOCKER_CONFIG="$HOME/.docker/config.json"
  if [ "$USE_DOCKER_CFG" = "true" ] && [ -f "$DOCKER_CONFIG" ]; then
    cat <<EOF >> "$TMP_KIND_CONFIG"
    - hostPath: $DOCKER_CONFIG
      containerPath: /var/lib/kubelet/config.json
EOF
  fi
  if [ -n "$LABEL" ]; then
    cat <<EOF >> "$TMP_KIND_CONFIG"
  labels:
    openebs.io/engine: mayastor
EOF
  fi
  mkdir -p "$host_path"/io-engine
  if [ -n "$POOL_SIZE" ]; then
    $FALLOCATE -l "$POOL_SIZE" "$host_path"/io-engine/disk.io
  fi
  corelist=$(seq -s, $((start_core+((node_index-1)*CORES))) 1 $((start_core-1+((node_index)*CORES))))
  printf "eal_opts:\n  core_list: %s\n  developer_delay: %s\n" "$corelist" "$DELAY" >"$host_path"/io-engine/config.yaml
done

if [ -n "$DRY_RUN" ]; then
  cat "$TMP_KIND_CONFIG"
fi

$KIND create cluster --config "$TMP_KIND_CONFIG"

$KUBECTL cluster-info --context kind-kind
if [ -z "$DRY_RUN" ]; then
  host_ip=$($DOCKER network inspect kind | jq -r 'first (.[0].IPAM.Config[].Gateway | select(.))')
fi
echo "HostIP: $host_ip"

# shellcheck disable=SC2068
for node in ${nodes[@]}; do
  $DOCKER exec "$node" mount -o remount,rw /sys

  # Note: this will go away if the node restarts...
  $DOCKER exec "$node" bash -c 'printf "'"$host_ip"' kvmhost\n" >> /etc/hosts'
done
