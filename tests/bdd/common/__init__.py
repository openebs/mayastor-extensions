import logging
import os
import subprocess

logger = logging.getLogger(__name__)


def root_dir():
    file_path = os.path.abspath(__file__)
    return file_path.split("tests/bdd")[0]


def chart_vnext():
    vnext = os.getenv("CHART_VNEXT")
    if vnext is not None:
        return vnext
    # return os.path.join(root_dir(), "./tests/bdd/chart-vnext")
    return os.path.join(root_dir(), "./chart")


def chart_vnext_skip():
    skip = os.getenv("CHART_VNEXT_SKIP")
    if skip is not None and skip.lower() in ("yes", "true", "1", "0"):
        return True
    return False


def run(
    command: str,
    args: list[str] = None,
    absolute=False,
    capture_output=True,
    log_run=True,
    **kwargs,
):
    if absolute:
        command = [command]
    else:
        command = [os.path.join(root_dir(), command)]
    if args is not None:
        command.extend(args)
    if log_run:
        logger.info(f"Running '{command}'")
    else:
        logger.debug(f"Running '{command}'")
    try:
        result = subprocess.run(
            command, capture_output=capture_output, check=True, text=True, **kwargs
        )
        logger.debug(
            f"Command '{command}' completed with:\nStdErr Output: {result.stderr}\nStdOut Output: {result.stdout}"
        )
        return result.stdout.strip()

    except subprocess.CalledProcessError as e:
        logger.error(
            f"Command '{command}' failed with exit code {e.returncode}\nStdErr Output: {e.stderr}\nStdOut Output: {e.stdout}"
        )
        raise e

    except Exception as e:
        logger.error(f"An unexpected error occurred: {e}")
        raise e


def env_cleanup():
    clean = os.getenv("CLEAN")
    if clean is not None and clean.lower() in ("no", "false", "f", "0"):
        return False
    return True
