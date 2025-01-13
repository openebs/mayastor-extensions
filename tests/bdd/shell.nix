{ pkgs ? import (import ../../nix/sources.nix).nixpkgs {
    overlays = [ (_: _: { inherit (import ../nix/../sources.nix); }) (import ../../nix/overlay.nix { }) ];
  }
}:
let
  k8sShellAttrs = import ../../scripts/k8s/shell.nix { inherit pkgs; };
  helmShellAttrs = import ../../chart/shell.nix { inherit pkgs; };
  bddBuildInputs = with pkgs; [
    autoflake
    black
    isort
    python3
    utillinux
    virtualenv
    which
  ];
in
pkgs.mkShell {
  name = "pytest-shell";
  buildInputs = k8sShellAttrs.buildInputs ++ helmShellAttrs.buildInputs ++
    bddBuildInputs;
}
