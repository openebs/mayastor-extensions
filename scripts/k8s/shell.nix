let
  sources = import ../../nix/sources.nix;
  pkgs = import sources.nixpkgs {
    overlays = [ (_: _: { inherit sources; }) (import ../../nix/overlay.nix { }) ];
  };
  inPureNixShell = builtins.getEnv "IN_NIX_SHELL" == "pure";
in
pkgs.mkShell {
  name = "k8s-cluster-shell";
  buildInputs = with pkgs; [
    kubernetes-helm-wrapped
    kubectl
    kind
    jq
  ] ++ pkgs.lib.optional (inPureNixShell) [
    docker
    util-linux
    sudo
  ];
}
