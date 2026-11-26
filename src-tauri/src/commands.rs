use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::config::{self, Config, Orientation};
use crate::cover::{self, matching, CoverCache};
use crate::error::{AppError, AppResult};
use crate::health::{self, Health};
use crate::journal::{JournalLine, PrintState, Selection};
use crate::print::{self, PrintOutcome, PrinterList};
use crate::render::{self, PreviewPaths, PrintLayout};
use crate::spotify::api::{PlaylistDto, SearchResponse, TrackDto};
use crate::spotify::auth::{self, LoginEndpoints};
use crate::spotify::AUTHORIZE_URL;
use crate::state::{AppState, Prepared};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareResult {
    pub prep_id: String,
    pub previews: PreviewPaths,
    pub cover_source: String,
    pub cover_px: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum SelectionInput {
    Track { track: TrackDto },
    Manual { title: String, artist: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitResult {
    pub prints_ahead: u32,
    pub print_queued: bool,
    pub print_file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicConfig {
    pub baby_name: String,
    pub welcome: String,
    pub default_orientation: Orientation,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentSelection {
    pub id: String,
    pub guest_name: String,
    pub track_title: String,
    pub artist: String,
    pub timestamp: String,
    pub orientation: String,
    pub layout: String,
    pub mode: String,
    pub track_uri: Option<String>,
    pub print_state: String,
    pub print_file: Option<String>,
    pub print_reason: Option<String>,
    pub playlist_skipped: bool,
    pub playlist_skip_reason: Option<String>,
}

impl From<&Selection> for RecentSelection {
    fn from(selection: &Selection) -> Self {
        Self {
            id: selection.id.clone(),
            guest_name: selection.guest_name.clone(),
            track_title: selection.track_title.clone(),
            artist: selection.artist.clone(),
            timestamp: selection.timestamp.clone(),
            orientation: selection.orientation.clone(),
            layout: selection.layout.clone(),
            mode: selection.mode.clone(),
            track_uri: selection.track_uri.clone(),
            print_state: match selection.print_state {
                PrintState::Pending => "pending",
                PrintState::Sent => "sent",
                PrintState::Failed => "failed",
            }
            .to_string(),
            print_file: selection.print_file.clone(),
            print_reason: selection.print_reason.clone(),
            playlist_skipped: selection.playlist_skipped,
            playlist_skip_reason: selection.playlist_skip_reason.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminStatus {
    pub health: Health,
    pub config: Config,
    pub print_backend: String,
    pub maintenance: bool,
    pub spotify_redirect_uri: String,
    pub recent: Vec<RecentSelection>,
    pub unprinted: Vec<RecentSelection>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearHistoryResult {
    pub archived: u32,
    pub pending_archived: u32,
    pub archive_path: Option<String>,
}

#[tauri::command]
pub fn clear_history(state: State<'_, AppState>) -> AppResult<ClearHistoryResult> {
    state.require_admin()?;
    let selections = state.journal.load()?;
    let archived = selections.len() as u32;
    let pending_archived = selections
        .iter()
        .filter(|selection| !selection.playlist_added && !selection.playlist_skipped)
        .count() as u32;
    let archive = state.journal.clear()?;
    state.clear_prepared();
    let _ = state.paths.purge_previews();
    state.clear_outbox_pause();
    state.outbox.notify();
    Ok(ClearHistoryResult {
        archived,
        pending_archived,
        archive_path: archive.map(|path| path.to_string_lossy().to_string()),
    })
}

#[tauri::command]
pub fn ping() -> &'static str {
    "pong"
}

#[tauri::command]
pub async fn search_tracks(state: State<'_, AppState>, query: String) -> AppResult<SearchResponse> {
    let config = state.config();
    let api = state.api();
    api.search_tracks(&query, &config.market).await
}

#[tauri::command]
pub async fn prefetch_cover(state: State<'_, AppState>, track: TrackDto) -> AppResult<()> {
    let config = state.config();
    let key = CoverCache::key_for(track.album_id.as_deref(), &track.id);
    let artist = track.artists.join(", ");
    let _ = state
        .cover_cache
        .resolve(
            &key,
            &track.title,
            &artist,
            Some(&track.album),
            track.image640.as_deref(),
            &config.country,
        )
        .await;
    Ok(())
}

#[tauri::command]
pub async fn prepare_selection(
    state: State<'_, AppState>,
    input: SelectionInput,
) -> AppResult<PrepareResult> {
    let (title, artist, album, key, track_uri, image640, mode) = match input {
        SelectionInput::Track { track } => {
            let artist = track.artists.join(", ");
            let title = clean_field(&track.title, "song title")?;
            let artist = clean_field(&artist, "artist")?;
            let key = CoverCache::key_for(track.album_id.as_deref(), &track.id);
            (
                title,
                artist,
                Some(track.album),
                key,
                Some(track.uri),
                track.image640,
                "spotify",
            )
        }
        SelectionInput::Manual { title, artist } => {
            let title = clean_field(&title, "song title")?;
            let artist = clean_field(&artist, "artist")?;
            let key = CoverCache::manual_key(&artist, &title);
            (title, artist, None, key, None, None, "manual")
        }
    };

    let config = state.config();
    let cover = state
        .cover_cache
        .resolve(
            &key,
            &title,
            &artist,
            album.as_deref(),
            image640.as_deref(),
            &config.country,
        )
        .await?;
    let cover_source = cover.source.clone();
    let cover_px = cover.px;
    let prep_id = new_id();
    let previews =
        render::render_previews(&prep_id, cover.bytes, &state.paths.previews_dir()).await?;

    state.insert_prepared(Prepared {
        prep_id: prep_id.clone(),
        title,
        artist,
        track_uri,
        mode: mode.to_string(),
        cover_source: cover_source.clone(),
        cover_px,
        previews: previews.clone(),
        created: std::time::Instant::now(),
    });

    Ok(PrepareResult {
        prep_id,
        previews,
        cover_source,
        cover_px,
    })
}

#[tauri::command]
pub async fn discard_preparation(state: State<'_, AppState>, prep_id: String) -> AppResult<()> {
    if let Some(prepared) = state.take_prepared(&prep_id) {
        remove_previews(&prepared.previews).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn commit_selection(
    state: State<'_, AppState>,
    id: String,
    prep_id: String,
    guest_name: String,
    orientation: Orientation,
    layout: PrintLayout,
) -> AppResult<CommitResult> {
    let guest_name = clean_guest_name(&guest_name)?;
    let id = clean_id(&id)?;
    if prep_id.trim().is_empty() {
        return Err(AppError::invalid_input("Missing preview id."));
    }
    let config = state.config();
    let selections = state.journal.load()?;

    if let Some(existing) = selections.iter().find(|selection| selection.id == id) {
        let prints_ahead = print::waiting(state.printer, config.printer.as_deref()).await;
        return Ok(CommitResult {
            prints_ahead,
            print_queued: existing.print_state == PrintState::Sent,
            print_file: existing.print_file.clone(),
        });
    }

    let prepared = state.take_prepared(&prep_id).ok_or_else(|| {
        AppError::invalid_input("This print expired. Please choose the song again.")
    })?;

    let preview_path = match (orientation, layout) {
        (Orientation::Portrait, PrintLayout::Cover) => {
            PathBuf::from(&prepared.previews.portrait_cover)
        }
        (Orientation::Portrait, PrintLayout::Fit) => PathBuf::from(&prepared.previews.portrait_fit),
        (Orientation::Landscape, PrintLayout::Cover) => {
            PathBuf::from(&prepared.previews.landscape_cover)
        }
        (Orientation::Landscape, PrintLayout::Fit) => {
            PathBuf::from(&prepared.previews.landscape_fit)
        }
    };
    let prints_dir = state.paths.ensure_prints_dir()?;
    let print_file = prints_dir.join(format!(
        "{id}-{}-{}.png",
        orientation.as_str(),
        layout.as_str()
    ));

    if let Err(err) = tokio::fs::copy(&preview_path, &print_file).await {
        state.insert_prepared(prepared);
        return Err(AppError::print_failed(format!(
            "Could not stage the print: {err}"
        )));
    }

    if let Err(err) = state
        .journal
        .append_synced(&JournalLine::selection(
            &id,
            &guest_name,
            &prepared.title,
            &prepared.artist,
            prepared.track_uri.as_deref(),
            orientation.as_str(),
            layout.as_str(),
            &prepared.cover_source,
            prepared.cover_px,
            &prepared.mode,
        ))
        .await
    {
        state.insert_prepared(prepared);
        return Err(err);
    }

    let duplicate = is_duplicate_selection(
        &selections,
        &id,
        &prepared.title,
        &prepared.artist,
        prepared.track_uri.as_deref(),
    );
    if duplicate {
        state
            .journal
            .append_synced(&JournalLine::playlist_skipped(&id, "duplicate"))
            .await?;
    }

    remove_previews(&prepared.previews).await;

    let prints_ahead = print::waiting(state.printer, config.printer.as_deref()).await;
    let outcome = print::submit(
        state.printer,
        config.printer.as_deref(),
        &print_file,
        orientation.as_str(),
        &state.paths.mock_prints_dir(),
    )
    .await;

    let staged_file = print_file.to_string_lossy().to_string();
    let (print_queued, returned_file) = match outcome {
        Ok(PrintOutcome { job, file: _ }) => {
            state
                .journal
                .append_synced(&JournalLine::print_sent(&id, &staged_file, &job))
                .await?;
            if state.printer == print::PrinterBackend::Mock {
                state.mark_mock_busy();
            }
            increment_refill_counters(&state);
            (true, Some(staged_file))
        }
        Err(err) => {
            state
                .journal
                .append_synced(&JournalLine::print_failed(&id, &err.message))
                .await?;
            (false, Some(staged_file))
        }
    };

    state.outbox.notify();
    Ok(CommitResult {
        prints_ahead,
        print_queued,
        print_file: returned_file,
    })
}

#[tauri::command]
pub fn admin_unlock(state: State<'_, AppState>, pin: String) -> AppResult<()> {
    let config = state.config();
    state.admin.attempt(pin == config.pin)
}

#[tauri::command]
pub fn admin_lock(state: State<'_, AppState>) -> AppResult<()> {
    state.admin.lock();
    Ok(())
}

#[tauri::command]
pub async fn admin_status(state: State<'_, AppState>) -> AppResult<AdminStatus> {
    state.require_admin()?;
    let selections = state.journal.load()?;
    let health = health::snapshot_with(&state, &selections).await;
    let config = state.config();
    let recent = selections
        .iter()
        .rev()
        .take(30)
        .map(RecentSelection::from)
        .collect();
    let unprinted = selections
        .iter()
        .filter(|selection| selection.print_state != PrintState::Sent)
        .map(RecentSelection::from)
        .collect();
    Ok(AdminStatus {
        health,
        config,
        print_backend: state.printer.label().to_string(),
        maintenance: state.is_maintenance(),
        spotify_redirect_uri: auth::callback_redirect_uri(),
        recent,
        unprinted,
    })
}

#[tauri::command]
pub fn get_public_config(state: State<'_, AppState>) -> AppResult<PublicConfig> {
    let config = state.config();
    Ok(PublicConfig {
        baby_name: config.baby_name.clone(),
        welcome: config.welcome(),
        default_orientation: config.default_orientation,
    })
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> AppResult<Config> {
    state.require_admin()?;
    Ok(state.config())
}

#[tauri::command]
pub async fn set_config(state: State<'_, AppState>, config: Config) -> AppResult<Config> {
    state.require_admin()?;
    config::validate_pin(&config.pin)?;
    if let Some(id) = config.playlist_id() {
        if state.tokens.is_authenticated() {
            let api = state.api();
            match api.check_owner(&id).await {
                Ok(true) => {}
                Ok(false) => {
                    return Err(AppError::invalid_input(
                        "That playlist is not owned by the logged-in Spotify account.",
                    ))
                }
                Err(err) if err.code() == crate::error::AUTH_REQUIRED => {}
                Err(err) if err.code() == crate::error::OFFLINE => {}
                Err(err) => return Err(err),
            }
        }
    } else if !config.playlist_url.trim().is_empty() {
        return Err(AppError::invalid_input("That playlist URL is not valid."));
    }
    let saved = state.update_config(|current| {
        let paper = current.paper_count;
        let ink = current.ink_count;
        *current = config.clone();
        current.paper_count = paper;
        current.ink_count = ink;
    })?;
    state.outbox.notify();
    Ok(saved)
}

#[tauri::command]
pub async fn auth_start(state: State<'_, AppState>) -> AppResult<()> {
    state.require_admin()?;
    let config = state.config();
    if config.spotify_client_id.trim().is_empty() {
        return Err(AppError::invalid_input("Set a Spotify client id first."));
    }
    let endpoints = LoginEndpoints {
        authorize_base: AUTHORIZE_URL.to_string(),
        token_url: state.token_url.clone(),
    };
    state.set_maintenance(true);
    let result = auth::run_login(
        &state.app,
        &state.http,
        &config.spotify_client_id,
        &endpoints,
        &state.tokens,
    )
    .await;
    state.set_maintenance(false);
    if result.is_ok() {
        state.clear_outbox_pause();
        state.outbox.notify();
    }
    result
}

#[tauri::command]
pub async fn auth_logout(state: State<'_, AppState>) -> AppResult<()> {
    state.require_admin()?;
    state.tokens.logout().await
}

#[tauri::command]
pub async fn list_spotify_playlists(state: State<'_, AppState>) -> AppResult<Vec<PlaylistDto>> {
    state.require_admin()?;
    if !state.tokens.is_authenticated() {
        return Err(AppError::auth_required("Connect Spotify first."));
    }
    state.api().list_playlists().await
}

#[tauri::command]
pub async fn create_spotify_playlist(
    state: State<'_, AppState>,
    name: String,
) -> AppResult<PlaylistDto> {
    state.require_admin()?;
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::invalid_input("Give the new playlist a name."));
    }
    if name.chars().count() > 100 {
        return Err(AppError::invalid_input(
            "Playlist names are limited to 100 characters.",
        ));
    }
    if !state.tokens.is_authenticated() {
        return Err(AppError::auth_required("Connect Spotify first."));
    }
    state.api().create_playlist(name).await
}

#[tauri::command]
pub async fn list_printers(state: State<'_, AppState>) -> AppResult<PrinterList> {
    state.require_admin()?;
    Ok(print::list_printers(state.printer).await)
}

#[tauri::command]
pub async fn test_print(state: State<'_, AppState>) -> AppResult<PrintOutcome> {
    state.require_admin()?;
    let config = state.config();
    let id = format!("test-{}", new_id());
    let preview = state.paths.previews_dir().join(format!("{id}.png"));
    render::save_bytes_as_png(&cover::placeholder().bytes, &preview)?;
    let result = print::submit(
        state.printer,
        config.printer.as_deref(),
        &preview,
        "portrait",
        &state.paths.mock_prints_dir(),
    )
    .await;
    let _ = tokio::fs::remove_file(&preview).await;
    let outcome = result?;
    if state.printer == print::PrinterBackend::Mock {
        state.mark_mock_busy();
    }
    Ok(outcome)
}

#[tauri::command]
pub fn list_recent(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> AppResult<Vec<RecentSelection>> {
    state.require_admin()?;
    let limit = limit.unwrap_or(30).clamp(1, 200) as usize;
    let selections = state.journal.load()?;
    Ok(selections
        .iter()
        .rev()
        .take(limit)
        .map(RecentSelection::from)
        .collect())
}

#[tauri::command]
pub async fn reprint(state: State<'_, AppState>, id: String) -> AppResult<CommitResult> {
    state.require_admin()?;
    let config = state.config();
    let selection = state
        .journal
        .find(&id)?
        .ok_or_else(|| AppError::not_found("Selection not found."))?;
    let file = match selection.print_file.clone() {
        Some(path) if Path::new(&path).exists() => PathBuf::from(path),
        _ => find_print_file(&state.paths.prints_root(), &id)
            .ok_or_else(|| AppError::not_found("No print file for this selection."))?,
    };
    let prints_ahead = print::waiting(state.printer, config.printer.as_deref()).await;
    let outcome = print::submit(
        state.printer,
        config.printer.as_deref(),
        &file,
        &selection.orientation,
        &state.paths.mock_prints_dir(),
    )
    .await;
    match outcome {
        Ok(PrintOutcome { job, file }) => {
            state
                .journal
                .append_synced(&JournalLine::print_sent(&id, &file, &job))
                .await?;
            if state.printer == print::PrinterBackend::Mock {
                state.mark_mock_busy();
            }
            increment_refill_counters(&state);
            Ok(CommitResult {
                prints_ahead,
                print_queued: true,
                print_file: Some(file),
            })
        }
        Err(err) => {
            state
                .journal
                .append_synced(&JournalLine::print_failed(&id, &err.message))
                .await?;
            Ok(CommitResult {
                prints_ahead,
                print_queued: false,
                print_file: Some(file.to_string_lossy().to_string()),
            })
        }
    }
}

#[tauri::command]
pub fn export_csv(state: State<'_, AppState>) -> AppResult<String> {
    state.require_admin()?;
    let selections = state.journal.load()?;
    let mut csv = String::from(
        "timestamp,guest_name,track_title,artist,track_uri,orientation,layout,mode,cover_source,cover_px,playlist_added,print_state,print_file,print_job,print_reason,playlist_skipped,playlist_skip_reason\n",
    );
    for selection in &selections {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            escape_csv(&selection.timestamp),
            escape_csv(&selection.guest_name),
            escape_csv(&selection.track_title),
            escape_csv(&selection.artist),
            escape_csv(selection.track_uri.as_deref().unwrap_or("")),
            escape_csv(&selection.orientation),
            escape_csv(&selection.layout),
            escape_csv(&selection.mode),
            escape_csv(&selection.cover_source),
            selection.cover_px,
            selection.playlist_added,
            match selection.print_state {
                PrintState::Pending => "pending",
                PrintState::Sent => "sent",
                PrintState::Failed => "failed",
            },
            escape_csv(selection.print_file.as_deref().unwrap_or("")),
            escape_csv(selection.print_job.as_deref().unwrap_or("")),
            escape_csv(selection.print_reason.as_deref().unwrap_or("")),
            selection.playlist_skipped,
            escape_csv(selection.playlist_skip_reason.as_deref().unwrap_or("")),
        ));
    }
    let target = desktop_dir(&state.paths)
        .unwrap_or_else(|| state.paths.root().to_path_buf())
        .join(format!(
            "soundtrack-{}.csv",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        ));
    std::fs::write(&target, csv)?;
    Ok(target.to_string_lossy().to_string())
}

#[tauri::command]
pub fn mark_refilled(state: State<'_, AppState>, kind: String) -> AppResult<Config> {
    state.require_admin()?;
    if kind != "paper" && kind != "ink" {
        return Err(AppError::invalid_input("kind must be paper or ink"));
    }
    state.update_config(|config| {
        if kind == "paper" {
            config.paper_count = 0;
        } else {
            config.ink_count = 0;
        }
    })
}

#[tauri::command]
pub fn reset_printer_counters(state: State<'_, AppState>) -> AppResult<Config> {
    state.require_admin()?;
    state.update_config(|config| {
        config.paper_count = 0;
        config.ink_count = 0;
    })
}

#[tauri::command]
pub fn set_maintenance(state: State<'_, AppState>, enabled: bool) -> AppResult<()> {
    state.require_admin()?;
    state.set_maintenance(enabled);
    Ok(())
}

#[tauri::command]
pub fn exit_kiosk(state: State<'_, AppState>, app: AppHandle) -> AppResult<()> {
    state.require_admin()?;
    state.allow_exit();
    app.exit(0);
    Ok(())
}

fn increment_refill_counters(state: &AppState) {
    if let Err(err) = state.update_config(|config| {
        config.paper_count = config.paper_count.saturating_add(1);
        config.ink_count = config.ink_count.saturating_add(1);
    }) {
        log::error!("could not persist refill counters: {err}");
    }
}

fn find_print_file(root: &Path, id: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(root).ok()?;
    let mut best: Option<PathBuf> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Ok(children) = std::fs::read_dir(&path) {
                for child in children.flatten() {
                    let file = child.path();
                    let name = file.file_name().unwrap_or_default().to_string_lossy();
                    if name.starts_with(id) && name.ends_with(".png") {
                        best = Some(file);
                    }
                }
            }
        }
    }
    best
}

fn desktop_dir(paths: &crate::paths::AppPaths) -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let desktop = PathBuf::from(home).join("Desktop");
    if desktop.is_dir() {
        Some(desktop)
    } else {
        Some(paths.root().to_path_buf())
    }
}

fn escape_csv(value: &str) -> String {
    let guarded = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_string()
    };
    if guarded.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", guarded.replace('"', "\"\""))
    } else {
        guarded
    }
}

fn is_duplicate_selection(
    selections: &[Selection],
    id: &str,
    title: &str,
    artist: &str,
    uri: Option<&str>,
) -> bool {
    let identity = matching::song_identity(title, artist);
    selections.iter().any(|other| {
        if other.id == id || other.playlist_skipped {
            return false;
        }
        let same_uri = uri.is_some() && other.track_uri.as_deref() == uri;
        same_uri || matching::song_identity(&other.track_title, &other.artist) == identity
    })
}

fn clean_id(input: &str) -> AppResult<String> {
    let id = input.trim();
    let valid = (8..=64).contains(&id.len())
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_');
    if valid {
        Ok(id.to_string())
    } else {
        Err(AppError::invalid_input("Invalid selection id."))
    }
}

async fn remove_previews(previews: &PreviewPaths) {
    for path in [
        &previews.portrait_cover,
        &previews.portrait_fit,
        &previews.landscape_cover,
        &previews.landscape_fit,
    ] {
        let _ = tokio::fs::remove_file(path).await;
    }
}

fn clean_guest_name(input: &str) -> AppResult<String> {
    let trimmed = input.trim();
    let length = trimmed.chars().count();
    if length == 0 || length > 40 {
        return Err(AppError::invalid_input(
            "Please enter a name between 1 and 40 characters.",
        ));
    }
    Ok(trimmed.to_string())
}

fn clean_field(input: &str, label: &str) -> AppResult<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 200 {
        return Err(AppError::invalid_input(format!(
            "Please enter a valid {label}."
        )));
    }
    Ok(trimmed.to_string())
}

fn new_id() -> String {
    let mut bytes = [0u8; 16];
    use rand::RngCore;
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_escaping() {
        assert_eq!(escape_csv("simple"), "simple");
        assert_eq!(escape_csv("a,b"), "\"a,b\"");
        assert_eq!(escape_csv("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(escape_csv("=SUM(A1)"), "'=SUM(A1)");
        assert_eq!(escape_csv("@cmd"), "'@cmd");
    }

    fn prior(title: &str, artist: &str, uri: Option<&str>) -> Selection {
        Selection {
            id: "prior".to_string(),
            guest_name: "Ada".to_string(),
            track_title: title.to_string(),
            artist: artist.to_string(),
            track_uri: uri.map(ToString::to_string),
            orientation: "portrait".to_string(),
            layout: "cover".to_string(),
            timestamp: "2026-11-25T12:00:00Z".to_string(),
            cover_source: "itunes".to_string(),
            cover_px: 3000,
            mode: "spotify".to_string(),
            playlist_added: false,
            added_uri: None,
            playlist_skipped: false,
            playlist_skip_reason: None,
            print_state: PrintState::Pending,
            print_file: None,
            print_job: None,
            print_reason: None,
        }
    }

    #[test]
    fn detects_duplicate_songs_by_identity_or_uri() {
        assert!(is_duplicate_selection(
            &[prior(
                "Wonderwall - Remastered",
                "OASIS",
                Some("spotify:track:a")
            )],
            "new",
            "wonderwall",
            "Oasis",
            Some("spotify:track:b")
        ));
        assert!(is_duplicate_selection(
            &[prior("Different Title", "Artist", Some("spotify:track:a"))],
            "new",
            "Other",
            "Other",
            Some("spotify:track:a")
        ));
        assert!(!is_duplicate_selection(
            &[prior("Wonderwall", "Oasis", Some("spotify:track:a"))],
            "new",
            "Champagne Supernova",
            "Oasis",
            Some("spotify:track:c")
        ));
    }

    #[test]
    fn skipped_selection_is_not_a_duplicate_anchor() {
        let mut skipped = prior("Wonderwall", "Oasis", Some("spotify:track:a"));
        skipped.playlist_skipped = true;
        assert!(!is_duplicate_selection(
            &[skipped],
            "new",
            "Wonderwall",
            "Oasis",
            Some("spotify:track:b")
        ));
    }

    #[test]
    fn selection_ids_are_sanitized() {
        assert!(clean_id("12345678-abcd").is_ok());
        assert!(clean_id("../../etc/passwd").is_err());
        assert!(clean_id("/tmp/evil").is_err());
        assert!(clean_id("short").is_err());
    }

    #[test]
    fn guest_name_is_trimmed_and_bounded() {
        assert_eq!(clean_guest_name("  Ada  ").unwrap(), "Ada");
        assert!(clean_guest_name("   ").is_err());
        assert!(clean_guest_name(&"a".repeat(41)).is_err());
    }

    #[test]
    fn ids_are_hex_and_unique() {
        let a = new_id();
        assert_eq!(a.len(), 32);
        assert_ne!(a, new_id());
    }
}
