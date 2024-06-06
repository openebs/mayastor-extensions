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
    kubernetes-helm-wrapped
    minikube
    which
    yq-go
  ];

  MINIKUBE_DIR = builtins.toString ./minikube;

  shellHook = ''
    export PATH=$MINIKUBE_DIR/bin:$PATH

    mkdir -p $MINIKUBE_DIR/bin
    ln -sf $MINIKUBE_DIR/setup.sh $MINIKUBE_DIR/bin/minikube-setup
    ln -sf $MINIKUBE_DIR/cleanup.sh $MINIKUBE_DIR/bin/minikube-cleanup
  '';
}

