Feature: Install Mayastor
  Install Mayastor on a kubernetes cluster, using helm

  Background:
    Given: A Kubernetes cluster

  Scenario: Mayastor helm chart install
    Given worker nodes on the kubernetes cluster are labelled with the label 'openebs.io/engine=mayastor'
    And a kubernetes namespace to install the Mayastor helm chart
    When the Mayastor helm chart is installed with callhome disabled and at most 3 replicas of etcd
    Then the helm chart release is in 'deployed' state
    And one api-rest Kubernetes Deployment is Available
    And one agent-core Kubernetes Deployment is Available
    And one csi-controller Kubernetes Deployment is Available
    And one operator-diskpool Kubernetes Deployment is Available
    And one localpv-provisioner Kubernetes Deployment is Available
    And one csi-node Kubernetes DaemonSet for which all replicas are Available
    And one ha-node Kubernetes DaemonSet for which all replicas are Available
    And one io-engine Kubernetes DaemonSet for which all replicas are Available
    And one promtail Kubernetes DaemonSet for which all replicas are Available
    And one etcd Kubernetes StatefulSet for which all replicas are Available
    And one loki Kubernetes StatefulSet for which all replicas are Available
    And one nats Kubernetes StatefulSet for which all replicas are Available
