#!/usr/bin/env bash
# Symlinks OVMF (UEFI firmware) into /usr/share/OVMF for talosctl's qemu
# provisioner. Only needed on NixOS, where the firmware lives in the Nix
# store instead of a distro path. No-op if not running NixOS.
# Pass --delete to remove the symlinks instead.
set -euo pipefail

DELETE=false
for arg in "$@"; do
  case "$arg" in
    --delete) DELETE=true ;;
    *) echo "Usage: $0 [--delete]" >&2; exit 1 ;;
  esac
done

if ! grep -qi '^ID=nixos$' /etc/os-release 2>/dev/null; then
  echo "Not running NixOS — skipping. If talosctl fails to find OVMF, install your distro's ovmf/edk2-ovmf package."
  exit 0
fi

if $DELETE; then
  echo "Removing OVMF symlinks from /usr/share/OVMF/"
  if [ -L /usr/share/OVMF/OVMF_CODE.fd ]; then sudo -E rm -f /usr/share/OVMF/OVMF_CODE.fd; fi
  if [ -L /usr/share/OVMF/OVMF_VARS.fd ]; then sudo -E rm -f /usr/share/OVMF/OVMF_VARS.fd; fi
  exit 0
fi

OVMF_DIR=$(find /nix/store -maxdepth 1 -iname '*-OVMF-*-fd' | sort -V | tail -1)
if [ -z "$OVMF_DIR" ]; then
  echo "Could not find an OVMF package under /nix/store." >&2
  echo "Install one, e.g.: nix profile install nixpkgs#OVMF" >&2
  exit 1
fi

echo "Using OVMF from: $OVMF_DIR"
sudo -E mkdir -p /usr/share/OVMF
sudo -E ln -sf "$OVMF_DIR/FV/OVMF_CODE.fd" /usr/share/OVMF/OVMF_CODE.fd
sudo -E ln -sf "$OVMF_DIR/FV/OVMF_VARS.fd" /usr/share/OVMF/OVMF_VARS.fd
echo "OVMF firmware linked into /usr/share/OVMF/"
