use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::admin::AdminSession;
use crate::config::{self, Config};
use crate::cover::{CoverCache, ITUNES_BASE};
use crate::error::{AppError, AppResult};
use crate::journal::Journal;
use crate::outbox::OutboxHandle;
use crate::paths::AppPaths;
use crate::print::PrinterBackend;
use crate::render::PreviewPaths;
use crate::spotify::api::SpotifyApi;
use crate::spotify::token::TokenManager;
use crate::spotify::{API_BASE, TOKEN_URL};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([redacted])")
    }
}

pub struct Prepared {
    pub prep_id: String,
    pub title: String,
    pub artist: String,
    pub track_uri: Option<String>,
    pub mode: String,
    pub cover_source: String,
    pub cover_px: u32,
    pub previews: PreviewPaths,
    pub created: Instant,
}

pub struct AppState {
    pub app: AppHandle,
    pub paths: AppPaths,
    pub config: RwLock<Config>,
    config_lock: Mutex<()>,
    pub http: reqwest::Client,
    pub tokens: TokenManager,
    pub journal: Journal,
    pub printer: PrinterBackend,
    pub admin: AdminSession,
    pub maintenance: AtomicBool,
    pub exit_allowed: AtomicBool,
    pub prepared: Mutex<HashMap<String, Prepared>>,
    pub network_ok: Arc<AtomicBool>,
    pub outbox: OutboxHandle,
    pub outbox_paused_until: Mutex<Option<Instant>>,
    pub mock_busy_until: Mutex<Option<Instant>>,
    pub cover_cache: CoverCache,
    pub api_base: String,
    pub token_url: String,
}

impl AppState {
    pub fn new(app: AppHandle, paths: AppPaths, config: Config) -> AppResult<Self> {
        let http = reqwest::Client::builder()
            .user_agent("soundtrack-kiosk/0.1")
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| AppError::internal(format!("could not build HTTP client: {e}")))?;
        let tokens = TokenManager::new(
            paths.tokens(),
            http.clone(),
            config.spotify_client_id.clone(),
            TOKEN_URL.to_string(),
        );
        let journal = Journal::open(paths.journal())?;
        let cover_cache = CoverCache::new(paths.covers_dir(), http.clone(), ITUNES_BASE);
        Ok(Self {
            app,
            paths,
            config: RwLock::new(config),
            config_lock: Mutex::new(()),
            http,
            tokens,
            journal,
            printer: default_printer(),
            admin: AdminSession::new(),
            maintenance: AtomicBool::new(false),
            exit_allowed: AtomicBool::new(false),
            prepared: Mutex::new(HashMap::new()),
            network_ok: Arc::new(AtomicBool::new(true)),
            outbox: OutboxHandle::new(),
            outbox_paused_until: Mutex::new(None),
            mock_busy_until: Mutex::new(None),
            cover_cache,
            api_base: API_BASE.to_string(),
            token_url: TOKEN_URL.to_string(),
        })
    }

    pub fn config(&self) -> Config {
        self.config
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub fn update_config(&self, mutate: impl FnOnce(&mut Config)) -> AppResult<Config> {
        let _guard = self
            .config_lock
            .lock()
            .map_err(|_| AppError::internal("settings lock poisoned"))?;
        let mut config = self.config();
        mutate(&mut config);
        config::save_config(&self.app, &config)?;
        if let Ok(mut guard) = self.config.write() {
            *guard = config.clone();
        }
        self.tokens.set_client_id(config.spotify_client_id.clone());
        Ok(config)
    }

    pub fn api(&self) -> SpotifyApi {
        SpotifyApi::new(
            self.http.clone(),
            self.tokens.clone(),
            self.api_base.clone(),
            self.network_ok.clone(),
        )
    }

    pub fn is_maintenance(&self) -> bool {
        self.maintenance.load(Ordering::Relaxed)
    }

    pub fn set_maintenance(&self, on: bool) {
        self.maintenance.store(on, Ordering::Relaxed);
        crate::kiosk::set_maintenance(&self.app, on);
    }

    pub fn exit_allowed(&self) -> bool {
        self.exit_allowed.load(Ordering::Relaxed)
    }

    pub fn allow_exit(&self) {
        self.exit_allowed.store(true, Ordering::Relaxed);
    }

    pub fn require_admin(&self) -> AppResult<()> {
        self.admin.require()
    }

    pub fn outbox_pause(&self, duration: Duration) {
        if let Ok(mut guard) = self.outbox_paused_until.lock() {
            *guard = Some(Instant::now() + duration);
        }
    }

    pub fn outbox_paused(&self) -> bool {
        self.outbox_paused_until
            .lock()
            .map(|guard| guard.is_some_and(|until| until > Instant::now()))
            .unwrap_or(false)
    }

    pub fn clear_outbox_pause(&self) {
        if let Ok(mut guard) = self.outbox_paused_until.lock() {
            *guard = None;
        }
    }

    pub fn mark_mock_busy(&self) {
        if let Ok(mut guard) = self.mock_busy_until.lock() {
            *guard = Some(Instant::now() + Duration::from_secs(45));
        }
    }

    pub fn mock_busy(&self) -> bool {
        self.mock_busy_until
            .lock()
            .map(|guard| guard.is_some_and(|until| until > Instant::now()))
            .unwrap_or(false)
    }

    pub fn insert_prepared(&self, prepared: Prepared) {
        let mut expired = Vec::new();
        if let Ok(mut guard) = self.prepared.lock() {
            guard.retain(|_, value| {
                if value.created.elapsed() >= Duration::from_secs(600) {
                    expired.push(value.previews.clone());
                    false
                } else {
                    true
                }
            });
            let _ = guard.insert(prepared.prep_id.clone(), prepared);
        }
        for previews in expired {
            remove_preview_files(&previews);
        }
    }

    pub fn clear_prepared(&self) {
        if let Ok(mut guard) = self.prepared.lock() {
            guard.clear();
        }
    }

    pub fn take_prepared(&self, prep_id: &str) -> Option<Prepared> {
        self.prepared
            .lock()
            .ok()
            .and_then(|mut guard| guard.remove(prep_id))
    }
}

fn remove_preview_files(previews: &PreviewPaths) {
    for path in [
        &previews.portrait_cover,
        &previews.portrait_fit,
        &previews.landscape_cover,
        &previews.landscape_fit,
    ] {
        let _ = std::fs::remove_file(path);
    }
}

fn default_printer() -> PrinterBackend {
    match std::env::var("BABY_KIOSK_PRINTER").ok().as_deref() {
        Some("mock") => PrinterBackend::Mock,
        Some("cups") => PrinterBackend::Cups,
        _ => {
            if cfg!(debug_assertions) {
                PrinterBackend::Mock
            } else {
                PrinterBackend::Cups
            }
        }
    }
}
