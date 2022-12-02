/// Grpc client module.
pub mod grpc_client;
/// Pool module.
pub mod pool;

#[derive(
    Debug, strum_macros::EnumString, strum_macros::AsRefStr, Clone, Ord, PartialOrd, Eq, PartialEq,
)]
#[strum(serialize_all = "lowercase")]
pub enum ApiVersion {
    V0,
    V1,
}
