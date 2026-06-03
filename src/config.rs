use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::auth::AuthTokens;

pub fn app_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("failed to resolve home directory")?;
    Ok(home.join(".spark-codex"))
}

pub fn auth_path() -> Result<PathBuf> {
    Ok(app_dir()?.join("auth.json"))
}

pub fn sessions_dir() -> Result<PathBuf> {
    Ok(app_dir()?.join("sessions"))
}

pub fn session_path(name: &str) -> Result<PathBuf> {
    if !is_valid_session_name(name) {
        anyhow::bail!(
            "invalid session name `{name}`; use letters, numbers, dots, dashes, and underscores"
        );
    }
    Ok(sessions_dir()?.join(format!("{name}.json")))
}

pub fn list_sessions() -> Result<Vec<String>> {
    let dir = sessions_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = std::fs::read_dir(&dir)
        .with_context(|| format!("failed to list {}", dir.display()))?
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                return None;
            }
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    sessions.sort();
    Ok(sessions)
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
    let mut file = OpenOptions::new().read(true).open(&path).with_context(|| {
        format!(
            "not logged in; run `spark login` first ({})",
            path.display()
        )
    })?;
    let mut body = String::new();
    file.read_to_string(&mut body)?;
    serde_json::from_str(&body).with_context(|| format!("failed to parse {}", path.display()))
}

fn is_valid_session_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'))
}

#[cfg(test)]
mod tests {
    use super::is_valid_session_name;

    #[test]
    fn session_names_are_filename_safe() {
        assert!(is_valid_session_name("default"));
        assert!(is_valid_session_name("repo.work-1"));
        assert!(!is_valid_session_name(""));
        assert!(!is_valid_session_name("../bad"));
        assert!(!is_valid_session_name("bad/name"));
        assert!(!is_valid_session_name("bad name"));
    }
}
