pub mod client;
pub mod common;
pub mod k8s_resource_dump;

/// The DiskPool definition.
/// TODO: rename utils crate to something less collision prone.
#[allow(dead_code)]
#[allow(clippy::empty_line_after_outer_attr)]
pub(crate) mod k8s_operators {
    include!(
        "../../../../../dependencies/control-plane/k8s/operators/src/pool/diskpool/crd/v1beta3.rs"
    );
}

/// The DiskPool quantity.
/// TODO: rename utils crate to something less collision prone.
#[allow(dead_code)]
#[allow(clippy::empty_line_after_outer_attr)]
pub(crate) mod quantity {
    include!(
        "../../../../../dependencies/control-plane/k8s/operators/src/pool/diskpool/crd/quantity.rs"
    );
}
