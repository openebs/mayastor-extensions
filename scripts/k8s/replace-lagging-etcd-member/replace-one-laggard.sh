#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# PURPOSE (WHAT THIS SCRIPT DOES, AT A GLANCE)
# ──────────────────────────────────────────────────────────────────────────────
# This script automates a *safe* single-node remediation for a Bitnami-based
# etcd cluster running as a Kubernetes StatefulSet. In plain terms:
#   1) Detect exactly one member that is lagging behind others (by raft
#      applied index, a.k.a. "revision").
#   2) Ensure the whole cluster is on the same StatefulSet ControllerRevision
#      (so templates/flags are consistent) — note: we *only* check labels, not
#      Pod Ready.
#   3) Perform a *double* health gate:
#        • Preflight health/quorum check *with deduplication by unique member_id*,
#          so we don't over-count endpoints behind a headless service.
#        • Immediate "stabilize" gate right before removing membership to catch
#          transient elections or revision drift.
#   4) Remove the lagging member from etcd membership using *explicit endpoints*
#      (headless svc FQDNs), deliberately avoiding `${STS}-0` and the laggard as
#      both senders and endpoints. Retries with short timeouts are used to be
#      fail-fast and clear.
#   5) After the membership change, re-verify the remaining cluster is healthy
#      (leader present; equal raft terms; equal revisions) before touching data.
#   6) Delete the laggard's PVC and Pod to force a clean rejoin and wait until
#      the cluster returns to equal revisions across all members.
#
# WHO THIS IS FOR
#   Operators/SREs who need a repeatable, guard-railed procedure to fix a single
#   out-of-sync etcd member in a small (REPLICAS=3 typical) Bitnami etcd set.
#
# WHAT THIS SCRIPT *DOES NOT* DO
#   • It will *not* proceed if multiple members appear behind.
#   • It will *not* proceed on major chart/app version >= 11 (safety gate).
#   • It will *not* proceed if quorum/health gates fail at any step.
#
# SAFETY INVARIANTS
#   • Majority/leader are verified *twice* (preflight and just-in-time).
#   • Membership changes are sent to explicit, known-stable endpoints; we avoid
#     the target member and `${STS}-0` both as *senders* and *endpoints*.
#   • No data is deleted until the *remaining* cluster proves stable/healthy.
#
# DEEP DIVE: HOW QUORUM & HEALTH ARE JUDGED (IMPORTANT!)
#   etcd maintains a Raft cluster over N voting members. "Quorum" is majority:
#     quorum = floor(N/2) + 1.
#   We must never remove membership unless the cluster can maintain quorum and
#   recognizes a leader — otherwise we could tip it into a write outage.
#
#   This script's quorum/health logic runs TWO complementary checks:
#
#   (A) endpoint status (JSON) — *structural* health:
#       We call `etcdctl endpoint status --cluster -w json` from inside a pod.
#       The payload (one object per endpoint) contains:
#         - "Endpoint": the URL we talked to (we filter to our headless FQDNs).
#         - "member_id": the numeric Raft ID for that endpoint.
#         - "leader": the leader's member_id as the endpoint believes it.
#         - "raftTerm"/"raft_term": the current Raft term.
#         - "revision": applied index, a monotonic-ish counter of progress.
#       We use this to:
#         • Map Endpoint -> member_id (EP2MID map).
#         • Build a set of UNIQUE member_ids visible (dedup against headless svc
#           which can present multiple hostnames/IPs).
#         • Capture a leader id if one is known.
#       PASS condition (structural):
#         • A leader id is present (non-empty), AND
#         • The count of **unique** member_ids visible >= quorum.
#
#   (B) endpoint health (plaintext) — *liveness* health:
#       We call `etcdctl endpoint health --cluster`, which prints one line per
#       endpoint like: "http://... is healthy: successfully committed proposal".
#       Using the EP2MID mapping built in (A), we count **unique** member_ids
#       whose line says "is healthy". This prevents double-counting the same
#       member via multiple headless endpoints.
#       PASS condition (liveness):
#         • The number of **unique** healthy member_ids >= quorum.
#
#   Why both? "status" proves a leader & visibility of a quorum; "health" proves
#   that those members are responsive *right now* and can commit a proposal.
#   Running both substantially reduces the chance of a false positive in the
#   tight window around elections or temporary connectivity hiccups.
#
# EXIT CODES (for quick triage)
#   2  : could not collect endpoint status from any pod
#   3  : merged cluster view didn't yield exactly REPLICAS distinct pods
#   4  : more than one laggard detected (unsafe to auto-repair)
#   5  : timed out while waiting for final resync (equal revisions)
#   6  : member remove failed from all control pods
#   20 : unable to determine chart/app version labels
#   21 : version gate failed (require < 11.0.0)
#   22 : could not determine current ControllerRevision
#   23 : timed out waiting for ControllerRevision convergence across pods
#   24 : cluster failed to become healthy (post-remove pre-delete gate)
#   25 : liveness majority not met (unique healthy < quorum)
#   26 : unable to fetch endpoint status JSON
#   27 : structural quorum not met (no leader or unique visible < quorum)
# ──────────────────────────────────────────────────────────────────────────────

