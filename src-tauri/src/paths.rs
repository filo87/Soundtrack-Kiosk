use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::Local;

pub(crate) const LEGACY_DATA_IDENTIFIER: &str = "com.soundtrack.babykiosk";

/// Copies the old Baby Playlist Kiosk data directory into the new identifier directory once.
/// The legacy directory is kept as a backup.
pub fn migrate_legacy_data(new_root: &Path) -> io::Result<bool> {
    let Some(parent) = new_root.parent() else {
        return Ok(false);
    };
    let legacy_root = parent.join(LEGACY_DATA_IDENTIFIER);
    if !legacy_root.is_dir() || legacy_root == new_root {
        return Ok(false);
    }
    if has_user_state(new_root) {
        return Ok(false);
    }

    let staging = parent.join(format!(".soundtrack-kiosk-migrate-{}", std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    if let Err(err) = copy_dir_all(&legacy_root, &staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(err);
    }
    if new_root.exists() {
        fs::remove_dir_all(new_root)?;
    }
    if let Err(err) = fs::rename(&staging, new_root) {
        let _ = fs::remove_dir_all(&staging);
        return Err(err);
    }
    Ok(true)
}

fn has_user_state(root: &Path) -> bool {
    ["settings.json", "tokens.json", "journal.jsonl"]
        .iter()
        .any(|name| root.join(name).exists())
}

fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct AppPaths {
    root: PathBuf,
}

impl AppPaths {
    pub fn new(root: PathBuf) -> io::Result<Self> {
        let paths = Self { root };
        for dir in [
            paths.covers_dir(),
            paths.previews_dir(),
            paths.prints_dir(),
            paths.logs_dir(),
        ] {
            fs::create_dir_all(dir)?;
        }
        Ok(paths)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn tokens(&self) -> PathBuf {
        self.root.join("tokens.json")
    }

    pub fn journal(&self) -> PathBuf {
        self.root.join("journal.jsonl")
    }

    pub fn covers_dir(&self) -> PathBuf {
        self.root.join("covers")
    }

    pub fn previews_dir(&self) -> PathBuf {
        self.root.join("previews")
    }

    pub fn prints_root(&self) -> PathBuf {
        self.root.join("prints")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn mock_prints_dir(&self) -> PathBuf {
        self.root.join("mock-prints")
    }

    /// `prints/YYYY-MM-DD`, created on demand.
    pub fn prints_dir(&self) -> PathBuf {
        self.prints_root()
            .join(Local::now().format("%Y-%m-%d").to_string())
    }

    pub fn ensure_prints_dir(&self) -> io::Result<PathBuf> {
        let dir = self.prints_dir();
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Preview files are transient: purge them on startup.
    pub fn purge_previews(&self) -> io::Result<()> {
        if self.previews_dir().exists() {
            for entry in fs::read_dir(self.previews_dir())? {
                let entry = entry?;
                if entry.path().is_file() {
                    fs::remove_file(entry.path())?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn migrates_legacy_data_once_and_keeps_the_old_copy() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join(LEGACY_DATA_IDENTIFIER);
        let new = dir.path().join("com.soundtrack.kiosk");
        write(&legacy.join("settings.json"), "old-settings");
        write(&legacy.join("tokens.json"), "old-tokens");
        write(&legacy.join("journal.jsonl"), "old-journal\n");
        write(&legacy.join("prints/2026-11-25/a.png"), "png");

        assert!(migrate_legacy_data(&new).unwrap());
        assert_eq!(
            fs::read_to_string(new.join("settings.json")).unwrap(),
            "old-settings"
        );
        assert_eq!(
            fs::read_to_string(new.join("journal.jsonl")).unwrap(),
            "old-journal\n"
        );
        assert_eq!(
            fs::read_to_string(new.join("prints/2026-11-25/a.png")).unwrap(),
            "png"
        );
        assert!(legacy.join("settings.json").exists());

        assert!(!migrate_legacy_data(&new).unwrap());
    }

    #[test]
    fn does_not_overwrite_new_state() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join(LEGACY_DATA_IDENTIFIER);
        let new = dir.path().join("com.soundtrack.kiosk");
        write(&legacy.join("settings.json"), "old-settings");
        write(&new.join("settings.json"), "new-settings");

        assert!(!migrate_legacy_data(&new).unwrap());
        assert_eq!(
            fs::read_to_string(new.join("settings.json")).unwrap(),
            "new-settings"
        );
    }

    #[test]
    fn replaces_a_cache_only_new_directory() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join(LEGACY_DATA_IDENTIFIER);
        let new = dir.path().join("com.soundtrack.kiosk");
        write(&legacy.join("journal.jsonl"), "old-journal\n");
        fs::create_dir_all(new.join("covers")).unwrap();
        fs::write(new.join(".DS_Store"), "junk").unwrap();

        assert!(migrate_legacy_data(&new).unwrap());
        assert_eq!(
            fs::read_to_string(new.join("journal.jsonl")).unwrap(),
            "old-journal\n"
        );
    }

    #[test]
    fn no_legacy_directory_is_a_noop() {
        let dir = tempfile::tempdir().unwrap();
        let new = dir.path().join("com.soundtrack.kiosk");
        assert!(!migrate_legacy_data(&new).unwrap());
    }
}
