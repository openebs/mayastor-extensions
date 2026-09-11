# migrate-mayastor-etcd.sh

:warning: This tool performs a **destructive** operation on the etcd cluster that holds the Mayastor
control plane's entire persistent state — the record of every volume, replica, nexus and pool. It
destroys and rebuilds one member at a time on purpose. Read this file before running it.

## Overview

Moves the members of the Mayastor etcd StatefulSet onto a specific set of nodes, identified by a node
label (`node/etcd.io=true` by default), one member at a time, with health and consistency gates
between every step.

### Why this is not just an affinity edit

Changing `nodeAffinity` on the pod template is accepted by the API server but moves nothing:

- **Pods only move when they are recreated.** Affinity lives in the pod template; existing pods keep
  running until something deletes them.
- **The volumes are pinned to their nodes.** etcd storage comes from `mayastor-etcd-localpv`, an
  OpenEBS localpv **hostpath** StorageClass. Each PV is a directory on one node's filesystem and
  carries `nodeAffinity` pinning it there. A recreated pod with the same PVC either goes back to the
  original node, or — once the new affinity excludes that node — sits `Pending` forever with a
  *volume node affinity conflict*.

So the only way a member moves is: **remove it from etcd membership, destroy its PVC, delete the pod,
and let it come back empty on a new node and re-sync from the leader.** The entire design of the
script is about making each repetition safe and verifiable.

## Prerequisites

Apply these with helm **before** running the script. Under the `openebs` umbrella chart everything
below nests under `mayastor:`.

```yaml
etcd:
  nodeAffinityPreset:
    type: hard
    key: "node/etcd.io"
    values: ["true"]
  podAntiAffinityPreset: "hard"     # keep; do NOT set etcd.affinity
  updateStrategy:
    type: OnDelete
  initialClusterState: "existing"
```

**`updateStrategy: OnDelete` is the safety model.** Under `RollingUpdate` the StatefulSet controller
reacts to the template change on its own schedule: it deletes the highest ordinal, the replacement
cannot schedule because its PVC is still pinned to the old node, the rollout stalls, and you are
sitting at N-1 members with no plan. Under `OnDelete` the template change is inert until *you* delete
a pod.

Expect `.status.currentRevision != .status.updateRevision` for the duration. That is correct under
`OnDelete`, not a fault.

**`initialClusterState: "existing"` is the other half.** The bitnami entrypoint branches on
`is_new_etcd_cluster`, which is true when `ETCD_INITIAL_CLUSTER_STATE=new` *and* the pod's own peer
URL appears in `ETCD_INITIAL_CLUSTER` — i.e. true for every member of a fresh install. A member whose
data directory we just destroyed would take that branch and **bootstrap a brand-new cluster over the
keyspace**. Pinning the value to `existing` forces the `add_self_to_cluster` branch instead. Note the
chart only emits the variable when `replicaCount > 1`.

Remove `initialClusterState` after the migration. On a `helm upgrade` the chart already defaults it to
`existing`, so dropping it changes nothing then — but leaving it pinned breaks a genuine from-scratch
install later.

### Do not cordon the old nodes

`mayastor-etcd-localpv` has `reclaimPolicy: Delete`. When a PVC is deleted the localpv provisioner
runs a cleanup pod **on the node hosting the directory**. A cordoned node cannot run it, leaving the
PV `Released` and the etcd data directory orphaned on disk. The node affinity already does everything
cordoning would.

## Usage

```bash
# inspect only (default)
NS=openebs STS=openebs-etcd ./migrate-mayastor-etcd.sh

# move every member that is not yet on a labelled node, highest ordinal first
NS=openebs STS=openebs-etcd DRY_RUN=false ./migrate-mayastor-etcd.sh

# one member at a time, which is the recommended way
NS=openebs STS=openebs-etcd DRY_RUN=false ONLY_ORDINAL=2 ./migrate-mayastor-etcd.sh
```

