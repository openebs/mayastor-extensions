{}:
let
  sources = import ../../nix/sources.nix;
  pkgs = import sources.nixpkgs {
    overlays = [ (_: _: { inherit sources; }) (import ../../nix/overlay.nix { }) ];
  };
in
with pkgs;
let
in
mkShell {
  name = "python-scripts-shell";
  buildInputs = [
    coreutils
    git
    kubectl
    kubernetes-helm
    python3
    semver-tool
    virtualenv
    yq-go
  ];
}
