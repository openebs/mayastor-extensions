import logging
import os

import common
from common import run

logger = logging.getLogger(__name__)


def deployer():
    return "./scripts/k8s/deployer.sh"


def start(workers: int):
    if carry_on():
        try:
            common.run(
                "helm",
                [
                    "uninstall",
                    "mayastor",
                    "-n=mayastor",
                    "--ignore-not-found",
                    "--wait",
                ],
                absolute=True,
            )
            common.run(
                "kubectl", ["delete", "jobs", "-n=mayastor", "--all"], absolute=True
            )
            return
        except:
            pass

    run(deployer(), ["start", "--label", "--cleanup", f"--workers={workers}"])


def stop():
    if common.env_cleanup():
        run(deployer(), ["stop"])


def carry_on():
    clean = os.getenv("REUSE_CLUSTER")
    if clean is not None and clean.lower() in ("yes", "true", "y", "1"):
        cluster = common.run("kind", ["get", "clusters"], absolute=True, log_run=False)
        return cluster == "kind"
    return False
