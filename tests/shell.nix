with (import <nixpkgs> { });
mkShell {
  buildInputs = [
    cacert
    conntrack-tools
    cri-tools
    curl
    gawkInteractive
    gnused
    jq
    minikube
  ];
}