| Variable | Default | Purpose |
|---|---|---|
| `NS` | `mayastor` | namespace |
| `STS` | `mayastor-etcd` | StatefulSet name |
| `CN` | `etcd` | container name |
| `LABEL_KEY` | `node/etcd.io` | target node label key |
| `LABEL_VALUE` | `true` | target node label value |
| `DRY_RUN` | `true` | **safe default**; set `false` to execute |
| `SNAPSHOT` | `true` | take and verify a pre-migration snapshot |
| `SNAPSHOT_DIR` | `./etcd-snapshots` | where it lands |
| `ONLY_ORDINAL` | *(unset)* | restrict the run to a single ordinal |
| `TIMEOUT_SEC` | `900` | bound on every wait loop |
| `SLEEP_SEC` | `5` | poll interval |
| `SETTLE_SEC` | `30` | quiet period after each move |
| `MAX_RESCHEDULE_KICKS` | `6` | retries when a pod lands on an unlabelled node |
| `ICS_OVERRIDE` | `false` | proceed without `initialClusterState=existing` |

Exit codes: `1` preflight/usage, `5` timeout, `6` member remove failed, `24` health gate failed,
`25` consistency violation, `30` repeatedly scheduled onto an unlabelled node.

## What the gates check

The health gate is deliberately stricter than a quorum check. A *deliberate* migration should never
begin from a degraded cluster, whereas `replace-one-laggard.sh` exists precisely to remediate one.
Every gate requires all expected members to be:

- present and answering,
- in the **same etcd cluster** (`cluster_id` identical, and identical to the one recorded at the
  start of the run),
- agreeing on **one** leader — every member must name the same one, not merely "a leader that is one
  of ours", which a split brain would satisfy,
- on one raft term,
- converged on one revision, and never a revision *lower* than the starting one,
- reporting an identical `endpoint hashkv`.

The last one matters: equal revisions only say the members agree on how far they have got. Equal
hashes say they are holding the same bytes. `cluster_id` changing, or the revision going backwards,
are the signatures of a member having bootstrapped over the keyspace instead of rejoining it — the
one failure this whole procedure exists to avoid — so both are hard stops (exit 25).

## Notes for anyone modifying this

- **Do not add `--cluster` to the etcdctl calls.** It discards `--endpoints` and rebuilds the list
  from the member list, which has two consequences: the `exclude` argument becomes silently
  ineffective, and every member's *second* advertised client URL — the load-balanced
  `<release>-etcd.<ns>.svc` ClusterIP — joins the query and answers as whichever pod it happens to
  route to. Explicit endpoints give exactly one row per pod and make exclusion real.
- **`etcdctl endpoint status` exits non-zero if any endpoint fails but still prints results for the
  ones that answered.** Do not throw that away with `|| return 1`; validate the JSON instead.
- **Two number bases coexist in the log output, on purpose.** `member_id_for()` returns a
  **hexadecimal** ID read straight from `member list` as text and feeds it to `member remove`, which
  expects hex. The gates compare **decimal** IDs from `endpoint status -w json`. Routing the ID you
  pass to `member remove` through JSON would expose it to the precision hazard below.
- **`quote_bigints` is not decoration.** etcd member and cluster IDs are uint64 and routinely exceed
  2^53; jq before 1.7 parses all numbers as IEEE doubles and silently rounds them. The observed IDs
  on a real cluster included `16092638608179729187`, well past the limit.
- **`set -e` interactions.** The script avoids `[[ ... ]] && cmd` and `((...)) && cmd` because a
  false condition there returns non-zero and can terminate the script. Preserve the explicit
  `if ...; then ...; fi` style.
- **Both scripts are deliberately self-contained.** Do not replace the local `say`/`warn`/`die` with
  `scripts/utils/log.sh`. These run against a live cluster, often from a jump host, and an operator
  should be able to copy a single file and run it; sourcing anything from the repo breaks that. The
  sibling `replace-lagging-etcd-member` script is self-contained for the same reason.
- **A larger labelled pool than `replicaCount` is normal.** The cordon check counts *schedulable*
  labelled nodes and only refuses when fewer than `replicaCount` remain; do not make it fail on any
  cordoned node, or labelling a whole zone becomes unusable.
- **Gate 2/3 is skipped under `DRY_RUN`, deliberately.** In a dry run the `member remove` is only
  printed, so the post-removal membership the gate exists to check never exists. Evaluating it anyway
  asks the survivors to agree on a leader drawn only from themselves while the member being moved is
  still a voter — which fails outright whenever that member happens to be the **current leader**, and
  passes misleadingly when it does not. Neither answer means anything.

