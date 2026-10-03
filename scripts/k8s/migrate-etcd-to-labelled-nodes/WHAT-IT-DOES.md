# What `migrate-mayastor-etcd.sh` does to your cluster

It moves the Mayastor etcd members onto nodes you have labelled, **one member at a time**. Each move
destroys that member's data and rebuilds it from the others. The etcd keyspace holds the record of
every volume, replica, pool and nexus, so the script is built around refusing to take the next step
until the cluster has proved it is healthy.

Read this before running it. The `README.md` next door has the reasoning and the knobs.

---

## Versions

| Component | Version |
|---|---|
| openebs umbrella chart | 4.2.0 |
| mayastor subchart | 2.8.0 |
| bitnami etcd subchart | 8.6.0 |
| etcd | 3.5.6 |
| etcd image | `docker.io/openebs/etcd:3.5.6-debian-11-r10` |
| Kubernetes (verified on) | v1.35.6 |

Verified end-to-end at `replicaCount` **2** and **3**, on a 3-worker and a 7-worker cluster (four runs,
44 assertions each, all passing). Preflight warns on any other etcd chart major, because the bootstrap
semantics it depends on are specific to this one.

> `docker.io/bitnami/etcd:3.5.6-debian-11-r10` and `docker.io/bitnami/kubectl:1.25.15` (the chart's
> `pre-upgrade` hook) now return **404** — Bitnami retired those tags. Use `openebs/etcd` and
> `bitnamilegacy/kubectl`, or every `helm upgrade` will hang.

---

## Why a plain affinity edit does not work

- Affinity lives in the pod template, so **nothing moves until a pod is recreated**.
- etcd storage is `mayastor-etcd-localpv` — a **hostpath** PV pinned to one node. A recreated pod
  either returns to the old node, or sits `Pending` with a *volume node affinity conflict*.

So a member can only move by being removed from etcd membership, having its PVC destroyed, and
rejoining empty on the new node. That is what the script automates.

---

## Before you start

**On your workstation:** `kubectl`, `helm`, `jq`, `awk`, `sed`, and **bash 4+** (macOS ships bash 3.2 —
use `brew install bash`). The script exits immediately if any are missing.

**Find your names.** The script defaults to namespace `mayastor` / StatefulSet `mayastor-etcd`; an
umbrella-chart install is usually `openebs` / `openebs-etcd`.

```bash
helm list -A
kubectl get sts -A -l app.kubernetes.io/name=etcd
```

**Label your destination nodes** — you need at least `replicaCount` of them, because
`podAntiAffinityPreset: hard` puts one member per node:

```bash
kubectl label node <node-a> <node-b> <node-c> node/etcd.io=true
kubectl get nodes -L node/etcd.io
```

`node/etcd.io=true` is only the default. **Any valid label key and value works** — set `LABEL_KEY`
and `LABEL_VALUE` and use the same pair in the helm values below. They must match, or preflight
refuses to run:

```bash
export LABEL_KEY=storage.example.com/etcd-tier LABEL_VALUE=primary
kubectl label node <node-a> <node-b> <node-c> "$LABEL_KEY=$LABEL_VALUE"
```

Labelling **more** nodes than you have members is fine and is the usual case — label a rack or a
zone and let the scheduler choose. Preflight only requires that at least `replicaCount` labelled nodes
are *schedulable*; cordoned ones in a larger pool are reported and skipped rather than being treated
as an error.

Destination nodes need room for a 2 GiB hostpath volume under
`/var/local/<release>/localpv-hostpath/etcd`, and any `NoSchedule` taint needs a matching
`etcd.tolerations`.

**Know where you are moving from.** Every member not already on a labelled node gets rebuilt. If you
label the nodes members are already on plus one spare, only one member moves.

---

## Two helm values you must apply first

```yaml
etcd:
  # key/values must match LABEL_KEY / LABEL_VALUE
  nodeAffinityPreset: { type: hard, key: "node/etcd.io", values: ["true"] }
  podAntiAffinityPreset: "hard"     # keep; do NOT set etcd.affinity
  updateStrategy: { type: OnDelete }
  initialClusterState: "existing"
```

