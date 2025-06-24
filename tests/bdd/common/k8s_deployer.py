import logging
import os

import common
from common import run

logger = logging.getLogger(__name__)


def deployer():
    return "./scripts/k8s/deployer.sh"


def start(args: list[str] = None):
    if carry_on():
        try:
            common.run(
                "kubectl",
                ["delete", "jobs", "-n=mayastor", "--all", "--cascade=foreground"],
                absolute=True,
            )
            return
        except:
            pass

    run(deployer(), ["start", "--cleanup"] + args)


def stop():
    if common.env_cleanup():
        run(deployer(), ["stop"])


def carry_on():
    clean = os.getenv("REUSE_CLUSTER")
    if clean is not None and clean.lower() in ("yes", "true", "y", "1"):
        cluster = common.run("kind", ["get", "clusters"], absolute=True, log_run=False)
        return cluster == "kind"
    return False
