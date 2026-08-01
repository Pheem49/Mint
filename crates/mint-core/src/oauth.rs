//! OAuth 2.0 PKCE Authorization & Token Exchange Module for Mint Core.
//! Manages PKCE state generation, browser authorization URLs, token exchange, and persistence.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use sha2::{Digest, Sha256};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

static PENDING_PKCE: OnceLock<Mutex<HashMap<String, PkceSession>>> = OnceLock::new();

fn get_pending_pkce() -> &'static Mutex<HashMap<String, PkceSession>> {
    PENDING_PKCE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PkceSession {
    pub provider: String,
    pub code_verifier: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokenData {
    pub provider: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>,
    pub account_email: Option<String>,
    pub account_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthStatusItem {
    pub provider: String,
    pub connected: bool,
    pub account_email: Option<String>,
    pub account_name: Option<String>,
}

/// Known OAuth provider metadata
pub struct OAuthProviderConfig {
    pub name: &'static str,
    pub auth_endpoint: &'static str,
    pub token_endpoint: &'static str,
    pub default_client_id: &'static str,
    pub scope: &'static str,
}

pub fn get_provider_config(provider: &str) -> Option<OAuthProviderConfig> {
    match provider {
        "google" | "gmail" | "google_calendar" | "youtube_music" => Some(OAuthProviderConfig {
            name: "google",
            auth_endpoint: "https://accounts.google.com/o/oauth2/v2/auth",
            token_endpoint: "https://oauth2.googleapis.com/token",
            default_client_id: "mint-default-google-client-id.apps.googleusercontent.com",
            scope: "https://www.googleapis.com/auth/userinfo.email https://www.googleapis.com/auth/calendar https://www.googleapis.com/auth/gmail.modify https://www.googleapis.com/auth/youtube.readonly",
        }),
        "vercel" => Some(OAuthProviderConfig {
            name: "vercel",
            auth_endpoint: "https://vercel.com/oauth/authorize",
            token_endpoint: "https://api.vercel.com/v2/oauth/access_token",
            default_client_id: "mint-default-vercel-client-id",
            scope: "user:read projects:read deployments:read",
        }),
        "github" => Some(OAuthProviderConfig {
            name: "github",
            auth_endpoint: "https://github.com/login/oauth/authorize",
            token_endpoint: "https://github.com/login/oauth/access_token",
            default_client_id: "mint-default-github-client-id",
            scope: "read:user user:email repo",
        }),
        "spotify" => Some(OAuthProviderConfig {
            name: "spotify",
            auth_endpoint: "https://accounts.spotify.com/authorize",
            token_endpoint: "https://accounts.spotify.com/api/token",
            default_client_id: "mint-default-spotify-client-id",
            scope: "user-read-private user-read-email user-modify-playback-state user-read-playback-state",
        }),
        "notion" => Some(OAuthProviderConfig {
            name: "notion",
            auth_endpoint: "https://api.notion.com/v1/oauth/authorize",
            token_endpoint: "https://api.notion.com/v1/oauth/token",
            default_client_id: "mint-default-notion-client-id",
            scope: "",
        }),
        _ => None,
    }
}

/// URL encoding helper
fn url_encode(input: &str) -> String {
    input
        .bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{:02X}", b),
        })
        .collect()
}

/// Generates PKCE code verifier and code challenge pair
pub fn generate_pkce_pair() -> (String, String) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let seed = format!("mint-pkce-{now}-{}", std::process::id());
    
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    let verifier = URL_SAFE_NO_PAD.encode(hasher.finalize_reset());

    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

    (verifier, challenge)
}

/// Generates OAuth authorization URL for popup / system browser
pub fn build_auth_url(provider: &str, redirect_uri: &str, custom_client_id: Option<&str>) -> Option<(String, String)> {
    let cfg = get_provider_config(provider)?;
    let (verifier, challenge) = generate_pkce_pair();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let state = format!("{provider}-{now_ms}");

    let client_id = custom_client_id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(cfg.default_client_id);

    let auth_url = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256&access_type=offline&prompt=consent",
        cfg.auth_endpoint,
        url_encode(client_id),
        url_encode(redirect_uri),
        url_encode(cfg.scope),
        url_encode(&state),
        url_encode(&challenge)
    );

    let now_secs = (now_ms / 1000) as u64;

    if let Ok(mut pending) = get_pending_pkce().lock() {
        pending.insert(
            state.clone(),
            PkceSession {
                provider: provider.to_string(),
                code_verifier: verifier,
                created_at: now_secs,
            },
        );
    }

    Some((auth_url, state))
}

