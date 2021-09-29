use reqwest::ClientBuilder;
use url::Url;

use crate::client::v2::ClientV2;

mod v2;

#[derive(Clone)]
pub struct Client {
    base_url: Url,
    client: reqwest::Client,
}

impl Client {
    pub fn new(base_url: Url) -> Self {
        Client {
            base_url,
            client: ClientBuilder::new()
                .use_rustls_tls()
                .build()
                .expect("ClientBuilder::build"),
        }
    }

    pub fn v2(&self) -> ClientV2 {
        ClientV2::new(self.base_url.clone(), self.client.clone())
    }
}
