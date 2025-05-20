use crate::collect::k8s_resources::client::K8sResourceError;
use pstor::Error as StoreError;

use std::io::Error;

pub mod etcd;

/// EtcdError holds the errors that can occur while trying to dump information
/// from etcd database
#[derive(Debug)]
#[allow(unused)]
pub enum EtcdError {
    Etcd(StoreError),
    K8sResource(K8sResourceError),
    IOError(std::io::Error),
    Custom(String),
    CreateClient(kube_proxy::Error),
}

impl From<StoreError> for EtcdError {
    fn from(e: StoreError) -> Self {
        EtcdError::Etcd(e)
    }
}

impl From<std::io::Error> for EtcdError {
    fn from(e: Error) -> Self {
        EtcdError::IOError(e)
    }
}

impl From<K8sResourceError> for EtcdError {
    fn from(e: K8sResourceError) -> Self {
        EtcdError::K8sResource(e)
    }
}
