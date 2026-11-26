use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::config::{INK_LIMIT, PAPER_LIMIT};
use crate::journal::{PrintState, Selection};
use crate::print::{self, PrinterHealth};
use crate::state::AppState;

pub const HEALTH_EVENT: &str = "kiosk://health";
const OUTBOX_STALE: i64 = 5 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    pub auth: bool,
    pub network: bool,
    pub printer: PrinterHealth,
    pub prints_waiting: u32,
    pub outbox_pending: u32,
    pub refill_due: bool,
    pub paper_limit: u32,
    pub ink_limit: u32,
    pub attention: bool,
}

pub async fn snapshot(state: &AppState) -> Health {
    let selections = state.journal.load().unwrap_or_default();
    snapshot_with(state, &selections).await
}

pub async fn snapshot_with(state: &AppState, selections: &[Selection]) -> Health {
    let config = state.config();
    let mut printer = print::status(state.printer, config.printer.as_deref()).await;
    if state.printer == crate::print::PrinterBackend::Mock && state.mock_busy() {
        printer = PrinterHealth {
            state: "printing".to_string(),
            reason: None,
        };
    }
    let prints_waiting = print::waiting(state.printer, config.printer.as_deref()).await;
    let pending: Vec<_> = selections
        .iter()
        .filter(|selection| !selection.playlist_added && !selection.playlist_skipped)
        .collect();
    let outbox_pending = pending.len() as u32;
    let outbox_stale = pending
        .iter()
        .any(|selection| is_stale(&selection.timestamp));
    let print_failed = selections
        .iter()
        .any(|selection| selection.print_state == PrintState::Failed);
    let auth = state.tokens.is_authenticated();
    let needs_login = state.tokens.needs_login();
    let network = state.network_ok.load(std::sync::atomic::Ordering::Relaxed);
    let refill_due = config.paper_count >= PAPER_LIMIT || config.ink_count >= INK_LIMIT;
    let pin_invalid = crate::config::validate_pin(&config.pin).is_err();
    let attention = !auth
        || needs_login
        || !network
        || !printer.ok()
        || outbox_stale
        || refill_due
        || print_failed
        || pin_invalid;
    Health {
        auth,
        network,
        printer,
        prints_waiting,
        outbox_pending,
        refill_due,
        paper_limit: PAPER_LIMIT,
        ink_limit: INK_LIMIT,
        attention,
    }
}

fn is_stale(timestamp: &str) -> bool {
    let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(timestamp) else {
        return false;
    };
    let age = chrono::Local::now().signed_duration_since(parsed);
    age.num_seconds() > OUTBOX_STALE
}

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_probe: Option<std::time::Instant> = None;
        loop {
            let health = {
                let state = app.state::<AppState>();
                snapshot(&state).await
            };
            let _ = app.emit(HEALTH_EVENT, &health);
            if !health.network
                && last_probe
                    .map(|at: std::time::Instant| at.elapsed() >= Duration::from_secs(30))
                    .unwrap_or(true)
            {
                last_probe = Some(std::time::Instant::now());
                probe_network(&app).await;
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });
}

async fn probe_network(app: &AppHandle) {
    let (http, network_ok) = {
        let state = app.state::<AppState>();
        (state.http.clone(), state.network_ok.clone())
    };
    let probe = http
        .get("https://open.spotify.com/")
        .timeout(Duration::from_secs(3))
        .send()
        .await;
    if probe.is_ok() {
        network_ok.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}
