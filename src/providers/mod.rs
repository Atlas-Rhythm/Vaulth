pub mod discord;
pub mod facebook;
pub mod github;
pub mod google;
pub mod microsoft;
pub mod steam;
pub mod twitter;

mod oauth2;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Params {
    redirect_uri: String,
    state: Option<String>,
    user: Option<String>,
}