/// Exchanges authorization code for access token using saved PKCE session
pub async fn exchange_code(
    provider: &str,
    code: &str,
    state: &str,
    redirect_uri: &str,
    custom_client_id: Option<&str>,
    custom_client_secret: Option<&str>,
) -> Result<OAuthTokenData, String> {
    let cfg = get_provider_config(provider)
        .ok_or_else(|| format!("Unsupported OAuth provider: {provider}"))?;

    let pkce_session = if let Ok(mut pending) = get_pending_pkce().lock() {
        pending.remove(state)
    } else {
        None
    };

    let verifier = pkce_session
        .map(|s| s.code_verifier)
        .unwrap_or_default();

    let client_id = custom_client_id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(cfg.default_client_id);

    let mut params = HashMap::new();
    params.insert("grant_type", "authorization_code");
    params.insert("client_id", client_id);
    params.insert("code", code);
    params.insert("redirect_uri", redirect_uri);
    if !verifier.is_empty() {
        params.insert("code_verifier", &verifier);
    }
    if let Some(secret) = custom_client_secret.filter(|s| !s.trim().is_empty()) {
        params.insert("client_secret", secret);
    }

    let client = crate::HTTP_CLIENT.clone();
    let res = client
        .post(cfg.token_endpoint)
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .await
        .map_err(|e| format!("Token request failed: {e}"))?;

    let json: Value = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse token response: {e}"))?;

    let access_token = json["access_token"]
        .as_str()
        .ok_or_else(|| format!("No access_token in response: {json:?}"))?
        .to_string();

    let refresh_token = json["refresh_token"].as_str().map(|s| s.to_string());
    let expires_in = json["expires_in"].as_u64();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let expires_at = expires_in.map(|secs| now + secs);

    let mut account_email = None;
    let mut account_name = None;

    // Fetch user profile metadata if available
    if provider == "google" || provider == "gmail" {
        if let Ok(user_info) = fetch_google_user_info(&client, &access_token).await {
            account_email = user_info.0;
            account_name = user_info.1;
        }
    } else if provider == "github" {
        if let Ok(user_info) = fetch_github_user_info(&client, &access_token).await {
            account_email = user_info.0;
            account_name = user_info.1;
        }
    }

    let token_data = OAuthTokenData {
        provider: provider.to_string(),
        access_token,
        refresh_token,
        expires_at,
        account_email,
        account_name,
    };

    save_oauth_tokens(&token_data)?;

    Ok(token_data)
}

async fn fetch_google_user_info(client: &reqwest::Client, token: &str) -> Result<(Option<String>, Option<String>), ()> {
    let res = client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| ())?;
    let json: Value = res.json().await.map_err(|_| ())?;
    let email = json["email"].as_str().map(|s| s.to_string());
    let name = json["name"].as_str().map(|s| s.to_string());
    Ok((email, name))
}

async fn fetch_github_user_info(client: &reqwest::Client, token: &str) -> Result<(Option<String>, Option<String>), ()> {
    let res = client
        .get("https://api.github.com/user")
        .header("User-Agent", "Mint-Agent")
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| ())?;
    let json: Value = res.json().await.map_err(|_| ())?;
    let email = json["email"].as_str().map(|s| s.to_string());
    let name = json["login"].as_str().map(|s| s.to_string());
    Ok((email, name))
}

/// Saves OAuth tokens to MemoryStore
pub fn save_oauth_tokens(data: &OAuthTokenData) -> Result<(), String> {
    if let Ok(memory) = crate::memory::MemoryStore::open_default() {
        let serialized = serde_json::to_string(data).map_err(|e| e.to_string())?;
        memory
            .set_profile(&format!("oauth_token_{}", data.provider), &serialized)
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    Err("Failed to open MemoryStore".to_string())
}

/// Gets saved OAuth token data for provider
pub fn get_oauth_tokens(provider: &str) -> Option<OAuthTokenData> {
    if let Ok(memory) = crate::memory::MemoryStore::open_default() {
        if let Ok(Some(json)) = memory.get_profile(&format!("oauth_token_{provider}")) {
            if let Ok(data) = serde_json::from_str::<OAuthTokenData>(&json) {
                return Some(data);
            }
        }
    }
    None
}

/// Revokes / deletes OAuth tokens for provider
pub fn revoke_oauth_tokens(provider: &str) -> Result<(), String> {
    if let Ok(memory) = crate::memory::MemoryStore::open_default() {
        memory
            .set_profile(&format!("oauth_token_{provider}"), "")
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    Err("Failed to open MemoryStore".to_string())
}

/// Returns list of all OAuth connection statuses
pub fn list_oauth_statuses() -> Vec<OAuthStatusItem> {
    let providers = ["google", "vercel", "github", "spotify", "notion"];
    providers
        .iter()
        .map(|p| {
            if let Some(tokens) = get_oauth_tokens(p) {
                OAuthStatusItem {
                    provider: p.to_string(),
                    connected: !tokens.access_token.is_empty(),
                    account_email: tokens.account_email,
                    account_name: tokens.account_name,
                }
            } else {
                OAuthStatusItem {
                    provider: p.to_string(),
                    connected: false,
                    account_email: None,
                    account_name: None,
                }
            }
        })
        .collect()
}
