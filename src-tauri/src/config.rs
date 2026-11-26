use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    #[default]
    Portrait,
    Landscape,
}

impl Orientation {
    pub fn as_str(self) -> &'static str {
        match self {
            Orientation::Portrait => "portrait",
            Orientation::Landscape => "landscape",
        }
    }
}

pub const DEFAULT_BABY_NAME: &str = "baby";
pub const DEFAULT_WELCOME_LINE: &str = "A song for {babyName}";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub baby_name: String,
    pub welcome_line: String,
    pub playlist_url: String,
    pub market: String,
    pub country: String,
    pub default_orientation: Orientation,
    pub printer: Option<String>,
    pub pin: String,
    pub spotify_client_id: String,
    pub paper_count: u32,
    pub ink_count: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            baby_name: DEFAULT_BABY_NAME.to_string(),
            welcome_line: DEFAULT_WELCOME_LINE.to_string(),
            playlist_url: String::new(),
            market: "US".to_string(),
            country: "US".to_string(),
            default_orientation: Orientation::Portrait,
            printer: None,
            pin: "0000".to_string(),
            spotify_client_id: String::new(),
            paper_count: 0,
            ink_count: 0,
        }
    }
}

impl Config {
    /// Extract a Spotify playlist id from a URL or a raw id.
    pub fn playlist_id(&self) -> Option<String> {
        parse_playlist_id(&self.playlist_url)
    }

    pub fn welcome(&self) -> String {
        self.welcome_line
            .replace("{babyName}", &self.baby_name)
            .replace("{name}", &self.baby_name)
    }
}

pub fn parse_playlist_id(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let candidate: String = if let Some(rest) = trimmed.strip_prefix("spotify:playlist:") {
        rest.split('?').next().unwrap_or(rest).to_string()
    } else if let Ok(url) = url::Url::parse(trimmed) {
        let mut segments = url.path_segments()?;
        let mut found = None;
        while let Some(segment) = segments.next() {
            if segment == "playlist" {
                found = segments.next();
                break;
            }
        }
        found.unwrap_or("").to_string()
    } else {
        trimmed.to_string()
    };
    let id: String = candidate
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    if id.len() >= 10 {
        Some(id)
    } else {
        None
    }
}

pub const STORE_FILE: &str = "settings.json";
pub const STORE_KEY: &str = "config";

pub fn validate_pin(pin: &str) -> AppResult<()> {
    let valid =
        (4..=12).contains(&pin.len()) && pin.chars().all(|c| c.is_ascii_digit()) && pin != "0000";
    if valid {
        Ok(())
    } else {
        Err(AppError::invalid_input(
            "Choose a 4-12 digit PIN other than 0000.",
        ))
    }
}

pub fn load_config<R: tauri::Runtime>(app: &AppHandle<R>) -> AppResult<Config> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| AppError::internal(format!("could not open settings: {e}")))?;
    let Some(value) = store.get(STORE_KEY) else {
        secure_settings(app);
        return Ok(Config::default());
    };
    match serde_json::from_value(value) {
        Ok(config) => {
            secure_settings(app);
            Ok(config)
        }
        Err(err) => {
            let backup = backup_settings(app);
            log::warn!(
                "settings.json could not be read ({err}); starting from defaults. Backup: {backup:?}"
            );
            secure_settings(app);
            Ok(Config::default())
        }
    }
}

fn backup_settings<R: tauri::Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    let dir = app.path().app_data_dir().ok()?;
    let path = dir.join(STORE_FILE);
    if !path.exists() {
        return None;
    }
    let backup = dir.join(format!(
        "settings.json.corrupt-{}",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    std::fs::rename(&path, &backup).ok()?;
    Some(backup)
}

pub fn save_config<R: tauri::Runtime>(app: &AppHandle<R>, config: &Config) -> AppResult<()> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| AppError::internal(format!("could not open settings: {e}")))?;
    store.set(STORE_KEY, serde_json::to_value(config)?);
    store
        .save()
        .map_err(|e| AppError::internal(format!("could not save settings: {e}")))?;
    secure_settings(app);
    Ok(())
}

fn secure_settings<R: tauri::Runtime>(app: &AppHandle<R>) {
    let Ok(dir) = app.path().app_data_dir() else {
        return;
    };
    let path = dir.join(STORE_FILE);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = &path;
    }
}

pub const PAPER_LIMIT: u32 = 16;
pub const INK_LIMIT: u32 = 34;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_admin_pin() {
        assert!(validate_pin("2468").is_ok());
        assert!(validate_pin("0000").is_err());
        assert!(validate_pin("123").is_err());
        assert!(validate_pin("abcdef").is_err());
        assert!(validate_pin("1234567890123").is_err());
    }

    #[test]
    fn parses_playlist_urls() {
        assert_eq!(
            parse_playlist_id("https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M?si=abc")
                .as_deref(),
            Some("37i9dQZF1DXcBWIGoYBM5M")
        );
        assert_eq!(
            parse_playlist_id("spotify:playlist:37i9dQZF1DXcBWIGoYBM5M").as_deref(),
            Some("37i9dQZF1DXcBWIGoYBM5M")
        );
        assert_eq!(
            parse_playlist_id("37i9dQZF1DXcBWIGoYBM5M").as_deref(),
            Some("37i9dQZF1DXcBWIGoYBM5M")
        );
        assert!(parse_playlist_id("https://open.spotify.com/album/abc").is_none());
        assert!(parse_playlist_id("").is_none());
    }
}
