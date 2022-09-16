use actix_web::{middleware, HttpServer};
use clap::{App, Arg, ArgMatches};

use operators::upgrade::{
    common::error::K8sResourceError, config::UpgradeOperatorConfig, rest::service,
};

/// Initialize upgrade operator config that are passed through arguments.
async fn initialize_operator(args: &ArgMatches) -> Result<(), K8sResourceError> {
    Ok(UpgradeOperatorConfig::initialize(args).await?)
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let args = App::new("Upgrade Operator")
        .settings(&[
            clap::AppSettings::ColoredHelp,
            clap::AppSettings::ColorAlways,
        ])
        .arg(
            Arg::with_name("mayastor-endpoint")
                .long("mayastor-endpoint")
                .short('e')
                .default_value("http://mayastor-api-rest:8081")
                .help("URL endpoint to the control plane's rest endpoint."),
        )
        .arg(
            Arg::with_name("namespace")
                .long("namespace")
                .short('n')
                .default_value("mayastor")
                .help("Namespace to be used for upgrade operator"),
        )
        .arg(
            Arg::with_name("chart-name")
                .long("chart-name")
                .short('c')
                .default_value("mayastor")
                .help("chart name used while installing the product"),
        )
        .get_matches();

    initialize_operator(&args)
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
        .unwrap()
        .run()
        .await
        .expect("Unable to run the server");
    Ok(())
}
