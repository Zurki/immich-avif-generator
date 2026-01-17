use crate::config::AuthConfig;
use anyhow::{anyhow, Result};
use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope,
    TokenResponse, TokenUrl,
};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub enum AuthProvider {
    ApiKey(String),
    OAuth(Arc<OAuthProvider>),
}

pub struct OAuthProvider {
    client: BasicClient,
    token: RwLock<Option<String>>,
    #[allow(dead_code)]
    refresh_token: RwLock<Option<String>>,
}

impl AuthProvider {
    pub fn from_config(config: &AuthConfig) -> Result<Self> {
        match config {
            AuthConfig::ApiKey { api_key } => Ok(AuthProvider::ApiKey(api_key.clone())),
            AuthConfig::OAuth {
                client_id,
                client_secret,
                token_url,
                auth_url,
                redirect_uri,
            } => {
                let client = BasicClient::new(
                    ClientId::new(client_id.clone()),
                    Some(ClientSecret::new(client_secret.clone())),
                    AuthUrl::new(auth_url.clone())?,
                    Some(TokenUrl::new(token_url.clone())?),
                )
                .set_redirect_uri(RedirectUrl::new(redirect_uri.clone())?);

                Ok(AuthProvider::OAuth(Arc::new(OAuthProvider {
                    client,
                    token: RwLock::new(None),
                    refresh_token: RwLock::new(None),
                })))
            }
        }
    }

    pub async fn get_auth_header(&self) -> Result<(String, String)> {
        match self {
            AuthProvider::ApiKey(key) => Ok(("x-api-key".to_string(), key.clone())),
            AuthProvider::OAuth(provider) => {
                let token = provider.token.read().await;
                match token.as_ref() {
                    Some(t) => Ok(("Authorization".to_string(), format!("Bearer {}", t))),
                    None => Err(anyhow!("OAuth token not set. Please authenticate first.")),
                }
            }
        }
    }

    pub fn get_oauth_url(&self) -> Result<(String, String)> {
        match self {
            AuthProvider::ApiKey(_) => Err(anyhow!("Cannot get OAuth URL for API key auth")),
            AuthProvider::OAuth(provider) => {
                let (auth_url, csrf_token) = provider
                    .client
                    .authorize_url(CsrfToken::new_random)
                    .add_scope(Scope::new("all".to_string()))
                    .url();

                Ok((auth_url.to_string(), csrf_token.secret().clone()))
            }
        }
    }

    pub async fn exchange_code(&self, code: &str) -> Result<()> {
        match self {
            AuthProvider::ApiKey(_) => Err(anyhow!("Cannot exchange code for API key auth")),
            AuthProvider::OAuth(provider) => {
                let token_result = provider
                    .client
                    .exchange_code(AuthorizationCode::new(code.to_string()))
                    .request_async(oauth2::reqwest::async_http_client)
                    .await
                    .map_err(|e| anyhow!("OAuth token exchange failed: {}", e))?;

                let mut token = provider.token.write().await;
                *token = Some(token_result.access_token().secret().clone());

                if let Some(refresh) = token_result.refresh_token() {
                    let mut refresh_token = provider.refresh_token.write().await;
                    *refresh_token = Some(refresh.secret().clone());
                }

                Ok(())
            }
        }
    }

    #[allow(dead_code)]
    pub async fn set_token(&self, access_token: &str) -> Result<()> {
        match self {
            AuthProvider::ApiKey(_) => Err(anyhow!("Cannot set token for API key auth")),
            AuthProvider::OAuth(provider) => {
                let mut token = provider.token.write().await;
                *token = Some(access_token.to_string());
                Ok(())
            }
        }
    }
}