## Replica-count caveats

Fault tolerance *during* the move is the quorum of the **remaining** members. At `replicaCount <= 2`
there is none, and the script warns about it in preflight.

On a 2-member cluster the migration causes a brief, unavoidable **write outage**. The bitnami
entrypoint rejoins via `etcdctl member add` as a full voting member, not a learner, so membership
goes 1 voter → 2 voters and quorum becomes 2 while the new member is still starting.

Both replica counts were measured by sampling `endpoint status` every 2s through the move, on a
3-worker and a 7-worker cluster, with the same result each time:

| | `replicaCount: 2` | `replicaCount: 3` |
|---|---|---|
| membership across the move | 2 → 1 → 2 | 3 → 2 → 3 |
| quorum while the member rejoins | 2 of 2 — **not met** | 2 of 3 — met |
| samples with no leader / unreachable | **2** (~11s) | **0** |
| raft term | advanced (re-election) | unchanged |
| leader | changed | unchanged |

The mechanism is the window between `member add` and the new member becoming live. At 3 replicas that
intermediate state is 3 voters with 2 alive, which still has quorum; at 2 replicas it is 2 voters with
1 alive, which does not.

Mayastor tolerates the outage: the data path does not consult etcd per-IO, so published volumes keep
serving — the writer workload recorded no gaps in either run. But volume create/delete/publish and
pool operations fail while it lasts, and an extended quorum loss eventually trips the io-engine's
`pstorRetries` bound (300 by default), after which a volume target self-shutdowns.

Note also that on a 2-member cluster *any* etcd pod restart loses write quorum until it returns —
including the rolling restart caused by reverting `updateStrategy` to `RollingUpdate` afterwards.
That is inherent to `replicaCount: 2`, not to this procedure. At 3 replicas that same revert rolled
two pods with zero unavailable samples.

---

# test-migration.sh

End-to-end test for the above against a real cluster. It installs `openebs/openebs`, puts genuine
Mayastor state into etcd (DiskPools, a 3-replica volume, a workload continuously writing to it),
migrates etcd onto labelled nodes, and then proves nothing was lost.

```bash
./test-migration.sh all          # setup -> prepare -> migrate -> verify -> revert
./test-migration.sh setup        # install + seed state
./test-migration.sh prepare      # apply the helm prerequisites, assert no pod churn
./test-migration.sh migrate      # dry run, then the real migration
./test-migration.sh verify       # assertions only, safe to repeat
./test-migration.sh revert       # back to RollingUpdate, then re-verify
./test-migration.sh teardown     # remove everything it created
```

Exit code is 0 only if every assertion passed.

### Cluster requirements

- ≥ 3 worker nodes labelled `openebs.io/engine=mayastor`
- hugepages configured and nvme kernel modules loaded on those workers
- one spare unformatted block device per worker (`BLOCK_DEVICE`, default `/dev/sdb`)
- `kubectl`, `helm`, `jq`, `yq` on the machine running the test

### Knobs

`CHART_VERSION` (4.2.0), `NS`/`REL` (openebs), `ETCD_REPLICAS` (2), `LABEL_KEY`/`LABEL_VALUE`,
`BLOCK_DEVICE`, `TEST_NS`/`TEST_SC`/`TEST_PVC`/`TEST_APP`, `REUSE_INSTALL`, `WORKDIR`,
`MIGRATE_SCRIPT`, `PATCH_DEAD_IMAGES`, `ETCD_TOLERATE_CONTROL_PLANE`, `EXTRA_LABELLED`.

`EXTRA_LABELLED=N` labels N nodes beyond the minimum, so the run exercises a larger labelled pool
with the scheduler free to pick a destination — closer to how this gets used in practice than the
exact fit the test otherwise sets up. The source node is never among them, so ordinal 0 still has to
move.

`setup` picks which nodes to label automatically: it labels the node of every member **except ordinal
0**, plus one node hosting no member at all. That is exactly `ETCD_REPLICAS` labelled nodes — which is
what `podAntiAffinityPreset: hard` requires — and leaves exactly one member to move. Leaving ordinal 0
as the one that moves is deliberate: it is the member whose bootstrap semantics could destroy the
keyspace if `initialClusterState` were wrong, so it is the most informative one to rebuild.

