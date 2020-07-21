pub mod discord;
pub mod facebook;
pub mod github;
pub mod google;
pub mod microsoft;
pub mod twitter;

pub mod oauth;

use serde::{Deserialize, Serialize};

/// Query parameters coming from the client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    pub client_id: String,
    pub redirect_uri: String,
    pub state: Option<String>,
    pub token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CodeJwt {
    pub provider_name: String,
    pub provider_id: String,
    pub client_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenJwt {
    pub sub: String,
}
