import logging
import os

import common

logger = logging.getLogger(__name__)


def plugin_vnext():
    chart_vnext = common.chart_vnext()
    return os.path.join(chart_vnext, "kubectl-plugin/bin/kubectl-mayastor")


def upgrade_vnext():
    run(["upgrade"], log_run=True)


def run(args: list[str], log_run=False):
    return common.run(plugin_vnext(), args, log_run=log_run)
