"""Upgrade feature tests."""

from common import helm
from pytest_bdd import (
    given,
    scenario,
    then,
    when,
)


@scenario("upgrade.feature", "upgrade command is issued")
def test_upgrade_command_is_issued():
    """upgrade command is issued."""


@given("an installed mayastor helm chart")
def an_installed_mayastor_helm_chart():
    """an installed mayastor helm chart."""


    mayastor = "mayastor"
    helm_client = helm.HelmReleaseClient(mayastor, "")
    assert helm_client.release_is_deployed(mayastor)


@when("a kubectl mayastor upgrade command is issued to")
def a_kubectl_mayastor_upgrade_command_is_issued_to():
    """a kubectl mayastor upgrade command is issued to."""
    raise NotImplementedError


@then("the installed chart should be upgraded to the kubectl mayastor plugin's version")
def the_installed_chart_should_be_upgraded_to_the_kubectl_mayastor_plugins_version():
    """the installed chart should be upgraded to the kubectl mayastor plugin's version."""
    raise NotImplementedError
