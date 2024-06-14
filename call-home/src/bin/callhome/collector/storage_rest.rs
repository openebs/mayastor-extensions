use openapi::{
    clients::tower::Error,
    models::{RestJsonError, Volume},
    tower::client::ApiClient,
};

const STORAGE_API_PAGE_SIZE: isize = 500;

pub(crate) async fn list_all_volumes(
    client: &ApiClient,
) -> Result<Vec<Volume>, Error<RestJsonError>> {
    let mut volumes: Vec<Volume> = Vec::new();
    let mut starting_token: Option<isize> = Some(0);

    // The last paginated request will set the `starting_token` to `None`.
    while starting_token.is_some() {
        let vols = client
            .volumes_api()
            .get_volumes(STORAGE_API_PAGE_SIZE, None, starting_token)
            .await
            .map(|response| response.into_body())?;

        volumes.extend(vols.entries);

        starting_token = vols.next_token;
    }

    Ok(volumes)
}
