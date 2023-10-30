from kubernetes import (
    client as kube_client,
    config as kube_config,
)


# Returns True if node has the following taint set
#     effect: NoSchedule
#     key: node-role.kubernetes.io/control-plane
def is_k8s_control_plane_node(node: kube_client.V1Node):
    if node.spec.taints is None:
        return False
    for taint in node.spec.taints:
        if "NoSchedule" in taint.effect and "node-role.kubernetes.io/control-plane" in taint.key:
            return True
    return False


# Returns True if the kubernetes Deployment's 'Available' type status condition is 'True'.
def deployment_is_available(name: str, namespace: str) -> bool:
    kube_config.load_kube_config()
    apps_v1 = kube_client.AppsV1Api()

    deployment = apps_v1.read_namespaced_deployment(
        name,
        namespace,
    )

    is_available = False
    for condition in deployment.status.conditions:
        if condition.type == "Available" and condition.status == "True":
            is_available = True

    return is_available


# Returns True if the kubernetes DaemonSet's desired replica count and available replica count match.
def daemon_set_is_available(name: str, namespace: str) -> bool:
    kube_config.load_kube_config()
    apps_v1 = kube_client.AppsV1Api()

    daemon_set = apps_v1.read_namespaced_daemon_set(
        name,
        namespace,
    )

    return daemon_set.status.desired_number_scheduled == daemon_set.status.number_available


# Returns True if the kubernetes StatefulSet's desired replica count and available replica count match.
def stateful_set_is_available(name: str, namespace: str) -> bool:
    kube_config.load_kube_config()
    apps_v1 = kube_client.AppsV1Api()

    stateful_set = apps_v1.read_namespaced_stateful_set(
        name,
        namespace,
    )

    return stateful_set.status.replicas == stateful_set.status.available_replicas
