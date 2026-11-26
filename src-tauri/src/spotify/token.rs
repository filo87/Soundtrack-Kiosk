use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::error::{AppError, AppResult};
use crate::state::Secret;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tokens {
    pub access_token: Secret,
    pub refresh_token: Secret,
    pub expires_at: i64,
}

impl Tokens {
    pub fn access(&self) -> &str {
        self.access_token.expose()
    }

    pub fn refresh(&self) -> &str {
        self.refresh_token.expose()
    }

    fn expired_or_soon(&self) -> bool {
        self.expires_at - 60 <= chrono::Utc::now().timestamp()
    }
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
    #[serde(default)]
    refresh_token: Option<String>,
}

struct Inner {
    tokens: Mutex<Option<Tokens>>,
    path: PathBuf,
    http: reqwest::Client,
    client_id: std::sync::RwLock<String>,
    token_url: String,
    needs_login: AtomicBool,
}

#[derive(Clone)]
pub struct TokenManager {
    inner: Arc<Inner>,
}

impl TokenManager {
    pub fn new(path: PathBuf, http: reqwest::Client, client_id: String, token_url: String) -> Self {
        let tokens = load_tokens(&path);
        let needs_login = tokens.is_none();
        Self {
            inner: Arc::new(Inner {
                tokens: Mutex::new(tokens),
                path,
                http,
                client_id: std::sync::RwLock::new(client_id),
                token_url,
                needs_login: AtomicBool::new(needs_login),
            }),
        }
    }

    pub fn is_authenticated(&self) -> bool {
        if self.inner.needs_login.load(Ordering::Relaxed) {
            return false;
        }
        self.inner
            .tokens
            .try_lock()
            .map(|guard| guard.as_ref().is_some_and(|t| !t.refresh().is_empty()))
            .unwrap_or(true)
    }

    pub fn needs_login(&self) -> bool {
        self.inner.needs_login.load(Ordering::Relaxed)
    }

    pub async fn access_token(&self) -> AppResult<String> {
        let mut guard = self.inner.tokens.lock().await;
        if self.inner.needs_login.load(Ordering::Relaxed) {
            return Err(AppError::auth_required("Spotify login required."));
        }
        let Some(tokens) = guard.as_ref() else {
            return Err(AppError::auth_required("Spotify login required."));
        };
        if !tokens.expired_or_soon() {
            return Ok(tokens.access().to_string());
        }
        self.refresh_locked(&mut guard, false).await?;
        guard
            .as_ref()
            .map(|t| t.access().to_string())
            .ok_or_else(|| AppError::auth_required("Spotify login required."))
    }

    pub async fn force_refresh(&self) -> AppResult<String> {
        let mut guard = self.inner.tokens.lock().await;
        self.refresh_locked(&mut guard, true).await?;
        guard
            .as_ref()
            .map(|t| t.access().to_string())
            .ok_or_else(|| AppError::auth_required("Spotify login required."))
    }

    async fn refresh_locked(&self, guard: &mut Option<Tokens>, force: bool) -> AppResult<()> {
        let Some(current) = guard.as_ref() else {
            return Err(AppError::auth_required("Spotify login required."));
        };
        if !force && !current.expired_or_soon() {
            return Ok(());
        }
        let refresh_token = current.refresh().to_string();
        let client_id = self
            .inner
            .client_id
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        if refresh_token.is_empty() {
            self.inner.needs_login.store(true, Ordering::Relaxed);
            return Err(AppError::auth_required("Spotify login required."));
        }

        let response = self
            .inner
            .http
            .post(&self.inner.token_url)
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh_token.as_str()),
                ("client_id", client_id.as_str()),
            ])
            .send()
            .await
            .map_err(|e| AppError::offline(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            if body.contains("invalid_grant") {
                self.inner.needs_login.store(true, Ordering::Relaxed);
                return Err(AppError::auth_required("Spotify login is no longer valid."));
            }
            return Err(AppError::internal(format!(
                "Token refresh failed with {status}"
            )));
        }

        let refreshed: TokenResponse = response
            .json()
            .await
            .map_err(|e| AppError::internal(format!("bad token response: {e}")))?;

        let refresh = refreshed
            .refresh_token
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| refresh_token.clone());

        let new_tokens = Tokens {
            access_token: Secret::new(refreshed.access_token),
            refresh_token: Secret::new(refresh),
            expires_at: chrono::Utc::now().timestamp() + refreshed.expires_in,
        };
        persist(&self.inner.path, &new_tokens)?;
        *guard = Some(new_tokens);
        self.inner.needs_login.store(false, Ordering::Relaxed);
        Ok(())
    }

    pub async fn store(&self, tokens: Tokens) -> AppResult<()> {
        let mut guard = self.inner.tokens.lock().await;
        persist(&self.inner.path, &tokens)?;
        *guard = Some(tokens);
        self.inner.needs_login.store(false, Ordering::Relaxed);
        Ok(())
    }

    pub fn set_client_id(&self, client_id: impl Into<String>) {
        if let Ok(mut guard) = self.inner.client_id.write() {
            *guard = client_id.into();
        }
    }

    pub async fn logout(&self) -> AppResult<()> {
        let mut guard = self.inner.tokens.lock().await;
        *guard = None;
        self.inner.needs_login.store(false, Ordering::Relaxed);
        match std::fs::remove_file(&self.inner.path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.into()),
        }
    }
}

