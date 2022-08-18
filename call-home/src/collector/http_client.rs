use std::time::Duration;
use reqwest::{Client};
use url::{Url};
use crate::collector::api_models;
use crate::common::{errors::HttpClientError,constants};

const APIVERSION: &str = constants::APIVERSION;


///HttpClient contains the reqwest client and the base url
#[derive(Clone,Debug)]
pub struct HttpClient {
    client: Client,
    base_url: Url
}

impl HttpClient {
    ///Creates a new reqwest client and parse the base url
    pub(crate) fn new(url: &str) -> Result<Self, HttpClientError>
    {
        let client = reqwest::Client::builder().timeout(Duration::from_secs(60)).build()?;
        let base_url = Url::parse(url)?;
        Ok(Self {
            client,
            base_url
        })
    }
    ///Fetch pools object from rest
    pub async fn get_pools(&self) -> Result<Vec<api_models::Pools>, HttpClientError> {
        let path = format!("/{}/pools",APIVERSION);
        let url = self.base_url.join(&path)?;
        let response = self.client.get(url)
            .send()
            .await?;
        match response.status().is_success(){
            true => {
                let pools = response.json::<Vec<api_models::Pools>>().await?;
                Ok(pools)
            }
            false => Err(HttpClientError::invalid_http_response_error(response.error_for_status().err().unwrap().to_string()))
        }
    }
    ///Fetch nodes object from rest
    pub async fn get_nodes(&self) -> Result<Vec<api_models::Nodes>, HttpClientError> {
        let path = format!("/{}/nodes",APIVERSION);
        let url = self.base_url.join(&path)?;
        let response = self.client.get(url)
            .send()
            .await?;
        match response.status().is_success(){
            true => {
                let nodes = response.json::<Vec<api_models::Nodes>>().await?;
                Ok(nodes)
            }
            false => Err(HttpClientError::invalid_http_response_error(response.error_for_status().err().unwrap().to_string()))
        }
    }

    ///Fetch volumes object from rest
    pub async fn get_volumes(&self, max_entries: u32) -> Result<api_models::Volumes, HttpClientError> {
        let path = format!("/{}/volumes",APIVERSION);
        let url = self.base_url.join(&path)?;
        let response = self.client.get(url)
            .query(&[("max_entries", max_entries)])
            .send()
            .await?;
        match response.status().is_success(){
            true => {
                let volumes = response.json::<api_models::Volumes>().await?;
                Ok(volumes)
            }
            false => Err(HttpClientError::invalid_http_response_error(response.error_for_status().err().unwrap().to_string()))
        }
    }

    ///Fetch replicas object from rest
    pub async fn get_replicas(&self) -> Result<Vec<api_models::Replicas>, HttpClientError> {
        let path = format!("/{}/replicas",APIVERSION);
        let url = self.base_url.join(&path)?;
        let response = self.client.get(url)
            .send()
            .await?;
        match response.status().is_success() {
            true => {
                let replicas = response.json::<Vec<api_models::Replicas>>().await?;
                Ok(replicas)
            }
            false => Err(HttpClientError::invalid_http_response_error(response.error_for_status().err().unwrap().to_string()))
        }
    }
}

