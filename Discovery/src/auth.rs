/******************************************************************************
 * Copyright ContinuousC. Licensed under the "Elastic License 2.0".           *
 ******************************************************************************/

use std::path::PathBuf;

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{Error, Result};

#[derive(clap::Args)]
pub(crate) struct AuthConfig {
    /// Url to use for retrieving the bearer token.
    #[clap(long, env = "CONTINUOUSC_TOKEN_URL", required = false)]
    token_url: Url,
    /// Service user to authenticate as.
    #[clap(long, env = "CONTINUOUSC_TOKEN_USER", required = false)]
    token_user: String,
    /// Path to the secret to use for authentication.
    #[clap(long, env = "CONTINUOUSC_TOKEN_SECRET_PATH", required = false)]
    token_secret_path: PathBuf,
}

#[derive(Default)]
pub(crate) struct Authenticator {
    token: Option<(TokenResponse, DateTime<Utc>)>,
}

#[derive(Serialize)]
struct TokenRequest<'a> {
    client_id: &'a str,
    client_secret: &'a str,
    grant_type: &'a str,
    scope: &'a str,
    // username: &'a str
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u32,
    // refresh_expires_in: u32,
    // token_type: "Bearer",
    // not-before-policy: 0,
    // scope: "email profile"
}

impl Authenticator {
    pub(crate) async fn get_token(
        &mut self,
        config: &AuthConfig,
        client: &reqwest::Client,
    ) -> Result<&str> {
        if let Some((token, t)) = self.token.as_ref() {
            if *t + TimeDelta::seconds(token.expires_in as i64)
                > Utc::now() + TimeDelta::seconds(30)
            {
                // Required while we wait for Polonius.
                let (token, _) = self.token.as_ref().unwrap();
                return Ok(&token.access_token);
            }
        }

        let secret = tokio::fs::read_to_string(&config.token_secret_path)
            .await
            .map_err(Error::ReadTokenSecret)?;
        let token = client
            .post(config.token_url.clone())
            .form(&TokenRequest {
                client_id: &config.token_user,
                client_secret: secret.strip_suffix('\n').unwrap_or(&secret),
                grant_type: "client_credentials",
                scope: "basic email",
            })
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(Error::TokenRequest)?
            .json::<TokenResponse>()
            .await
            .map_err(Error::TokenRequest)?;
        let (token, _) = self.token.insert((token, Utc::now()));
        Ok(&token.access_token)
    }
}
