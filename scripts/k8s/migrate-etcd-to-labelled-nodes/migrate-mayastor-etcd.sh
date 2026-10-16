#!/usr/bin/env bash
#
# migrate-mayastor-etcd.sh
#
# Relocate the members of the Mayastor etcd StatefulSet onto a labelled set of
# nodes, ONE MEMBER AT A TIME, with quorum/health/consistency gates between
# every step.
#
# Verified against openebs/openebs 4.2.0 (mayastor 2.8.0, bitnami etcd subchart
# 8.6.0, etcd 3.5.6) with replicaCount=2 and replicaCount=3. The assumptions it
# relies on are asserted in preflight(), not just documented here:
#
#   * etcd runs as StatefulSet <release>-etcd with ordinal pods and a
#     <release>-etcd-headless service; member name == pod name.
#   * auth.rbac.create=false / allowNoneAuthentication=true, so etcdctl needs
#     no credentials.
#   * removeMemberOnContainerTermination=false. Mayastor disables the PreStop
#     hook, so NOTHING removes membership for you. Deleting a pod+PVC without
#     an explicit `member remove` leaves a member whose ID no longer matches
#     its (now empty) data dir -> cluster ID mismatch -> CrashLoopBackOff.
#     That is why step 2 of each move is a manual member remove.
#   * persistence via mayastor-etcd-localpv: hostpath, WaitForFirstConsumer,
#     reclaimPolicy Delete. The PV is pinned to its node, so a member cannot
#     move without destroying its PVC. WaitForFirstConsumer is what lets the
#     scheduler pick the new node before the volume is provisioned.
#   * podAntiAffinityPreset=hard, so you need at least replicaCount
#     schedulable labelled nodes.
#
# PREREQUISITE -- apply this with helm BEFORE running the script. For the
# openebs umbrella chart everything below nests under `mayastor:`.
#
#   etcd:
#     nodeAffinityPreset:
#       type: hard
#       key: "node/etcd.io"
#       values: ["true"]
#     podAntiAffinityPreset: "hard"     # keep; do not set etcd.affinity
#     updateStrategy:
#       type: OnDelete
#     initialClusterState: "existing"
#
# OnDelete is the entire safety model. Under RollingUpdate the StatefulSet
# controller recycles pods on its own schedule into PVCs pinned to the old
# nodes, stalls, and leaves you degraded. This script refuses to run without
# it.
#
# initialClusterState is the other half. The bitnami entrypoint branches on
# `is_new_etcd_cluster`, which is true when ETCD_INITIAL_CLUSTER_STATE=new and
# the pod's own peer URL appears in ETCD_INITIAL_CLUSTER -- i.e. true for every
# member of a fresh install. A member whose data directory we just destroyed
# would take that branch and bootstrap a brand-new cluster over the keyspace.
# Pinning the value to "existing" forces the `add_self_to_cluster` branch
# instead. The chart only emits the variable when replicaCount > 1.
#
# DO NOT CORDON THE OLD NODES. With reclaimPolicy=Delete the localpv
# provisioner runs a cleanup pod on the vacated node to remove the hostpath
# directory. A cordoned node cannot run it, leaving Released PVs and orphaned
# etcd data. The node affinity already does the job cordoning would.
#
# Usage:
#   DRY_RUN=true  ./migrate-mayastor-etcd.sh     # default: inspect only
#   DRY_RUN=false ./migrate-mayastor-etcd.sh     # actually move members
#   DRY_RUN=false ONLY_ORDINAL=2 ./migrate-mayastor-etcd.sh   # one at a time
#
# Exit codes:
#   1  preflight / usage failure
#   5  timed out waiting for a pod or for resync
#   6  member remove failed
#   24 health gate failed
#   25 consistency violation -- cluster identity or keyspace changed under us
#   30 pod repeatedly scheduled onto an unlabelled node
#
set -euo pipefail

# ---------------------------------------------------------------- config ----

NS="${NS:-mayastor}"                       # namespace holding the StatefulSet
STS="${STS:-mayastor-etcd}"                # StatefulSet name
CN="${CN:-etcd}"                           # container name inside the pods
LABEL_KEY="${LABEL_KEY:-node/etcd.io}"     # node label key to migrate onto
LABEL_VALUE="${LABEL_VALUE:-true}"         # node label value

TIMEOUT_SEC="${TIMEOUT_SEC:-900}"          # bound on every wait loop
SLEEP_SEC="${SLEEP_SEC:-5}"                # poll interval
SETTLE_SEC="${SETTLE_SEC:-30}"             # quiet period after each move
MAX_RESCHEDULE_KICKS="${MAX_RESCHEDULE_KICKS:-6}"

DRY_RUN="${DRY_RUN:-true}"                 # SAFE DEFAULT: change nothing
SNAPSHOT="${SNAPSHOT:-true}"               # take an etcd snapshot first
SNAPSHOT_DIR="${SNAPSHOT_DIR:-./etcd-snapshots}"
ONLY_ORDINAL="${ONLY_ORDINAL:-}"           # restrict to a single ordinal
ICS_OVERRIDE="${ICS_OVERRIDE:-false}"      # proceed without initialClusterState

REPLICAS=0
declare -a PODS=()
declare -a LABELLED=()

# Cluster identity and keyspace census, captured once before the first move and
# re-asserted after every step. BASE_CLUSTER_ID changing, or BASE_REVISION going
# backwards, is the signature of a member having bootstrapped over the keyspace
# rather than rejoining it -- the one failure this whole procedure exists to
# avoid. Neither can be explained away, so both are hard stops.
BASE_CLUSTER_ID=""
BASE_REVISION=""
BASE_KEYS_FILE=""

# --------------------------------------------------------------- helpers ----

