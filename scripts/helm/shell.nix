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
  name = "helm-scripts-shell";
  buildInputs = [
    git
    semver-tool
    yq-go
  ];
}
