import logging
import os
import subprocess

logger = logging.getLogger(__name__)


def root_dir():
    file_path = os.path.abspath(__file__)
    return file_path.split("tests/bdd")[0]


def run_script(script: str):
    command = os.path.join(root_dir(), script)
    logger.info(f"Running script '{command}'")
    try:
        result = subprocess.run(
            command,
            capture_output=True,
            check=True,
            shell=True,
            text=True,
        )
        logger.info(f"Script succeeded")
        return result.stdout.strip()

    except subprocess.CalledProcessError as e:
        logger.error(f"Error: command {command} failed with exit code {e.returncode}")
        logger.error(f"Error Output: {e.stderr}")
        raise e

    except Exception as e:
        logger.error(f"An unexpected error occurred: {e}")
        raise e
