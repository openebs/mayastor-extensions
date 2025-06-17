import logging

import common

logger = logging.getLogger(__name__)


def plugin_vnext():
    return common.plugin_path()


def upgrade_vnext():
    args = [
        "upgrade",
        "--allow-unstable",
        f"--registry={common.upgrade_registry()}",
        f"--repo-namespace={common.upgrade_namespace()}",
    ]
    run(args, log_run=True)


def run(args: list[str], log_run=False):
    return common.run(plugin_vnext(), args, log_run=log_run)
