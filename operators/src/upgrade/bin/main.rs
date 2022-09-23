use actix_web::{middleware, HttpServer};
use operators::upgrade::{
    common::error::Error,
    config::{CliArgs, UpgradeOperatorConfig},
    rest::service,
};

/// Initialize upgrade operator config that are passed through arguments.
async fn initialize_operator(args: CliArgs) -> Result<(), Error> {
    UpgradeOperatorConfig::initialize(args).await
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = CliArgs::args();

    initialize_operator(args)
        .await
        .expect("Error while initializing config for upgrade operator");

    let app = move || {
        actix_web::App::new()
            .wrap(middleware::Logger::default())
            .service(service::apply_upgrade)
            .service(service::get_upgrade)
    };

    HttpServer::new(app)
        .bind(("127.0.0.1", 8080))
        .expect("Unable to bind address")
        .run()
        .await
        .expect("Unable to run the server");
    Ok(())
}