pub async fn exchange_code(
    http: &reqwest::Client,
    token_url: &str,
    client_id: &str,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
) -> AppResult<Tokens> {
    let response = http
        .post(token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", client_id),
            ("code_verifier", verifier),
        ])
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::auth_required(format!(
            "Spotify code exchange failed ({status}): {}",
            body.chars().take(200).collect::<String>()
        )));
    }
    let parsed: TokenResponse = response
        .json()
        .await
        .map_err(|e| AppError::internal(format!("bad token response: {e}")))?;
    let Some(refresh) = parsed.refresh_token.filter(|r| !r.is_empty()) else {
        return Err(AppError::auth_required(
            "Spotify did not return a refresh token.",
        ));
    };
    Ok(Tokens {
        access_token: Secret::new(parsed.access_token),
        refresh_token: Secret::new(refresh),
        expires_at: chrono::Utc::now().timestamp() + parsed.expires_in,
    })
}

fn load_tokens(path: &PathBuf) -> Option<Tokens> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn persist(path: &PathBuf, tokens: &Tokens) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_vec_pretty(tokens)?;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "tokens.json".to_string());
    let tmp = path.with_file_name(format!("{file_name}.{}.tmp", rand::random::<u64>()));
    {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        file.write_all(&json)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn expired_tokens(refresh: &str) -> Tokens {
        Tokens {
            access_token: Secret::new("old-access"),
            refresh_token: Secret::new(refresh),
            expires_at: chrono::Utc::now().timestamp() - 100,
        }
    }

    fn manager(server: &MockServer, dir: &std::path::Path, tokens: Tokens) -> TokenManager {
        let path = dir.join("tokens.json");
        persist(&path, &tokens).unwrap();
        TokenManager::new(
            path,
            reqwest::Client::new(),
            "client-id".to_string(),
            format!("{}/api/token", server.uri()),
        )
    }

    #[tokio::test]
    async fn refresh_rotates_and_persists_before_use() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new-access",
                "expires_in": 3600,
                "refresh_token": "rotated"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let manager = manager(&server, dir.path(), expired_tokens("old-refresh"));
        assert_eq!(manager.access_token().await.unwrap(), "new-access");

        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join("tokens.json")).unwrap())
                .unwrap();
        assert_eq!(saved["refresh_token"], "rotated");
        assert_eq!(saved["access_token"], "new-access");
    }

    #[tokio::test]
    async fn missing_rotation_keeps_old_refresh_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "new-access",
                "expires_in": 3600
            })))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let manager = manager(&server, dir.path(), expired_tokens("keep-me"));
        manager.access_token().await.unwrap();
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.path().join("tokens.json")).unwrap())
                .unwrap();
        assert_eq!(saved["refresh_token"], "keep-me");
    }

    #[tokio::test]
    async fn invalid_grant_requires_login() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/token"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "invalid_grant"
            })))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let manager = manager(&server, dir.path(), expired_tokens("revoked"));
        let err = manager.access_token().await.unwrap_err();
        assert_eq!(err.code(), crate::error::AUTH_REQUIRED);
        assert!(manager.needs_login());
    }

    #[tokio::test]
    async fn ten_concurrent_callers_refresh_once() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "fresh",
                "expires_in": 3600,
                "refresh_token": "rotated"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let manager = manager(&server, dir.path(), expired_tokens("old"));
        let mut handles = Vec::new();
        for _ in 0..10 {
            let clone = manager.clone();
            handles.push(tokio::spawn(
                async move { clone.access_token().await.unwrap() },
            ));
        }
        for handle in handles {
            assert_eq!(handle.await.unwrap(), "fresh");
        }
        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
    }

    #[test]
    fn token_file_is_private() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tokens.json");
        persist(&path, &expired_tokens("r")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }
}
