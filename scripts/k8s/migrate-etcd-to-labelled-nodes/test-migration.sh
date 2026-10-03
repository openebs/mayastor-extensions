#!/usr/bin/env bash
#
# test-migration.sh
#
# End-to-end test for migrate-mayastor-etcd.sh against a real Kubernetes
# cluster. It installs openebs/openebs, puts genuine Mayastor state into etcd
# (pools, a replicated volume, a workload writing to it), migrates the etcd
# members onto labelled nodes, and then proves that nothing was lost.
#
# The point of the test is the verify phase. Everything else exists to create a
# situation worth verifying. The assertions that matter:
#
#   * etcd's cluster_id is the SAME before and after. A member that bootstrapped
#     a fresh cluster over the keyspace instead of rejoining ours would show a
#     new one while otherwise looking perfectly healthy.
#   * the revision never goes backwards.
#   * every member reports an identical `endpoint hashkv`. Equal revisions only
#     say the members agree on how far they have got; equal hashes say they are
#     holding the same bytes.
#   * every key present before the migration is still present after.
#   * the workload's data has no gaps and its pod never restarted -- Mayastor's
#     data path is supposed to keep serving I/O even while etcd is unavailable.
#   * the control plane can still WRITE: provisioning a new volume afterwards
#     proves etcd is functional, not merely intact.
#
# Requirements on the cluster (a "cluster of this sort"):
#   * >= 3 worker nodes labelled openebs.io/engine=mayastor
#   * hugepages configured and nvme kernel modules loaded on those workers
#   * one spare unformatted block device per worker (BLOCK_DEVICE, default
#     /dev/sdb) for the DiskPools
#   * kubectl, helm, jq, yq on the machine running this
#
# Usage:
#   ./test-migration.sh all          # the whole thing
#   ./test-migration.sh setup        # install + seed state
#   ./test-migration.sh prepare      # apply the helm prerequisites
#   ./test-migration.sh migrate      # run the migration under test
#   ./test-migration.sh verify       # assertions only (safe to repeat)
#   ./test-migration.sh revert       # post-migration cleanup, back to RollingUpdate
#   ./test-migration.sh teardown     # remove everything this test created
#
# Exit code is 0 only if every assertion passed.
#
set -euo pipefail

# ---------------------------------------------------------------- config ----

CHART_VERSION="${CHART_VERSION:-4.2.0}"
NS="${NS:-openebs}"
REL="${REL:-openebs}"
STS="${STS:-${REL}-etcd}"
CN="${CN:-etcd}"

ETCD_REPLICAS="${ETCD_REPLICAS:-2}"        # the interesting case; 3 also works
LABEL_KEY="${LABEL_KEY:-node/etcd.io}"
LABEL_VALUE="${LABEL_VALUE:-true}"

BLOCK_DEVICE="${BLOCK_DEVICE:-/dev/sdb}"   # spare disk on each worker
TEST_NS="${TEST_NS:-default}"
TEST_SC="${TEST_SC:-mayastor-test-3}"
TEST_PVC="${TEST_PVC:-migration-test-data}"
TEST_APP="${TEST_APP:-migration-test-writer}"

# openebs/openebs 4.2.0 ships two image references that Docker Hub no longer
# serves: Bitnami retired their legacy tags in Aug 2025, so
# docker.io/bitnami/etcd:3.5.6-debian-11-r10 and docker.io/bitnami/kubectl:1.25.15
# both 404. OpenEBS republishes the etcd image under its own org; kubectl is
# still available under bitnamilegacy. Without these the install never starts
# and, worse, every `helm upgrade` hangs on the pre-upgrade hook Job.
# Set PATCH_DEAD_IMAGES=false once the chart itself is fixed.
PATCH_DEAD_IMAGES="${PATCH_DEAD_IMAGES:-true}"
ETCD_IMAGE_REPO="${ETCD_IMAGE_REPO:-openebs/etcd}"
ETCD_IMAGE_TAG="${ETCD_IMAGE_TAG:-3.5.6-debian-11-r10}"
KUBECTL_IMAGE_REPO="${KUBECTL_IMAGE_REPO:-bitnamilegacy/kubectl}"
KUBECTL_IMAGE_TAG="${KUBECTL_IMAGE_TAG:-1.25.15}"

# The migration needs somewhere to move a member TO, so the pool of nodes etcd
# may occupy has to be strictly larger than ETCD_REPLICAS. On a cluster with
# exactly as many workers as replicas (3 workers, replicaCount 3) there is no
# spare, and podAntiAffinityPreset=hard means a member cannot double up. Setting
# this brings the control-plane node into the pool and gives etcd a matching
# toleration, which is enough to make the 3-replica case testable on a 3-worker
# cluster. Leave it off wherever there are genuinely spare workers.
ETCD_TOLERATE_CONTROL_PLANE="${ETCD_TOLERATE_CONTROL_PLANE:-false}"

# Label MORE nodes than there are etcd members. The migration only requires at
# least replicaCount labelled nodes; a real deployment would label a whole rack
# or zone and leave the scheduler a choice of destination. Setting this exercises
# that, rather than the artificially exact fit the test otherwise sets up.
EXTRA_LABELLED="${EXTRA_LABELLED:-0}"

