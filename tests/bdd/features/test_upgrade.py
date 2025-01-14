"""Upgrade feature tests."""

import json
import logging
import os

import common
import pytest
from common import k8s_deployer
from common.environment import get_env
from common.helm import ChartSource, HelmReleaseClient, latest_chart_so_far
from common.kubectl_mayastor import upgrade_vnext
from common.repo import run_script
from kubernetes import client, config
from pytest_bdd import given, scenario, then, when
from retrying import retry

logger = logging.getLogger(__name__)

helm = HelmReleaseClient()


@scenario("upgrade.feature", "Upgrading to the local chart as v-next")
def test_upgrade_to_vnext():
    """Upgrading to the local chart as v-next."""


@given("a 2-worker node kind kubernetes cluster")
def _():
    """a 2-worker node kind kubernetes cluster."""
    k8s_deployer.start(workers=2)
    yield
    k8s_deployer.stop()


@given("the latest mayastor helm chart is installed")
def the_latest_mayastor_is_installed(latest_chart_version):
    """the latest mayastor helm chart is installed."""
    helm.install_mayastor(ChartSource.HOSTED, latest_chart_version)


@given("all io-engine nodes shall be listed by kubectl-mayastor")
def all_io_engine_nodes_shall_be_listed(latest_chart_version):
    """all io-engine nodes shall be listed by kubectl-mayastor."""
    wait_rest_nodes_version(latest_chart_version)


@given("a v-next chart is prepared")
def _():
    """a v-next chart is prepared."""
    if common.chart_vnext_skip():
        return
    # todo: fork once build system supports alternate chart path
    #  common.run("./scripts/python/upgrade-test-helper.sh", ["--fork", "--tag"])


@given("the images and plugin are built for v-next")
def _():
    """the images and plugin are built for v-next."""
    if common.chart_vnext_skip():
        return
    chart = os.path.join(common.root_dir(), "./chart")
    common.run("./scripts/python/upgrade-test-helper.sh", ["--build", "--chart-tag", "--chart", chart])


@given("the images are loadable from the cluster")
def _():
    """the images are loadable from the cluster."""
    if common.chart_vnext_skip():
        return
    common.run("./scripts/python/upgrade-test-helper.sh", ["--load"])


@when("a kubectl mayastor upgrade command is issued")
def a_kubectl_mayastor_upgrade_command_is_issued():
    """a kubectl mayastor upgrade command is issued."""
    upgrade_vnext()


@then(
    "eventually the installed chart should be upgraded to the kubectl mayastor plugin's version"
)
def eventually_the_installed_chart_should_be_upgraded_to_the_kubectl_mayastor_plugins_version(
        latest_chart_version,
):
    """the installed chart should be upgraded to the kubectl mayastor plugin's version."""

    upgrade_target_version = get_env("UPGRADE_TARGET_VERSION")
    if upgrade_target_version is None:
        upgrade_target_version = run_script("scripts/python/generate-test-tag.sh")
    upgrade_target_version = upgrade_target_version.lstrip("v")
    logger.info(f"Value of upgrade_target_version={upgrade_target_version}")

    def log_it():
        log = (pytest.attempts % 10) == 0
        pytest.attempts += 1
        return log

    @retry(
        stop_max_attempt_number=60,
        wait_fixed=2000,
    )
    def helm_upgrade_succeeded():
        log = log_it()
        if log:
            logger.info("Checking if helm upgrade succeeded...")
        metadata = helm.get_metadata_mayastor()
        if log:
            logger.info(f"helm get metadata output={metadata}")
        if metadata:
            assert metadata["version"] == upgrade_target_version
            return
        raise ValueError("helm get metadata returned a None")

    @retry(
        stop_max_attempt_number=600,
        wait_fixed=2000,
    )
    def data_plane_upgrade_succeeded(not_target_tag):
        log = log_it()
        if log:
            logger.info("Checking if data-plane upgrade succeeded...")
        config.load_kube_config()
        v1 = client.CoreV1Api()
        label_selector = "app=io-engine"
        pods = v1.list_namespaced_pod(
            namespace="mayastor", label_selector=label_selector
        )
        io_engines = list(
            filter(
                lambda pod: any(
                    container.name == "io-engine" for container in pod.spec.containers
                ),
                pods.items,
            )
        )
        if len(io_engines) == 0:
            return

        all_done = True
        for pod in io_engines:
            for i, container in enumerate(pod.spec.containers):
                if container.name == "io-engine":
                    # Not straightforward to know which version to expect here, so let's check that
                    # the version is not the latest instead?
                    if container.image.endswith(f":v{not_target_tag.strip('v')}"):
                        all_done = False
                    if log:
                        logger.info(
                            f"pod.metadata.name={pod.metadata.name}, pod.spec.containers[{i}].image={container.image}"
                        )
                    break
        assert all_done is True

        nodes = client.CoreV1Api().list_node(
            label_selector="openebs.io/engine=mayastor"
        )

        assert len(nodes.items) == len(io_engines)

    pytest.attempts = 0
    helm_upgrade_succeeded()
    pytest.attempts = 0

    data_plane_upgrade_succeeded(latest_chart_version)

    # Not straightforward to know which version to expect here, so let's check that
    # the version is not the latest instead?
    wait_rest_nodes_version(latest_chart_version, match=False)


@pytest.fixture(scope="module")
def latest_chart_version():
    yield latest_chart_so_far()


@retry(
    stop_max_attempt_number=60,
    wait_fixed=1000,
)
def wait_rest_nodes_version(version, match=True):
    config.load_kube_config()
    nodes = client.CoreV1Api().list_node(label_selector="openebs.io/engine=mayastor")
    k8s_nodes = len(nodes.items)

    rest_nodes = json.loads(
        common.kubectl_mayastor.run(["get", "nodes", "-o=json"], log_run=True)
    )
    rest_io_engines = len(rest_nodes)
    logger.info(f"Mayastor Nodes: {rest_nodes}")

    assert (
            k8s_nodes == rest_io_engines
    ), f"Found {k8s_nodes} k8s nodes with the io-engine label, but only {rest_io_engines} nodes from kubectl-mayastor"

    assert all(
        node["spec"]["version"] == node["state"]["version"] for node in rest_nodes
    )

    version_stripped = version.strip("v")
    if match:
        all_on_version = all(
            node["spec"]["version"].strip("v") == version_stripped
            for node in rest_nodes
        )
        assert all_on_version, f"Not all nodes on the version v{version_stripped}"
    else:
        all_not_on_version = all(
            node["spec"]["version"].strip("v") != version_stripped
            for node in rest_nodes
        )
        assert (
            all_not_on_version
        ), f"Some of the nodes are still on the version v{version_stripped}"
