use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use directories::{BaseDirs, ProjectDirs};
use serde::Deserialize;

use crate::auth::AuthTokens;

pub fn app_dir() -> Result<PathBuf> {
    let legacy = legacy_app_dir()?;
    if legacy.exists() {
        return Ok(legacy);
    }
    if let Some(project_dirs) = ProjectDirs::from("com", "ifbars", "spark") {
        return Ok(project_dirs.data_local_dir().to_path_buf());
    }
    Ok(legacy)
}

fn legacy_app_dir() -> Result<PathBuf> {
    let base_dirs = BaseDirs::new().context("failed to resolve home directory")?;
    Ok(base_dirs.home_dir().join(".spark-codex"))
}

pub fn auth_path() -> Result<PathBuf> {
    Ok(app_dir()?.join("auth.json"))
}

pub fn sessions_dir() -> Result<PathBuf> {
    Ok(app_dir()?.join("sessions"))
}

pub fn sessions_db_path() -> Result<PathBuf> {
    Ok(app_dir()?.join("sessions.sqlite3"))
}

pub fn memory_dir() -> Result<PathBuf> {
    Ok(app_dir()?.join("memory"))
}

pub fn save_auth(tokens: &AuthTokens) -> Result<()> {
    let dir = app_dir()?;
    std::fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
    let path = auth_path()?;
    let body = serde_json::to_vec_pretty(tokens)?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    file.write_all(&body)
        .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

pub fn load_auth() -> Result<AuthTokens> {
    let path = auth_path()?;
    let mut file = match OpenOptions::new().read(true).open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return load_codex_auth().with_context(|| {
                format!(
                    "not logged in; run `spark login` first ({})",
                    path.display()
                )
            });
        }
        Err(error) => {
            return Err(error).with_context(|| format!("failed to open {}", path.display()));
        }
    };
    let mut body = String::new();
    file.read_to_string(&mut body)?;
    serde_json::from_str(&body).with_context(|| format!("failed to parse {}", path.display()))
}

fn load_codex_auth() -> Result<AuthTokens> {
    let path = codex_auth_path()?;
    let body = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to open Codex auth at {}", path.display()))?;
    parse_codex_auth(&body).with_context(|| format!("failed to parse {}", path.display()))
}

fn codex_auth_path() -> Result<PathBuf> {
    if let Some(home) = std::env::var_os("CODEX_HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(home).join("auth.json"));
    }
    let base_dirs = BaseDirs::new().context("failed to resolve home directory")?;
    Ok(base_dirs.home_dir().join(".codex").join("auth.json"))
}

#[derive(Debug, Deserialize)]
struct CodexAuthFile {
    tokens: Option<CodexAuthTokens>,
}

#[derive(Debug, Deserialize)]
struct CodexAuthTokens {
    id_token: String,
    access_token: String,
    refresh_token: String,
    account_id: Option<String>,
}

fn parse_codex_auth(body: &str) -> Result<AuthTokens> {
    let file: CodexAuthFile = serde_json::from_str(body)?;
    let tokens = file
        .tokens
        .context("Codex auth does not contain ChatGPT tokens")?;
    Ok(AuthTokens {
        expires_at: jwt_expiry(&tokens.access_token).unwrap_or_default(),
        id_token: tokens.id_token,
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        account_id: tokens.account_id,
    })
}

fn jwt_expiry(token: &str) -> Option<i64> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()?
        .get("exp")?
        .as_i64()
}

pub(crate) fn is_valid_session_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'))
}

#[cfg(test)]
mod tests {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

    use super::{is_valid_session_name, parse_codex_auth};

    #[test]
    fn session_names_are_filename_safe() {
        assert!(is_valid_session_name("default"));
        assert!(is_valid_session_name("repo.work-1"));
        assert!(!is_valid_session_name(""));
        assert!(!is_valid_session_name("../bad"));
        assert!(!is_valid_session_name("bad/name"));
        assert!(!is_valid_session_name("bad name"));
    }

    #[test]
    fn imports_chatgpt_tokens_from_codex_auth() {
        let payload = URL_SAFE_NO_PAD.encode(br#"{"exp":4102444800}"#);
        let body = serde_json::json!({
            "auth_mode": "chatgpt",
            "tokens": {
                "id_token": "id",
                "access_token": format!("header.{payload}.signature"),
                "refresh_token": "refresh",
                "account_id": "account"
            }
        })
        .to_string();

        let imported = parse_codex_auth(&body).expect("Codex auth should import");

        assert_eq!(imported.id_token, "id");
        assert_eq!(imported.access_token, format!("header.{payload}.signature"));
        assert_eq!(imported.refresh_token, "refresh");
        assert_eq!(imported.account_id.as_deref(), Some("account"));
        assert_eq!(imported.expires_at, 4_102_444_800);
    }

    #[test]
    fn rejects_codex_auth_without_chatgpt_tokens() {
        let error = parse_codex_auth(r#"{"auth_mode":"apikey"}"#)
            .expect_err("API-key auth is not a Spark subscription token");
        assert!(
            error
                .to_string()
                .contains("does not contain ChatGPT tokens")
        );
    }
}
