pub mod discord;
pub mod facebook;
pub mod github;
pub mod google;
pub mod microsoft;
pub mod steam;
pub mod twitter;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct State {
    redirect_uri: String,
    state: Option<String>,
}