- **`OnDelete`** stops the StatefulSet controller from recycling pods on its own schedule into PVCs
  still pinned to the old nodes. The script refuses to run without it.
- **`initialClusterState: "existing"`** stops a member with a freshly-emptied data directory from
  deciding it is a brand-new cluster and **bootstrapping over the keyspace**. This is the single most
  important value on the page. Remove it after the migration.

Applying these should restart **nothing**. If pods cycle, `OnDelete` did not take effect — stop.

Put them in whatever values file you already manage the release with; do not hand-patch the
StatefulSet. Two helm traps worth knowing:

- **Do not set `etcd.affinity` directly.** It replaces all three presets at once, including the
  anti-affinity — which, with node-pinned hostpath PVs, is how you end up with two members on one
  node. Use `nodeAffinityPreset`.
- **Helm replaces lists, it does not merge them.** If you override `etcd.extraEnvVars` you must
  re-include Mayastor's `ETCD_QUOTA_BACKEND_BYTES: "8589934592"`, or you silently reset the backend
  quota.

---

## Running it

```bash
chmod +x migrate-mayastor-etcd.sh
export NS=openebs STS=openebs-etcd          # your names, from above

# 1. Dry run. Changes nothing; runs the full preflight and prints the planned moves.
./migrate-mayastor-etcd.sh 2>&1 | tee dryrun.log

# 2. Move ONE member and watch it. This is the recommended way.
#    Start at the HIGHEST ordinal: 2 on a 3-member cluster, 1 on a 2-member one.
DRY_RUN=false ONLY_ORDINAL=2 ./migrate-mayastor-etcd.sh 2>&1 | tee move-2.log

# 3. Once you are happy, the rest. Work downwards; ordinal 0 goes last.
DRY_RUN=false ONLY_ORDINAL=1 ./migrate-mayastor-etcd.sh 2>&1 | tee move-1.log
DRY_RUN=false ONLY_ORDINAL=0 ./migrate-mayastor-etcd.sh 2>&1 | tee move-0.log

#    ...or let it do every remaining member in that order, unattended:
DRY_RUN=false ./migrate-mayastor-etcd.sh 2>&1 | tee move-rest.log
```

Ordinal 0 goes last on purpose: it is the member whose bootstrap semantics could destroy the keyspace
if `initialClusterState` were wrong, so you want the procedure to have proved itself first.

`DRY_RUN=false` gives you a **10-second abort window** before anything is touched. A whole run takes
roughly 3–4 minutes per member; most of that is the pod being rescheduled and rejoining.

Keep a second terminal on it:

```bash
watch -n2 'kubectl -n openebs get pods -o wide -l app.kubernetes.io/name=etcd; \
           kubectl -n openebs get pvc; kubectl get pv | grep -E "etcd|Released|Failed"'
```

Expect to see, per member: the pod disappear, its PVC and PV disappear, a new PVC bind, and the pod
return on a labelled node. A pod sitting `Pending` for a few seconds while its new volume is
provisioned is normal.

### Settings

| Variable | Default | Purpose |
|---|---|---|
| `NS` / `STS` / `CN` | `mayastor` / `mayastor-etcd` / `etcd` | namespace, StatefulSet, container |
| `LABEL_KEY` / `LABEL_VALUE` | `node/etcd.io` / `true` | the node label to migrate onto |
| `DRY_RUN` | `true` | **safe default**; `false` to execute |
| `ONLY_ORDINAL` | *(unset)* | restrict the run to one member |
| `SNAPSHOT` / `SNAPSHOT_DIR` | `true` / `./etcd-snapshots` | pre-migration backup |
| `TIMEOUT_SEC` / `SLEEP_SEC` / `SETTLE_SEC` | `900` / `5` / `30` | wait bound, poll interval, quiet period |
| `MAX_RESCHEDULE_KICKS` | `6` | retries if a pod lands on an unlabelled node |
| `ICS_OVERRIDE` | `false` | proceed without `initialClusterState=existing` — **don't** |

