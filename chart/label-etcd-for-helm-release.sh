#!/usr/bin/env sh

# This script was introduced in v2.10.0, to upgrade the 'etcd' dependency helm chart from 8.6.0 to 12.0.11.
# The etcd chart, in v8.6.0, used to have a podAntiAffinity section which looked like this, when used with
#   the 'hard' podAntiAffinityPreset..
#
#   podAntiAffinity:
#     requiredDuringSchedulingIgnoredDuringExecution:
#     - labelSelector:
#         matchLabels:
#           app.kubernetes.io/instance: mayastor
#           app.kubernetes.io/name: etcd
#       topologyKey: kubernetes.io/hostname
#
#   v9.0.0 introduced a new label, 'app.kubernetes.io/component: etcd'. This was put into the StatefulSet labels,
#   the .spec.selector, the PodSpec labels, and the podAntiAffinity section. So, v9.0.0 onwards, this is what the
#   podAntiAffinity labels look like..
#
#   matchLabels:
#     app.kubernetes.io/component: etcd
#     app.kubernetes.io/instance: mayastor
#     app.kubernetes.io/name: etcd
#
#   The instructions to upgrade to v9.0.0 were as follows --
#   1. Label the Etcd Pods with the new label
#   2. Delete the Etcd StatefulSet with the `--cascade=orphan` on the the kubectl binary
#   3. Helm upgrade
#   Step 2 was necessary because StatefulSet selectors and affinity are immutable. The idea is that we delete the
#   StatefulSet, leaving the Pods behind. We add the new label to the Pods (1) so that the new StatefulSet can find
#   them using the new set of selector labels. We run `helm upgrade` and the new StatefulSet comes up and reclaims the
#   Pods. Then it uses its StatefulSet rollout strategy to bring down the Pods one-by-one and update their affinity.
#   Because of the orphan cascade option, the Pods don't come down all at once and there is no full blown downtime.
#
# v11.0.0 introduces a pre-upgrade Job hook on the Etcd chart. The podAntiAffinity on the pre-upgrade Job Pods uses the
#   'soft' podAntiAffinityPreset and so it is not of much concern to us. The Job Pods are stateless and they don't need
#   the podAntiAffinity w.r.t. the Etcd StatefulSet Pods. The labels on the Job Pods look like this..
#
#   labels:
#     app.kubernetes.io/component: etcd-pre-upgrade-job
#     app.kubernetes.io/instance: mayastor
#     app.kubernetes.io/name: etcd
#     ..
#
#   What stands out here is that the Job Pods don't match the criteria for podAntiAffinity of the v11.x Etcd chart, but
#   they meet the criteria for the Etcd charts < v9.0.0. This means that to successfully upgrade to a v11.x or newer
#   chart, Mayastor users would require at least one extra node, where they don't have any Etcd Pod scheduled.
#
# But, that's easily fixable with the v9.0.0 upgrade steps.. we label the Pods, we delete the StatefulSet and orphan the
#   Pods, and after helm upgrade the new StatefulSet comes up and fixes everything, right? No! The new StatefulSet gets
#   to come up only after the Etcd pre-upgrade Job is through. And for users for whom, no. of nodes == Etcd replicas,
#   and they're on v8.6.0, they can't get the pre-upgrade Job to schedule. So, they can only fix podAntiAffinity w.r.t.
#   the pre-upgrade Job after the pre-upgrade Job has been scheduled and has run to completion.
#
# The solution for this problem, on this script, is to grab the existing 8.6.0 Etcd StatefulSet object, plug the labels
#   in, set the cluster env to join an existing cluster. Then we follow the usual v9.x-like flow, we label the Pods and
#   delete the StatefulSet, while orphaning the Pods. Then we re-create the labelled StatefulSet. The new 8.6.0
#   StatefulSet's rollout controller will fix the affinity on the Pods one-by-one and bring them up. All of this happens
#   as a part of a Mayastor chart pre-upgrade Job, which runs before the Etcd one. And then the Etcd Job schedules easy
#   peasy. Success!

set -o errexit

# Write output to stdout.
# Arguments:
#   $1 -- Message
# Returns:
#   None
log() {
  echo "${1}"
}

# Write log output along with Kubernetes Namespace, if any.
# Arguments:
#   $1 -- Message
#   $2 -- Namespace (optional)
# Returns:
#   None
log_with_ns() {
  message="$1"
  namespace="${2:-$NAMESPACE}"

  printf "%s" "$message"
  if [ -n "$namespace" ]; then
    printf ", in namespace %s" "$namespace"
  fi

  # final newline
  printf '\n'
}

# Write output to stderr output stream.
# Arguments:
#   $1 -- Message
# Returns:
#   None
log_to_stderr() {
  echo "${1}" >&2
}

# Print log message as an error message.
# Arguments:
#   $1 -- Output message
# Returns:
#   None
log_error() {
  log_to_stderr "ERROR: $1"
}

# Exit with error status and print error.
# Arguments:
#   $1 -- Output message
#   $2 -- Exit code (default: 1)
# Returns:
#   None
log_fatal() {
  _return="${2:-1}"
  log_error "$1"
  exit "${_return}"
}

# Print the help text for this script.
# Arguments:
#   None
# Returns:
#   None
print_help() {
    cat <<EOF
Usage: $0 [OPTIONS] <helm_release_name>

  <helm_release_name>          (required) The release name of the helm
                               release whose Etcd

Options:
  -h, --help                   Display this text.
  -n, --namespace <namespace>  Set the kubernetes namespace of the Etcd
                               cluster. (default: )

Examples:
  $0 -n mayastor openebs-mayastor
EOF
}

