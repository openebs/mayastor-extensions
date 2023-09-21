# Tests

## Set up Minikube
Platform prerequisites:
- Systemd-based linux-amd64 system
- Docker (does not require containerd)
- Nix

Usage:
```console
Usage: setup.sh [OPTIONS]

Options:
  -h, --help                      Display this text.
  --kubernetes-version <version>  Specify the version of kubernetes.
  -i, --install-prerequisites     Install cri-dockerd and containernetworking plugins before creating minikube cluster.
  -y, --assume-yes                Assume the answer 'Y' for all interactive questions.
  -k, --skip-kube-context-switch  Skip switching kubectl cluster to the created minikube cluster's context.

Examples:
  setup.sh --kubernetes-version v1.25.11
```

Run the following command to set up a Minikube single-node kubernetes cluster:
```console
nix-channel --update
sudo -E env "PATH=$PATH" nix-shell
./minikube/setup.sh [OPTIONS]
```

Verify kubernetes control-plane pod statuses
```console
kubectl get pods -n kube-system
```
Sample output:
```console
$ kubectl get pods -n kube-system

NAME                                       READY   STATUS    RESTARTS   AGE
calico-kube-controllers-85578c44bf-g2tfx   1/1     Running   0          59s
calico-node-sn6kk                          1/1     Running   0          59s
coredns-5d78c9869d-q6r6q                   1/1     Running   0          59s
etcd-niladri-vm                            1/1     Running   0          71s
kube-apiserver-niladri-vm                  1/1     Running   0          71s
kube-controller-manager-niladri-vm         1/1     Running   0          71s
kube-proxy-dqb2v                           1/1     Running   0          59s
kube-scheduler-niladri-vm                  1/1     Running   0          71s
```

## Tear down Minikube

Tear down an existing minikube cluster using the following command while in the nix-shell:
```console
minikube stop
# Exit the nix-shell using the 'exit' command.
# exit
```
Alternatively, use `minikube delete` to delete the minikube profile and all files related to your minikube cluster.