import logging
import os
import subprocess
from shutil import which

from common.environment import get_env

logger = logging.getLogger(__name__)


def get_bin_path():
    bins = get_env("TEST_DIR")
    if bins:
        return os.path.join(bins, "kubectl-plugin/bin/kubectl-mayastor")
    bin = which("kubectl-mayastor")
    if bin is None:
        msg = f"Failed to find kubectl-mayastor binary"
        logging.error(msg)
        raise Exception(msg)
    return bin


def kubectl_mayastor(args: list[str]):
    command = [get_bin_path()]
    command.extend(args)
    logger.info(f"Running kubectl-mayastor: {command}")

    try:
        result = subprocess.run(
            command,
            capture_output=True,
            check=True,
            text=True,
        )
        logger.debug(f"Error Output: {result.stderr}\nOut Output: {result.stdout}")
        return result.stdout.strip()

    except subprocess.CalledProcessError as e:
        logger.error(
            f"Error: command '{command}' failed with exit code {e.returncode}\nError Output: {e.stderr}\nOut Output: {e.stdout}")
        raise e

    except Exception as e:
        logger.error(f"An unexpected error occurred whilst running kubectl-mayastor: {e}")
        raise e
