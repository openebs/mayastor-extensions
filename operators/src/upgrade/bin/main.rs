use actix_web::{middleware, HttpServer};
use operators::upgrade::{
    common::{constants::UPGRADE_OPERATOR_INTERNAL_PORT, error::Error},
    config::{CliArgs, UpgradeOperatorConfig},
    rest::service,
};
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

/// Initialize upgrade operator config that are passed through arguments.
async fn initialize_operator(args: CliArgs) -> Result<(), Error> {
    info!("Initializing Upgrade operator...");
    UpgradeOperatorConfig::initialize(args).await
}

#[actix_web::main]
async fn main() -> Result<(), Error> {
    // Initialize logging.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = CliArgs::args();

    initialize_operator(args).await.map_err(|error| {
        error!(?error, "Failed to initialize Upgrade Operator");
        error
    })?;

    let app = move || {
        actix_web::App::new()
            .wrap(middleware::Logger::default())
            .service(service::apply_upgrade)
            .service(service::get_upgrade)
    };

    let app_server = HttpServer::new(app)
        .bind(("0.0.0.0", UPGRADE_OPERATOR_INTERNAL_PORT))
        .map_err(|error| {
            error!(
                ?error,
                "Failed to bind API to socket address 0.0.0.0:{}", UPGRADE_OPERATOR_INTERNAL_PORT
            );
            Error::from(error)
        })?;

    // Start Upgrade API.
    info!(
        "Starting to listen on port {}",
        UPGRADE_OPERATOR_INTERNAL_PORT
    );
    app_server.workers(2_usize).run().await.map_err(|error| {
        error!(?error, "Failed to start Upgrade API server");
        Error::from(error)
    })
}
