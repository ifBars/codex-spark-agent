use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const ISSUER: &str = "https://auth.openai.com";
const REDIRECT_HOST: &str = "127.0.0.1";
const REDIRECT_PUBLIC_HOST: &str = "localhost";
const REDIRECT_PATH: &str = "/auth/callback";
// Keep these in sync with the Codex CLI OAuth redirect URI allow-list.
const DEFAULT_CALLBACK_PORT: u16 = 1455;
const FALLBACK_CALLBACK_PORT: u16 = 1457;
const ORIGINATOR: &str = "codex_cli_rs";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthTokens {
    pub id_token: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
    pub account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    id_token: String,
    access_token: String,
    refresh_token: String,
    expires_in: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct DeviceUserCodeResponse {
    device_auth_id: String,
    #[serde(alias = "usercode")]
    user_code: String,
    #[serde(default, deserialize_with = "deserialize_interval")]
    interval: u64,
}

#[derive(Debug, Deserialize)]
struct DeviceTokenResponse {
    authorization_code: String,
    #[serde(rename = "code_challenge")]
    code_challenge: String,
    code_verifier: String,
}

#[derive(Debug, Deserialize)]
struct Claims {
    chatgpt_account_id: Option<String>,
    organizations: Option<Vec<Organization>>,
    #[serde(rename = "https://api.openai.com/auth")]
    openai_auth: Option<OpenAiAuth>,
}

#[derive(Debug, Deserialize)]
struct Organization {
    id: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiAuth {
    chatgpt_account_id: Option<String>,
}

pub async fn login(open_browser: bool) -> Result<AuthTokens> {
    let listener = bind_callback_listener().await?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://{REDIRECT_PUBLIC_HOST}:{port}{REDIRECT_PATH}");
    let verifier = random_urlsafe(64);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = random_urlsafe(32);
    let auth_url = authorize_url(&redirect_uri, &challenge, &state);

    println!("Open this URL to authenticate:\n{auth_url}\n");
    if open_browser {
        let _ = open::that(&auth_url);
    }

    let code = wait_for_callback(listener, &state).await?;
    let response = exchange_code(&code, &redirect_uri, &verifier).await?;
    Ok(tokens_from_response(response))
}

async fn bind_callback_listener() -> Result<TcpListener> {
    bind_callback_listener_on_ports(DEFAULT_CALLBACK_PORT, FALLBACK_CALLBACK_PORT).await
}

async fn bind_callback_listener_on_ports(
    preferred_port: u16,
    fallback_port: u16,
) -> Result<TcpListener> {
    match TcpListener::bind((REDIRECT_HOST, preferred_port)).await {
        Ok(listener) => Ok(listener),
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
            TcpListener::bind((REDIRECT_HOST, fallback_port))
                .await
                .with_context(|| {
                    format!(
                        "failed to bind OAuth callback listener on registered ports {preferred_port} or {fallback_port}"
                    )
                })
        }
        Err(error) => Err(error).with_context(|| {
            format!("failed to bind OAuth callback listener on registered port {preferred_port}")
        }),
    }
}

pub async fn login_device_code() -> Result<AuthTokens> {
    let device = request_device_code().await?;
    println!(
        "Open this URL and enter the code:\n{}\n\nCode: {}\n",
        device.verification_url, device.user_code
    );
    let code_response = poll_device_code(&device).await?;
    if code_response.code_challenge.trim().is_empty() {
        anyhow::bail!("device auth response did not include a PKCE challenge");
    }
    let redirect_uri = format!("{ISSUER}/deviceauth/callback");
    let response = exchange_code(
        &code_response.authorization_code,
        &redirect_uri,
        &code_response.code_verifier,
    )
    .await?;
    Ok(tokens_from_response(response))
}

pub async fn refresh(tokens: &AuthTokens) -> Result<AuthTokens> {
    let params = [
        ("grant_type", "refresh_token"),
        ("refresh_token", tokens.refresh_token.as_str()),
        ("client_id", CLIENT_ID),
    ];
    let response = reqwest::Client::new()
        .post(format!("{ISSUER}/oauth/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&params)
        .send()
        .await
        .context("token refresh request failed")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("token refresh failed ({status}): {body}");
    }

    Ok(tokens_from_response(
        response.json::<TokenResponse>().await?,
    ))
}

pub fn is_expired(tokens: &AuthTokens) -> bool {
    tokens.expires_at - 30 <= now_unix()
}

fn authorize_url(redirect_uri: &str, challenge: &str, state: &str) -> String {
    let params = [
        ("response_type", "code"),
        ("client_id", CLIENT_ID),
        ("redirect_uri", redirect_uri),
        (
            "scope",
            "openid profile email offline_access api.connectors.read api.connectors.invoke",
        ),
        ("code_challenge", challenge),
        ("code_challenge_method", "S256"),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("state", state),
        ("originator", ORIGINATOR),
    ];
    let query = params
        .iter()
        .map(|(key, value)| format!("{key}={}", urlencoding::encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{ISSUER}/oauth/authorize?{query}")
}

async fn exchange_code(code: &str, redirect_uri: &str, verifier: &str) -> Result<TokenResponse> {
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
        ("client_id", CLIENT_ID),
        ("code_verifier", verifier),
    ];
    let response = reqwest::Client::new()
        .post(format!("{ISSUER}/oauth/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&params)
        .send()
        .await
        .context("token exchange request failed")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("token exchange failed ({status}): {body}");
    }

    response
        .json::<TokenResponse>()
        .await
        .context("failed to parse token response")
}

async fn request_device_code() -> Result<DeviceCode> {
    #[derive(Serialize)]
    struct Request<'a> {
        client_id: &'a str,
    }

    let response = reqwest::Client::new()
        .post(format!("{ISSUER}/api/accounts/deviceauth/usercode"))
        .header("Content-Type", "application/json")
        .json(&Request {
            client_id: CLIENT_ID,
        })
        .send()
        .await
        .context("device code request failed")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("device code request failed ({status}): {body}");
    }

    let body = response.json::<DeviceUserCodeResponse>().await?;
    Ok(DeviceCode {
        verification_url: format!("{ISSUER}/codex/device"),
        user_code: body.user_code,
        device_auth_id: body.device_auth_id,
        interval: body.interval,
    })
}

async fn poll_device_code(device: &DeviceCode) -> Result<DeviceTokenResponse> {
    #[derive(Serialize)]
    struct Request<'a> {
        device_auth_id: &'a str,
        user_code: &'a str,
    }

    let client = reqwest::Client::new();
    let started = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(15 * 60);
    let interval = std::time::Duration::from_secs(device.interval.max(1));

    loop {
        let response = client
            .post(format!("{ISSUER}/api/accounts/deviceauth/token"))
            .header("Content-Type", "application/json")
            .json(&Request {
                device_auth_id: &device.device_auth_id,
                user_code: &device.user_code,
            })
            .send()
            .await
            .context("device token poll failed")?;

        if response.status().is_success() {
            return response
                .json::<DeviceTokenResponse>()
                .await
                .context("failed to parse device token response");
        }

        if started.elapsed() >= timeout {
            anyhow::bail!("device auth timed out after 15 minutes");
        }

        tokio::time::sleep(interval).await;
    }
}

async fn wait_for_callback(listener: TcpListener, expected_state: &str) -> Result<String> {
    let (mut stream, _) = listener.accept().await?;
    let mut buf = vec![0_u8; 8192];
    let n = stream.read(&mut buf).await?;
    let request = String::from_utf8_lossy(&buf[..n]);
    let first_line = request.lines().next().unwrap_or_default();
    let target = first_line
        .split_whitespace()
        .nth(1)
        .context("malformed OAuth callback request")?;
    let parsed = Url::parse(&format!("http://localhost{target}"))?;
    let params = parsed.query_pairs().collect::<Vec<_>>();
    let state = params
        .iter()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value.to_string())
        .context("OAuth callback missing state")?;
    let code = params
        .iter()
        .find(|(key, _)| key == "code")
        .map(|(_, value)| value.to_string())
        .context("OAuth callback missing code")?;

    let ok = state == expected_state;
    let body = if ok {
        "Spark login complete. You can close this tab."
    } else {
        "Spark login failed: state mismatch."
    };
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes()).await;
    if !ok {
        anyhow::bail!("OAuth state mismatch");
    }
    Ok(code)
}