# Deliberately self-contained -- do NOT replace these with scripts/utils/log.sh.
# This runs against a live cluster, often from a jump host, and an operator
# should be able to copy the single file and run it. Sourcing anything from the
# repo would make that fail. The sibling replace-lagging-etcd-member script is
# self-contained for the same reason.
#
# The timestamps are not decoration either: the procedure is a sequence of
# gated, destructive steps, and lining a gate up against `kubectl get events` or
# a pod's restart time is how you work out what happened.
say()  { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*"; }
warn() { printf '[%s] WARN  %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
die()  { printf '[%s] FATAL %s\n' "$(date +%H:%M:%S)" "$1" >&2; exit "${2:-1}"; }

need_cmd() {
  local c
  for c in "$@"; do
    if ! command -v "$c" >/dev/null 2>&1; then die "required command not found: $c"; fi
  done
}

# Run a mutating command, or describe it under DRY_RUN.
mutate() {
  if [[ "$DRY_RUN" == "true" ]]; then
    say "  DRY-RUN: would run: $*"
    return 0
  fi
  "$@"
}

k() { kubectl -n "$NS" "$@"; }

etcd_exec() {                              # <pod> <etcdctl args...>
  local pod="$1"; shift
  k exec "$pod" -c "$CN" -- env ETCDCTL_API=3 etcdctl \
      --dial-timeout=5s --command-timeout=15s "$@"
}

ep_for() { printf 'http://%s.%s-headless.%s.svc.cluster.local:2379' "$1" "$STS" "$NS"; }

eps_excluding() {                          # <pods to exclude...> -> CSV
  local -a out=()
  local p a skip
  for p in "${PODS[@]}"; do
    skip=0
    for a in "$@"; do
      if [[ -n "$a" && "$p" == "$a" ]]; then skip=1; fi
    done
    if (( skip )); then continue; fi
    out+=("$(ep_for "$p")")
  done
  ( IFS=,; printf '%s' "${out[*]}" )
}

# The StatefulSet name lands inside both a jq regex and an awk regex. Release
# names are DNS labels so only '-' and '.' can reach us, but '.' as a wildcard
# would let <release>-etcd match <release>xetcd. Escape it rather than trust it.
rx_quote() { printf '%s' "$1" | sed -e 's#[^a-zA-Z0-9_-]#\\&#g'; }

# etcd member IDs and cluster IDs are uint64 and routinely exceed 2^53, which
# jq (<1.7) mangles through double precision. Quote them into JSON strings
# before jq parses, so equality comparisons stay exact.
quote_bigints() {
  sed -E 's/"(member_id|leader|cluster_id)":[[:space:]]*([0-9]+)/"\1":"\2"/g'
}

# etcdctl's `endpoint status`/`endpoint health` exit non-zero if ANY endpoint
# failed, but still emit results for the ones that answered. Callers want the
# partial answer -- the gates decide for themselves whether what came back is
# enough -- so run the command, keep stdout, and drop the exit status.
#
# NOTE: deliberately NO `--cluster`. That flag discards --endpoints and rebuilds
# the list from the member list, which here means (a) the exclude argument is
# silently ignored and (b) every member's SECOND advertised client URL, the
# load-balanced `<release>-etcd.<ns>.svc` ClusterIP, joins the query and answers
# as whichever pod it happens to route to. Explicit endpoints give exactly one
# row per pod and make exclusion real.
etcdctl_soft() {                           # <pod> <etcdctl args...> -> stdout
  local pod="$1"; shift
  etcd_exec "$pod" "$@" 2>/dev/null || true
}

# Pick a pod that can answer etcdctl. Iterates highest ordinal first so that
# ${STS}-0 -- the one with special bootstrap semantics in the bitnami
# entrypoint -- is only used as a last resort.
ctrl_pod() {                               # <pods to avoid...> -> pod name
  local i p a skip phase
  for (( i=${#PODS[@]}-1; i>=0; i-- )); do
    p="${PODS[$i]}"
    skip=0
    for a in "$@"; do
      if [[ -n "$a" && "$p" == "$a" ]]; then skip=1; fi
    done
    if (( skip )); then continue; fi
    phase="$(k get pod "$p" -o jsonpath='{.status.phase}' 2>/dev/null || true)"
    if [[ "$phase" != "Running" ]]; then continue; fi
    if etcd_exec "$p" endpoint status -w json >/dev/null 2>&1; then
      printf '%s' "$p"
      return 0
    fi
  done
  return 1
}

# One TSV row per reachable member:
#   <pod> <member_id> <revision> <term> <leader_id> <cluster_id>
cluster_view() {                           # <ctrl_pod> [exclude_pod]
  local pod="$1" exclude="${2:-}" json eps sts_rx
  eps="$(eps_excluding "$exclude")"
  if [[ -z "$eps" ]]; then return 1; fi
  json="$(etcdctl_soft "$pod" --endpoints="$eps" endpoint status -w json)"
  if [[ -z "$json" ]]; then return 1; fi
  if ! printf '%s' "$json" | jq -e 'type=="array" and length>0' >/dev/null 2>&1; then
    return 1
  fi
  sts_rx="$(rx_quote "$STS")"
  printf '%s' "$json" | quote_bigints | jq -r --arg sts "$sts_rx" '
    .[]
    | select(.Endpoint | test("//\($sts)-[0-9]+\\."))
    | [ (.Endpoint | capture("//(?<p>[^.:/]+)\\.").p),
        (.Status.header.member_id // "0"),
        ((.Status.header.revision // 0) | tostring),
        (((.Status.header.raft_term // .Status.raftTerm) // 0) | tostring),
        (.Status.leader // "0"),
        (.Status.header.cluster_id // "0") ]
    | @tsv'
}

# Pod names whose endpoint reports healthy. Pod names are unique, so matching
# on the hostname in the endpoint URL deduplicates implicitly.
healthy_pods() {                           # <ctrl_pod> [exclude_pod]
  local pod="$1" exclude="${2:-}" out eps sts_rx
  eps="$(eps_excluding "$exclude")"
  if [[ -z "$eps" ]]; then return 0; fi
  out="$(etcd_exec "$pod" --endpoints="$eps" endpoint health 2>&1 || true)"
  sts_rx="$(rx_quote "$STS")"
  printf '%s\n' "$out" | awk -v sts="$sts_rx" '
    /is healthy/ {
      if (match($0, "//" sts "-[0-9]+\\.")) print substr($0, RSTART+2, RLENGTH-3)
    }'
}

# <pod> <hash> <compact_revision> per member. Two members that agree on the
# revision but disagree here are not holding the same data, which no amount of
# "healthy" reporting would tell you.
#
# The revision is pinned with --rev rather than letting each member hash
# "whatever it holds right now". On a keyspace that is still being written --
# which Mayastor's is, whenever volumes or pools are changing -- an unpinned
# hash samples the two members microseconds apart and reports a mismatch that
# means nothing. Pinning makes the comparison deterministic. Retention is 100
# revisions, so the revision we just agreed on is never compacted away.
keyspace_hashes() {                        # <ctrl_pod> <revision> [exclude_pod]
  local pod="$1" rev="$2" exclude="${3:-}" json eps sts_rx
  eps="$(eps_excluding "$exclude")"
  if [[ -z "$eps" ]]; then return 1; fi
  json="$(etcdctl_soft "$pod" --endpoints="$eps" endpoint hashkv --rev "$rev" -w json)"
  if [[ -z "$json" ]]; then return 1; fi
  if ! printf '%s' "$json" | jq -e 'type=="array" and length>0' >/dev/null 2>&1; then
    return 1
  fi
  sts_rx="$(rx_quote "$STS")"
  printf '%s' "$json" | jq -r --arg sts "$sts_rx" '
    .[]
    | select(.Endpoint | test("//\($sts)-[0-9]+\\."))
    | [ (.Endpoint | capture("//(?<p>[^.:/]+)\\.").p),
        ((.HashKV.hash // 0) | tostring),
        ((.HashKV.compact_revision // 0) | tostring) ]
    | @tsv'
}

# Strict gate: every expected member must be present, live, in the SAME etcd
# cluster, sharing one raft term, agreeing on one leader, converged on one
# revision, and holding a byte-identical keyspace. Stricter than a bare quorum
# check on purpose -- a deliberate migration should never start from a degraded
# cluster.
gate_healthy() {                           # [exclude_pod]
  local exclude="${1:-}"
  local -a expect=()
  local p
  for p in "${PODS[@]}"; do
    if [[ "$p" == "$exclude" ]]; then continue; fi
    expect+=("$p")
  done

  local ctrl view
  ctrl="$(ctrl_pod "$exclude")" || { warn "  no pod able to answer etcdctl"; return 1; }
  view="$(cluster_view "$ctrl" "$exclude")" || { warn "  endpoint status failed via $ctrl"; return 1; }

  local -A MID=() REV=() TERM=() LDR=() CID=()
  local pod mid rev term ldr cid
  while IFS=$'\t' read -r pod mid rev term ldr cid; do
    if [[ -z "$pod" || "$pod" == "$exclude" ]]; then continue; fi
    MID["$pod"]="$mid"; REV["$pod"]="$rev"; TERM["$pod"]="$term"
    LDR["$pod"]="$ldr"; CID["$pod"]="$cid"
  done <<< "$view"

  for p in "${expect[@]}"; do
    if [[ -z "${REV[$p]:-}" ]]; then warn "  no status from $p"; return 1; fi
  done

  # Cluster identity. A member that bootstrapped its own cluster instead of
  # rejoining ours reports a different cluster_id while looking perfectly
  # healthy in isolation. This is the check that catches it.
  local cid0=""
  for p in "${expect[@]}"; do
    if [[ -z "$cid0" ]]; then cid0="${CID[$p]}"; fi
    if [[ "${CID[$p]}" != "$cid0" ]]; then
      warn "  SPLIT BRAIN: $p is in cluster ${CID[$p]}, expected $cid0"
      return 1
    fi
  done
  if [[ -n "$BASE_CLUSTER_ID" && "$cid0" != "$BASE_CLUSTER_ID" ]]; then
    die "cluster_id changed from $BASE_CLUSTER_ID to $cid0 -- the keyspace has been replaced, STOP" 25
  fi

  # One leader, and every member must name the same one. Taking the first
  # non-zero leader and checking only that it is "one of ours" would accept a
  # split brain where each half elected itself.
  local leader=""
  for p in "${expect[@]}"; do
    ldr="${LDR[$p]}"
    if [[ "$ldr" == "0" ]]; then warn "  $p reports no leader (election in progress?)"; return 1; fi
    if [[ -z "$leader" ]]; then leader="$ldr"; fi
    if [[ "$ldr" != "$leader" ]]; then
      warn "  members disagree on the leader: $p says $ldr, expected $leader"
      return 1
    fi
  done
  local leader_ok=0
  for p in "${expect[@]}"; do
    if [[ "${MID[$p]}" == "$leader" ]]; then leader_ok=1; fi
  done
  if (( ! leader_ok )); then warn "  leader $leader is not one of the expected members"; return 1; fi

  local t0="" minr="" maxr="" r
  for p in "${expect[@]}"; do
    if [[ -z "$t0" ]]; then t0="${TERM[$p]}"; fi
    if [[ "${TERM[$p]}" != "$t0" ]]; then
      warn "  raft term mismatch: $p=${TERM[$p]} expected $t0 (election in progress?)"
      return 1
    fi
    r="${REV[$p]}"
    if [[ -z "$minr" ]]; then minr="$r"; maxr="$r"; fi
    if (( r < minr )); then minr="$r"; fi
    if (( r > maxr )); then maxr="$r"; fi
  done
  if (( minr != maxr )); then
    warn "  revisions not converged: min=$minr max=$maxr (still catching up)"
    return 1
  fi

  # The keyspace only ever moves forward. Going backwards means somebody
  # restored or re-bootstrapped underneath us.
  if [[ -n "$BASE_REVISION" ]] && (( maxr < BASE_REVISION )); then
    die "revision went backwards: $maxr < $BASE_REVISION -- the keyspace has been rolled back, STOP" 25
  fi

  local -a hp=()
  mapfile -t hp < <(healthy_pods "$ctrl" "$exclude")
  local n=0 h found
  for p in "${expect[@]}"; do
    found=0
    for h in "${hp[@]}"; do
      if [[ "$h" == "$p" ]]; then found=1; fi
    done
    if (( found )); then n=$(( n + 1 )); fi
  done
  if (( n != ${#expect[@]} )); then
    warn "  only $n/${#expect[@]} members report healthy"
    return 1
  fi

  # Same revision is necessary but not sufficient: compare the actual keyspace.
  local hashes hpod hhash hcomp h0="" c0=""
  hashes="$(keyspace_hashes "$ctrl" "$maxr" "$exclude")" || { warn "  endpoint hashkv failed"; return 1; }
  local -A SEEN=()
  while IFS=$'\t' read -r hpod hhash hcomp; do
    if [[ -z "$hpod" || "$hpod" == "$exclude" ]]; then continue; fi
    SEEN["$hpod"]=1
    if [[ -z "$h0" ]]; then h0="$hhash"; c0="$hcomp"; fi
    if [[ "$hhash" != "$h0" || "$hcomp" != "$c0" ]]; then
      warn "  keyspace hash mismatch: $hpod=$hhash/$hcomp expected $h0/$c0"
      return 1
    fi
  done <<< "$hashes"
  for p in "${expect[@]}"; do
    if [[ -z "${SEEN[$p]:-}" ]]; then warn "  no keyspace hash from $p"; return 1; fi
  done

  say "  OK: ${#expect[@]} member(s), cluster=$cid0, leader=$leader, term=$t0, revision=$maxr, hashkv=$h0"
  return 0
}

wait_gate() {                              # <message> [exclude_pod]
  local msg="$1" exclude="${2:-}" deadline
  deadline=$(( $(date +%s) + TIMEOUT_SEC ))
  say "$msg"
  while :; do
    if gate_healthy "$exclude"; then return 0; fi
    if (( $(date +%s) >= deadline )); then return 1; fi
    sleep "$SLEEP_SEC"
  done
}

# Hex member ID as text, straight from `member list` -- never through jq, so
# no uint64 precision loss.
member_id_for() {                          # <pod> <ctrl_pod>
  local pod="$1" ctrl="$2" out id peer
  out="$(etcd_exec "$ctrl" --endpoints="$(eps_excluding "$pod")" member list 2>/dev/null)" || return 1
  # format: <hex id>, started, <name>, <peerURLs>, <clientURLs>, <isLearner>
  id="$(printf '%s\n' "$out" | awk -F', *' -v n="$pod" '$3==n {gsub(/[ \t]/,"",$1); print $1; exit}')"
  if [[ -z "$id" ]]; then
    # An added-but-not-yet-started member has an empty NAME column; match its
    # peer URL instead.
    peer="http://${pod}.${STS}-headless.${NS}.svc.cluster.local:2380"
    id="$(printf '%s\n' "$out" | awk -F', *' -v u="$peer" 'index($4,u)>0 {gsub(/[ \t]/,"",$1); print $1; exit}')"
  fi
  if [[ -z "$id" ]]; then return 1; fi
  printf '%s' "$id"
}

labelled_nodes() {
  kubectl get nodes -l "${LABEL_KEY}=${LABEL_VALUE}" \
    -o jsonpath='{range .items[*]}{.metadata.name}{"\n"}{end}'
}

is_labelled_node() {                       # <node>
  local n
  for n in "${LABELLED[@]}"; do
    if [[ "$n" == "$1" ]]; then return 0; fi
  done
  return 1
}

pod_node() { k get pod "$1" -o jsonpath='{.spec.nodeName}' 2>/dev/null || true; }
pod_uid()  { k get pod "$1" -o jsonpath='{.metadata.uid}' 2>/dev/null || true; }

# -------------------------------------------------------------- preflight ----

preflight() {
  need_cmd kubectl jq awk sed
  if (( BASH_VERSINFO[0] < 4 )); then die "bash >= 4 required (associative arrays)"; fi

  if ! k get sts "$STS" >/dev/null 2>&1; then die "StatefulSet $NS/$STS not found"; fi

  REPLICAS="$(k get sts "$STS" -o jsonpath='{.spec.replicas}')"
  if ! [[ "$REPLICAS" =~ ^[0-9]+$ ]] || (( REPLICAS < 1 )); then
    die "could not read a sane .spec.replicas from $NS/$STS (got '${REPLICAS:-<empty>}')"
  fi
  PODS=()
  local i
  for (( i=0; i<REPLICAS; i++ )); do PODS+=("$STS-$i"); done
  say "StatefulSet $NS/$STS, replicas=$REPLICAS"

  # Fault tolerance during the move is (quorum of the REMAINING members). At
  # replicaCount<=2 there is none: the survivors are a bare quorum, and the
  # window where the rejoining member has been `member add`ed but is not yet
  # live is a window where the cluster cannot commit writes at all.
  if (( REPLICAS < 3 )); then
    warn "replicaCount=$REPLICAS: this cluster has NO fault tolerance during the move."
    warn "  After the member remove the survivor(s) are a bare quorum, and while the"
    warn "  rejoining member is being added back etcd briefly cannot commit writes."
    warn "  Mayastor tolerates this (already-published volumes keep serving I/O and"
    warn "  the control plane retries), but do not run this alongside other work."
  fi
  if (( REPLICAS % 2 == 0 )); then
    warn "replicaCount=$REPLICAS is even: $(( REPLICAS / 2 + 1 )) of $REPLICAS must be up for quorum, so an even count buys no extra tolerance over $(( REPLICAS - 1 ))."
  fi

  if [[ -n "$ONLY_ORDINAL" ]]; then
    if ! [[ "$ONLY_ORDINAL" =~ ^[0-9]+$ ]] || (( ONLY_ORDINAL >= REPLICAS )); then
      die "ONLY_ORDINAL='$ONLY_ORDINAL' is not a valid ordinal for a $REPLICAS-replica StatefulSet (expected 0..$(( REPLICAS - 1 )))"
    fi
  fi

  local chart appver
  chart="$(k get sts "$STS" -o jsonpath='{.metadata.labels.helm\.sh/chart}' 2>/dev/null || true)"
  appver="$(k get sts "$STS" -o jsonpath='{.metadata.labels.app\.kubernetes\.io/version}' 2>/dev/null || true)"
  say "chart=${chart:-<none>} appVersion=${appver:-<none>}"
  if [[ "$chart" != etcd-8.* ]]; then
    warn "expected bitnami etcd chart 8.x (Mayastor 2.8.0); saw '${chart:-<none>}'."
    warn "Re-check the bootstrap semantics before trusting this script on another chart major."
  fi

  # OnDelete is the whole point -- without it the controller recycles pods into
  # PVCs pinned to the old nodes on its own schedule.
  local us
  us="$(k get sts "$STS" -o jsonpath='{.spec.updateStrategy.type}')"
  if [[ "$us" != "OnDelete" ]]; then
    die "updateStrategy.type is '${us:-<unset>}', need OnDelete. Apply the helm values in the header first, then confirm with: kubectl -n $NS get sts $STS -o jsonpath='{.spec.updateStrategy}'"
  fi
  say "updateStrategy=OnDelete"

  local spec
  spec="$(k get sts "$STS" -o json)"

  # The new node affinity must actually be on the template, or nothing moves.
  #
  # Check the VALUE too, not just the key. Matching on the key alone lets a
  # mismatch through -- template says values ["true"], LABEL_VALUE says
  # "primary" -- and that combination passes preflight happily: the key is
  # present, and the nodes you labelled do carry key=primary. The member is then
  # removed and its data destroyed before anyone discovers the replacement can
  # never satisfy an affinity asking for "true".
  local -a tmpl_values=()
  mapfile -t tmpl_values < <(printf '%s' "$spec" | jq -r --arg k "$LABEL_KEY" '
    [ ( .spec.template.spec.affinity.nodeAffinity
        | (.requiredDuringSchedulingIgnoredDuringExecution.nodeSelectorTerms // [])[]?
        | (.matchExpressions // [])[]? | select(.key == $k) | (.values // [])[]? ),
      ( .spec.template.spec.affinity.nodeAffinity
        | (.preferredDuringSchedulingIgnoredDuringExecution // [])[]?
        | .preference | (.matchExpressions // [])[]? | select(.key == $k) | (.values // [])[]? ),
      ( (.spec.template.spec.nodeSelector // {}) | to_entries[] | select(.key == $k) | .value )
    ] | unique[]' 2>/dev/null || true)
  if (( ${#tmpl_values[@]} == 0 )); then
    die "pod template does not select on '$LABEL_KEY' in nodeAffinity or nodeSelector; run the helm upgrade first (etcd.nodeAffinityPreset.key must be '$LABEL_KEY')"
  fi
  local tv hit=0
  for tv in "${tmpl_values[@]}"; do
    if [[ "$tv" == "$LABEL_VALUE" ]]; then hit=1; fi
  done
  if (( ! hit )); then
    die "pod template selects $LABEL_KEY in (${tmpl_values[*]}), but you asked to migrate onto $LABEL_KEY=$LABEL_VALUE. The nodes you labelled would never satisfy the affinity. Make etcd.nodeAffinityPreset.values match LABEL_VALUE."
  fi
  say "nodeAffinity selects $LABEL_KEY=$LABEL_VALUE"

  # Mayastor sets podAntiAffinityPreset=hard. Setting etcd.affinity directly
  # silently discards it, which with hostpath PVs is how you end up with two
  # members on one node.
  if ! printf '%s' "$spec" | jq -e '.spec.template.spec.affinity.podAntiAffinity' >/dev/null; then
    die "pod template has no podAntiAffinity. Mayastor sets podAntiAffinityPreset=hard; you have probably overridden etcd.affinity, which disables all three presets. Use etcd.nodeAffinityPreset instead."
  fi
  say "podAntiAffinity present"

  local ics
  ics="$(printf '%s' "$spec" | jq -r --arg cn "$CN" '
    .spec.template.spec.containers[] | select(.name==$cn)
    | (.env // [])[] | select(.name=="ETCD_INITIAL_CLUSTER_STATE") | .value' 2>/dev/null || true)"
  if [[ "$ics" != "existing" ]]; then
    if [[ "$ICS_OVERRIDE" == "true" ]]; then
      warn "ETCD_INITIAL_CLUSTER_STATE='${ics:-<unset>}'; relying on the bitnami entrypoint to auto-detect (ICS_OVERRIDE=true)"
    else
      die "ETCD_INITIAL_CLUSTER_STATE is '${ics:-<unset>}', expected 'existing'. Set etcd.initialClusterState='existing' via helm. This is what stops a rejoining member -- especially ${STS}-0 -- from being misread as a fresh cluster and bootstrapping over the keyspace. Set ICS_OVERRIDE=true to proceed anyway."
    fi
  else
    say "ETCD_INITIAL_CLUSTER_STATE=existing"
  fi

  local sc bm rp
  sc="$(printf '%s' "$spec" | jq -r '.spec.volumeClaimTemplates[0].spec.storageClassName // empty')"
  if [[ -z "$sc" ]]; then die "could not read the volumeClaimTemplate storageClassName"; fi
  bm="$(kubectl get sc "$sc" -o jsonpath='{.volumeBindingMode}')"
  rp="$(kubectl get sc "$sc" -o jsonpath='{.reclaimPolicy}')"
  say "storageClass=$sc bindingMode=$bm reclaimPolicy=$rp"
  if [[ "$bm" != "WaitForFirstConsumer" ]]; then
    die "storageClass $sc has volumeBindingMode=$bm. Relocation needs WaitForFirstConsumer so the scheduler picks the node before the hostpath is provisioned."
  fi
  if [[ "$rp" != "Delete" ]]; then
    warn "reclaimPolicy=$rp -- old hostpath directories on the vacated nodes will NOT be cleaned up automatically"
  fi

  mapfile -t LABELLED < <(labelled_nodes)
  if (( ${#LABELLED[@]} < REPLICAS )); then
    die "only ${#LABELLED[@]} node(s) carry ${LABEL_KEY}=${LABEL_VALUE}; podAntiAffinityPreset=hard needs at least $REPLICAS"
  fi
  say "labelled nodes (${#LABELLED[@]}): ${LABELLED[*]}"

  # What matters is how many labelled nodes can actually take a pod, not whether
  # every single one can. Labelling a whole rack or zone for a 3-member etcd is
  # normal, and one unrelated cordoned node in that pool should not abort the
  # migration while plenty of valid destinations remain.
  local n unsched taints
  local -a cordoned=() schedulable=()
  for n in "${LABELLED[@]}"; do
    unsched="$(kubectl get node "$n" -o jsonpath='{.spec.unschedulable}' 2>/dev/null || true)"
    if [[ "$unsched" == "true" ]]; then
      cordoned+=("$n")
    else
      schedulable+=("$n")
    fi
    taints="$(kubectl get node "$n" -o jsonpath='{range .spec.taints[?(@.effect=="NoSchedule")]}{.key}{" "}{end}' 2>/dev/null || true)"
    if [[ -n "$taints" ]]; then
      warn "node $n has NoSchedule taints ($taints) -- etcd pods need matching tolerations via etcd.tolerations"
    fi
  done
  if (( ${#cordoned[@]} > 0 )); then
    if (( ${#schedulable[@]} < REPLICAS )); then
      die "only ${#schedulable[@]} of ${#LABELLED[@]} labelled node(s) are schedulable (cordoned: ${cordoned[*]}); podAntiAffinityPreset=hard needs at least $REPLICAS. Uncordon them -- localpv cleanup pods need to schedule too."
    fi
    warn "labelled but cordoned, so unusable as destinations: ${cordoned[*]} (${#schedulable[@]} schedulable, need $REPLICAS)"
  fi

  say "current placement:"
  local p
  for p in "${PODS[@]}"; do
    n="$(pod_node "$p")"
    if [[ -n "$n" ]] && is_labelled_node "$n"; then
      say "  $p -> ${n} (labelled)"
    else
      say "  $p -> ${n:-<unscheduled>}"
    fi
  done

  if ! wait_gate "Preflight: cluster healthy and converged?"; then
    die "cluster is not healthy before we start; fix that first" 24
  fi
}

# Record what the cluster is and what it holds, so that every later gate has
# something to be measured against rather than merely being internally
# consistent.
capture_baseline() {
  local ctrl view line
  ctrl="$(ctrl_pod "")" || die "no pod available to read the baseline from"
  view="$(cluster_view "$ctrl" "")" || die "could not read cluster state for the baseline"
  line="$(printf '%s\n' "$view" | head -1)"
  BASE_REVISION="$(printf '%s' "$line" | cut -f3)"
  BASE_CLUSTER_ID="$(printf '%s' "$line" | cut -f6)"

  BASE_KEYS_FILE="$(mktemp -t etcd-keys-before.XXXXXX)"
  etcd_exec "$ctrl" get --prefix "" --keys-only 2>/dev/null \
    | grep -v '^$' | LC_ALL=C sort > "$BASE_KEYS_FILE" || true
  say "Baseline: cluster=$BASE_CLUSTER_ID revision=$BASE_REVISION keys=$(wc -l < "$BASE_KEYS_FILE")"
  say "  key census: $BASE_KEYS_FILE"
}

# Compare the keyspace against the baseline census. Mayastor's keyspace is live
# -- lease locks come and go -- so a key appearing is normal and a key vanishing
# is reported rather than fatal. The fatal signals (cluster_id, revision going
# backwards) are asserted inside gate_healthy, on every single gate.
verify_keyspace() {                        # <label>
  local label="$1" ctrl now missing
  if [[ -z "$BASE_KEYS_FILE" || ! -s "$BASE_KEYS_FILE" ]]; then return 0; fi
  ctrl="$(ctrl_pod "")" || { warn "  $label: no pod available to re-read the keyspace"; return 0; }
  now="$(mktemp -t etcd-keys-now.XXXXXX)"
  etcd_exec "$ctrl" get --prefix "" --keys-only 2>/dev/null \
    | grep -v '^$' | LC_ALL=C sort > "$now" || true
  missing="$(LC_ALL=C comm -23 "$BASE_KEYS_FILE" "$now" || true)"
  if [[ -n "$missing" ]]; then
    warn "  $label: keys present at baseline are now absent:"
    printf '%s\n' "$missing" | sed 's/^/      /' >&2
    warn "  (StoreLease* keys are lease-bound and may legitimately churn; anything else is not)"
  else
    say "  $label: all $(wc -l < "$BASE_KEYS_FILE") baseline keys still present ($(wc -l < "$now") total)"
  fi
  rm -f "$now"
}

take_snapshot() {
  local ctrl f
  ctrl="$(ctrl_pod "")" || die "no pod available to snapshot from"
  mkdir -p "$SNAPSHOT_DIR"
  f="$SNAPSHOT_DIR/${STS}-$(date +%Y%m%d-%H%M%S).db"
  say "Snapshotting keyspace via $ctrl -> $f"
  if [[ "$DRY_RUN" == "true" ]]; then
    say "  DRY-RUN: would snapshot"
    return 0
  fi
  k exec "$ctrl" -c "$CN" -- sh -c \
    'ETCDCTL_API=3 etcdctl snapshot save /tmp/pre-migration.db >/dev/null 2>&1 && cat /tmp/pre-migration.db && rm -f /tmp/pre-migration.db' > "$f"
  if [[ ! -s "$f" ]]; then die "snapshot came back empty -- refusing to continue without a backup"; fi
  # A non-empty file is not a valid one. `snapshot status` parses the bbolt
  # pages and the integrity hash, which is what catches a stream mangled in
  # transit through kubectl exec.
  local st=""
  if command -v etcdctl >/dev/null 2>&1; then
    st="$(ETCDCTL_API=3 etcdctl snapshot status "$f" -w json 2>/dev/null || true)"
  else
    k exec -i "$ctrl" -c "$CN" -- sh -c 'cat > /tmp/verify.db' < "$f"
    st="$(k exec "$ctrl" -c "$CN" -- sh -c 'ETCDCTL_API=3 etcdctl snapshot status /tmp/verify.db -w json 2>/dev/null; rm -f /tmp/verify.db' || true)"
  fi
  if [[ -z "$st" ]] || ! printf '%s' "$st" | jq -e '.totalKey >= 0' >/dev/null 2>&1; then
    die "snapshot at $f did not pass 'etcdctl snapshot status' -- refusing to continue without a usable backup"
  fi
  say "Snapshot written: $f ($(wc -c < "$f") bytes, $(printf '%s' "$st" | jq -r '.totalKey') keys, revision $(printf '%s' "$st" | jq -r '.revision'))"
}

# ------------------------------------------------------------- the move ----

wait_pvc_gone() {                          # <pvc> <old_uid>
  local pvc="$1" olduid="$2" deadline uid
  deadline=$(( $(date +%s) + TIMEOUT_SEC ))
  say "  waiting for old PVC $pvc to be released"
  while :; do
    uid="$(k get pvc "$pvc" -o jsonpath='{.metadata.uid}' 2>/dev/null || true)"
    if [[ -z "$uid" ]] || [[ -n "$olduid" && "$uid" != "$olduid" ]]; then
      say "  old PVC released"
      return 0
    fi
    if (( $(date +%s) >= deadline )); then
      die "  timed out waiting for $pvc to delete (pvc-protection finalizer -- is the pod still running?)" 5
    fi
    sleep "$SLEEP_SEC"
  done
}

wait_pod_on_labelled_node() {              # <pod> <old_pod_uid>
  local pod="$1" olduid="${2:-}" deadline kicks=0 node phase ready msg uid
  deadline=$(( $(date +%s) + TIMEOUT_SEC ))
  say "  waiting for $pod to come back Ready on a labelled node"
  while :; do
    uid="$(pod_uid "$pod")"

    # Until the replacement pod exists, everything we can observe still
    # describes the pod we just deleted. Acting on it would burn a reschedule
    # kick per poll and delete the new pod the moment it appeared.
    if [[ -z "$uid" ]] || { [[ -n "$olduid" ]] && [[ "$uid" == "$olduid" ]]; }; then
      if (( $(date +%s) >= deadline )); then
        die "  $pod was not recreated within ${TIMEOUT_SEC}s" 5
      fi
      sleep "$SLEEP_SEC"
      continue
    fi

    phase="$(k get pod "$pod" -o jsonpath='{.status.phase}' 2>/dev/null || true)"
    node="$(pod_node "$pod")"
    ready="$(k get pod "$pod" -o jsonpath='{range .status.conditions[?(@.type=="Ready")]}{.status}{end}' 2>/dev/null || true)"

    if [[ -n "$node" ]] && ! is_labelled_node "$node"; then
      # It rebound the stale PV, or affinity is not what we think it is.
      if (( kicks < MAX_RESCHEDULE_KICKS )); then
        kicks=$(( kicks + 1 ))
        warn "  $pod landed on unlabelled node $node; deleting to force rescheduling (kick $kicks/$MAX_RESCHEDULE_KICKS)"
        k delete pod "$pod" --wait=false --ignore-not-found >/dev/null || true
        olduid="$uid"
        sleep "$SLEEP_SEC"
        continue
      fi
      die "  $pod keeps landing on unlabelled nodes -- check the StatefulSet nodeAffinity" 30
    fi

    if [[ "$phase" == "Running" && "$ready" == "True" ]]; then
      say "  $pod Ready on $node"
      return 0
    fi

    if [[ "$phase" == "Pending" ]]; then
      msg="$(k get pod "$pod" -o jsonpath='{range .status.conditions[?(@.type=="PodScheduled")]}{.message}{end}' 2>/dev/null || true)"
      if [[ -n "$msg" ]]; then say "  pending: $msg"; fi
    fi

    if (( $(date +%s) >= deadline )); then
      die "  $pod did not become Ready on a labelled node within ${TIMEOUT_SEC}s" 5
    fi
    sleep "$SLEEP_SEC"
  done
}

move_one() {                               # <pod>
  local pod="$1"
  local ordinal="${pod##*-}"
  local pvc="data-${pod}"
  local from_node ctrl mid pvc_uid pod_uid_before pv_name unsched

  from_node="$(pod_node "$pod")"
  say "=== $pod  (ordinal $ordinal, currently on ${from_node:-<unscheduled>}) ==="

  if [[ -n "$from_node" ]] && is_labelled_node "$from_node"; then
    say "  already on a labelled node -- nothing to do"
    return 0
  fi

  if [[ -n "$from_node" ]]; then
    unsched="$(kubectl get node "$from_node" -o jsonpath='{.spec.unschedulable}' 2>/dev/null || true)"
    if [[ "$unsched" == "true" ]]; then
      warn "  source node $from_node is cordoned; the localpv cleanup pod cannot run there and the old hostpath dir will be left behind"
    fi
  fi

  if ! wait_gate "  Gate 1/3: whole cluster healthy and converged before touching $pod"; then
    die "cluster unhealthy before moving $pod -- aborting without changes" 24
  fi

  ctrl="$(ctrl_pod "$pod")" || die "no control pod available to issue member remove" 6
  mid="$(member_id_for "$pod" "$ctrl")" || mid=""
  if [[ -n "$mid" ]]; then
    say "  removing membership for $pod (member $mid) via $ctrl"
    if ! mutate etcd_exec "$ctrl" --endpoints="$(eps_excluding "$pod")" member remove "$mid"; then
      say "  current membership:"
      etcd_exec "$ctrl" --endpoints="$(eps_excluding "$pod")" member list -w table 2>/dev/null | sed 's/^/    /' || true
      die "member remove failed for $pod" 6
    fi
  else
    warn "  no member found for $pod (already removed?) -- continuing"
  fi

  # Nothing destructive happens until the REMAINING cluster proves it is fine.
  #
  # Under DRY_RUN the member remove above was only printed, so the membership
  # this gate exists to check does not exist. Evaluating it anyway asks the
  # surviving members to agree on a leader drawn only from themselves while
  # $pod is still a voting member -- which fails outright whenever $pod happens
  # to be the current leader, and passes misleadingly when it does not. Neither
  # answer means anything, so say so and move on.
  if [[ "$DRY_RUN" == "true" ]]; then
    say "  Gate 2/3: skipped under DRY_RUN (membership was not actually changed)"
  elif ! wait_gate "  Gate 2/3: remaining members healthy after membership change" "$pod"; then
    die "remaining cluster is not healthy -- STOP. Do not delete $pvc. Investigate before continuing." 24
  fi

  pvc_uid="$(k get pvc "$pvc" -o jsonpath='{.metadata.uid}' 2>/dev/null || true)"
  pod_uid_before="$(pod_uid "$pod")"
  pv_name="$(k get pvc "$pvc" -o jsonpath='{.spec.volumeName}' 2>/dev/null || true)"
  say "  destroying $pvc (pv=${pv_name:-<none>}) and pod $pod"

  # PVC first: pvc-protection holds it Terminating until the pod is gone, so
  # deleting the pod second is what actually releases it. The StatefulSet
  # controller will not recreate the pod while its PVC has a deletionTimestamp,
  # which is what stops the replacement from rebinding the old node-pinned PV.
  mutate k delete pvc "$pvc" --wait=false --ignore-not-found
  mutate k delete pod "$pod" --wait=false --ignore-not-found

  if [[ "$DRY_RUN" == "true" ]]; then
    say "  DRY-RUN: would now wait for $pod to be rescheduled onto a labelled node and resync"
    return 0
  fi

  wait_pvc_gone "$pvc" "$pvc_uid"
  wait_pod_on_labelled_node "$pod" "$pod_uid_before"

  if ! wait_gate "  Gate 3/3: all $REPLICAS members healthy and revisions converged"; then
    die "cluster did not converge after moving $pod" 5
  fi
  verify_keyspace "after $pod"

  say "  $pod is now on $(pod_node "$pod"); settling for ${SETTLE_SEC}s"
  sleep "$SETTLE_SEC"
}

# ------------------------------------------------------------------ main ----

main() {
  preflight

  # Descending ordinal order, so ${STS}-0 -- the riskiest member to rebuild --
  # goes last, once the procedure has already proven itself.
  local -a todo=()
  local i pod node
  for (( i=REPLICAS-1; i>=0; i-- )); do
    pod="$STS-$i"
    node="$(pod_node "$pod")"
    if [[ -n "$node" ]] && is_labelled_node "$node"; then
      say "$pod already on labelled node $node -- no move needed"
    else
      todo+=("$pod")
    fi
  done

  if [[ -n "$ONLY_ORDINAL" ]]; then
    # Validated against REPLICAS in preflight. move_one() still re-checks
    # placement, so naming an already-placed ordinal is a no-op rather than a
    # needless rebuild.
    todo=("$STS-$ONLY_ORDINAL")
    say "ONLY_ORDINAL=$ONLY_ORDINAL -- restricting this run to ${todo[*]}"
  fi

  if (( ${#todo[@]} == 0 )); then
    say "Nothing to do: every member is already on a labelled node."
    return 0
  fi

  capture_baseline
  if [[ "$SNAPSHOT" == "true" ]]; then take_snapshot; fi

  say "Members to move, in order: ${todo[*]}"

  if [[ "$DRY_RUN" == "true" ]]; then
    say "DRY_RUN=true -- nothing will be changed. Re-run with DRY_RUN=false to execute."
  else
    warn "DRY_RUN=false -- this WILL remove etcd members and delete their PVCs."
    warn "Ctrl-C within 10 seconds to abort."
    sleep 10
  fi

  for pod in "${todo[@]}"; do
    move_one "$pod"
  done

  if [[ "$DRY_RUN" != "true" ]]; then
    if ! wait_gate "Final verification: all $REPLICAS members healthy and converged"; then
      die "final health check failed" 24
    fi
    verify_keyspace "final"
  fi

  say ""
  if [[ "$DRY_RUN" == "true" ]]; then say "DRY RUN complete. No changes were made. Placement is unchanged:"; else say "Migration complete. Placement:"; fi
  for pod in "${PODS[@]}"; do
    say "  $pod -> $(pod_node "$pod")"
  done
  say ""
  say "Follow-up, once you are satisfied:"
  say "  - revert etcd.updateStrategy.type to RollingUpdate"
  say "  - remove etcd.initialClusterState='existing' (it would break a genuine fresh bootstrap)"
  say "  - confirm the old hostpath dirs were cleaned up on the vacated nodes"
  say "  - kubectl get pv | grep -E 'Released|Failed'   # should be empty"
}

main "$@"
