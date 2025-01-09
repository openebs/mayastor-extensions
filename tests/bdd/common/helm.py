import json
import logging
import os
import subprocess
from enum import Enum
from shutil import which

from common.environment import get_env
from common.repo import root_dir, run_script

logger = logging.getLogger(__name__)

helm_bin = which("helm")


def repo_ls():
    try:
        result = subprocess.run(
            [helm_bin, "repo", "ls", "-o", "json"],
            capture_output=True,
            check=True,
            text=True,
        )
        return json.loads(result.stdout.strip())

    except subprocess.CalledProcessError as e:
        logger.error(
            f"Error: command 'helm repo ls -o json' failed with exit code {e.returncode}"
        )
        logger.error(f"Error Output: {e.stderr}")
        return None

    except Exception as e:
        logger.error(f"An unexpected error occurred: {e}")
        return None


def repo_add_mayastor():
    repos = repo_ls()
    if repos is not None:
        for r in repos:
            if r["url"] == "https://openebs.github.io/mayastor-extensions":
                return r["name"]

    try:
        repo_name = "mayastor"
        subprocess.run(
            [
                helm_bin,
                "repo",
                "add",
                repo_name,
                "https://openebs.github.io/mayastor-extensions",
            ],
            capture_output=True,
            check=True,
            text=True,
        )

        subprocess.run(
            [
                helm_bin,
                "repo",
                "update",
            ],
            capture_output=True,
            check=True,
            text=True,
        )
        return repo_name

    except subprocess.CalledProcessError as e:
        logger.error(
            f"Error: command 'helm repo add mayastor https://openebs.github.io/mayastor-extensions' failed with exit code {e.returncode}"
        )
        logger.error(f"Error Output: {e.stderr}")
        return None

    except Exception as e:
        logger.error(f"An unexpected error occurred: {e}")
        return None


def latest_chart_so_far(version=None):
    if version is None:
        v = get_env("UPGRADE_TARGET_VERSION")
        if v is None:
            version = generate_test_tag()
        else:
            version = v

    repo_name = repo_add_mayastor()
    assert repo_name is not None

    helm_search_command = [
        helm_bin,
        "search",
        "repo",
        repo_name + "/mayastor",
        "--version",
        "<" + version,
        "-o",
        "json",
    ]
    try:
        result = subprocess.run(
            helm_search_command,
            capture_output=True,
            check=True,
            text=True,
        )
        result_chart_info = json.loads(result.stdout.strip())
        return result_chart_info[0]["version"]

    except subprocess.CalledProcessError as e:
        logger.error(
            f"Error: command {helm_search_command} failed with exit code {e.returncode}"
        )
        logger.error(f"Error Output: {e.stderr}")
        return None

    except Exception as e:
        logger.error(f"An unexpected error occurred: {e}")
        return None


class ChartSource(Enum):
    HOSTED = [
        "bash",
        "-c",
        os.path.join(root_dir(), "scripts/helm/install.sh") + " --hosted-chart --wait",
    ]
    LOCAL = [
        "bash",
        "-c",
        os.path.join(root_dir(), "scripts/helm/install.sh") + " --dep-update --wait",
    ]


class HelmReleaseClient:
    """
    A client for interacting with Helm releases in a specified Kubernetes namespace.

    Attributes:
        namespace (str): The Kubernetes namespace where the Helm releases are managed.
    """

    def __init__(self):
        """
        Initializes the HelmReleaseClient.
        """
        self.namespace = "mayastor"

    def get_metadata_mayastor(self):
        command = [
            helm_bin,
            "get",
            "metadata",
            "mayastor",
            "-n",
            self.namespace,
            "-o",
            "json",
        ]
        try:
            result = subprocess.run(
                command,
                capture_output=True,
                check=True,
                text=True,
            )
            return json.loads(result.stdout.strip())

        except subprocess.CalledProcessError as e:
            logger.error(
                f"Error: command '{command}' failed with exit code {e.returncode}"
            )
            logger.error(f"Error Output: {e.stderr}")
            raise e

        except Exception as e:
            logger.error(f"An unexpected error occurred: {e}")
            raise e

    def get_deployed(self, release: str):
        """
        Get the deployed Helm release in the specified namespace as json

        Executes the 'helm ls' command to retrieve a list of deployed releases.

        Returns:
            str: A newline-separated string of deployed release names, or None if an error occurs.
        """
        args = [
            helm_bin,
            "ls",
            "-n",
            self.namespace,
            "--deployed",
            f"--filter=^{release}$",
            "-o=json",
        ]
        try:
            result = subprocess.run(
                args,
                capture_output=True,
                check=True,
                text=True,
            )
            return result.stdout.strip()

        except subprocess.CalledProcessError as e:
            logger.error(
                f"command '{args}' failed with exit code {e.returncode}"
            )
            logger.error(f"Error Output: {e.stderr}")
            raise e

        except Exception as e:
            logger.error(f"An unexpected error occurred: {e}")
            raise e

    def install_mayastor(self, source: ChartSource, version=None):
        output_json = json.loads(self.get_deployed("mayastor"))
        if len(output_json) == 1:
            current_version = output_json[0]["app_version"]
            logger.warning(
                f"Helm release 'mayastor' already exists in the 'mayastor' namespace @ v{current_version}."
            )
            assert current_version == version, f"Wanted to install {version}, but {current_version} already installed"
            return

        install_command = []
        if source == ChartSource.HOSTED:
            install_command = source.value
            if version is not None:
                install_command[-1] = install_command[-1] + " --version " + version
            logger.info(
                f"Installing mayastor helm chart from hosted registry, version='{version}'"
            )

        if source == ChartSource.LOCAL:
            install_command = source.value
            logger.info("Installing mayastor helm chart from local directory")

        try:
            result = subprocess.run(
                install_command,
                capture_output=True,
                check=True,
                text=True,
            )
            logger.info("Installation succeeded")
            return result.stdout.strip()

        except subprocess.CalledProcessError as e:
            logger.error(
                f"Error: command {install_command} failed with exit code {e.returncode}"
            )
            logger.error(f"Error Output: {e.stderr}")
            raise e

        except Exception as e:
            logger.error(f"An unexpected error occurred: {e}")
            raise e


def generate_test_tag():
    return run_script("scripts/python/generate-test-tag.sh")
