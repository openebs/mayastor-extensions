{}:
let
  sources = import ../nix/sources.nix;
  pkgs = import sources.nixpkgs {
    overlays = [ (_: _: { inherit sources; }) (import ../nix/overlay.nix { }) ];
  };
in
with pkgs;
let
in
mkShell {
  name = "helm-scripts-shell";
  buildInputs = [
    coreutils
    git
    helm-docs
    kubernetes-helm-wrapped
    semver-tool
    yq-go
  ];
}
