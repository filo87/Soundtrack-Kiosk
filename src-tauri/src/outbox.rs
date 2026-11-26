use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tokio::sync::Notify;

use crate::cover::matching;
use crate::error::AppError;
use crate::journal::{JournalLine, Selection};
use crate::state::AppState;

const BATCH_SIZE: usize = 100;
const QUOTA_PAUSE: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Default)]
pub struct OutboxHandle {
    notify: Arc<Notify>,
}

impl OutboxHandle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn notify(&self) {
        self.notify.notify_one();
    }

    pub async fn notified(&self) {
        self.notify.notified().await;
    }
}

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let handle = {
            let state = app.state::<AppState>();
            state.outbox.clone()
        };
        loop {
            tokio::select! {
                _ = handle.notified() => {}
                _ = tokio::time::sleep(Duration::from_secs(30)) => {}
            }
            if let Err(err) = drain_once(&app).await {
                log::error!("outbox: {err}");
            }
        }
    });
}

pub async fn drain_once(app: &AppHandle) -> crate::error::AppResult<()> {
    let (paused, authenticated, config, selections, journal) = {
        let state = app.state::<AppState>();
        (
            state.outbox_paused(),
            state.tokens.is_authenticated(),
            state.config(),
            state.journal.load()?,
            state.journal.clone(),
        )
    };
    if paused || !authenticated {
        return Ok(());
    }
    let Some(playlist_id) = config.playlist_id() else {
        return Ok(());
    };
    let pending: Vec<&Selection> = selections
        .iter()
        .filter(|selection| !selection.playlist_added && !selection.playlist_skipped)
        .collect();
    if pending.is_empty() {
        return Ok(());
    }

    let api = {
        let state = app.state::<AppState>();
        state.api()
    };

    let mut taken_identities: HashSet<String> = selections
        .iter()
        .filter(|selection| selection.playlist_added)
        .map(|selection| matching::song_identity(&selection.track_title, &selection.artist))
        .collect();
    let mut resolved: Vec<(String, String)> = Vec::new();
    let mut skipped: Vec<(String, &'static str)> = Vec::new();
    for selection in &pending {
        let identity = matching::song_identity(&selection.track_title, &selection.artist);
        if taken_identities.contains(&identity) {
            skipped.push((selection.id.clone(), "duplicate"));
            continue;
        }
        if let Some(uri) = selection.track_uri.clone() {
            taken_identities.insert(identity);
            resolved.push((selection.id.clone(), uri));
            continue;
        }
        if selection.track_title.is_empty() {
            continue;
        }
        match api
            .resolve_track(&selection.track_title, &selection.artist, &config.market)
            .await
        {
            Ok(Some(track)) => {
                taken_identities.insert(identity);
                resolved.push((selection.id.clone(), track.uri));
            }
            Ok(None) => {}
            Err(err) => handle_api_error(app, &err)?,
        }
    }
    if resolved.is_empty() && skipped.is_empty() {
        return Ok(());
    }

    let already_added: HashSet<String> = selections
        .iter()
        .filter_map(|selection| selection.added_uri.clone())
        .collect();

    let mut insert_uris = Vec::new();
    let mut seen = HashSet::new();
    let mut mark_now = Vec::new();
    for (id, uri) in &resolved {
        if already_added.contains(uri) {
            mark_now.push(JournalLine::playlist_added(id, uri));
        } else if seen.insert(uri.clone()) {
            insert_uris.push(uri.clone());
        }
    }
    if !mark_now.is_empty() {
        journal.append_many_synced(&mark_now).await?;
    }

    for chunk in insert_uris.chunks(BATCH_SIZE) {
        if let Err(err) = api.add_items(&playlist_id, chunk).await {
            handle_api_error(app, &err)?;
            return Ok(());
        }
        let chunk_set: HashSet<&str> = chunk.iter().map(String::as_str).collect();
        let lines: Vec<JournalLine> = resolved
            .iter()
            .filter(|(_, uri)| chunk_set.contains(uri.as_str()))
            .map(|(id, uri)| JournalLine::playlist_added(id, uri))
            .collect();
        if !lines.is_empty() {
            journal.append_many_synced(&lines).await?;
        }
    }
    if !skipped.is_empty() {
        let lines: Vec<JournalLine> = skipped
            .iter()
            .map(|(id, reason)| JournalLine::playlist_skipped(id, reason))
            .collect();
        journal.append_many_synced(&lines).await?;
    }
    Ok(())
}

fn handle_api_error(app: &AppHandle, err: &AppError) -> crate::error::AppResult<()> {
    let state = app.state::<AppState>();
    match err.code() {
        crate::error::QUOTA => {
            state.outbox_pause(QUOTA_PAUSE);
            Ok(())
        }
        crate::error::AUTH_REQUIRED | crate::error::OFFLINE => Ok(()),
        _ => Err(err.clone()),
    }
}