# Parse inputs to this script.
# Arguments:
#   $@ -- Shell args
# Returns:
#   None
parse_args() {
  while test "$#" -gt 0; do
    arg="$1"
    case "$arg" in
    --)
      shift
      break
      ;;
    -n* | --namespace*)
      case "$arg" in
      -n | --namespace)
        test $# -lt 2 && log_fatal "missing value for the optional argument '$arg'."
        NAMESPACE=$2
        shift
        ;;
      *)
        NAMESPACE=${arg#*=}
        ;;
      esac
      ;;
    -h* | --help*)
      print_help
      exit 0
      ;;
    -*)
      print_help
      log_fatal "unexpected argument '$arg'"
      ;;
    *)
      if [ -z "$RELEASE_NAME" ]; then
        RELEASE_NAME=$arg
      else
        print_help
        log_fatal "unexpected extra argument '$arg'"
      fi
      ;;
    esac
    shift
  done

  # Handling args after the "--".
  for arg; do
    if [ -z "$RELEASE_NAME" ]; then
      RELEASE_NAME=$arg
    else
      print_help
      log_fatal "unexpected extra argument: '$arg'"
    fi
  done

  # Handling missing arguments.
  if [ -z "$RELEASE_NAME" ]; then
    print_help
    log_fatal "missing required <helm_release_name>"
  fi
}

# Run kubectl with namespace arg, if any.
# Arguments:
#   kubectl command arguments and options related to namespaced resources
# Returns:
#   kubectl command outout
kubectl_ns() {
  if [ -n "$NAMESPACE" ]; then
    "$KUBECTL" -n "$NAMESPACE" "$@"
  else
    "$KUBECTL"
  fi
}

# Prints the Yaml to an Etcd StatefulSet
# Arguments:
#   $1 -- Kubernetes label selector for the Etcd StatefulSet
# Returns:
#   Etcd StatefulSet YAML
get_etcd_sts_yaml_or_die() {
  etcd_selector=$1

  sts_name="$(kubectl_ns get sts -l "${etcd_selector}" \
-o jsonpath='{range .items[*]}{.metadata.name}{"\n"}{end}')" || exit $?

  # Make sure that there's just one such StatefulSet.
  name_count=$(printf "%s" "$sts_name" | grep -c "^")
  if [ "$name_count" -eq 0 ]; then
    log_with_ns "Nothing to do: no such StatefulSet"
    exit 0
  elif [ "$name_count" -gt 1 ]; then
    log_fatal "$(log_with_ns "expected 1 but found $name_count StatefulSets: $sts_name")"
  fi

  EXISTING_ETCD_STS="$(kubectl_ns get sts "$sts_name" -o yaml)"
}

# =============================================================================

KUBECTL="kubectl"
NAMESPACE=
# Mandatory input.
RELEASE_NAME=
EXISTING_ETCD_STS=

parse_args "$@"

# The 'app.kubernetes.io/component!=etcd' selector makes sure we're not picking up a StatefulSet
#   which already has the label.
# The 'helm.sh/chart=etcd-8.6.0' selector makes sure we get 8.6.0 and nothing else.
etcd_selector="app.kubernetes.io/name=etcd,\
app.kubernetes.io/instance=${RELEASE_NAME},\
helm.sh/chart=etcd-8.6.0,\
app.kubernetes.io/component!=etcd"

# This step needs to happen in the outer scope of this script, so that it's able to exit when it fails.
# This will exit 0 if there is no StatefulSet of 8.6.0 and w/o the label. This makes the create step idempotent.
get_etcd_sts_yaml_or_die "$etcd_selector"

# This step does these few things..
#  - Add 'app.kubernetes.io/component: etcd' to .metadata.labels
#  - Add 'app.kubernetes.io/component: etcd' to .spec.selector
#  - Add 'app.kubernetes.io/component: etcd' to the PodSpec's .metadata.labels
#  - Add 'app.kubernetes.io/component: etcd' to podAntiAffinity's matchLabels
#  - Change the value of ETCD_INITIAL_CLUSTER_STATE to 'existing'
modified_etcd_yaml="$(echo "$EXISTING_ETCD_STS" | \
awk -v label="app.kubernetes.io/component: etcd" \
    -v init_state='value: "existing"' '
  # As soon as we hit "status:", stop processing.
  /^[[:space:]]*status:/ { exit }

  # If pending==1, this is the line after ETCD_INITIAL_CLUSTER_STATE.
  pending {
    print indent "  " init_state
    pending = 0
    next
  }

  # Match the name=etcd label, capture indent, insert component label line.
  $0 ~ /^[[:space:]]*app\.kubernetes\.io\/name: etcd$/ {
    match($0, /^[[:space:]]*/)
    prefix = substr($0, RSTART, RLENGTH)
    print
    print prefix label
    next
  }

  # Match the ETCD_INITIAL_CLUSTER_STATE entry, capture indent,
  # and mark pending so we replace the next line.
  $0 ~ /^[[:space:]]*- name: ETCD_INITIAL_CLUSTER_STATE/ {
    match($0, /^[[:space:]]*/)
    indent = substr($0, RSTART, RLENGTH)
    print
    pending = 1
    next
  }

  # All other lines pass through unchanged.
  { print }
')" || exit $?

set -o xtrace
# Label the Etcd StatefulSet Pods. This step is idempotent all by itself.
kubectl_ns label --overwrite po -l "$etcd_selector" app.kubernetes.io/component=etcd
# Delete the StatefulSet, leaving the Pods behind. The --ignore-not-found makes the delete step idempotent.
kubectl_ns delete sts -l "$etcd_selector" --cascade=orphan --ignore-not-found
# Create the StatefulSet with the new label. This is idempotent because we exit early if we find no StatefulSet.
echo "$modified_etcd_yaml" | "$KUBECTL" create -f -

set +o errexit +o xtrace