fn tokens_from_response(response: TokenResponse) -> AuthTokens {
    let account_id = account_id_from_jwt(&response.id_token)
        .or_else(|| account_id_from_jwt(&response.access_token));
    AuthTokens {
        id_token: response.id_token,
        access_token: response.access_token,
        refresh_token: response.refresh_token,
        expires_at: now_unix() + response.expires_in.unwrap_or(3600) as i64,
        account_id,
    }
}

#[derive(Debug, Clone)]
struct DeviceCode {
    verification_url: String,
    user_code: String,
    device_auth_id: String,
    interval: u64,
}

fn deserialize_interval<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Number(number) => number
            .as_u64()
            .ok_or_else(|| serde::de::Error::custom("invalid interval")),
        serde_json::Value::String(value) => value
            .trim()
            .parse::<u64>()
            .map_err(serde::de::Error::custom),
        _ => Err(serde::de::Error::custom("invalid interval")),
    }
}

fn account_id_from_jwt(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let claims = serde_json::from_slice::<Claims>(&decoded).ok()?;
    claims
        .chatgpt_account_id
        .or_else(|| claims.openai_auth.and_then(|auth| auth.chatgpt_account_id))
        .or_else(|| {
            claims
                .organizations
                .and_then(|orgs| orgs.first().map(|org| org.id.clone()))
        })
}

fn random_urlsafe(bytes: usize) -> String {
    let mut data = vec![0_u8; bytes];
    rand::thread_rng().fill_bytes(&mut data);
    URL_SAFE_NO_PAD.encode(data)
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_login_uses_registered_codex_callback_port() {
        let redirect_uri =
            format!("http://{REDIRECT_PUBLIC_HOST}:{DEFAULT_CALLBACK_PORT}{REDIRECT_PATH}");
        let url = authorize_url(&redirect_uri, "challenge", "state");

        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback"));
    }

    #[tokio::test]
    async fn callback_listener_falls_back_when_preferred_port_is_busy() {
        let occupied = TcpListener::bind((REDIRECT_HOST, 0))
            .await
            .expect("test listener should bind");
        let occupied_port = occupied
            .local_addr()
            .expect("test listener should have an address")
            .port();

        let listener = bind_callback_listener_on_ports(occupied_port, 0)
            .await
            .expect("fallback listener should bind");

        assert_ne!(
            listener
                .local_addr()
                .expect("fallback listener should have an address")
                .port(),
            occupied_port
        );
    }
}
