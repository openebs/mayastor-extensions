# BDD Tests

## Set up Minikube
Platform prerequisites:
- Systemd-based linux-amd64 system
- Docker
- Nix

**Note:**
The minikube-setup script makes changes to systemd unit file when installing prerequisites. This doesn't work with systems where the /etc/systemd/system directory is read-only. If you have such a system you may install the prerequisites yourself, and run `minikube-setup` with the `-p` option set.

Usage:
```console
Usage: minikube-setup [OPTIONS]

Options:
  -h, --help                      Display this text.
  --kubernetes-version <version>  Specify the version of kubernetes.
  -p, --skip-prerequisites        Skip installing cri-dockerd and containernetworking plugins before creating
                                  minikube cluster.
  -y, --assume-yes                Assume the answer 'Y' for all interactive questions.
  -k, --skip-kube-context-switch  Skip switching kubectl cluster to the created minikube cluster's context.

Examples:
  minikube-setup --kubernetes-version v1.25.11
```

Run the following command to set up a Minikube single-node kubernetes cluster:
```console
nix-channel --update
sudo -E env "PATH=$PATH" nix-shell
minikube-setup
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
etcd-host-00001                            1/1     Running   0          71s
kube-apiserver-host-00001                  1/1     Running   0          71s
kube-controller-manager-host-00001         1/1     Running   0          71s
kube-proxy-dqb2v                           1/1     Running   0          59s
kube-scheduler-host-00001                  1/1     Running   0          71s
```

## Tear down Minikube

Tear down an existing minikube cluster using the following script, while in the nix-shell:
```console
sudo -E env "PATH=$PATH" nix-shell --run minikube-cleanup
```