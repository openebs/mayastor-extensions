import json
import os
import repo
import semver
import subprocess
from enum import Enum


def local_chart_dir():
    return os.path.join(repo.root_dir(), "chart")


def repo_ls():
    try:
        result = subprocess.run(
            ["helm", "repo", "ls", "-o", "json"],
            capture_output=True,
            check=True,
            text=True,
        )
        return json.loads(result.stdout.strip())

    except subprocess.CalledProcessError as e:
        print(
            f"Error: command 'helm repo ls -o json' failed with exit code {e.returncode}"
        )
        print(f"Error Output: {e.stderr}")
        return None

    except Exception as e:
        print(f"An unexpected error occurred: {e}")
        return None


def repo_add_mayastor():
    repos = repo_ls()
    if repos is not None:
        for repo in repos:
            if repo["url"] == "https://openebs.github.io/mayastor-extensions":
                return repo["name"]

    try:
        subprocess.run(
            [
                "helm",
                "repo",
                "add",
                "mayastor",
                "https://openebs.github.io/mayastor-extensions",
            ],
            capture_output=True,
            check=True,
            text=True,
        )
        return "mayastor"

    except subprocess.CalledProcessError as e:
        print(
            f"Error: command 'helm repo add mayastor https://openebs.github.io/mayastor-extensions' failed with exit code {e.returncode}"
        )
        print(f"Error Output: {e.stderr}")
        return None

    except Exception as e:
        print(f"An unexpected error occurred: {e}")
        return None


class ChartSource(Enum):
    HOSTED = "mayastor"
    LOCAL = local_chart_dir()


class HelmReleaseClient:
    """
    A client for interacting with Helm releases in a specified Kubernetes namespace.

    Attributes:
        namespace (str): The Kubernetes namespace where the Helm releases are managed.
        storage_driver (str): The Helm storage driver to use.
    """

    def __init__(self, namespace: str, storage_driver: str):
        """
        Initializes the HelmReleaseClient.

        Args:
            namespace (str): The Kubernetes namespace where the Helm releases are managed.
            storage_driver (str): The Helm storage driver to use.
        """
        self.namespace = namespace
        self.storage_driver = storage_driver

    def list(self):
        """
        Lists the deployed Helm releases in the specified namespace.

        Executes the 'helm ls' command to retrieve a list of deployed releases.

        Returns:
            str: A newline-separated string of deployed release names, or None if an error occurs.
        """
        try:
            result = subprocess.run(
                ["helm", "ls", "-n", self.namespace, "--deployed", "--short"],
                capture_output=True,
                check=True,
                text=True,
                env={"HELM_DRIVER": self.storage_driver},
            )
            return result.stdout.strip()

        except subprocess.CalledProcessError as e:
            print(
                f"Error: command 'helm ls -n {self.namespace} --deployed --short' failed with exit code {e.returncode}"
            )
            print(f"Error Output: {e.stderr}")
            return None

        except Exception as e:
            print(f"An unexpected error occurred: {e}")
            return None

    def release_is_deployed(self, release_name: str):
        releases = self.list()
        if releases is not None:
            for release in releases:
                if release == release_name:
                    return True
        return False

    def install_mayastor(self, source: ChartSource, version=None):
        install_command = []
        if source == ChartSource.HOSTED:
            repo_name = repo_add_mayastor()
            assert repo_name is not None
            install_command += [
                repo_name + "/" + source.value,
                "-n",
                self.namespace,
                "--create-namespace",
                "--set",
                "obs.callhome.sendReport=false,localpv-provisioner.analytics.enabled=false",
                "--wait",
            ]
            if version is not None:
                install_command += ["--version", version]

        if source == ChartSource.LOCAL:
            install_command = [
                "/bin/bash",
                "-c",
                os.path.join(repo.root_dir(), "scripts/helm/install.sh")
                + " --dep-update --wait",
            ]

        try:
            result = subprocess.run(
                install_command,
                capture_output=True,
                check=True,
                text=True,
                env={"HELM_DRIVER": self.storage_driver},
            )
            return result.stdout.strip()

        except subprocess.CalledProcessError as e:
            print(
                f"Error: command {install_command} failed with exit code {e.returncode}"
            )
            print(f"Error Output: {e.stderr}")
            return None

        except Exception as e:
            print(f"An unexpected error occurred: {e}")
            return None
