use actix_web::{middleware, HttpServer};
use operators::upgrade::{
    common::{constants::UPGRADE_OPERATOR_INTERNAL_PORT, error::Error},
    config::{CliArgs, UpgradeConfig},
    controller::reconciler::start_upgrade_worker,
    rest::service,
};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

/// Initialize upgrade operator config that are passed through arguments.
async fn initialize_upgrade_config(args: CliArgs) -> Result<(), Error> {
    info!("Initializing Upgrade operator...");
    UpgradeConfig::initialize(args).await
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize logging.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = CliArgs::args();

    initialize_upgrade_config(args).await.map_err(|error| {
        error!(?error, "Failed to initialize Upgrade Operator");
        error
    })?;

    match UpgradeConfig::get_config()
        .k8s_client()
        .create_upgrade_action_crd()
        .await
    {
        Ok(()) => info!("UpgradeAction CRD created"),
        Err(error) => {
            error!(?error, "Failed to create UpgradeAction CRD");
            std::process::exit(1);
        }
    };
    info!("Starting Upgrade controller...");
    tokio::task::spawn(async move { start_upgrade_worker().await });

    // Start Upgrade API.
    info!(
        "Starting to listen on port {}...",
        UPGRADE_OPERATOR_INTERNAL_PORT
    );
    HttpServer::new(move || {
        actix_web::App::new()
            .wrap(middleware::Logger::default())
            .service(service::apply_upgrade)
            .service(service::get_upgrade)
    })
    .bind(("0.0.0.0", UPGRADE_OPERATOR_INTERNAL_PORT))
    .map_err(|error| {
        error!(
            ?error,
            "Failed to bind API to socket address 0.0.0.0:{}", UPGRADE_OPERATOR_INTERNAL_PORT
        );
        Error::from(error)
    })?
    .workers(2_usize)
    .run()
    .await
    .map_err(|error| {
        error!(?error, "Failed to start Upgrade API server");
        Error::from(error)
    })
}
