"""Install Mayastor feature tests."""

from common.k8s import (
    deployment_is_available,
    daemon_set_is_available,
    is_k8s_control_plane_node,
    stateful_set_is_available,
)
from common.utils import retry_predicate
from kubernetes import (
    client as kube_client,
    config as kube_config,
)
import os
import pytest
from pytest_bdd import (
    given,
    scenario,
    then,
    when,
)
from subprocess import run


@scenario('install.feature', 'Mayastor helm chart install')
def test_mayastor_helm_chart_install():
    """Mayastor helm chart install."""


@given('a kubernetes namespace to install the Mayastor helm chart')
def a_kubernetes_namespace_to_install_the_mayastor_helm_chart():
    """a kubernetes namespace to install the Mayastor helm chart."""
    kube_config.load_kube_config()
    core_v1 = kube_client.CoreV1Api()

    # Creates a unique namespace
    ns = core_v1.create_namespace({
        "metadata": {
            "generateName": "mayastor-test-"
        }
    })

    pytest.helm_namespace = ns.metadata.name
    pytest.helm_release = "mayastor-test"
    # Retry for 1 min when checking for kubernetes available states.
    pytest.retry_seconds = 2
    pytest.max_retries = 30


@given('worker nodes on the kubernetes cluster are labelled with the label \'openebs.io/engine=mayastor\'')
def worker_nodes_on_the_kubernetes_cluster_are_labelled_with_the_label_openebsioenginemayastor():
    """worker nodes on the kubernetes cluster are labelled with the label 'openebs.io/engine=mayastor'."""
    kube_config.load_kube_config()
    core_v1 = kube_client.CoreV1Api()
    patch_body = {
        "metadata": {
            "labels": {
                "openebs.io/engine": "mayastor",
            }
        }
    }

    # Listing the cluster nodes
    node_list = core_v1.list_node()

    # Patching the node labels
    for node in node_list.items:
        # Skipping control-plane nodes
        if is_k8s_control_plane_node(node):
            continue
        core_v1.patch_node(node.metadata.name, patch_body)


@when('the Mayastor helm chart is installed with callhome disabled and at most 3 replicas of etcd')
def the_mayastor_helm_chart_is_installed_with_callhome_disabled_and_at_most_3_replicas_of_etcd():
    """the Mayastor helm chart is installed with callhome disabled and at most 3 replicas of etcd."""
    root_dir = os.environ["ROOT_DIR"]
    helm_chart_path = root_dir + "/chart"

    kube_config.load_kube_config()
    core_v1 = kube_client.CoreV1Api()
    # Listing the cluster nodes
    node_list = core_v1.list_node()
    # Checking to see if cluster can accommodate more than one, and less than or equal to 3 replicas of etcd.
    usable_etcd_nodes = 0
    for node in node_list.items:
        # Skipping control-plane nodes
        if is_k8s_control_plane_node(node):
            continue

        usable_etcd_nodes += 1

        if usable_etcd_nodes == 3:
            # If we're here, then clearly there's enough nodes to house a 3-replica etcd. We can break
            # and no further counting is required.
            break

    if usable_etcd_nodes == 0:
        raise ValueError("No kubernetes worker nodes in cluster")

    # Install helm release.
    result = run(["helm",
                  "install",
                  # Helm release name
                  pytest.helm_release,
                  # Path to the helm chart in this repository
                  helm_chart_path,
                  "-n",
                  # Kubernetes namespace
                  pytest.helm_namespace,
                  "--dependency-update",
                  "--set",
                  # Disable callhome and eventing.
                  "obs.callhome.enabled=false",
                  "--set",
                  # Number of etcd StatefulSet replicas
                  "etcd.replicaCount=" + str(usable_etcd_nodes),
                  # Wait no more than 15 minutes for all Pods, PVCs, Services to be ready.
                  "--wait",
                  "--timeout",
                  "15m"
                  ], check=True)


@then('one agent-core Kubernetes Deployment is Available')
def one_agentcore_kubernetes_deployment_is_available():
    """one agent-core Kubernetes Deployment is Available."""
    def predicate() -> bool:
        return deployment_is_available(pytest.helm_release + "-agent-core", pytest.helm_namespace)

    assert retry_predicate(predicate, pytest.max_retries, pytest.retry_seconds)


@then('one api-rest Kubernetes Deployment is Available')
def one_apirest_kubernetes_deployment_is_available():
    """one api-rest Kubernetes Deployment is Available."""
    def predicate() -> bool:
        return deployment_is_available(pytest.helm_release + "-api-rest", pytest.helm_namespace)

    assert retry_predicate(predicate, pytest.max_retries, pytest.retry_seconds)


