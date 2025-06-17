"""Basic feature tests."""

import json
import logging

from common import dsp, k8s_deployer, kubectl_mayastor
from common.helm import ChartSource, HelmReleaseClient
from kubernetes import client, config
from kubernetes.client.rest import ApiException
from pytest_bdd import given, parsers, scenario, then, when
from retrying import retry

logger = logging.getLogger(__name__)
helm = HelmReleaseClient()


def gen_pvc_name(replicas):
    return f"pvc-r{replicas}"


@scenario("basic.feature", "Creating a DiskPool on all nodes")
def test_creating_a_diskpool_on_all_nodes():
    """Creating a DiskPool on all nodes."""


@scenario("basic.feature", "Creating a PVC")
def test_creating_a_pvc():
    """Creating a PVC."""


@given("a 2-worker node kind kubernetes cluster")
def _():
    """a 2-worker node kind kubernetes cluster."""
    k8s_deployer.start(["--workers", "2", "--label", "--disk", "1GiB"])
    config.load_kube_config()
    yield
    k8s_deployer.stop()


@given(
    "all io-engine nodes shall be listed by kubectl-mayastor", target_fixture="nodes"
)
def _():
    """all io-engine nodes shall be listed by kubectl-mayastor."""
    nodes = json.loads(kubectl_mayastor.run(["get", "nodes", "-o=json"], log_run=True))
    logger.info(f"Mayastor Nodes: {nodes}")

    assert 2 == len(
        nodes
    ), f"Expected 2 but found only {len(nodes)} nodes from kubectl-mayastor"

    assert all(node["state"]["status"] == "Online" for node in nodes)
    yield nodes


@given("the mayastor helm chart is installed")
def _():
    """the mayastor helm chart is installed."""
    helm.install_mayastor(ChartSource.LOCAL, args="--no-loki")


@given("a DiskPool CR is created on all nodes")
@when("a DiskPool CR is created on all nodes")
def _(nodes):
    """a DiskPool CR is created on all nodes."""

    for node in nodes:
        name = node["id"]
        try:
            disk = "/var/local/mayastor/io-engine/disk.io"
            dsp.create(dsp=name, disk=disk, node=name)
        except ApiException as e:
            if e.status != 409:
                raise e
    yield
    for node in nodes:
        try:
            dsp.delete(name)
        except ApiException as e:
            if e.status != 404:
                raise e


@when(
    parsers.parse("a PVC with {repl} replicas is created with Immediate"),
    target_fixture="pvc",
)
def _(repl):
    """a PVC with <repl> replicas is created with Immediate."""
    pvc_name = gen_pvc_name(repl)
    logger.info(f"Creating PVC: {pvc_name}")

    sc_name = f"mayastor-r{repl}"
    sc = client.V1StorageClass(
        metadata=client.V1ObjectMeta(name=sc_name),
        provisioner="io.openebs.csi-mayastor",
        volume_binding_mode="Immediate",
        parameters={"repl": repl},
    )
    stor_v1 = client.StorageV1Api()
    try:
        stor_v1.create_storage_class(body=sc)
    except ApiException as e:
        if e.status != 409:
            raise e

    pvc = client.V1PersistentVolumeClaim(
        metadata=client.V1ObjectMeta(name=pvc_name),
        spec=client.V1PersistentVolumeClaimSpec(
            access_modes=["ReadWriteOnce"],
            resources=client.V1ResourceRequirements(requests={"storage": "100Mi"}),
            volume_mode="Block",
            storage_class_name=sc_name,
        ),
    )
    core_v1 = client.CoreV1Api()
    pvc = core_v1.create_namespaced_persistent_volume_claim(
        body=pvc, namespace="default"
    )
    yield pvc
    logger.info(f"Deleting PVC: {pvc_name}")
    stor_v1.delete_storage_class(name=sc_name)
    core_v1.delete_namespaced_persistent_volume_claim(
        name=pvc_name, namespace="default"
    )
    wait_pvc_deleted(pvc_name)


@then("eventually it will become bound")
def _(pvc):
    """eventually it will become bound."""
    wait_pvc_bound(pvc.metadata.name)


@then("eventually the diskpool CRs shall be created and Online")
def _(nodes):
    """eventually the diskpool CRs shall be created and Online."""
    for node in nodes:
        wait_dsp_online(node["id"])


@then("the diskpools shall be listed by kubectl-mayastor as Online")
def _(nodes):
    """the diskpools shall be listed by kubectl-mayastor as Online."""
    for node in nodes:
        pool = json.loads(
            kubectl_mayastor.run(["get", "pool", node["id"], "-o=json"], log_run=True)
        )
        logger.info(f"Mayastor Pool: {pool}")
        assert pool["state"]["status"] == "Online"


@retry(
    stop_max_attempt_number=200,
    wait_fixed=100,
)
def wait_pvc_bound(name):
    pvc = client.CoreV1Api().read_namespaced_persistent_volume_claim(
        name=name, namespace="default"
    )
    assert pvc.status.phase == "Bound", f"PVC {pvc} not bound yet"


@retry(
    stop_max_attempt_number=200,
    wait_fixed=100,
)
def wait_pvc_deleted(name):
    try:
        client.CoreV1Api().read_namespaced_persistent_volume_claim(
            name=name, namespace="default"
        )
        raise Exception(f"PVC {name} not deleted yet")
    except ApiException as e:
        if e.status != 404:
            raise e


@retry(
    stop_max_attempt_number=200,
    wait_fixed=100,
)
def wait_dsp_online(name):
    cr = dsp.get(name)
    assert cr["status"]["pool_status"] == "Online"
