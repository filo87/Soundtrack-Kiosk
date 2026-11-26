use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

pub const EVENT_SELECTION: &str = "selection";
pub const EVENT_PLAYLIST_ADDED: &str = "playlist_added";
pub const EVENT_PRINT_SENT: &str = "print_sent";
pub const EVENT_PRINT_FAILED: &str = "print_failed";
pub const EVENT_PLAYLIST_SKIPPED: &str = "playlist_skipped";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalLine {
    pub v: u8,
    pub event: String,
    pub id: String,
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guest_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_px: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl JournalLine {
    fn base(event: &str, id: &str) -> Self {
        Self {
            v: 1,
            event: event.to_string(),
            id: id.to_string(),
            timestamp: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            guest_name: None,
            track_title: None,
            artist: None,
            track_uri: None,
            orientation: None,
            layout: None,
            cover_source: None,
            cover_px: None,
            mode: None,
            uri: None,
            file: None,
            job: None,
            reason: None,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn selection(
        id: &str,
        guest_name: &str,
        track_title: &str,
        artist: &str,
        track_uri: Option<&str>,
        orientation: &str,
        layout: &str,
        cover_source: &str,
        cover_px: u32,
        mode: &str,
    ) -> Self {
        let mut line = Self::base(EVENT_SELECTION, id);
        line.guest_name = Some(guest_name.to_string());
        line.track_title = Some(track_title.to_string());
        line.artist = Some(artist.to_string());
        line.track_uri = track_uri.map(ToString::to_string);
        line.orientation = Some(orientation.to_string());
        line.layout = Some(layout.to_string());
        line.cover_source = Some(cover_source.to_string());
        line.cover_px = Some(cover_px);
        line.mode = Some(mode.to_string());
        line
    }

    pub fn playlist_added(id: &str, uri: &str) -> Self {
        let mut line = Self::base(EVENT_PLAYLIST_ADDED, id);
        line.uri = Some(uri.to_string());
        line
    }

    pub fn print_sent(id: &str, file: &str, job: &str) -> Self {
        let mut line = Self::base(EVENT_PRINT_SENT, id);
        line.file = Some(file.to_string());
        line.job = Some(job.to_string());
        line
    }

    pub fn playlist_skipped(id: &str, reason: &str) -> Self {
        let mut line = Self::base(EVENT_PLAYLIST_SKIPPED, id);
        line.reason = Some(reason.to_string());
        line
    }

    pub fn print_failed(id: &str, reason: &str) -> Self {
        let mut line = Self::base(EVENT_PRINT_FAILED, id);
        line.reason = Some(reason.to_string());
        line
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintState {
    Pending,
    Sent,
    Failed,
}

#[derive(Debug, Clone)]
pub struct Selection {
    pub id: String,
    pub guest_name: String,
    pub track_title: String,
    pub artist: String,
    pub track_uri: Option<String>,
    pub orientation: String,
    pub layout: String,
    pub timestamp: String,
    pub cover_source: String,
    pub cover_px: u32,
    pub mode: String,
    pub playlist_added: bool,
    pub added_uri: Option<String>,
    pub playlist_skipped: bool,
    pub playlist_skip_reason: Option<String>,
    pub print_state: PrintState,
    pub print_file: Option<String>,
    pub print_job: Option<String>,
    pub print_reason: Option<String>,
}

struct CacheEntry {
    modified: Option<SystemTime>,
    len: u64,
    selections: Vec<Selection>,
}

struct JournalInner {
    path: PathBuf,
    file: Mutex<File>,
    cache: Mutex<Option<CacheEntry>>,
}

#[derive(Clone)]
pub struct Journal {
    inner: Arc<JournalInner>,
}

impl Journal {
    pub fn open(path: impl AsRef<Path>) -> AppResult<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)?;
        repair_tail(&mut file)?;
        Ok(Self {
            inner: Arc::new(JournalInner {
                path,
                file: Mutex::new(file),
                cache: Mutex::new(None),
            }),
        })
    }

    /// Append and wait for the fsync: used before a print is committed.
    pub async fn append_synced(&self, line: &JournalLine) -> AppResult<()> {
        self.append_many_synced(std::slice::from_ref(line)).await
    }

    pub async fn append_many_synced(&self, lines: &[JournalLine]) -> AppResult<()> {
        if lines.is_empty() {
            return Ok(());
        }
        let mut payload = Vec::new();
        for line in lines {
            serde_json::to_writer(&mut payload, line)?;
            payload.push(b'\n');
        }
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || write_sync(&inner, &payload))
            .await
            .map_err(|e| AppError::internal(e.to_string()))?
    }

    /// Archives the current journal and truncates it in place.
    pub fn clear(&self) -> AppResult<Option<PathBuf>> {
        let mut file = self
            .inner
            .file
            .lock()
            .map_err(|_| AppError::internal("journal mutex poisoned"))?;
        file.flush()?;
        file.sync_data()?;
        let len = file.metadata()?.len();
        let mut archive = None;
        if len > 0 {
            let dir = self
                .inner
                .path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            let name = format!(
                "journal-{}.jsonl",
                chrono::Local::now().format("%Y%m%d-%H%M%S-%3f")
            );
            let target = dir.join(name);
            std::fs::copy(&self.inner.path, &target)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600));
            }
            archive = Some(target);
        }
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        file.sync_data()?;
        drop(file);
        if let Ok(mut cache) = self.inner.cache.lock() {
            *cache = None;
        }
        Ok(archive)
    }

    pub fn load(&self) -> AppResult<Vec<Selection>> {
        let metadata = match std::fs::metadata(&self.inner.path) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Vec::new());
            }
            Err(err) => return Err(err.into()),
        };
        let modified = metadata.modified().ok();
        let len = metadata.len();
        if let Ok(cache) = self.inner.cache.lock() {
            if let Some(entry) = cache.as_ref() {
                if entry.modified == modified && entry.len == len {
                    return Ok(entry.selections.clone());
                }
            }
        }
        let mut selections = load_file(&self.inner.path)?;
        self.remap_legacy_print_paths(&mut selections);
        if let Ok(mut cache) = self.inner.cache.lock() {
            *cache = Some(CacheEntry {
                modified,
                len,
                selections: selections.clone(),
            });
        }
        Ok(selections)
    }

    fn remap_legacy_print_paths(&self, selections: &mut [Selection]) {
        let Some(parent) = self.inner.path.parent() else {
            return;
        };
        let legacy_root = parent.join(crate::paths::LEGACY_DATA_IDENTIFIER);
        for selection in selections {
            let Some(file) = selection.print_file.as_deref() else {
                continue;
            };
            if let Ok(relative) = Path::new(file).strip_prefix(&legacy_root) {
                selection.print_file = Some(parent.join(relative).to_string_lossy().to_string());
            }
        }
    }

    pub fn find(&self, id: &str) -> AppResult<Option<Selection>> {
        Ok(self.load()?.into_iter().find(|s| s.id == id))
    }
}

