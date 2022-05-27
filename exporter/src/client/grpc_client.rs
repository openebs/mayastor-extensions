#![allow(dead_code)]
use std::{str::FromStr, time::Duration};

use actix_web::http;
use tokio::time::sleep;
use tonic::transport::Channel;

use crate::error::ExporterError;

/// Timeout for gRPC
#[derive(Debug, Clone)]
pub struct Timeouts {
    connect: std::time::Duration,
    request: std::time::Duration,
}

impl Timeouts {
    /// return a new `Self` with the connect and request timeouts
    pub fn new(connect: std::time::Duration, request: std::time::Duration) -> Self {
        Self { connect, request }
    }
    /// timeout to establish connection to the node
    pub fn connect(&self) -> std::time::Duration {
        self.connect
    }
    /// timeout for the request itself
    pub fn request(&self) -> std::time::Duration {
        self.request
    }
}

/// Context for Grpc client
#[derive(Debug, Clone)]
pub struct GrpcContext {
    endpoint: tonic::transport::Endpoint,
    timeouts: Timeouts,
}

impl GrpcContext {
    /// initialize context
    pub fn new(endpoint: &str, timeouts: Timeouts) -> Result<Self, ExporterError> {
        let uri = format!("http://{}", endpoint);
        let uri = match http::uri::Uri::from_str(&uri) {
            Ok(uri) => uri,
            Err(err) => {
                println!("Error while parsing uri:{}", err);
                return Err(ExporterError::InvalidURI("Invalid uri:{}".to_string()));
            }
        };
        let endpoint = tonic::transport::Endpoint::from(uri)
            .connect_timeout(timeouts.connect() + Duration::from_millis(500))
            .timeout(timeouts.request);
        Ok(Self { endpoint, timeouts })
    }
}

/// Wrapper for all grpc client types present in mayastor
#[derive(Debug, Clone)]
pub struct Clients {
    mayastor_client: MayastorClient,
}

impl Clients {
    /// get mutable mayastor client
    pub fn mayastor_client_mut(&mut self) -> &mut MayastorClient {
        &mut self.mayastor_client
    }
}

/// Grpc client
#[derive(Debug, Clone)]
pub struct GrpcClient {
    ctx: GrpcContext,
    clients: Clients,
}

type MayastorClient = rpc::mayastor::mayastor_client::MayastorClient<Channel>;

impl GrpcClient {
    /// initialize gRPC client
    pub async fn new(ctx: GrpcContext) -> Result<Self, ExporterError> {
        let mut ct;
        // To establish grpc client connection for mayastor.
        loop {
            ct = match tokio::time::timeout(
                ctx.clone().timeouts.connect(),
                MayastorClient::connect(ctx.endpoint.clone()),
            )
            .await
            {
                Err(_) => Err(ExporterError::GrpcConnectTimeout {
                    endpoint: ctx.clone().endpoint.uri().to_string(),
                    timeout: ctx.clone().timeouts.connect(),
                }),
                Ok(client) => Ok(client),
            }?;
            if ct.is_err() {
                println!("Unable to connect to mayastor grpc server");
                sleep(Duration::from_secs(10)).await;
            } else {
                break;
            }
        }
        Ok(Self {
            ctx: ctx.clone(),
            clients: Clients {
                mayastor_client: ct.unwrap(),
            },
        })
    }

    /// get gRPC clients
    pub fn clients_mut(&mut self) -> &mut Clients {
        &mut self.clients
    }
}
