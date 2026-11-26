pub mod itunes;
pub mod matching;

use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};
use tokio::time::timeout;

use crate::error::{AppError, AppResult};

use self::itunes::ItunesCandidate;

pub const ITUNES_BASE: &str = "https://itunes.apple.com";
const OVERALL_DEADLINE: Duration = Duration::from_secs(6);
const MIN_ITUNES_PX: u32 = 1000;
const PLACEHOLDER_BYTES: &[u8] = include_bytes!("../../resources/placeholder.jpg");

#[derive(Debug, Clone)]
pub struct Coverage {
    pub bytes: Vec<u8>,
    pub source: String,
    pub px: u32,
    pub ext: String,
}

#[derive(Clone)]
pub struct CoverCache {
    dir: PathBuf,
    http: reqwest::Client,
    itunes_base: String,
}

impl CoverCache {
    pub fn new(dir: PathBuf, http: reqwest::Client, itunes_base: impl Into<String>) -> Self {
        Self {
            dir,
            http,
            itunes_base: itunes_base.into(),
        }
    }

    pub fn manual_key(artist: &str, title: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(matching::normalize(artist));
        hasher.update(b"|");
        hasher.update(matching::normalize(title));
        format!("{:x}", hasher.finalize())
    }

    pub fn key_for(album_id: Option<&str>, track_id: &str) -> String {
        match album_id.filter(|id| !id.is_empty()) {
            Some(id) => format!("album-{id}"),
            None => format!("track-{track_id}"),
        }
    }

    pub async fn resolve(
        &self,
        key: &str,
        title: &str,
        artist: &str,
        album: Option<&str>,
        spotify_url: Option<&str>,
        country: &str,
    ) -> AppResult<Coverage> {
        let key = sanitize_key(key);
        let cache_dir = self.dir.clone();
        let lookup_key = key.clone();
        let cached = tokio::task::spawn_blocking(move || read_cache(&cache_dir, &lookup_key))
            .await
            .map_err(|err| AppError::internal(format!("cover cache lookup failed: {err}")))??;
        if let Some(cached) = cached {
            return Ok(cached);
        }

        let remote = timeout(
            OVERALL_DEADLINE,
            self.resolve_remote(title, artist, album, spotify_url, country),
        )
        .await;

        let coverage = match remote {
            Ok(Ok(Some(found))) => found,
            _ => placeholder(),
        };

        if coverage.source != "placeholder" {
            let cache_dir = self.dir.clone();
            let write_key = key.clone();
            let value = coverage.clone();
            let _ =
                tokio::task::spawn_blocking(move || write_cache(&cache_dir, &write_key, &value))
                    .await;
        }
        Ok(coverage)
    }

