import logging
import os

logger = logging.getLogger(__name__)


def get_env(variable: str, warn=True):
    value = os.getenv(variable)
    if value is None:
        if warn:
            logger.warning(f"The env {variable} does not exist")
        return None

    if len(value) == 0:
        if warn:
            logger.warning(f"The env {variable} is an empty string")
        return None

    logger.info(f"Found env {variable}={value}")
    return value