fn write_sync(inner: &JournalInner, payload: &[u8]) -> AppResult<()> {
    let mut file = inner
        .file
        .lock()
        .map_err(|_| AppError::internal("journal mutex poisoned"))?;
    file.write_all(payload)?;
    file.sync_data()?;
    drop(file);
    if let Ok(mut cache) = inner.cache.lock() {
        *cache = None;
    }
    Ok(())
}

fn repair_tail(file: &mut File) -> AppResult<()> {
    let len = file.metadata()?.len();
    if len == 0 {
        return Ok(());
    }
    file.seek(SeekFrom::End(-1))?;
    let mut last = [0u8; 1];
    file.read_exact(&mut last)?;
    if last[0] != b'\n' {
        file.seek(SeekFrom::End(0))?;
        file.write_all(b"\n")?;
        file.sync_data()?;
    }
    Ok(())
}

fn read_lines(path: &Path) -> AppResult<Vec<String>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut lines = Vec::new();
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        let read = reader.read_until(b'\n', &mut buffer)?;
        if read == 0 {
            break;
        }
        let line = String::from_utf8_lossy(&buffer);
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
    }
    Ok(lines)
}

pub fn load_file(path: &Path) -> AppResult<Vec<Selection>> {
    let lines = read_lines(path)?;
    let mut order: Vec<String> = Vec::new();
    let mut map: HashMap<String, Selection> = HashMap::new();
    let last_index = lines.len().saturating_sub(1);

    for (index, raw) in lines.iter().enumerate() {
        match serde_json::from_str::<JournalLine>(raw) {
            Ok(line) => apply_line(&mut order, &mut map, line),
            Err(err) => {
                if index == last_index {
                    log::warn!("journal: skipping truncated last line: {err}");
                } else {
                    log::warn!("journal: skipping malformed line {index}: {err}");
                }
            }
        }
    }

    Ok(order.into_iter().filter_map(|id| map.remove(&id)).collect())
}