REUSE_INSTALL="${REUSE_INSTALL:-false}"    # skip install if the release exists
WORKDIR="${WORKDIR:-$(pwd)/.migration-test}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MIGRATE_SCRIPT="${MIGRATE_SCRIPT:-$SCRIPT_DIR/migrate-mayastor-etcd.sh}"

TIMEOUT_SEC="${TIMEOUT_SEC:-900}"

# ------------------------------------------------------------- machinery ----

PASS=0
FAIL=0

# Self-contained, like the script it tests -- see the note in
# migrate-mayastor-etcd.sh. Do not replace with scripts/utils/log.sh.
say()  { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*"; }
warn() { printf '[%s] WARN  %s\n' "$(date +%H:%M:%S)" "$*" >&2; }
die()  { printf '[%s] FATAL %s\n' "$(date +%H:%M:%S)" "$*" >&2; exit 1; }
hdr()  { printf '\n[%s] ===== %s =====\n' "$(date +%H:%M:%S)" "$*"; }

ok()   { PASS=$(( PASS + 1 )); printf '  PASS  %s\n' "$*"; }
bad()  { FAIL=$(( FAIL + 1 )); printf '  FAIL  %s\n' "$*" >&2; }

assert_eq() {                              # <what> <expected> <actual>
  if [[ "$2" == "$3" ]]; then ok "$1 ($3)"; else bad "$1: expected '$2', got '$3'"; fi
}

k() { kubectl -n "$NS" "$@"; }

etcd_exec() {                              # <pod> <etcdctl args...>
  local pod="$1"; shift
  k exec "$pod" -c "$CN" -- env ETCDCTL_API=3 etcdctl \
      --dial-timeout=5s --command-timeout=15s "$@"
}

ep_for() { printf 'http://%s.%s-headless.%s.svc.cluster.local:2379' "$1" "$STS" "$NS"; }

all_eps() {
  local -a out=()
  local i
  for (( i=0; i<ETCD_REPLICAS; i++ )); do out+=("$(ep_for "${STS}-$i")"); done
  ( IFS=,; printf '%s' "${out[*]}" )
}

# A live etcd pod we can shell into. Never assume ordinal 0 is up.
any_etcd_pod() {
  local i p
  for (( i=ETCD_REPLICAS-1; i>=0; i-- )); do
    p="${STS}-$i"
    if [[ "$(k get pod "$p" -o jsonpath='{.status.phase}' 2>/dev/null || true)" == "Running" ]] \
       && etcd_exec "$p" endpoint status -w json >/dev/null 2>&1; then
      printf '%s' "$p"; return 0
    fi
  done
  return 1
}

wait_for() {                               # <desc> <timeout> <shell predicate...>
  local desc="$1" t="$2"; shift 2
  local deadline=$(( $(date +%s) + t ))
  say "waiting: $desc"
  while :; do
    if "$@"; then return 0; fi
    if (( $(date +%s) >= deadline )); then
      warn "timed out after ${t}s: $desc"
      return 1
    fi
    sleep 5
  done
}

etcd_pods_ready() {
  local n
  n="$(k get pods -l app.kubernetes.io/name=etcd \
        -o jsonpath='{range .items[*]}{.status.containerStatuses[0].ready}{"\n"}{end}' 2>/dev/null \
      | grep -c true || true)"
  [[ "$n" == "$ETCD_REPLICAS" ]]
}

# "Nothing is unready" is NOT the same as "everything is up". `helm install`
# returns before the controllers have created any pods, so an empty pod list
# makes a naive sweep pass instantly and lets the rest of setup race a cluster
# that has not started. Anchor on the etcd StatefulSet reporting its full
# complement first, then sweep for stragglers.
all_pods_ready() {
  local ready
  ready="$(k get sts "$STS" -o jsonpath='{.status.readyReplicas}' 2>/dev/null || true)"
  if [[ "${ready:-0}" != "$ETCD_REPLICAS" ]]; then return 1; fi
  ! k get pods --no-headers 2>/dev/null \
    | awk '{split($2,a,"/"); if (a[1]!=a[2] && $3!="Completed") print}' | grep -q .
}

need() {
  local c
  for c in "$@"; do command -v "$c" >/dev/null 2>&1 || die "required command not found: $c"; done
}

worker_nodes() {
  kubectl get nodes -l 'openebs.io/engine=mayastor' \
    -o jsonpath='{range .items[*]}{.metadata.name}{"\n"}{end}'
}

control_plane_nodes() {
  kubectl get nodes -l 'node-role.kubernetes.io/control-plane' \
    -o jsonpath='{range .items[*]}{.metadata.name}{"\n"}{end}'
}

# Nodes etcd is allowed to occupy. DiskPools still only ever live on the
# mayastor workers; this pool is about etcd placement alone.
candidate_nodes() {
  worker_nodes
  if [[ "$ETCD_TOLERATE_CONTROL_PLANE" == "true" ]]; then control_plane_nodes; fi
}

# ------------------------------------------------------------------ setup ----

phase_setup() {
  hdr "SETUP: install openebs $CHART_VERSION with a ${ETCD_REPLICAS}-member etcd"
  need kubectl helm jq yq
  mkdir -p "$WORKDIR"

  local -a workers=()
  mapfile -t workers < <(worker_nodes)
  if (( ${#workers[@]} < 3 )); then
    die "need >= 3 nodes labelled openebs.io/engine=mayastor, found ${#workers[@]}. Label them first."
  fi
  say "mayastor workers (${#workers[@]}): ${workers[*]}"

  if helm -n "$NS" status "$REL" >/dev/null 2>&1; then
    if [[ "$REUSE_INSTALL" == "true" ]]; then
      say "release $REL already installed and REUSE_INSTALL=true -- skipping install"
    else
      die "release $REL already exists in $NS. Run '$0 teardown' first, or set REUSE_INSTALL=true."
    fi
  else
    helm repo add openebs https://openebs.github.io/openebs >/dev/null 2>&1 || true
    helm repo update openebs >/dev/null
    rm -rf "$WORKDIR/chart"
    mkdir -p "$WORKDIR/chart"
    helm pull openebs/openebs --version "$CHART_VERSION" --untar -d "$WORKDIR/chart"

    {
      printf 'mayastor:\n  etcd:\n    replicaCount: %s\n' "$ETCD_REPLICAS"
      if [[ "$PATCH_DEAD_IMAGES" == "true" ]]; then
        printf '    image:\n      registry: docker.io\n      repository: %s\n      tag: %s\n' \
               "$ETCD_IMAGE_REPO" "$ETCD_IMAGE_TAG"
      fi
      if [[ "$ETCD_TOLERATE_CONTROL_PLANE" == "true" ]]; then
        printf '    tolerations:\n      - key: node-role.kubernetes.io/control-plane\n        operator: Exists\n        effect: NoSchedule\n'
      fi
      printf '  obs:\n    callhome:\n      sendReport: false\n'
      if [[ "$PATCH_DEAD_IMAGES" == "true" ]]; then
        printf 'preUpgradeHook:\n  image:\n    registry: docker.io\n    repo: %s\n    tag: "%s"\n' \
               "$KUBECTL_IMAGE_REPO" "$KUBECTL_IMAGE_TAG"
      fi
    } > "$WORKDIR/install-values.yaml"
    say "install values:"; sed 's/^/    /' "$WORKDIR/install-values.yaml"

    kubectl create namespace "$NS" --dry-run=client -o yaml | kubectl apply -f - >/dev/null
    helm install "$REL" "$WORKDIR/chart/openebs" -n "$NS" \
      -f "$WORKDIR/install-values.yaml" --timeout 15m
  fi

  wait_for "all $NS pods ready" "$TIMEOUT_SEC" all_pods_ready || die "install did not come up"
  k get pods -o wide

  hdr "SETUP: DiskPools on $BLOCK_DEVICE"
  local n
  for n in "${workers[@]}"; do
    cat <<EOF | kubectl apply -f - >/dev/null
apiVersion: openebs.io/v1beta2
kind: DiskPool
metadata:
  name: pool-${n}
  namespace: ${NS}
spec:
  node: ${n}
  disks: ["${BLOCK_DEVICE}"]
EOF
  done
  wait_for "all DiskPools Online" 300 bash -c \
    "kubectl -n '$NS' get dsp --no-headers 2>/dev/null | awk '\$4!=\"Online\"{f=1} END{exit f?1:0}'" \
    || die "DiskPools did not come Online -- is $BLOCK_DEVICE free on every worker?"
  k get dsp

  hdr "SETUP: replicated volume + writer workload"
  cat <<EOF | kubectl apply -f - >/dev/null
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: ${TEST_SC}
parameters:
  protocol: nvmf
  repl: "3"
  fsType: ext4
provisioner: io.openebs.csi-mayastor
volumeBindingMode: Immediate
---
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: ${TEST_PVC}
  namespace: ${TEST_NS}
spec:
  accessModes: [ReadWriteOnce]
  resources:
    requests:
      storage: 1Gi
  storageClassName: ${TEST_SC}
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ${TEST_APP}
  namespace: ${TEST_NS}
spec:
  replicas: 1
  selector: {matchLabels: {app: ${TEST_APP}}}
  template:
    metadata: {labels: {app: ${TEST_APP}}}
    spec:
      containers:
      - name: writer
        image: busybox:1.36
        command: ["/bin/sh","-c"]
        args:
          - |
            i=0
            while true; do
              i=\$((i+1))
              echo "\$(date -Iseconds) seq=\$i" >> /data/log.txt
              sync
              sleep 2
            done
        volumeMounts: [{name: d, mountPath: /data}]
      volumes:
      - name: d
        persistentVolumeClaim: {claimName: ${TEST_PVC}}
EOF
  wait_for "writer pod running" 300 bash -c \
    "kubectl -n '$TEST_NS' get pod -l app='$TEST_APP' -o jsonpath='{.items[0].status.phase}' 2>/dev/null | grep -q Running"
  sleep 20   # let it write something worth losing

  hdr "SETUP: choose the nodes to label"
  # Label the nodes of every member EXCEPT ordinal 0, plus one node hosting no
  # member at all. That is exactly ETCD_REPLICAS labelled nodes (which is what
  # podAntiAffinityPreset=hard requires) and leaves exactly one member to move.
  # Leaving ordinal 0 as the one that moves is deliberate: it is the member
  # whose bootstrap semantics could destroy the keyspace if initialClusterState
  # were wrong, so it is the most informative one to rebuild.
  local -a etcd_nodes=() candidates=() to_label=()
  local i p node spare=""
  for (( i=0; i<ETCD_REPLICAS; i++ )); do
    p="${STS}-$i"
    node="$(k get pod "$p" -o jsonpath='{.spec.nodeName}' 2>/dev/null || true)"
    etcd_nodes+=("$node")
    say "  $p -> $node"
  done
  for (( i=1; i<ETCD_REPLICAS; i++ )); do to_label+=("${etcd_nodes[$i]}"); done

  mapfile -t candidates < <(candidate_nodes)
  for n in "${candidates[@]}"; do
    if [[ " ${etcd_nodes[*]} " != *" $n "* ]]; then spare="$n"; break; fi
  done
  if [[ -z "$spare" ]]; then
    die "every candidate node already hosts an etcd member, so there is nowhere to migrate to. Candidates (${#candidates[@]}): ${candidates[*]}. Either free up a node, lower ETCD_REPLICAS, or set ETCD_TOLERATE_CONTROL_PLANE=true to bring the control-plane node into the pool."
  fi
  to_label+=("$spare")

  # Spare destinations beyond the minimum. The SOURCE node is deliberately never
  # labelled -- if it were, ordinal 0 would already be "in place" and the test
  # would migrate nothing.
  local extra=0
  for n in "${candidates[@]}"; do
    if (( extra >= EXTRA_LABELLED )); then break; fi
    if [[ " ${to_label[*]} " == *" $n "* ]]; then continue; fi
    if [[ "$n" == "${etcd_nodes[0]}" ]]; then continue; fi
    to_label+=("$n")
    extra=$(( extra + 1 ))
  done
  if (( EXTRA_LABELLED > 0 && extra < EXTRA_LABELLED )); then
    warn "asked for $EXTRA_LABELLED extra labelled node(s), only $extra spare candidate(s) available"
  fi

  for n in "${to_label[@]}"; do
    kubectl label node "$n" "${LABEL_KEY}=${LABEL_VALUE}" --overwrite
  done
  say "labelled ${#to_label[@]} node(s) for ${ETCD_REPLICAS} member(s): ${to_label[*]}"
  say "  $spare hosts no etcd member; ${etcd_nodes[0]} is left unlabelled so ${STS}-0 must move there"
  kubectl get nodes -L "${LABEL_KEY}" 2>/dev/null || kubectl get nodes
  say "SETUP complete."
}

# ---------------------------------------------------------------- prepare ----

phase_prepare() {
  hdr "PREPARE: apply the helm prerequisites (OnDelete, nodeAffinity, initialClusterState)"
  mkdir -p "$WORKDIR"
  [[ -d "$WORKDIR/chart/openebs" ]] || die "chart source missing at $WORKDIR/chart/openebs -- run setup first"

  cat > "$WORKDIR/etcd-migration.yaml" <<EOF
mayastor:
  etcd:
    nodeAffinityPreset:
      type: hard
      key: "${LABEL_KEY}"
      values: ["${LABEL_VALUE}"]
    podAntiAffinityPreset: "hard"
    updateStrategy:
      type: OnDelete
    initialClusterState: "existing"
EOF

  k get pods -l app.kubernetes.io/name=etcd \
    -o custom-columns=NAME:.metadata.name,UID:.metadata.uid,NODE:.spec.nodeName,RESTARTS:.status.containerStatuses[0].restartCount \
    > "$WORKDIR/pods-before-prepare.txt"
  cat "$WORKDIR/pods-before-prepare.txt"

  helm -n "$NS" upgrade "$REL" "$WORKDIR/chart/openebs" \
    -f "$WORKDIR/install-values.yaml" -f "$WORKDIR/etcd-migration.yaml" --timeout 10m >/dev/null
  sleep 30

  hdr "PREPARE: assertions"
  k get pods -l app.kubernetes.io/name=etcd \
    -o custom-columns=NAME:.metadata.name,UID:.metadata.uid,NODE:.spec.nodeName,RESTARTS:.status.containerStatuses[0].restartCount \
    > "$WORKDIR/pods-after-prepare.txt"

  # The whole safety model is that this upgrade moves nothing. If OnDelete did
  # not take effect, pods are already cycling into PVCs pinned to the old nodes.
  if diff -u "$WORKDIR/pods-before-prepare.txt" "$WORKDIR/pods-after-prepare.txt" >/dev/null; then
    ok "helm upgrade caused no etcd pod churn"
  else
    bad "helm upgrade churned etcd pods -- OnDelete did not take effect"
    diff -u "$WORKDIR/pods-before-prepare.txt" "$WORKDIR/pods-after-prepare.txt" || true
  fi

  assert_eq "updateStrategy" "OnDelete" \
    "$(k get sts "$STS" -o jsonpath='{.spec.updateStrategy.type}')"
  assert_eq "ETCD_INITIAL_CLUSTER_STATE" "existing" \
    "$(k get sts "$STS" -o json | jq -r '.spec.template.spec.containers[]|select(.name=="etcd")|.env[]|select(.name=="ETCD_INITIAL_CLUSTER_STATE")|.value')"

  # Assert the key AND the value. A template selecting the right key with the
  # wrong value is the mismatch that strands the replacement pod, so the test
  # should notice it here rather than during the move.
  assert_eq "nodeAffinity selects ${LABEL_KEY}=${LABEL_VALUE}" "$LABEL_VALUE" \
    "$(k get sts "$STS" -o json | jq -r --arg k "$LABEL_KEY" '
        [ ( .spec.template.spec.affinity.nodeAffinity
            | (.requiredDuringSchedulingIgnoredDuringExecution.nodeSelectorTerms // [])[]?
            | (.matchExpressions // [])[]? | select(.key == $k) | (.values // [])[]? ),
          ( (.spec.template.spec.nodeSelector // {}) | to_entries[] | select(.key == $k) | .value )
        ] | unique | join(",")')"

  if k get sts "$STS" -o json | jq -e '.spec.template.spec.affinity.podAntiAffinity' >/dev/null 2>&1; then
    ok "podAntiAffinity still present"
  else
    bad "podAntiAffinity was discarded (did something set etcd.affinity directly?)"
  fi

  # Expected and correct: the template changed but nothing was recycled.
  local cur upd
  cur="$(k get sts "$STS" -o jsonpath='{.status.currentRevision}')"
  upd="$(k get sts "$STS" -o jsonpath='{.status.updateRevision}')"
  if [[ "$cur" != "$upd" ]]; then
    ok "currentRevision != updateRevision (pods deliberately not cycled)"
  else
    warn "currentRevision == updateRevision; the template may not have changed"
  fi
}

# ------------------------------------------------------- baseline + migrate ----

writer_pod() { kubectl -n "$TEST_NS" get pod -l "app=$TEST_APP" -o jsonpath='{.items[0].metadata.name}'; }

capture_baseline() {
  mkdir -p "$WORKDIR"
  local pod
  pod="$(any_etcd_pod)" || die "no etcd pod able to answer etcdctl"

  etcd_exec "$pod" --endpoints="$(all_eps)" endpoint status -w json \
    | jq -r '.[0].Status.header.cluster_id' > "$WORKDIR/base-cluster-id.txt"
  etcd_exec "$pod" --endpoints="$(all_eps)" endpoint status -w json \
    | jq -r '[.[].Status.header.revision]|max' > "$WORKDIR/base-revision.txt"
  etcd_exec "$pod" get --prefix "" --keys-only 2>/dev/null \
    | grep -v '^$' | LC_ALL=C sort > "$WORKDIR/base-keys.txt"

  local w
  w="$(writer_pod)"
  kubectl -n "$TEST_NS" exec "$w" -- sh -c 'wc -l < /data/log.txt' | tr -d ' ' > "$WORKDIR/base-writer-lines.txt"
  kubectl -n "$TEST_NS" get pod "$w" -o jsonpath='{.status.containerStatuses[0].restartCount}' > "$WORKDIR/base-writer-restarts.txt"

  say "baseline: cluster_id=$(cat "$WORKDIR/base-cluster-id.txt") revision=$(cat "$WORKDIR/base-revision.txt") keys=$(wc -l < "$WORKDIR/base-keys.txt") writerLines=$(cat "$WORKDIR/base-writer-lines.txt")"
}

phase_migrate() {
  hdr "MIGRATE: baseline"
  [[ -x "$MIGRATE_SCRIPT" ]] || die "migration script not executable: $MIGRATE_SCRIPT"
  capture_baseline

  hdr "MIGRATE: dry run"
  NS="$NS" STS="$STS" CN="$CN" LABEL_KEY="$LABEL_KEY" LABEL_VALUE="$LABEL_VALUE" \
  DRY_RUN=true "$MIGRATE_SCRIPT" 2>&1 | tee "$WORKDIR/dryrun.log"
  if [[ "${PIPESTATUS[0]}" == "0" ]]; then ok "dry run exited 0"; else bad "dry run failed"; fi

  hdr "MIGRATE: for real"
  local start rc
  start=$(date +%s)
  set +e
  NS="$NS" STS="$STS" CN="$CN" LABEL_KEY="$LABEL_KEY" LABEL_VALUE="$LABEL_VALUE" \
  DRY_RUN=false SNAPSHOT=true SNAPSHOT_DIR="$WORKDIR/etcd-snapshots" \
  "$MIGRATE_SCRIPT" 2>&1 | tee "$WORKDIR/migrate.log"
  rc=${PIPESTATUS[0]}
  set -e
  say "migration finished in $(( $(date +%s) - start ))s with exit $rc"
  if [[ "$rc" == "0" ]]; then ok "migration exited 0"; else bad "migration exited $rc"; fi

  local snap
  snap="$(find "$WORKDIR/etcd-snapshots" -maxdepth 1 -type f -name '*.db' 2>/dev/null | sort | tail -1)"
  if [[ -s "$snap" ]]; then
    ok "pre-migration snapshot written ($snap, $(wc -c < "$snap") bytes)"
  else
    bad "no pre-migration snapshot was written"
  fi
}

# ----------------------------------------------------------------- verify ----

phase_verify() {
  hdr "VERIFY"
  [[ -s "$WORKDIR/base-cluster-id.txt" ]] || die "no baseline captured -- run migrate first"

  local pod eps
  pod="$(any_etcd_pod)" || { bad "no etcd pod able to answer etcdctl"; return; }
  eps="$(all_eps)"

  local status
  status="$(etcd_exec "$pod" --endpoints="$eps" endpoint status -w json 2>/dev/null || true)"
  if [[ -z "$status" ]]; then bad "endpoint status returned nothing"; return; fi

  # --- 1. cluster identity ---------------------------------------------------
  local base_cid cids
  base_cid="$(cat "$WORKDIR/base-cluster-id.txt")"
  cids="$(printf '%s' "$status" | jq -r '[.[].Status.header.cluster_id]|unique|join(",")')"
  assert_eq "cluster_id unchanged and identical on all members" "$base_cid" "$cids"

  # --- 2. every member answered ---------------------------------------------
  assert_eq "all members reachable" "$ETCD_REPLICAS" \
    "$(printf '%s' "$status" | jq -r 'length')"

  # --- 3. one leader, one term, one revision --------------------------------
  assert_eq "single leader agreed by all members" "1" \
    "$(printf '%s' "$status" | jq -r '[.[].Status.leader]|unique|length')"
  assert_eq "single raft term" "1" \
    "$(printf '%s' "$status" | jq -r '[.[].Status.header.raft_term]|unique|length')"
  assert_eq "revisions converged" "1" \
    "$(printf '%s' "$status" | jq -r '[.[].Status.header.revision]|unique|length')"

  # --- 4. revision did not go backwards -------------------------------------
  local base_rev now_rev
  base_rev="$(cat "$WORKDIR/base-revision.txt")"
  now_rev="$(printf '%s' "$status" | jq -r '[.[].Status.header.revision]|max')"
  if (( now_rev >= base_rev )); then
    ok "revision moved forward ($base_rev -> $now_rev)"
  else
    bad "revision went BACKWARDS ($base_rev -> $now_rev) -- keyspace was rolled back"
  fi

  # --- 5. members hold identical bytes --------------------------------------
  local hashes
  hashes="$(etcd_exec "$pod" --endpoints="$eps" endpoint hashkv -w json 2>/dev/null || true)"
  if [[ -z "$hashes" ]]; then
    bad "endpoint hashkv returned nothing"
  else
    assert_eq "all members report the same keyspace hash" "1" \
      "$(printf '%s' "$hashes" | jq -r '[.[].HashKV.hash]|unique|length')"
    say "  hashkv=$(printf '%s' "$hashes" | jq -r '.[0].HashKV.hash')"
  fi

  # --- 6. no key was lost ----------------------------------------------------
  local now_keys missing
  now_keys="$(mktemp)"
  etcd_exec "$pod" get --prefix "" --keys-only 2>/dev/null \
    | grep -v '^$' | LC_ALL=C sort > "$now_keys"
  missing="$(LC_ALL=C comm -23 "$WORKDIR/base-keys.txt" "$now_keys" || true)"
  if [[ -z "$missing" ]]; then
    ok "all $(wc -l < "$WORKDIR/base-keys.txt") baseline keys still present ($(wc -l < "$now_keys") total)"
  else
    bad "keys lost during migration:"
    printf '%s\n' "$missing" | sed 's/^/          /' >&2
  fi
  rm -f "$now_keys"

  # --- 7. placement ----------------------------------------------------------
  local i p node bad_placement=0
  for (( i=0; i<ETCD_REPLICAS; i++ )); do
    p="${STS}-$i"
    node="$(k get pod "$p" -o jsonpath='{.spec.nodeName}' 2>/dev/null || true)"
    # Resolve via a label selector, the same way the migration script does, so any
    # valid label key works without having to escape it into a jsonpath.
    if kubectl get nodes -l "${LABEL_KEY}=${LABEL_VALUE}" \
         -o jsonpath='{range .items[*]}{.metadata.name}{"\n"}{end}' 2>/dev/null \
       | grep -qxF "$node"; then
      say "  $p -> $node (labelled)"
    else
      bad_placement=1
      say "  $p -> $node (NOT labelled)"
    fi
  done
  if (( bad_placement == 0 )); then
    ok "every etcd member is on a labelled node"
  else
    bad "some etcd members are not on labelled nodes"
  fi

  # --- 8. anti-affinity still honoured --------------------------------------
  local distinct
  distinct="$(k get pods -l app.kubernetes.io/name=etcd \
      -o jsonpath='{range .items[*]}{.spec.nodeName}{"\n"}{end}' | sort -u | grep -c . || true)"
  assert_eq "members on distinct nodes" "$ETCD_REPLICAS" "$distinct"

  # --- 9. storage was reclaimed ---------------------------------------------
  if kubectl get pv --no-headers 2>/dev/null | grep -E 'Released|Failed' | grep -q etcd; then
    bad "etcd PVs left in Released/Failed"
    kubectl get pv --no-headers | grep -E 'Released|Failed' | sed 's/^/          /' >&2
  else
    ok "no Released/Failed etcd PVs"
  fi

  # --- 10. the data path never noticed --------------------------------------
  local w lines restarts base_lines base_restarts gaps
  w="$(writer_pod)"
  base_lines="$(cat "$WORKDIR/base-writer-lines.txt")"
  base_restarts="$(cat "$WORKDIR/base-writer-restarts.txt")"
  restarts="$(kubectl -n "$TEST_NS" get pod "$w" -o jsonpath='{.status.containerStatuses[0].restartCount}')"
  lines="$(kubectl -n "$TEST_NS" exec "$w" -- sh -c 'wc -l < /data/log.txt' | tr -d ' ')"
  assert_eq "writer pod did not restart" "$base_restarts" "$restarts"
  if (( lines > base_lines )); then
    ok "writer kept writing through the migration ($base_lines -> $lines lines)"
  else
    bad "writer stopped writing ($base_lines -> $lines lines)"
  fi
  gaps="$(kubectl -n "$TEST_NS" exec "$w" -- sh -c \
      "awk -F'seq=' '{print \$2}' /data/log.txt | awk 'NR>1 && \$1 != prev+1 {print prev\" -> \"\$1} {prev=\$1}'" || true)"
  if [[ -z "$gaps" ]]; then
    ok "no gaps in the written sequence -- replicated volume data intact"
  else
    bad "gaps in the written sequence:"; printf '%s\n' "$gaps" | sed 's/^/          /' >&2
  fi

  # --- 11. the control plane can still WRITE --------------------------------
  # Intact is not the same as working. Provisioning a fresh volume forces
  # agent-core to write new VolumeSpec/ReplicaSpec records into etcd.
  local probe="migration-probe-$$"
  cat <<EOF | kubectl apply -f - >/dev/null
apiVersion: v1
kind: PersistentVolumeClaim
metadata: {name: ${probe}, namespace: ${TEST_NS}}
spec:
  accessModes: [ReadWriteOnce]
  resources: {requests: {storage: 500Mi}}
  storageClassName: ${TEST_SC}
EOF
  if wait_for "probe PVC to bind (proves etcd accepts writes)" 180 bash -c \
       "kubectl -n '$TEST_NS' get pvc '$probe' -o jsonpath='{.status.phase}' 2>/dev/null | grep -q Bound"; then
    ok "control plane provisioned a new volume after the migration"
  else
    bad "control plane could NOT provision a new volume -- etcd is intact but not usable"
  fi
  kubectl -n "$TEST_NS" delete pvc "$probe" --wait=false >/dev/null 2>&1 || true

  # --- 12. mayastor itself is healthy ---------------------------------------
  if k get pods --no-headers | awk '{split($2,a,"/"); if (a[1]!=a[2] && $3!="Completed") print}' | grep -q .; then
    bad "some $NS pods are not ready:"
    k get pods --no-headers | awk '{split($2,a,"/"); if (a[1]!=a[2] && $3!="Completed") print "          "$0}' >&2
  else
    ok "all $NS pods ready"
  fi
  if k get dsp --no-headers 2>/dev/null | awk '$4!="Online"{f=1} END{exit f?1:0}'; then
    ok "all DiskPools Online"
  else
    bad "some DiskPools are not Online"; k get dsp >&2
  fi
}

# ----------------------------------------------------------------- revert ----

phase_revert() {
  hdr "REVERT: back to RollingUpdate, drop the pinned initialClusterState"
  [[ -d "$WORKDIR/chart/openebs" ]] || die "chart source missing -- run setup first"

  cat > "$WORKDIR/etcd-post.yaml" <<EOF
mayastor:
  etcd:
    nodeAffinityPreset:
      type: hard
      key: "${LABEL_KEY}"
      values: ["${LABEL_VALUE}"]
    podAntiAffinityPreset: "hard"
    updateStrategy:
      type: RollingUpdate
    # initialClusterState deliberately removed: on an upgrade the chart already
    # defaults it to "existing", and leaving it pinned would break a genuine
    # from-scratch install later.
EOF

  helm -n "$NS" upgrade "$REL" "$WORKDIR/chart/openebs" \
    -f "$WORKDIR/install-values.yaml" -f "$WORKDIR/etcd-post.yaml" --timeout 10m >/dev/null

  # Any member still on the pre-migration ControllerRevision gets rolled now.
  # On a 2-member cluster that is another brief write outage, which is inherent
  # to replicaCount=2 rather than to this procedure.
  wait_for "StatefulSet rollout to converge" "$TIMEOUT_SEC" bash -c \
    "[ \"\$(kubectl -n '$NS' get sts '$STS' -o jsonpath='{.status.currentRevision}')\" = \"\$(kubectl -n '$NS' get sts '$STS' -o jsonpath='{.status.updateRevision}')\" ] && [ \"\$(kubectl -n '$NS' get sts '$STS' -o jsonpath='{.status.readyReplicas}')\" = '$ETCD_REPLICAS' ]" \
    || bad "rollout did not converge after the revert"

  hdr "REVERT: assertions"
  assert_eq "updateStrategy back to RollingUpdate" "RollingUpdate" \
    "$(k get sts "$STS" -o jsonpath='{.spec.updateStrategy.type}')"
  wait_for "etcd pods ready" 300 etcd_pods_ready || bad "etcd pods not ready after revert"
  phase_verify
}

# --------------------------------------------------------------- teardown ----

no_test_pvs() {
  ! kubectl get pv --no-headers 2>/dev/null | grep -q "$TEST_SC"
}

phase_teardown() {
  hdr "TEARDOWN"
  # Order matters. A DiskPool carries an openebs.io/diskpool-protection
  # finalizer and will sit in Terminating forever while any replica still lives
  # on it, so every Mayastor volume has to be gone -- PV reclaimed, not merely
  # PVC deleted -- before the pools can be removed.
  say "removing the workload"
  kubectl -n "$TEST_NS" delete deploy "$TEST_APP" --ignore-not-found --wait=true --timeout=180s >/dev/null 2>&1 || true
  say "removing test PVCs"
  kubectl -n "$TEST_NS" delete pvc "$TEST_PVC" --ignore-not-found --timeout=180s >/dev/null 2>&1 || true
  # Anything else this test provisioned on the mayastor StorageClass (probe
  # PVCs from a failed verify, for instance).
  local pvc
  for pvc in $(kubectl -n "$TEST_NS" get pvc -o json 2>/dev/null \
                 | jq -r --arg sc "$TEST_SC" '.items[]|select(.spec.storageClassName==$sc)|.metadata.name'); do
    kubectl -n "$TEST_NS" delete pvc "$pvc" --ignore-not-found --timeout=180s >/dev/null 2>&1 || true
  done
  wait_for "Mayastor PVs to be reclaimed" 300 no_test_pvs \
    || warn "Mayastor PVs still present; DiskPool deletion may block"

  say "removing DiskPools"
  k delete dsp --all --ignore-not-found --timeout=180s >/dev/null 2>&1 \
    || warn "DiskPool deletion timed out -- check for leftover replicas: kubectl -n $NS get dsp"
  kubectl delete sc "$TEST_SC" --ignore-not-found >/dev/null 2>&1 || true

  # The etcd and loki PVCs come from volumeClaimTemplates, so helm does not own
  # them and `helm uninstall` leaves them behind. Getting rid of them cleanly is
  # a three-way squeeze:
  #   - a PVC cannot be deleted while a pod still mounts it (pvc-protection),
  #   - the pods only go away when the StatefulSets do,
  #   - but `helm uninstall` also deletes the localpv provisioner, and without
  #     it nothing reclaims the hostpath PVs or removes the directories.
  # So: drop the StatefulSets by hand first, then the PVCs, and only then
  # uninstall. Delete the PVCs after the uninstall instead and you are left with
  # Released PVs and tens of megabytes of orphaned etcd data on every node.
  say "deleting StatefulSets so their pods release the PVCs"
  k delete sts --all --ignore-not-found --timeout=300s >/dev/null 2>&1 || true
  say "removing retained PVCs while the provisioner is still alive"
  k delete pvc --all --ignore-not-found --timeout=300s >/dev/null 2>&1 || true
  wait_for "hostpath PVs to be reclaimed" 300 bash -c \
    "! kubectl get pv --no-headers 2>/dev/null | grep -qE 'localpv|Released'" \
    || warn "hostpath PVs not reclaimed; expect orphaned directories under /var/local/$REL"

  say "uninstalling the release"
  helm -n "$NS" uninstall "$REL" --wait --timeout 10m >/dev/null 2>&1 || true
  # Strip the label from every node it could have been put on, not just the
  # mayastor workers -- the control-plane node is a candidate too.
  local n
  for n in $(kubectl get nodes -o jsonpath='{range .items[*]}{.metadata.name}{"\n"}{end}'); do
    kubectl label node "$n" "${LABEL_KEY}-" >/dev/null 2>&1 || true
  done
  say "teardown done (namespace $NS left in place)"
}

# ------------------------------------------------------------------- main ----

summary() {
  printf '\n===============================================\n'
  printf '  PASSED: %d\n  FAILED: %d\n' "$PASS" "$FAIL"
  printf '  artifacts: %s\n' "$WORKDIR"
  printf '===============================================\n'
  if (( FAIL > 0 )); then return 1; fi
  return 0
}

main() {
  local phase="${1:-all}"
  case "$phase" in
    setup)    phase_setup ;;
    prepare)  phase_prepare ;;
    migrate)  phase_migrate ;;
    verify)   phase_verify ;;
    revert)   phase_revert ;;
    teardown) phase_teardown; return 0 ;;
    all)      phase_setup; phase_prepare; phase_migrate; phase_verify; phase_revert ;;
    *)        die "unknown phase '$phase' (setup|prepare|migrate|verify|revert|teardown|all)" ;;
  esac
  summary
}

main "$@"
