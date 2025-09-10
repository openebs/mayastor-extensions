# replace-one-laggard.sh

## Overview

This script provides automated, safe single-node remediation for a Bitnami-based etcd cluster running as a Kubernetes StatefulSet. It detects and replaces exactly one lagging member that has fallen behind others in terms of raft applied index (revision), ensuring the cluster maintains quorum and health throughout the operation.

## What It Does

The script performs the following operations:

1. **Detects** exactly one member that is lagging behind others by comparing raft applied indices
2. **Validates** the entire cluster is on the same StatefulSet ControllerRevision (ensuring consistent templates/flags)
3. **Performs double health gates**:
   - Preflight health/quorum check with deduplication by unique member_id
   - Immediate stabilization gate before removing membership
4. **Removes** the lagging member from etcd membership using explicit endpoints
5. **Re-verifies** the remaining cluster is healthy (leader present, equal raft terms, equal revisions)
6. **Deletes** the laggard's PVC and Pod to force a clean rejoin
7. **Waits** until the cluster returns to equal revisions across all members

## Safety Features

- **Version Gate**: Refuses to run on chart/app versions >= 11.0.0
- **Multiple Member Protection**: Will not proceed if multiple members appear behind
- **Quorum Protection**: Verifies majority/leader twice (preflight and just-in-time)
- **Explicit Endpoints**: Membership changes use known-stable endpoints, avoiding unreliable nodes
- **Data Safety**: No data is deleted until the remaining cluster proves stable/healthy

## Prerequisites

- `kubectl` configured with access to the target Kubernetes cluster
- Permissions to:
  - Execute commands in etcd pods
  - Delete pods and PVCs
  - Modify StatefulSet environment variables
  - Read StatefulSet and Pod metadata
- Bitnami etcd chart version < 11.0.0

## Configuration Parameters

The script uses environment variables for configuration:

| Variable | Default | Description |
|----------|---------|-------------|
| `NS` | `mayastor` | Namespace containing the etcd StatefulSet |
| `STS` | `mayastor-etcd` | StatefulSet name (pods will be named `${STS}-0`, `${STS}-1`, etc.) |
| `CN` | `etcd` | Container name inside the pods that runs etcd |
| `REPLICAS` | `3` | Expected number of etcd cluster members |
| `TIMEOUT_SEC` | `900` | Generic timeout for long waits (in seconds) |
| `SLEEP_SEC` | `5` | Poll interval for waits/retries (in seconds) |
| `REMOVE_TRIES` | `6` | Number of retries per sender for `etcdctl member remove` |

## Usage

### Basic Usage

```bash
# Run with defaults
./replace-one-laggard.sh

# Run with custom namespace and StatefulSet name
NS=my-namespace STS=my-etcd ./replace-one-laggard.sh

# Run with custom timeout
TIMEOUT_SEC=1200 ./replace-one-laggard.sh
```

### Example Scenarios

#### Scenario 1: Fix a lagging member in default configuration
```bash
./replace-one-laggard.sh
```

#### Scenario 2: Custom namespace and StatefulSet
```bash
NS=production STS=critical-etcd REPLICAS=5 ./replace-one-laggard.sh
```

#### Scenario 3: Quick timeout for testing
```bash
TIMEOUT_SEC=300 SLEEP_SEC=2 ./replace-one-laggard.sh
```

## Exit Codes

The script uses specific exit codes for different failure scenarios:

| Code | Description |
|------|-------------|
| `0` | Success - cluster is in sync or successfully remediated |
| `2` | Could not collect endpoint status from any pod |
| `3` | Merged cluster view didn't yield exactly REPLICAS distinct pods |
| `4` | More than one laggard detected (unsafe to auto-repair) |
| `5` | Timed out while waiting for final resync (equal revisions) |
| `6` | Member remove failed from all control pods |
| `20` | Unable to determine chart/app version labels |
| `21` | Version gate failed (requires < 11.0.0) |
| `22` | Could not determine current ControllerRevision |
| `23` | Timed out waiting for ControllerRevision convergence across pods |
| `24` | Cluster failed to become healthy (post-remove pre-delete gate) |
| `25` | Liveness majority not met (unique healthy < quorum) |
| `26` | Unable to fetch endpoint status JSON |
| `27` | Structural quorum not met (no leader or unique visible < quorum) |

## How Quorum & Health Are Judged

The script performs two complementary health checks:

### Structural Health (endpoint status)
- Calls `etcdctl endpoint status --cluster -w json`
- Verifies:
  - A leader ID is present
  - Count of unique member_ids visible >= quorum
  - Deduplicates by member_id to avoid counting the same member multiple times

### Liveness Health (endpoint health)
- Calls `etcdctl endpoint health --cluster`
- Verifies:
  - Number of unique healthy member_ids >= quorum
  - Uses member_id mapping to prevent double-counting

Both checks must pass for the script to proceed with remediation.

## Important Notes

### What This Script Will NOT Do

- Will not proceed if multiple members appear behind
- Will not proceed on major chart/app version >= 11
- Will not proceed if quorum/health gates fail at any step
- Will not touch data until the remaining cluster is verified as stable

### When to Use This Script

Use this script when:
- You have exactly one etcd member that has fallen behind in revisions
- The cluster is otherwise healthy with a clear majority
- You need an automated, repeatable remediation procedure
- You're running Bitnami etcd chart version < 11.0.0

### When NOT to Use This Script

Do not use this script when:
- Multiple members are out of sync
- The cluster has lost quorum
- You're running etcd chart version >= 11.0.0
- The cluster is experiencing network partitions
- You need to perform manual troubleshooting

## Troubleshooting

### Common Issues

1. **Exit code 21**: Chart version is >= 11.0.0
   - Solution: This script is designed for older versions. Manual intervention required.

2. **Exit code 4**: Multiple laggards detected
   - Solution: Manual investigation needed - this indicates a more serious issue.

3. **Exit code 25 or 27**: Quorum/health check failed
   - Solution: Ensure cluster has majority of healthy members before running.

4. **Exit code 5**: Timeout during final resync
   - Solution: Increase `TIMEOUT_SEC` or investigate why the new member isn't syncing.

### Debug Information

The script provides timestamped log output showing:
- Current revision for each member
- Health check results
- Member removal attempts
- Pod recreation progress
- Final synchronization status