### The node pool has to be bigger than the replica count

There must be somewhere to move a member *to*. With hard anti-affinity a member cannot double up on a
node that already has one, so the pool of nodes etcd may occupy must be **strictly larger** than
`ETCD_REPLICAS`. On a cluster with three workers this is automatic at `ETCD_REPLICAS=2` and impossible
at `ETCD_REPLICAS=3`.

`ETCD_TOLERATE_CONTROL_PLANE=true` brings the control-plane node into the pool and gives etcd a
matching `NoSchedule` toleration, which is enough to make the 3-replica case testable on a 3-worker
cluster:

```bash
ETCD_REPLICAS=3 ETCD_TOLERATE_CONTROL_PLANE=true ./test-migration.sh all
```

Leave it off wherever there are genuinely spare workers. DiskPools are unaffected either way — they
only ever live on the mayastor workers.

### What it asserts

| # | Assertion |
|---|---|
| 1 | `cluster_id` unchanged from the baseline and identical across members |
| 2 | every member reachable |
| 3 | one leader, one raft term, one revision |
| 4 | revision moved forward, never backwards |
| 5 | all members report the same `endpoint hashkv` |
| 6 | every key present at baseline is still present |
| 7 | every member on a labelled node |
| 8 | members on distinct nodes (anti-affinity honoured) |
| 9 | no `Released`/`Failed` etcd PVs |
| 10 | the writer pod never restarted and its data has no sequence gaps |
| 11 | the control plane can still provision a **new** volume (etcd is usable, not merely intact) |
| 12 | all `openebs` pods ready, all DiskPools Online |

`prepare` additionally asserts that the helm upgrade causes **zero** etcd pod churn, which is the
observable proof that `OnDelete` took effect.

### Dead upstream images

`openebs/openebs` 4.2.0 references two images Docker Hub no longer serves, because Bitnami retired
their legacy tags in August 2025:

| Chart reference | Status | Replacement used by the test |
|---|---|---|
| `docker.io/bitnami/etcd:3.5.6-debian-11-r10` | 404 | `docker.io/openebs/etcd:3.5.6-debian-11-r10` |
| `docker.io/bitnami/kubectl:1.25.15` (chart `pre-upgrade` hook Job) | 404 | `docker.io/bitnamilegacy/kubectl:1.25.15` |

Without the second one, **every `helm upgrade` hangs** on the pre-upgrade hook — which is fatal here,
since applying and reverting the migration prerequisites are both upgrades. `test-migration.sh`
applies both overrides by default; set `PATCH_DEAD_IMAGES=false` once the chart itself is fixed.

### `--dry-run` will always show a `checksum/token-secret` diff on the first upgrade

The etcd subchart annotates the pod template with a checksum of its JWT token Secret, and the helper
that renders that Secret uses `lookup` to preserve an existing key. Plain `helm upgrade --dry-run`
cannot perform lookups, so it generates a fresh RSA key and reports a spurious checksum change.

On the first real upgrade after install the checksum *does* change once — the install-time render had
no Secret to look up — but the Secret's key material is preserved (verified by comparing its sha256
before and after), and the value is stable across every subsequent upgrade. Under `OnDelete` the
change is inert. Do not stop on it.

### Teardown ordering

Two orderings in `teardown` are load-bearing and were both found the hard way:

1. **Every Mayastor volume must be gone — PV reclaimed, not merely PVC deleted — before the
   DiskPools are deleted.** A DiskPool carries an `openebs.io/diskpool-protection` finalizer and sits
   in `Terminating` indefinitely while any replica still lives on it.
2. **The StatefulSets must be deleted, then their retained PVCs, and only then `helm uninstall`.**
   This is a three-way squeeze: a PVC cannot be deleted while a pod still mounts it
   (`pvc-protection`); the etcd and loki pods only go away when their StatefulSets do; but
   `helm uninstall` — the obvious way to remove the StatefulSets — *also* deletes the localpv
   provisioner, and without it nothing reclaims the hostpath PVs or removes the directories. Deleting
   the StatefulSets by hand first breaks the cycle. Get this wrong and you are left with `Released`
   PVs and tens of megabytes of orphaned etcd data on every node.