---

## What it does, per member

Highest ordinal first, so `etcd-0` — the riskiest to rebuild — goes last. Members already on a
labelled node are skipped, so re-running it is safe.

1. **Gate 1** — whole cluster healthy and converged. Nothing happens if it is not.
2. `etcdctl member remove <member>` via a *different* pod.
3. **Gate 2** — the **remaining** members are healthy. Nothing is deleted until this passes.
4. Delete the PVC, then the pod. (The StatefulSet controller will not recreate the pod while its PVC
   is terminating, which is what stops it rebinding the old node-pinned PV.)
5. Wait for the old PVC to actually be gone. (The localpv provisioner then removes the hostpath
   directory on the vacated node asynchronously — check for leftovers afterwards.)
6. Wait for the pod to come back **Ready on a labelled node**.
7. **Gate 3** — all members healthy, converged, holding identical data.
8. Re-check the keyspace against the census taken at the start, then settle 30s.

Before any of that: a **snapshot** is taken and validated with `etcdctl snapshot status` (not just a
size check), and a census of every key is recorded.

---

## Availability — what breaks, and for how long

Measured by sampling `endpoint status` every 2s through a real move, reproduced on both clusters:

| | `replicaCount: 3` | `replicaCount: 2` |
|---|---|---|
| membership across the move | 3 → 2 → 3 | 2 → 1 → 2 |
| quorum while the member rejoins | 2 of 3 — held | 2 of 2 — **not met** |
| etcd write outage | **none observed** | **~11s** |
| raft term / leader | unchanged | re-election |

The window exists because bitnami rejoins a member with `member add` as a **full voter, not a
learner**: quorum rises before the new member is live. At 3 replicas there is still a majority
without it; at 2 there is not.

**What keeps working regardless:** the Mayastor data path does not consult etcd per-IO, so already-published
volumes keep serving. In both test runs a pod writing continuously to a 3-replica volume recorded
**zero gaps** and never restarted.

**What fails while quorum is lost:** volume create/delete/publish/unpublish, pool operations, rebuild
orchestration. Sustained quorum loss eventually exhausts the io-engine's `pstorRetries` (default 300),
after which a volume target self-shutdowns — so a stuck migration is urgent, not merely degraded.

At `replicaCount: 2` **any** etcd pod restart loses write quorum — including the rolling restart when
you later revert `updateStrategy` to `RollingUpdate`. That is inherent to running 2 members.

---

## Consistency — what the gates actually check

Every gate requires **all** expected members to be:

- present and answering, and reporting **healthy**;
- in the **same etcd cluster** — `cluster_id` identical across members *and* identical to the one
  recorded before the first move;
- naming the **same leader** (not merely "a leader that is one of ours", which a split brain
  satisfies);
- on one raft term;
- converged on one revision, never **lower** than the starting revision;
- reporting an identical `endpoint hashkv`, pinned to the agreed revision.

The last one matters: equal revisions only prove the members agree on *how far* they have got. Equal
hashes prove they hold *the same bytes*.

**Two things are treated as unrecoverable and stop the run immediately (exit 25):**
`cluster_id` changing, and the revision going backwards. Both are the signature of a member having
bootstrapped over the keyspace instead of rejoining it — the exact failure this procedure exists to
avoid.

This is stricter than a quorum check on purpose. A deliberate migration should never *begin* from a
degraded cluster.

---

## What is verified, and when

| When | Check |
|---|---|
| Preflight | `OnDelete` set; `nodeAffinity` references your label; `podAntiAffinity` intact; `ETCD_INITIAL_CLUSTER_STATE=existing`; StorageClass is `WaitForFirstConsumer` + `Delete`; enough labelled, uncordoned nodes; cluster healthy |
| Before first move | snapshot taken **and** validated; `cluster_id`, revision and full key list recorded |
| Between every step | Gates 1–3 above |
| After each move | every key from the census still present |
| End of run | final health gate, final key census, placement printed |