@then('one csi-controller Kubernetes Deployment is Available')
def one_csicontroller_kubernetes_deployment_is_available():
    """one csi-controller Kubernetes Deployment is Available."""
    def predicate() -> bool:
        return deployment_is_available(pytest.helm_release + "-csi-controller", pytest.helm_namespace)

    assert retry_predicate(predicate, pytest.max_retries, pytest.retry_seconds)


@then('one csi-node Kubernetes DaemonSet for which all replicas are Available')
def one_csinode_kubernetes_daemonset_for_which_all_replicas_are_available():
    """one csi-node Kubernetes DaemonSet for which all replicas are Available."""
    def predicate() -> bool:
        return daemon_set_is_available(pytest.helm_release + "-csi-node", pytest.helm_namespace)

    assert retry_predicate(predicate, pytest.max_retries, pytest.retry_seconds)


@then('one etcd Kubernetes StatefulSet for which all replicas are Available')
def one_etcd_kubernetes_statefulset_for_which_all_replicas_are_available():
    """one etcd Kubernetes StatefulSet for which all replicas are Available."""
    def predicate() -> bool:
        return stateful_set_is_available(pytest.helm_release + "-etcd", pytest.helm_namespace)

    assert retry_predicate(predicate, pytest.max_retries, pytest.retry_seconds)


@then('one ha-node Kubernetes DaemonSet for which all replicas are Available')
def one_hanode_kubernetes_daemonset_for_which_all_replicas_are_available():
    """one ha-node Kubernetes DaemonSet for which all replicas are Available."""
    def predicate() -> bool:
        return daemon_set_is_available(pytest.helm_release + "-agent-ha-node", pytest.helm_namespace)

    assert retry_predicate(predicate, pytest.max_retries, pytest.retry_seconds)


@then('one io-engine Kubernetes DaemonSet for which all replicas are Available')
def one_ioengine_kubernetes_daemonset_for_which_all_replicas_are_available():
    """one io-engine Kubernetes DaemonSet for which all replicas are Available."""
    def predicate() -> bool:
        return daemon_set_is_available(pytest.helm_release + "-io-engine", pytest.helm_namespace)

    assert retry_predicate(predicate, pytest.max_retries, pytest.retry_seconds)


@then('one localpv-provisioner Kubernetes Deployment is Available')
def one_localpvprovisioner_kubernetes_deployment_is_available():
    """one localpv-provisioner Kubernetes Deployment is Available."""
    def predicate() -> bool:
        return deployment_is_available(pytest.helm_release + "-localpv-provisioner", pytest.helm_namespace)

    assert retry_predicate(predicate, pytest.max_retries, pytest.retry_seconds)


@then('one loki Kubernetes StatefulSet for which all replicas are Available')
def one_loki_kubernetes_statefulset_for_which_all_replicas_are_available():
    """one loki Kubernetes StatefulSet for which all replicas are Available."""
    def predicate() -> bool:
        return stateful_set_is_available(pytest.helm_release + "-loki", pytest.helm_namespace)

    assert retry_predicate(predicate, pytest.max_retries, pytest.retry_seconds)


@then('one nats Kubernetes StatefulSet for which all replicas are Available')
def one_nats_kubernetes_statefulset_for_which_all_replicas_are_available():
    """one nats Kubernetes StatefulSet for which all replicas are Available."""
    def predicate() -> bool:
        return stateful_set_is_available(pytest.helm_release + "-nats", pytest.helm_namespace)

    assert retry_predicate(predicate, pytest.max_retries, pytest.retry_seconds)


@then('one operator-diskpool Kubernetes Deployment is Available')
def one_operatordiskpool_kubernetes_deployment_is_available():
    """one operator-diskpool Kubernetes Deployment is Available."""
    def predicate() -> bool:
        return deployment_is_available(pytest.helm_release + "-operator-diskpool", pytest.helm_namespace)

    assert retry_predicate(predicate, pytest.max_retries, pytest.retry_seconds)


@then('one promtail Kubernetes DaemonSet for which all replicas are Available')
def one_promtail_kubernetes_daemonset_for_which_all_replicas_are_available():
    """one promtail Kubernetes DaemonSet for which all replicas are Available."""
    def predicate() -> bool:
        return daemon_set_is_available(pytest.helm_release + "-promtail", pytest.helm_namespace)

    assert retry_predicate(predicate, pytest.max_retries, pytest.retry_seconds)


@then('the helm chart release is in \'deployed\' state')
def the_helm_chart_release_is_in_deployed_state():
    """the helm chart release is in 'deployed' state."""
    result = run(["helm",
                  "list",
                  "-n",
                  # Kubernetes namespace
                  pytest.helm_namespace,
                  "--deployed",
                  "--short",
                  ], encoding="utf-8", capture_output=True, check=True)
    release_list = result.stdout.strip().split("\n")
    release_found = False
    for release in release_list:
        if release == pytest.helm_release:
            release_found = True
            break

    assert release_found