fn apply_line(order: &mut Vec<String>, map: &mut HashMap<String, Selection>, line: JournalLine) {
    let id = line.id.clone();
    match line.event.as_str() {
        EVENT_SELECTION => {
            if map.contains_key(&id) {
                return;
            }
            order.push(id.clone());
            map.insert(
                id.clone(),
                Selection {
                    id,
                    guest_name: line.guest_name.unwrap_or_default(),
                    track_title: line.track_title.unwrap_or_default(),
                    artist: line.artist.unwrap_or_default(),
                    track_uri: line.track_uri,
                    orientation: line.orientation.unwrap_or_else(|| "portrait".to_string()),
                    layout: line.layout.unwrap_or_else(|| "cover".to_string()),
                    timestamp: line.timestamp,
                    cover_source: line
                        .cover_source
                        .unwrap_or_else(|| "placeholder".to_string()),
                    cover_px: line.cover_px.unwrap_or(0),
                    mode: line.mode.unwrap_or_else(|| "spotify".to_string()),
                    playlist_added: false,
                    added_uri: None,
                    playlist_skipped: false,
                    playlist_skip_reason: None,
                    print_state: PrintState::Pending,
                    print_file: None,
                    print_job: None,
                    print_reason: None,
                },
            );
        }
        EVENT_PLAYLIST_ADDED => {
            if let Some(selection) = map.get_mut(&id) {
                selection.playlist_added = true;
                selection.added_uri = line.uri;
            }
        }
        EVENT_PLAYLIST_SKIPPED => {
            if let Some(selection) = map.get_mut(&id) {
                selection.playlist_skipped = true;
                selection.playlist_skip_reason = line.reason;
            }
        }
        EVENT_PRINT_SENT => {
            if let Some(selection) = map.get_mut(&id) {
                selection.print_state = PrintState::Sent;
                selection.print_file = line.file;
                selection.print_job = line.job;
                selection.print_reason = None;
            }
        }
        EVENT_PRINT_FAILED => {
            if let Some(selection) = map.get_mut(&id) {
                selection.print_state = PrintState::Failed;
                selection.print_reason = line.reason;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_journal() -> (tempfile::TempDir, Journal) {
        let dir = tempfile::tempdir().unwrap();
        let journal = Journal::open(dir.path().join("journal.jsonl")).unwrap();
        (dir, journal)
    }

    #[tokio::test]
    async fn folds_events_by_id() {
        let (_dir, journal) = temp_journal();
        journal
            .append_synced(&JournalLine::selection(
                "a",
                "Ada",
                "Song",
                "Artist",
                Some("spotify:track:1"),
                "portrait",
                "cover",
                "itunes",
                3000,
                "spotify",
            ))
            .await
            .unwrap();
        journal
            .append_synced(&JournalLine::playlist_added("a", "spotify:track:1"))
            .await
            .unwrap();
        journal
            .append_synced(&JournalLine::print_sent("a", "/tmp/x.png", "job-1"))
            .await
            .unwrap();

        let all = journal.load().unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].playlist_added);
        assert!(!all[0].playlist_skipped);
        assert_eq!(all[0].print_state, PrintState::Sent);
    }

    #[test]
    fn skips_truncated_last_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.jsonl");
        let mut file = File::create(&path).unwrap();
        let good = serde_json::to_string(&JournalLine::selection(
            "a",
            "Ada",
            "Song",
            "Artist",
            None,
            "landscape",
            "cover",
            "placeholder",
            640,
            "manual",
        ))
        .unwrap();
        writeln!(file, "{good}").unwrap();
        write!(file, "{{\"v\":1,\"event\":\"selection\",\"id\":\"b\"").unwrap();
        drop(file);

        let all = load_file(&path).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "a");
    }

    #[tokio::test]
    async fn records_playlist_skips() {
        let (_dir, journal) = temp_journal();
        journal
            .append_synced(&JournalLine::selection(
                "a",
                "Ada",
                "Wonderwall",
                "Oasis",
                None,
                "portrait",
                "cover",
                "placeholder",
                640,
                "manual",
            ))
            .await
            .unwrap();
        journal
            .append_synced(&JournalLine::playlist_skipped("a", "duplicate"))
            .await
            .unwrap();
        let all = journal.load().unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].playlist_skipped);
        assert_eq!(all[0].playlist_skip_reason.as_deref(), Some("duplicate"));
        assert!(!all[0].playlist_added);
    }

    #[tokio::test]
    async fn remaps_legacy_print_paths_to_the_new_data_dir() {
        let (dir, journal) = temp_journal();
        let old_root = dir.path().join(crate::paths::LEGACY_DATA_IDENTIFIER);
        let old_file = old_root.join("prints/2026-11-25/a.png");
        journal
            .append_synced(&JournalLine::selection(
                "a",
                "Ada",
                "Song",
                "Artist",
                None,
                "portrait",
                "cover",
                "placeholder",
                640,
                "manual",
            ))
            .await
            .unwrap();
        journal
            .append_synced(&JournalLine::print_sent(
                "a",
                &old_file.to_string_lossy(),
                "job-1",
            ))
            .await
            .unwrap();

        let all = journal.load().unwrap();
        let expected = dir.path().join("prints/2026-11-25/a.png");
        assert_eq!(
            all[0].print_file.as_deref(),
            Some(expected.to_string_lossy().as_ref())
        );
    }

    #[tokio::test]
    async fn clear_archives_and_empties_history() {
        let (_dir, journal) = temp_journal();
        journal
            .append_synced(&JournalLine::selection(
                "a",
                "Ada",
                "Song",
                "Artist",
                None,
                "portrait",
                "cover",
                "placeholder",
                640,
                "manual",
            ))
            .await
            .unwrap();
        journal
            .append_synced(&JournalLine::selection(
                "b",
                "Grace",
                "Song 2",
                "Artist",
                None,
                "portrait",
                "cover",
                "placeholder",
                640,
                "manual",
            ))
            .await
            .unwrap();

        let archive = journal.clear().unwrap().expect("archive path");
        assert!(archive.exists());
        assert!(journal.load().unwrap().is_empty());
        let archived = std::fs::read_to_string(&archive).unwrap();
        assert_eq!(archived.lines().count(), 2);

        journal
            .append_synced(&JournalLine::selection(
                "c",
                "Linus",
                "Song 3",
                "Artist",
                None,
                "portrait",
                "cover",
                "placeholder",
                640,
                "manual",
            ))
            .await
            .unwrap();
        assert_eq!(journal.load().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn repairs_missing_trailing_newline() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.jsonl");
        let first = JournalLine::selection(
            "a",
            "Ada",
            "Song",
            "Artist",
            None,
            "portrait",
            "cover",
            "placeholder",
            640,
            "manual",
        );
        std::fs::write(&path, serde_json::to_vec(&first).unwrap()).unwrap();

        let journal = Journal::open(&path).unwrap();
        let second = JournalLine::selection(
            "b",
            "Grace",
            "Song 2",
            "Artist",
            None,
            "portrait",
            "cover",
            "placeholder",
            640,
            "manual",
        );
        journal.append_synced(&second).await.unwrap();
        assert_eq!(journal.load().unwrap().len(), 2);
    }

    #[test]
    fn continues_after_invalid_utf8_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.jsonl");
        let mut file = File::create(&path).unwrap();
        file.write_all(b"\xff\xfe\n").unwrap();
        let good = JournalLine::selection(
            "a",
            "Ada",
            "Song",
            "Artist",
            None,
            "portrait",
            "cover",
            "placeholder",
            640,
            "manual",
        );
        writeln!(file, "{}", serde_json::to_string(&good).unwrap()).unwrap();
        drop(file);

        let all = load_file(&path).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "a");
    }

    #[test]
    fn duplicate_selection_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("journal.jsonl");
        let mut file = File::create(&path).unwrap();
        let line = JournalLine::selection(
            "a",
            "Ada",
            "Song",
            "Artist",
            None,
            "portrait",
            "cover",
            "placeholder",
            640,
            "manual",
        );
        writeln!(file, "{}", serde_json::to_string(&line).unwrap()).unwrap();
        writeln!(file, "{}", serde_json::to_string(&line).unwrap()).unwrap();
        drop(file);

        assert_eq!(load_file(&path).unwrap().len(), 1);
    }
}