Afterwards, confirm yourself: all members on labelled nodes, `kubectl get pv` shows no
`Released`/`Failed`, and the vacated nodes' hostpath directories are gone.

`test-migration.sh` next door automates all of this against a throwaway cluster, including a workload
whose data is checked for gaps and a fresh volume provisioned afterwards to prove etcd is *usable*,
not merely intact.

---

## Things it will not do

- **Run destructively by default.** `DRY_RUN=true` unless you say otherwise, plus a 10s abort window.
- **Touch a second member** before the first has fully rejoined and converged.
- **Proceed past a failed gate.** It stops and tells you what it saw.
- **Cordon anything.** With `reclaimPolicy: Delete` the localpv cleanup pod must run *on the vacated
  node*; cordoning it strands the PV and leaves etcd data on disk. Do not cordon them yourself.

Exit codes: `1` preflight · `5` timeout · `6` member remove failed · `24` health gate failed ·
`25` **consistency violation** · `30` pod keeps landing on an unlabelled node.

---

## If it stops

It stops on purpose. The exit code tells you how worried to be.

| Exit | Meaning | What to do |
|---|---|---|
| `1` | preflight failed | It names the missing prerequisite. Nothing was touched. |
| `5` | timed out waiting | Usually a pod that cannot schedule. Check `kubectl describe pod`. |
| `6` | `member remove` failed | Membership is unchanged. Check `etcdctl member list`. |
| `24` | a health gate failed | The cluster was not healthy *before* the destructive step, or did not recover after one. Capture state; do not re-run blindly. |
| `25` | **consistency violation** | `cluster_id` changed or the revision went backwards. **Stop everything and escalate.** You have the snapshot. |
| `30` | pod keeps landing on an unlabelled node | The pod template's `nodeAffinity` is not what preflight thought. |

Common situations:

- **New pod `Pending`, *volume node affinity conflict*** — it rebound the old node-pinned PV. The
  script re-deletes the pod up to `MAX_RESCHEDULE_KICKS` times by itself. If it still exits 30, fix
  the affinity rather than the pod.
- **PVC stuck `Terminating`** — the `pvc-protection` finalizer holds it until no pod uses it. Check the
  pod is really gone, and that the vacated node is not cordoned or `NotReady` (the localpv cleanup pod
  has to run *there*). **Do not remove finalizers by hand** — that orphans the PV and leaves data on disk.
- **Revisions never converge** — check the new member's logs for a slow snapshot transfer. Raise
  `TIMEOUT_SEC` only after confirming the revision is actually climbing between samples.
- **Quorum lost** — urgent, not merely degraded. See the availability section above.

Capture this before asking anyone for help (`$HEALTHY` = any member that is still up — not the one
being moved):

```bash
kubectl -n $NS get pods -o wide -l app.kubernetes.io/name=etcd
kubectl -n $NS get pvc; kubectl get pv
kubectl -n $NS describe pod <the-problem-pod> | tail -40
kubectl -n $NS logs <the-problem-pod> -c etcd --tail=200
kubectl -n $NS logs <the-problem-pod> -c etcd --previous --tail=200 2>/dev/null
kubectl -n $NS get events --sort-by=.lastTimestamp | tail -40
kubectl -n $NS exec $HEALTHY -c etcd -- etcdctl member list -w table
kubectl -n $NS exec $HEALTHY -c etcd -- etcdctl endpoint status --cluster -w table
```

**Do not improvise recovery.** `etcdctl member remove`, `--force-new-cluster`, deleting PVCs, or
`helm rollback` run by hand mid-procedure will make a recoverable situation much worse.

---

## Afterwards

Revert `updateStrategy` to `RollingUpdate` and drop `initialClusterState`. Keep
`nodeAffinityPreset`/`podAntiAffinityPreset`. Leaving `initialClusterState: "existing"` pinned will
break a genuine from-scratch install later.

Keep the pre-migration snapshot.
