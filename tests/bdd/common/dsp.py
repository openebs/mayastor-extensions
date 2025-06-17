import logging

import common
from kubernetes import client

logger = logging.getLogger(__name__)

group = "openebs.io"
version = "v1beta3"
namespace = common.namespace()
plural = "diskpools"


def create(dsp, node, disk, secret: str = None):
    cr = {
        "apiVersion": f"{group}/{version}",
        "kind": "DiskPool",
        "metadata": {"name": dsp, "namespace": namespace},
        "spec": {"node": node, "disks": [disk]},
    }
    if secret:
        cr["spec"]["encryptionConfig"] = {"source": {"secret": {"name": secret}}}

    logger.info(f"Creating DSP: {cr}")
    custom_api = client.CustomObjectsApi()
    return custom_api.create_namespaced_custom_object(
        group=group, version=version, namespace=namespace, plural=plural, body=cr
    )


def get(dsp):
    logger.debug(f"Getting DSP: {dsp}")
    custom_api = client.CustomObjectsApi()
    return custom_api.get_namespaced_custom_object(
        group=group, version=version, namespace=namespace, plural=plural, name=dsp
    )


def delete(dsp):
    logger.info(f"Deleting DSP: {dsp}")
    custom_api = client.CustomObjectsApi()
    custom_api.delete_namespaced_custom_object(
        group=group, version=version, namespace=namespace, plural=plural, name=dsp
    )
