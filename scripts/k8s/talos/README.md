# Local Talos + QEMU + KubeVirt dev cluster

This directory has everything needed to stand up a local, disposable Kubernetes
cluster on [Talos Linux](https://www.talos.dev/) running under QEMU, install
[Mayastor](https://github.com/openebs/mayastor) for storage, and run
[KubeVirt](https://kubevirt.io/) VMs backed by Mayastor volumes on top of it.
It's meant for local development/testing, not production.

- `controlplane.patch.yaml` / `worker.patch.yaml` — Talos machine config
  patches applied at cluster creation time. Named `*.patch.yaml` (not
  `controlplane.yaml`/`worker.yaml`) because `talosctl cluster create`
  overwrites whatever file path you give `--config-patch-*` with the full
  generated machine config (secrets included) — see below.
- `kubevirt.yaml` — patches to right-size the KubeVirt control plane for a
  small dev cluster.
- `storage-pool.yaml` / `storage-class.yaml` — Mayastor `DiskPool` template
  and the RWX `StorageClass` used by the VM's disk.
- `cdi.yaml` — a `DataVolume` that imports an Ubuntu cloud image onto a
  Mayastor RWX block volume.
- `vm.yaml` — a `VirtualMachine` that boots from that volume.
- `0-setup-ovmf.sh` … `5-create-vm.sh` — one script per step below, plus
  `up.sh` to run them all and `down.sh` to tear the cluster down. Each is
  safe to re-run on its own (`kubectl apply`, not `create`).

## Quick start

```bash
./up.sh          # runs every step below in order
# ...
./down.sh        # talosctl cluster destroy
```

Or run the steps individually — each script below is documented with what it
does and which env vars it reads (all have sane defaults matching the
commands this doc used to spell out by hand).

## Part 1 — Create the Talos/QEMU cluster

### Prerequisites

- Linux host with KVM (`/dev/kvm`) and QEMU installed.
- [`talosctl`](https://www.talos.dev/latest/talos-guides/install/talosctl/) and `kubectl`.
- OVMF (UEFI firmware for QEMU). `talosctl cluster create ... qemu` boots the
  VMs via UEFI and needs `OVMF_CODE.fd`/`OVMF_VARS.fd` available on the host.

### OVMF firmware setup (NixOS / Nix)

On most distros, installing an `ovmf`/`edk2-ovmf` package drops the firmware
under `/usr/share/OVMF/` (or `/usr/share/edk2/`) automatically. On Nix/NixOS
the firmware lives in the Nix store instead, so it needs symlinking into the
path `talosctl` expects. Run:

```bash
./0-setup-ovmf.sh
```

It finds the `OVMF-*-fd` package under `/nix/store` and links
`OVMF_CODE.fd`/`OVMF_VARS.fd` into `/usr/share/OVMF/`. It's a no-op (with a
message) if you're not on Nix — install your distro's `ovmf`/`edk2-ovmf`
package instead. Safe to re-run any time (e.g. after upgrading the `ovmf`
package, when the store path/hash changes).

### Create the cluster

```bash
./1-create-cluster.sh
```

Equivalent to:

```bash
sudo -E talosctl cluster create \
  --workers 2 \
  --cpus-workers 4 \
  --memory-workers 8GiB \
  --memory-controlplanes 4GiB \
  --config-patch-controlplanes .generated/controlplane.yaml \
  --config-patch-workers .generated/worker.yaml \
  qemu
```

(override `CLUSTER_NAME`, `WORKERS`, `CPUS_WORKERS`, `MEMORY_WORKERS`,
`MEMORY_CONTROLPLANES` env vars to change the defaults — e.g. run multiple
clusters side by side with `CLUSTER_NAME=talos-2 ./1-create-cluster.sh`).
`sudo -E` is needed because
`talosctl cluster create qemu` manages QEMU/network devices as root, while
`-E` preserves your environment (so it still picks up your normal
`$HOME`/Nix env, kubeconfig merge target, etc).

**Why `.generated/` and not the patch files directly:** `talosctl cluster
create` overwrites whatever path you pass to `--config-patch-controlplanes`/
`--config-patch-workers` with the full generated machine config for that
node type — cluster secrets, PKI keys, join tokens and all. The script first
copies `controlplane.patch.yaml`/`worker.patch.yaml` into the git-ignored
`.generated/` directory and points talosctl at those copies, so the tracked
patch files are never touched (and no secrets ever land in git). Don't call
`talosctl cluster create` with the `*.patch.yaml` files directly.

What the two config patches do:

- **`controlplane.patch.yaml`** — adds a `PodSecurity` admission exemption
  for the `mayastor` namespace, since Mayastor's `io-engine` pods need
  privileges (hugepages, host devices) that the default `baseline` Pod
  Security level would otherwise block.
- **`worker.patch.yaml`** — configures each worker for running Mayastor's
  `io-engine`:
  - `sysctls.vm.nr_hugepages: "1024"` reserves hugepages the `io-engine`
    needs for its SPDK-based NVMe-oF target.
  - `nodeLabels."openebs.io/engine": mayastor` labels the node so Mayastor's
    `io-engine` DaemonSet schedules onto it.
  - `kubelet.extraMounts` bind-mounts `/var/local` with `rshared` propagation
    so device links created on the host are visible inside the kubelet
    container (and hence to CSI node plugins).
  - The `files` entry drops a containerd CRI config snippet enabling
    `device_ownership_from_security_context`, so a pod's `securityContext`
    (fsGroup/runAsUser) is respected when it's granted access to a host block
    device — needed for Mayastor's NVMe-oF device nodes.

This creates a cluster named `talos-default` (or `$CLUSTER_NAME`, if set)
with a control plane reachable at `https://10.5.0.1:6443` (Talos's default
QEMU CIDR), and merges/updates your local `~/.kube/config` and talosconfig
automatically. `./1-create-cluster.sh --delete` (or `./down.sh --talos`)
reads the same `CLUSTER_NAME` to destroy the right one.

Useful checks once it's up:

```bash
kubectl get nodes -o wide
talosctl -n 10.5.0.2 dmesg   # control-plane node logs, if something looks off
```

To tear it down: `./down.sh` (or `talosctl cluster destroy`).

### Accessing the cluster remotely (e.g. from a Mac, with Headlamp)

If you're running the QEMU cluster on a remote/other Linux box (`lobox.local`
below) and want to point a local tool like
[Headlamp](https://headlamp.dev/) at it from your Mac:

```bash
# Copy the kubeconfig generated by talosctl cluster create
scp lobox.local:~/.kube/config ~/.kube/config

# Talos's QEMU clusters default to the 10.5.0.0/24 range, with the
# control-plane endpoint at 10.5.0.1 — that address only exists on the
# Linux host, so alias it locally too, matching the kubeconfig's server IP.
sudo ip addr add 10.5.0.1/32 dev lo

# Forward the local alias:port to the same address:port on the Linux host,
# tunneled over SSH so the apiserver traffic reaches it.
ssh -L 10.5.0.1:6443:10.5.0.1:6443 lobox.local
```

Leave the `ssh -L` session running while you use Headlamp/`kubectl` locally;
it's what makes `https://10.5.0.1:6443` resolve from your Mac.

## Part 2 — KubeVirt + CDI + Mayastor VMs

With the Talos cluster up, install KubeVirt (VMs on Kubernetes), CDI (imports
disk images into PVCs), and Mayastor (the storage backing the VM disks), then
boot a VM.

### 1. Install KubeVirt

```bash
./2-install-kubevirt.sh
```

Installs the KubeVirt operator + CR pinned to the current `stable.txt`
release, applies `kubevirt.yaml` to lower memory requests (and drop limits)
on `virt-api`/`virt-controller`/`virt-handler` — the upstream defaults are
sized for real clusters, not a local dev box — then waits for the KubeVirt
CR to report `Deployed`.

### 2. Install CDI (Containerized Data Importer)

```bash
./3-install-cdi.sh
```

Installs the latest CDI operator + CR and waits for it to report `Deployed`.

### 3. Install Mayastor and set up storage

```bash
./4-install-mayastor.sh
```

Runs `scripts/helm/install.sh` from the repo root with dev-friendly flags —
`--no-loki` skips the logging stack (unnecessary for local dev),
`allowNonPersistentDevlink=true` relaxes device-link checks that don't hold
up under QEMU's virtio disks, and `io_engine.cpuCount=1` keeps the io-engine's
CPU affinity footprint small enough for a laptop/dev box.

It then discovers every worker node and creates a `DiskPool` on each one's
spare disk (`DISK` env var, default `/dev/vdb` — the second QEMU-attached
disk, distinct from the `/dev/vda` install disk) by templating
[`storage-pool.yaml`](storage-pool.yaml) with `envsubst`, and creates the
RWX/NVMe-oF storage class from [`storage-class.yaml`](storage-class.yaml).

`rwxBlock: 'true'` in the storage class is what lets a `volumeMode: Block`
PVC be mounted `ReadWriteMany` — required below so a VM's disk can survive
live migration between nodes.

### 4. Import a disk image and boot a VM

```bash
./5-create-vm.sh
```

Applies [`cdi.yaml`](cdi.yaml) (a `DataVolume` that imports the Ubuntu cloud
image straight onto a Mayastor RWX block volume) and waits for it to reach
phase `Succeeded`, then applies [`vm.yaml`](vm.yaml) (a `VirtualMachine`
using that `DataVolume`'s PVC as its disk) and waits for it to become ready:

```bash
kubectl get vm
# NAME     AGE   STATUS    READY
# vm-rwx   21s   Running   True
```

### 5. Day-2 VM operations

```bash
# Serial console
kubectl virt console vm-rwx

# Live-migrate the VM (works because the disk is RWX and can be attached on both nodes)
kubectl virt migrate vm-rwx
# VM vm-rwx was scheduled to migrate

kubectl get VirtualMachineInstanceMigration
# NAME                        PHASE     VMI
# kubevirt-migrate-vm-tkrw9   Running   vm-rwx
```

If a migration fails with something like:

```
libvirtError: Timed out during operation: cannot acquire state change lock
(held by monitor=remoteDispatchDomainMigratePrepare3Params)
```

that's libvirt lock contention on the destination node — on a small dev box
the destination virt-handler/libvirtd can be slow enough setting up the
migration that the default KubeVirt migration timeouts trip before it
finishes. `kubevirt.yaml` sets a longer `progressTimeout`/
`completionTimeoutPerGiB` (`spec.configuration.migrations`) to give it more
room; bump those further if it still happens.

```bash
# SSH in (the cloud-init in vm.yaml sets user "ubuntu", password auth enabled)
kubectl virt ssh ubuntu@vmi/vm-rwx
```

`kubectl virt` comes from the [KubeVirt `virtctl`
plugin](https://kubevirt.io/user-guide/user_workflows/virtctl_client_tool/);
install it (or `krew install virt`) if `kubectl virt ...` isn't recognized.