# ---- config ----
NS="${NS:-mayastor}"              # Namespace containing the StatefulSet
STS="${STS:-mayastor-etcd}"       # StatefulSet name (pods are ${STS}-0, ${STS}-1, ...)
CN="${CN:-etcd}"                  # Container name inside the pods that holds etcd
REPLICAS="${REPLICAS:-3}"         # Expected etcd member count (used for sanity and majority calc)
TIMEOUT_SEC="${TIMEOUT_SEC:-900}" # Generic timeout bound for long waits
SLEEP_SEC="${SLEEP_SEC:-5}"       # Poll interval for waits/retries
REMOVE_TRIES="${REMOVE_TRIES:-6}" # Retries per sender for `etcdctl member remove`

say() { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*"; } # timestamped operator log lines

# Build the deterministic pod list: ${STS}-0 .. ${STS}-$((REPLICAS-1))
# This assumes ordinal pod names and equal REPLICAS <-> cluster members.
declare -a PODS=()
for i in $(seq 0 $((REPLICAS - 1))); do PODS+=("${STS}-${i}"); done

# ──────────────────────────────────────────────────────────────────────────────
# VERSION GATE (safety) — refuse major >= 11
# Rationale: newer charts may alter env flags/bootstrapping semantics such that
# automation assumptions would be unsafe. We parse app version or chart tag.
# ──────────────────────────────────────────────────────────────────────────────
normalize_ver() { printf '%s' "$1" | sed -E 's/^v//; s/[^0-9.].*$//'; }
major_of() { awk -F. '{print ($1==""?0:$1)}' <<<"$1"; }
ensure_chart_lt_11() {
  say "Checking StatefulSet version labels (< 11.0.0 required)…"
  local ver chart major
  ver="$(kubectl -n "$NS" get sts "$STS" -o jsonpath='{.metadata.labels.app\.kubernetes\.io/version}' || true)"
  if [[ -z "$ver" ]]; then
    chart="$(kubectl -n "$NS" get sts "$STS" -o jsonpath='{.metadata.labels.helm\.sh/chart}' || true)"
    ver="$(printf '%s\n' "$chart" | sed -E 's/.*-([0-9]+\.[0-9]+(\.[0-9]+)?).*/\1/')" || true
  fi
  ver="$(normalize_ver "${ver:-}")"
  [[ -z "$ver" ]] && {
    say "Unable to determine chart/app version from labels; refusing to proceed."
    exit 20
  }
  major="$(major_of "$ver")"
  say "Detected chart/app version: $ver (major=$major)"
  ((major >= 11)) && {
    say "Version gate failed: require < 11.0.0 (found $ver). Aborting."
    exit 21
  }
  say "Version gate OK (< 11.0.0)."
}

# ──────────────────────────────────────────────────────────────────────────────
# JSON UTIL (array → one-object-per-line)
# Why: we avoid a jq dependency; etcdctl returns a JSON array we split with sed.
# ──────────────────────────────────────────────────────────────────────────────
json_array_to_lines() {
  tr -d '\n' | sed -E 's/^\s*\[//; s/\]\s*$//; s/\}\s*,\s*\{/\}\n\{/g'
}

# ──────────────────────────────────────────────────────────────────────────────
# etcd ENDPOINT STATUS PARSER (pod → "<pod>\t<revision>")
# From a pod-local `etcdctl endpoint status --cluster -w json`, filter to our
# headless addresses, extract pod name and applied index ("revision").
# ──────────────────────────────────────────────────────────────────────────────
parse_status_json_to_pod_rev() {
  local headless_re="http://${STS}-[0-9]+\.${STS}-headless\.${NS}\.svc"
  json_array_to_lines |
    while IFS= read -r obj; do
      ep=$(printf '%s' "$obj" | grep -Eo '"Endpoint":"[^"]+"' | cut -d'"' -f4 || true)
      [[ -z "$ep" ]] && continue
      if printf '%s' "$ep" | grep -Eq "$headless_re"; then
        pod=$(printf '%s' "$ep" | sed -n "s#.*http://\(${STS}-[0-9]\+\)\..*#\1#p")
        rev=$(printf '%s' "$obj" | grep -Eo '"revision":[0-9]+' | head -1 | cut -d: -f2)
        [[ -n "$pod" && "$rev" =~ ^[0-9]+$ ]] && printf '%s\t%s\n' "$pod" "$rev"
      fi
    done
}

# Collects a single pod's cluster view and normalizes it to "<pod>\t<revision>"
collect_from_pod() {
  local pod="$1"
  kubectl -n "$NS" exec "$pod" -c "$CN" -- etcdctl endpoint status --cluster -w json | parse_status_json_to_pod_rev
}

# ──────────────────────────────────────────────────────────────────────────────
# CONTROL POD SELECTION
# Ordered candidates where avoids come last; used for failover of control ops.
# ──────────────────────────────────────────────────────────────────────────────
ctrl_candidates() {
  local -a avoid_list=("$@")
  local p a skip
  for p in "${PODS[@]}"; do
    skip=0
    for a in "${avoid_list[@]}"; do
      [[ -n "$a" && "$p" == "$a" ]] && {
        skip=1
        break
      }
    done
    ((!skip)) && echo "$p"
  done
  for a in "${avoid_list[@]}"; do [[ -n "$a" ]] && echo "$a"; done
}

# Run etcdctl on the first pod that returns success; print stdout if any.
run_etcdctl_any_output() {
  local -a avoids=()
  while [[ "$#" -gt 0 && "${1:-}" != "--" ]]; do
    avoids+=("$1")
    shift
  done
  [[ "${1:-}" == "--" ]] && shift
  local out cand rc=1
  while read -r cand; do
    if out="$(kubectl -n "$NS" exec "$cand" -c "$CN" -- etcdctl "$@")"; then
      printf '%s' "$out"
      return 0
    fi
  done < <(ctrl_candidates "${avoids[@]}")
  return $rc
}

# Same approach, but only interested in exit status (no stdout).
run_etcdctl_any_exec() {
  local -a avoids=()
  while [[ "$#" -gt 0 && "${1:-}" != "--" ]]; do
    avoids+=("$1")
    shift
  done
  [[ "${1:-}" == "--" ]] && shift
  local cand
  while read -r cand; do
    if kubectl -n "$NS" exec "$cand" -c "$CN" -- etcdctl "$@"; then return 0; fi
  done < <(ctrl_candidates "${avoids[@]}")
  return 1
}

# ──────────────────────────────────────────────────────────────────────────────
# EXPLICIT ENDPOINT LIST (exclude some pods)
# Builds a CSV of http://<pod>.<headless>...:2379 for all pods not in avoids.
# Why: for control-plane ops (member remove), we do *not* want implicit 127.0.0.1
# or volatile targets (the laggard and `${STS}-0`).
# ──────────────────────────────────────────────────────────────────────────────
list_endpoints_excluding() {
  local -a avoids=("$@")
  local p a skip out=()
  for p in "${PODS[@]}"; do
    skip=0
    for a in "${avoids[@]}"; do [[ -n "$a" && "$p" == "$a" ]] && {
      skip=1
      break
    }; done
    ((skip)) && continue
    out+=("http://${p}.${STS}-headless.${NS}.svc.cluster.local:2379")
  done
  (
    IFS=,
    echo "${out[*]}"
  )
}

# ──────────────────────────────────────────────────────────────────────────────
# MEMBER-ID LOOKUP (hex)
# Obtain the member's raft ID (hex) by pod name or, failing that, by peerURL.
# ──────────────────────────────────────────────────────────────────────────────
member_id_hex_for_pod() {
  local avoid="$1" pod="$2" json obj id_hex id_dec peer
  if ! json="$(run_etcdctl_any_output "$avoid" -- member list -w json)"; then
    say "Failed to fetch member list JSON (for ${pod})."
    return 1
  fi
  while IFS= read -r obj; do
    if printf '%s' "$obj" | grep -q "\"name\":\"$pod\""; then
      id_hex="$(printf '%s' "$obj" | sed -nE 's/.*"ID":[[:space:]]*"0x?([0-9a-fA-F]+)".*/\1/ip' | head -1)"
      [[ -n "$id_hex" ]] && {
        echo "${id_hex,,}"
        return 0
      }
      id_dec="$(printf '%s' "$obj" | sed -nE 's/.*"ID":[[:space:]]*([0-9]+).*/\1/p' | head -1)"
      [[ -n "$id_dec" ]] && {
        printf '%x\n' "$id_dec"
        return 0
      }
    fi
  done < <(printf '%s' "$json" | json_array_to_lines)
  peer="http://${pod}.${STS}-headless.${NS}.svc.cluster.local:2380"
  while IFS= read -r obj; do
    printf '%s' "$obj" | grep -Fq "$peer" || continue
    id_hex="$(printf '%s' "$obj" | sed -nE 's/.*"ID":[[:space:]]*"0x?([0-9a-fA-F]+)".*/\1/ip' | head -1)"
    [[ -n "$id_hex" ]] && {
      echo "${id_hex,,}"
      return 0
    }
    id_dec="$(printf '%s' "$obj" | sed -nE 's/.*"ID":[[:space:]]*([0-9]+).*/\1/p' | head -1)"
    [[ -n "$id_dec" ]] && {
      printf '%x\n' "$id_dec"
      return 0
    }
  done < <(printf '%s' "$json" | json_array_to_lines)
  return 1
}

# ──────────────────────────────────────────────────────────────────────────────
# STS REVISION HELPERS — ensure all pods on same ControllerRevision (labels)
# We do not check "Ready" here by design; we only require label convergence to
# avoid mixing template versions during membership ops.
# ──────────────────────────────────────────────────────────────────────────────
sts_update_rev() { kubectl -n "$NS" get sts "$STS" -o jsonpath='{.status.updateRevision}' || true; }
sts_current_rev() { kubectl -n "$NS" get sts "$STS" -o jsonpath='{.status.currentRevision}' || true; }
pod_rev() { kubectl -n "$NS" get pod "$1" -o jsonpath='{.metadata.labels.controller-revision-hash}' || true; }

# ──────────────────────────────────────────────────────────────────────────────
# QUORUM & HEALTH (PREFLIGHT) — DEDUP BY UNIQUE MEMBER ID
# *** This is the heart of our safety gates. See the top-of-file deep dive. ***
# Steps:
#   1) Build EP2MID map and capture leader via endpoint *status* JSON.
#   2) Use that map to deduplicate endpoint *health* lines by unique member_id.
#   3) Require both:
#        • unique healthy >= quorum (liveness),
#        • leader present AND unique visible >= quorum (structural).
# ──────────────────────────────────────────────────────────────────────────────
ensure_cluster_quorum_and_health() {
  say "Checking etcd cluster health and quorum…"

  local majority=$((REPLICAS / 2 + 1))
  local headless_re="http://${STS}-[0-9]+\.${STS}-headless\.${NS}\.svc"

  # (1) STATUS JSON → EP2MID + leader
  local json ldr="" ep mid
  declare -A EP2MID=()
  declare -A SEEN_IDS=()
  if ! json="$(run_etcdctl_any_output "" -- endpoint status --cluster -w json)"; then
    say "Unable to fetch endpoint status JSON; aborting."
    exit 26
  fi
  while IFS= read -r obj; do
    ep=$(printf '%s' "$obj" | grep -Eo '"Endpoint":"[^"]+"' | cut -d'"' -f4 || true)
    [[ -z "$ldr" ]] && ldr=$(printf '%s' "$obj" | grep -Eo '"leader":[0-9]+' | head -1 | cut -d: -f2)
    [[ -z "$ep" ]] && continue
    if printf '%s' "$ep" | grep -Eq "$headless_re"; then
      mid=$(printf '%s' "$obj" | grep -Eo '"member_id":[0-9]+' | head -1 | cut -d: -f2)
      [[ -n "$mid" ]] && {
        EP2MID["$ep"]="$mid"
        SEEN_IDS["$mid"]=1
      }
    fi
  done < <(printf '%s' "$json" | json_array_to_lines)
  local visible=${#SEEN_IDS[@]}

  # (2) HEALTH plaintext → count UNIQUE healthy member_ids
  local out
  if ! out="$(run_etcdctl_any_output "" -- endpoint health --cluster)"; then
    say "endpoint health failed."
    exit 25
  fi
  declare -A HEALTHY_IDS=()
  while IFS= read -r line; do
    [[ "$line" == http* ]] || continue # ignore non-endpoint lines
    ep="${line%% *}"                   # extract first token (URL)
    if printf '%s' "$ep" | grep -Eq "$headless_re"; then
      [[ "$line" == *"is healthy"* ]] || continue # require "is healthy"
      mid="${EP2MID[$ep]:-}"                      # map endpoint to member_id
      [[ -n "$mid" ]] && HEALTHY_IDS["$mid"]=1    # set semantics → dedup
    fi
  done <<<"$out"
  local healthy=${#HEALTHY_IDS[@]}
  say "  endpoint health (dedup by member_id): healthy=${healthy} total_unique=${visible} (majority=${majority})"

  # (3) Combined judgement
  if ((healthy < majority)); then
    say "Cluster does not have healthy majority; aborting."
    exit 25
  fi
  say "  endpoint status: leader=${ldr:-<none>} visible_members=${visible}"
  if [[ -z "$ldr" ]] || ((visible < majority)); then
    say "Cluster lacks leader or visible quorum; aborting."
    exit 27
  fi
  say "Cluster health + quorum OK."
}

# ──────────────────────────────────────────────────────────────────────────────
# HEALTH AFTER REMOVAL (EXCLUDING ONE MEMBER)
# Post-change safety gate: ensure remaining cluster is steady before deleting
# any storage. We require:
#   • all remaining pods observed;
#   • a leader whose member_id is among the remaining members;
#   • equal raft terms (no election churn);
#   • equal revisions (fully caught up).
# ──────────────────────────────────────────────────────────────────────────────
wait_healthy_excluding() {
  local exclude="$1"
  say "Verifying etcd cluster health (excluding ${exclude:-<none>})…"
  local deadline=$(($(date +%s) + TIMEOUT_SEC))
  while :; do
    local json headless_re objs ep pod rev term mid ldr
    local goto_sleep=""
    if ! json="$(run_etcdctl_any_output "$exclude" -- endpoint status --cluster -w json)"; then
      say "  - endpoint status failed; retrying…"
      goto_sleep=1
    else
      headless_re="http://${STS}-[0-9]+\.${STS}-headless\.${NS}\.svc"
      declare -A REV=() TERM=() MID=()
      ldr=""
      while IFS= read -r objs; do
        ep=$(printf '%s' "$objs" | grep -Eo '"Endpoint":"[^"]+"' | cut -d'"' -f4 || true)
        [[ -z "$ep" ]] && continue
        if printf '%s' "$ep" | grep -Eq "$headless_re"; then
          pod=$(printf '%s' "$ep" | sed -n "s#.*http://\(${STS}-[0-9]\+\)\..*#\1#p")
          rev=$(printf '%s' "$objs" | grep -Eo '"revision":[0-9]+' | head -1 | cut -d: -f2)
          term=$(printf '%s' "$objs" | grep -Eo '"raftTerm":[0-9]+' | head -1 | cut -d: -f2)
          [[ -z "$term" ]] && term=$(printf '%s' "$objs" | grep -Eo '"raft_term":[0-9]+' | head -1 | cut -d: -f2)
          mid=$(printf '%s' "$objs" | grep -Eo '"member_id":[0-9]+' | head -1 | cut -d: -f2)
          [[ -z "$ldr" ]] && ldr=$(printf '%s' "$objs" | grep -Eo '"leader":[0-9]+' | head -1 | cut -d: -f2)
          [[ -n "$pod" && "$pod" != "$exclude" && "$rev" =~ ^[0-9]+$ ]] && REV["$pod"]="$rev"
          [[ -n "$pod" && "$pod" != "$exclude" && "$term" =~ ^[0-9]+$ ]] && TERM["$pod"]="$term"
          [[ -n "$pod" && "$pod" != "$exclude" && "$mid" =~ ^[0-9]+$ ]] && MID["$pod"]="$mid"
        fi
      done < <(printf '%s' "$json" | json_array_to_lines)

      # Presence: did we see every remaining pod?
      local present=1
      for p in "${PODS[@]}"; do
        [[ "$p" == "$exclude" ]] && continue
        [[ -z "${REV[$p]+_}" ]] && present=0
      done

      if ((present)); then
        # Leader must be among remaining members
        local leader_ok=0
        for p in "${PODS[@]}"; do
          [[ "$p" == "$exclude" ]] && continue
          [[ "${MID[$p]:-}" == "$ldr" ]] && leader_ok=1
        done
        if ((!leader_ok)); then
          say "  - no leader among remaining members (leader=$ldr); retrying…"
          goto_sleep=1
        else
          # Terms must match across remaining members
          local min_rev=0 max_rev=0 term0=""
          for p in "${PODS[@]}"; do
            [[ "$p" == "$exclude" ]] && continue
            local r="${REV[$p]}" t="${TERM[$p]}"
            ((r > max_rev)) && max_rev="$r"
            ((min_rev == 0 || r < min_rev)) && min_rev="$r"
            [[ -z "$term0" ]] && term0="$t"
            if [[ "$t" != "$term0" ]]; then
              say "  - raft terms mismatch: $p has $t (expected $term0)"
              goto_sleep=1
              break
            fi
          done
          # Revisions must converge (equal)
          if [[ -z "$goto_sleep" && "$min_rev" == "$max_rev" ]]; then
            say "  - healthy: leader=$ldr, revision=$max_rev, term=$term0"
            return 0
          fi
          [[ -z "$goto_sleep" ]] && {
            say "  - revisions differ among remaining members: min=$min_rev max=$max_rev; retrying…"
            goto_sleep=1
          }
        fi
      else
        goto_sleep=1
      fi
    fi
    (($(date +%s) >= deadline)) && {
      say "Cluster did not become healthy in time (post-remove pre-delete); aborting."
      exit 24
    }
    sleep "$SLEEP_SEC"
  done
}

# ──────────────────────────────────────────────────────────────────────────────
# MEMBER REMOVE WITH EXPLICIT ENDPOINTS & RETRIES
# Rationale: Avoid the laggard and `${STS}-0` as both senders and endpoints to
# reduce the chance we hit a restarting/unreliable node. Use short timeouts to
# fail fast; retry with mild backoff across multiple senders.
# ──────────────────────────────────────────────────────────────────────────────
member_remove_with_retries() {
  local id="$1" endpoints_csv="$2" tries="${REMOVE_TRIES}" backoff=2
  for cand in $(ctrl_candidates "${STS}-0" "$lagpod"); do
    for ((i = 1; i <= tries; i++)); do
      say "  - member remove try ${i}/${tries} via ${cand} (endpoints: ${endpoints_csv})"
      if kubectl -n "$NS" exec "$cand" -c "$CN" -- \
        env ETCDCTL_API=3 etcdctl \
        --dial-timeout=5s --command-timeout=10s \
        --endpoints="${endpoints_csv}" member remove "${id}"; then
        return 0
      fi
      sleep "$backoff"
      backoff=$((backoff < 10 ? backoff + 1 : backoff))
    done
  done
  return 1
}

# ---------------- main ----------------
ensure_chart_lt_11

# ──────────────────────────────────────────────────────────────────────────────
# DISCOVER PER-POD “BEST SEEN” REVISION
# We query each pod's cluster view and, for every pod name observed, record the
# *maximum* revision seen across all vantage points. This hedges against split
# visibility (one pod might temporarily not see another). With a healthy 3-node
# cluster, this should yield exactly REPLICAS rows.
# ──────────────────────────────────────────────────────────────────────────────
declare -A BEST_REV=()
collected_any=0
say "Collecting cluster views from: ${PODS[*]}"
for p in "${PODS[@]}"; do
  if ! lines="$(collect_from_pod "$p")" || [[ -z "$lines" ]]; then
    say "  - $p: unable to fetch or parse status (skipping)"
    continue
  fi
  collected_any=1
  while IFS=$'\t' read -r pod rev; do
    [[ -z "$pod" || ! "$rev" =~ ^[0-9]+$ ]] && continue
    if [[ -z "${BEST_REV[$pod]+_}" || "$rev" -gt "${BEST_REV[$pod]}" ]]; then BEST_REV["$pod"]="$rev"; fi
  done <<<"$lines"
done
((!collected_any)) && {
  say "Failed to collect from all pods. Verify container name and etcdctl path."
  exit 2
}
((${#BEST_REV[@]} != REPLICAS)) && {
  say "Expected ${REPLICAS} members, but merged ${#BEST_REV[@]} (maybe a pod missing headless advertise?). Aborting."
  for k in "${!BEST_REV[@]}"; do printf '  - %s rev=%s\n' "$k" "${BEST_REV[$k]}"; done
  exit 3
}

# Determine who is behind (strictly less than global max revision)
max_rev=0
for pod in "${!BEST_REV[@]}"; do ((BEST_REV[$pod] > max_rev)) && max_rev="${BEST_REV[$pod]}"; done
declare -a LAGGARDS=()
for pod in "${!BEST_REV[@]}"; do ((BEST_REV[$pod] < max_rev)) && LAGGARDS+=("$pod"); done

say "Merged best-known revisions (per pod):"
for pod in "${!BEST_REV[@]}"; do printf '  - %s  %s\n' "$pod" "${BEST_REV[$pod]}"; done

# Guardrails: either everybody matches, or exactly one laggard; otherwise bail.
if ((${#LAGGARDS[@]} == 0)); then
  say "All ${REPLICAS} etcd instances are in sync. Revision=${max_rev}"
  exit 0
fi
if ((${#LAGGARDS[@]} > 1)); then
  say "More than one member appears behind: ${LAGGARDS[*]}. Refusing to auto-replace."
  exit 4
fi

# One laggard → replace it
lagpod="${LAGGARDS[0]}"
ordinal="${lagpod##*-}"
pvc="data-${STS}-${ordinal}"
say "Lagging member: ${lagpod} (rev=${BEST_REV[$lagpod]} vs max=${max_rev})"

# Ensure the Bitnami chart takes the "existing cluster" path on rejoin.
say "Patching StatefulSet ETCD_INITIAL_CLUSTER_STATE=existing"
kubectl -n "$NS" set env statefulset/"$STS" ETCD_INITIAL_CLUSTER_STATE=existing >/dev/null

# Determine latest ControllerRevision and ensure label convergence across pods.
latest_rev="$(sts_update_rev)"
[[ -z "$latest_rev" ]] && latest_rev="$(sts_current_rev)"
[[ -z "$latest_rev" ]] && {
  say "Could not determine current ControllerRevision; aborting."
  exit 22
}
latest_hash="${latest_rev##*-}"
say "ControllerRevision in use: ${latest_rev} (hash=${latest_hash})"

say "Ensuring all pods report ControllerRevision ${latest_rev} / hash=${latest_hash}…"
roll_deadline=$(($(date +%s) + TIMEOUT_SEC))
while :; do
  all_ok=1
  for p in "${PODS[@]}"; do
    cur="$(pod_rev "$p")"
    ok_rev=0
    [[ "$cur" == "$latest_hash" ]] && ok_rev=1
    [[ "$cur" == "$latest_rev" ]] && ok_rev=1
    [[ "${latest_rev#"${STS}"-}" == "$cur" ]] && ok_rev=1
    if ((ok_rev)); then :; else
      all_ok=0
      say "  - $p label=${cur:-<none>} expected={${latest_hash}|${latest_rev}}"
    fi
  done
  ((all_ok)) && {
    say "All pods are on ControllerRevision ${latest_rev}/${latest_hash}."
    break
  }
  (($(date +%s) >= roll_deadline)) && {
    say "Timed out waiting for pods to match ControllerRevision."
    exit 23
  }
  sleep "$SLEEP_SEC"
done

# Preflight — must be healthy & have quorum (dedup + leader). See deep dive.
ensure_cluster_quorum_and_health

# Just-in-time stabilization — catch last-moment elections or drift.
say "Stabilizing: ensuring full-cluster health before member removal…"
wait_healthy_excluding ""

# Remove the laggard’s cluster membership (by HEX member ID) using explicit endpoints
say "Attempting to remove cluster member for ${lagpod}…"
member_id_hex="$(member_id_hex_for_pod "$lagpod" "$lagpod" || true)"
if [[ -n "${member_id_hex:-}" ]]; then
  say "Removing member ID (hex) ${member_id_hex} for ${lagpod}"
  endpoints_csv="$(list_endpoints_excluding "$lagpod" "${STS}-0")"
  if ! member_remove_with_retries "${member_id_hex}" "${endpoints_csv}"; then
    say "Member remove failed from all control pods (see errors above). Current members:"
    # Table display helps the operator see the live membership set at failure time.
    run_etcdctl_any_output "$lagpod" -- member list -w table | sed 's/^/  /' || true
    exit 6
  fi
else
  say "No matching member found for ${lagpod} (maybe already removed)."
fi

# Post-remove guard: ensure remaining cluster is steady before deleting data.
wait_healthy_excluding "$lagpod"

# Delete PVC & Pod so the member is recreated with a fresh data dir and rejoins.
say "Deleting PVC ${pvc} (background) and Pod ${lagpod}…"
(kubectl -n "$NS" delete pvc "$pvc" --ignore-not-found || true) &
kubectl -n "$NS" delete pod "$lagpod" --wait=false || true

# Wait for the recreated pod to come up Ready; the STS will re-provision storage.
say "Waiting for ${lagpod} to become Ready…"
kubectl -n "$NS" wait --for=condition=ready "pod/${lagpod}" --timeout="${TIMEOUT_SEC}s"

# Final convergence: loop until all members report equal revision again.
say "Waiting for cluster to re-balance (all equal revisions)…"
start_ts="$(date +%s)"
while :; do
  declare -A BEST_REV=()
  collected_any=0
  for p in "${PODS[@]}"; do
    lines="$(collect_from_pod "$p" || true)"
    [[ -z "$lines" ]] && continue
    collected_any=1
    while IFS=$'\t' read -r pod rev; do
      [[ -z "$pod" || ! "$rev" =~ ^[0-9]+$ ]] && continue
      if [[ -z "${BEST_REV[$pod]+_}" || "$rev" -gt "${BEST_REV[$pod]}" ]]; then BEST_REV["$pod"]="$rev"; fi
    done <<<"$lines"
  done
  if ((collected_any && ${#BEST_REV[@]} == REPLICAS)); then
    cur_max=0
    cur_min=0
    first=1
    for p in "${!BEST_REV[@]}"; do
      r="${BEST_REV[$p]}"
      ((r > cur_max)) && cur_max="$r"
      if ((first)); then
        cur_min="$r"
        first=0
      else ((r < cur_min)) && cur_min="$r"; fi
    done
    say "Current min/max: min=${cur_min} max=${cur_max}"
    if [[ "$cur_min" == "$cur_max" ]]; then
      say "Cluster back in sync. Revision=${cur_max}"
      exit 0
    fi
  else
    say "Cluster view incomplete; retrying…"
  fi
  (($(date +%s) - start_ts > TIMEOUT_SEC)) && {
    say "Timed out."
    exit 5
  }
  sleep "$SLEEP_SEC"
done