    async fn resolve_remote(
        &self,
        title: &str,
        artist: &str,
        album: Option<&str>,
        spotify_url: Option<&str>,
        country: &str,
    ) -> AppResult<Option<Coverage>> {
        let term = format!("{artist} {title}");
        if let Ok(candidates) = itunes::search(&self.http, &self.itunes_base, &term, country).await
        {
            if let Some(best) = matching::select_best(&candidates, title, artist, album) {
                if let Some(found) = self.download_itunes(best).await {
                    return Ok(Some(found));
                }
            }
        }

        if let Some(url) =
            spotify_url.filter(|u| u.starts_with("https://") || u.starts_with("http://"))
        {
            if let Ok(response) = self.http.get(url).send().await {
                if response.status().is_success() {
                    if let Ok(bytes) = response.bytes().await {
                        let bytes = bytes.to_vec();
                        if let Some((w, h)) = crate::render::image_dimensions(&bytes) {
                            return Ok(Some(Coverage {
                                bytes,
                                source: "spotify".to_string(),
                                px: w.min(h),
                                ext: extension_from_url(url).unwrap_or_else(|| "jpg".to_string()),
                            }));
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    async fn download_itunes(&self, candidate: &ItunesCandidate) -> Option<Coverage> {
        let url = itunes::upgrade_artwork_url(&candidate.artwork_url)
            .unwrap_or_else(|| candidate.artwork_url.clone());
        let response = self.http.get(&url).send().await.ok()?;
        if !response.status().is_success() {
            return None;
        }
        let bytes = response.bytes().await.ok()?.to_vec();
        let (w, h) = crate::render::image_dimensions(&bytes)?;
        if w.min(h) < MIN_ITUNES_PX {
            return None;
        }
        Some(Coverage {
            bytes,
            source: "itunes".to_string(),
            px: w.min(h),
            ext: extension_from_url(&url).unwrap_or_else(|| "jpg".to_string()),
        })
    }
}

fn read_cache(dir: &Path, key: &str) -> AppResult<Option<Coverage>> {
    let key_dir = dir.join(key);
    if !key_dir.exists() {
        return Ok(None);
    }
    let mut best: Option<Coverage> = None;
    for entry in std::fs::read_dir(&key_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        let Some((source, tail)) = name.split_once("--") else {
            continue;
        };
        let Some((px_text, ext)) = tail.split_once('.') else {
            continue;
        };
        let Ok(px) = px_text.parse::<u32>() else {
            continue;
        };
        if best.as_ref().is_some_and(|existing| existing.px >= px) {
            continue;
        }
        if let Ok(bytes) = std::fs::read(entry.path()) {
            best = Some(Coverage {
                bytes,
                source: source.to_string(),
                px,
                ext: ext.to_string(),
            });
        }
    }
    Ok(best)
}

fn write_cache(dir: &Path, key: &str, coverage: &Coverage) -> AppResult<()> {
    let key_dir = dir.join(key);
    std::fs::create_dir_all(&key_dir)?;
    let path = key_dir.join(format!(
        "{}--{}.{}",
        sanitize_key(&coverage.source),
        coverage.px,
        sanitize_key(&coverage.ext)
    ));
    std::fs::write(path, &coverage.bytes)?;
    Ok(())
}

pub fn placeholder() -> Coverage {
    Coverage {
        bytes: PLACEHOLDER_BYTES.to_vec(),
        source: "placeholder".to_string(),
        px: 1200,
        ext: "jpg".to_string(),
    }
}

fn sanitize_key(input: &str) -> String {
    input
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

fn extension_from_url(url: &str) -> Option<String> {
    let path = url.split('?').next().unwrap_or(url);
    let file = path.rsplit('/').next()?;
    let ext = file.rsplit_once('.')?.1.to_ascii_lowercase();
    if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp") {
        Some(ext)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn png_bytes() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(1200, 1200, image::Rgba([10, 20, 30, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        bytes
    }

    #[tokio::test]
    async fn falls_back_to_placeholder_when_offline() {
        let dir = tempfile::tempdir().unwrap();
        let cache = CoverCache::new(
            dir.path().to_path_buf(),
            reqwest::Client::new(),
            "http://127.0.0.1:1",
        );
        let cover = cache
            .resolve("album-x", "Song", "Artist", Some("Album"), None, "US")
            .await
            .unwrap();
        assert_eq!(cover.source, "placeholder");
        assert_eq!(cover.px, 1200);
    }

    #[tokio::test]
    async fn wrong_artist_match_falls_through_to_spotify() {
        let server = MockServer::start().await;
        let image_url = format!("{}/cover/640x640bb.png", server.uri());
        Mock::given(method("GET"))
            .and(path("/search"))
            .and(query_param("entity", "song"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [{
                    "trackName": "Song",
                    "artistName": "Someone Else",
                    "collectionName": "Album",
                    "artworkUrl100": format!("{}/art/100x100bb.jpg", server.uri())
                }]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/cover/640x640bb.png"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(png_bytes()))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let cache = CoverCache::new(
            dir.path().to_path_buf(),
            reqwest::Client::new(),
            server.uri(),
        );
        let cover = cache
            .resolve(
                "album-y",
                "Song",
                "Artist",
                Some("Album"),
                Some(&image_url),
                "US",
            )
            .await
            .unwrap();
        assert_eq!(cover.source, "spotify");
        assert_eq!(cover.px, 1200);
    }

    #[tokio::test]
    async fn strict_itunes_match_is_used_and_cached() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "results": [{
                    "trackName": "Song",
                    "artistName": "Artist",
                    "collectionName": "Album",
                    "artworkUrl100": format!("{}/art/100x100bb.png", server.uri())
                }]
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/art/3000x3000bb.png"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(png_bytes()))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().unwrap();
        let cache = CoverCache::new(
            dir.path().to_path_buf(),
            reqwest::Client::new(),
            server.uri(),
        );
        let first = cache
            .resolve("album-z", "Song", "Artist", Some("Album"), None, "US")
            .await
            .unwrap();
        assert_eq!(first.source, "itunes");
        let second = cache
            .resolve("album-z", "Song", "Artist", Some("Album"), None, "US")
            .await
            .unwrap();
        assert_eq!(second.source, "itunes");
    }

    #[test]
    fn keys_are_filesystem_safe() {
        assert_eq!(sanitize_key("album-abc/../x"), "album-abcx");
        assert_eq!(manual_key_len(), 64);
    }

    fn manual_key_len() -> usize {
        CoverCache::manual_key("Beyoncé", "Halo").len()
    }
}
