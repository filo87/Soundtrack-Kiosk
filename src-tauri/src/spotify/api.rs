use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{AppError, AppResult, AUTH_REQUIRED, OFFLINE, QUOTA, RATE_LIMITED};
use crate::spotify::token::TokenManager;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrackDto {
    pub id: String,
    pub uri: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    #[serde(default)]
    pub album_id: Option<String>,
    #[serde(default)]
    pub image640: Option<String>,
    #[serde(default)]
    pub image64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub tracks: Vec<TrackDto>,
    pub degraded: bool,
}

#[derive(Clone)]
pub struct SpotifyApi {
    http: reqwest::Client,
    tokens: TokenManager,
    base: String,
    network_ok: Arc<AtomicBool>,
}

impl SpotifyApi {
    pub fn new(
        http: reqwest::Client,
        tokens: TokenManager,
        base: impl Into<String>,
        network_ok: Arc<AtomicBool>,
    ) -> Self {
        Self {
            http,
            tokens,
            base: base.into().trim_end_matches('/').to_string(),
            network_ok,
        }
    }

    pub async fn search_tracks(&self, query: &str, market: &str) -> AppResult<SearchResponse> {
        let result = self
            .request(
                reqwest::Method::GET,
                "/v1/search",
                &[
                    ("q", query.to_string()),
                    ("type", "track".to_string()),
                    ("limit", "8".to_string()),
                    ("market", market.to_string()),
                ],
                None,
            )
            .await;
        match result {
            Ok(response) => {
                let body: Value = response.json().await?;
                Ok(SearchResponse {
                    tracks: parse_search(&body),
                    degraded: false,
                })
            }
            Err(err) if is_degradable(&err) => Ok(SearchResponse {
                tracks: Vec::new(),
                degraded: true,
            }),
            Err(err) => Err(err),
        }
    }

    pub async fn resolve_track(
        &self,
        title: &str,
        artist: &str,
        market: &str,
    ) -> AppResult<Option<TrackDto>> {
        let q = format!(
            "track:\"{}\" artist:\"{}\"",
            title.replace('"', " "),
            artist.replace('"', " ")
        );
        let response = self
            .request(
                reqwest::Method::GET,
                "/v1/search",
                &[
                    ("q", q),
                    ("type", "track".to_string()),
                    ("limit", "1".to_string()),
                    ("market", market.to_string()),
                ],
                None,
            )
            .await?;
        let body: Value = response.json().await?;
        Ok(parse_search(&body).into_iter().next())
    }

    pub async fn add_items(&self, playlist_id: &str, uris: &[String]) -> AppResult<()> {
        if uris.is_empty() {
            return Ok(());
        }
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/v1/playlists/{playlist_id}/items"),
                &[],
                Some(json!({ "uris": uris })),
            )
            .await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(AppError::internal(format!(
                "playlist insert failed with {}",
                response.status()
            )))
        }
    }

    pub async fn me(&self) -> AppResult<String> {
        let response = self
            .request(reqwest::Method::GET, "/v1/me", &[], None)
            .await?;
        let body: Value = response.json().await?;
        body.get("id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .ok_or_else(|| AppError::internal("Spotify did not return a user id."))
    }

    pub async fn playlist_owner(&self, playlist_id: &str) -> AppResult<String> {
        let response = self
            .request(
                reqwest::Method::GET,
                &format!("/v1/playlists/{playlist_id}"),
                &[("fields", "id,owner".to_string())],
                None,
            )
            .await?;
        let body: Value = response.json().await?;
        body.pointer("/owner/id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .ok_or_else(|| AppError::not_found("Playlist has no owner."))
    }

    pub async fn check_owner(&self, playlist_id: &str) -> AppResult<bool> {
        let me = self.me().await?;
        let owner = self.playlist_owner(playlist_id).await?;
        Ok(me == owner)
    }

    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<Value>,
    ) -> AppResult<reqwest::Response> {
        let url = format!("{}{}", self.base, path);
        let mut auth_retried = false;
        let mut rate_retried = false;
        let mut server_retries = 0u8;

        loop {
            let token = self.tokens.access_token().await?;
            let mut request = self.http.request(method.clone(), &url).bearer_auth(token);
            if !query.is_empty() {
                request = request.query(query);
            }
            if let Some(payload) = &body {
                request = request.json(payload);
            }

            let response = match request.send().await {
                Ok(response) => response,
                Err(err) => {
                    self.network_ok.store(false, Ordering::Relaxed);
                    return Err(AppError::offline(err.to_string()));
                }
            };
            self.network_ok.store(true, Ordering::Relaxed);

            let status = response.status();
            if status.as_u16() == 401 && !auth_retried {
                auth_retried = true;
                self.tokens.force_refresh().await?;
                continue;
            }
            if status.as_u16() == 429 {
                let retry_after = response
                    .headers()
                    .get("retry-after")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok());
                let text = response.text().await.unwrap_or_default();
                if text.contains("QUOTA_EXCEEDED") {
                    return Err(AppError::quota("Spotify quota exceeded."));
                }
                if !rate_retried {
                    rate_retried = true;
                    let wait = retry_after.unwrap_or(1).clamp(1, 10);
                    tokio::time::sleep(Duration::from_secs(wait)).await;
                    continue;
                }
                return Err(AppError::rate_limited("Spotify rate limit reached."));
            }
            if status.is_server_error() && server_retries < 2 {
                server_retries += 1;
                let jitter = rand::random::<u64>() % 250;
                tokio::time::sleep(Duration::from_millis(200 + jitter)).await;
                continue;
            }
            if status.as_u16() == 404 {
                return Err(AppError::not_found("Spotify item not found."));
            }
            if !status.is_success() {
                let text = response.text().await.unwrap_or_default();
                return Err(AppError::internal(format!(
                    "Spotify returned {status}: {}",
                    text.chars().take(200).collect::<String>()
                )));
            }
            return Ok(response);
        }
    }
}

fn is_degradable(err: &AppError) -> bool {
    matches!(err.code(), AUTH_REQUIRED | OFFLINE | QUOTA | RATE_LIMITED)
}

pub fn parse_search(body: &Value) -> Vec<TrackDto> {
    let Some(items) = body.pointer("/tracks/items").and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let id = item.get("id")?.as_str()?.to_string();
            let uri = item.get("uri")?.as_str()?.to_string();
            let title = item.get("name")?.as_str().unwrap_or_default().to_string();
            let artists = item
                .pointer("/artists")
                .and_then(Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(|artist| artist.get("name").and_then(Value::as_str))
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let album_value = item.get("album");
            let album = album_value
                .and_then(|album| album.get("name"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let album_id = album_value
                .and_then(|album| album.get("id"))
                .and_then(Value::as_str)
                .map(ToString::to_string);
            let images = album_value
                .and_then(|album| album.get("images"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            Some(TrackDto {
                id,
                uri,
                title,
                artists,
                album,
                album_id,
                image640: pick_image(&images, 640),
                image64: pick_image(&images, 64),
            })
        })
        .collect()
}

fn pick_image(images: &[Value], target: u32) -> Option<String> {
    let mut candidates: Vec<(u32, String)> = images
        .iter()
        .filter_map(|image| {
            let url = image.get("url")?.as_str()?.to_string();
            let width = image
                .get("width")
                .and_then(Value::as_u64)
                .map(|w| w as u32)
                .unwrap_or(0);
            Some((width, url))
        })
        .collect();
    if candidates.is_empty() {
        return None;
    }
    if candidates.iter().all(|(width, _)| *width == 0) {
        return if target < 300 {
            candidates.pop().map(|(_, url)| url)
        } else {
            candidates.into_iter().next().map(|(_, url)| url)
        };
    }
    candidates.sort_by_key(|(width, _)| width.abs_diff(target));
    candidates.into_iter().next().map(|(_, url)| url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_results_and_images() {
        let body = json!({
            "tracks": {
                "items": [{
                    "id": "track-1",
                    "uri": "spotify:track:track-1",
                    "name": "Song",
                    "artists": [{"name": "Artist"}, {"name": "Guest"}],
                    "album": {
                        "id": "album-1",
                        "name": "Album",
                        "images": [
                            {"url": "https://img/64.jpg", "width": 64},
                            {"url": "https://img/300.jpg", "width": 300},
                            {"url": "https://img/640.jpg", "width": 640}
                        ]
                    }
                }]
            }
        });
        let tracks = parse_search(&body);
        assert_eq!(tracks.len(), 1);
        let track = &tracks[0];
        assert_eq!(track.title, "Song");
        assert_eq!(track.artists, vec!["Artist", "Guest"]);
        assert_eq!(track.album_id.as_deref(), Some("album-1"));
        assert_eq!(track.image640.as_deref(), Some("https://img/640.jpg"));
        assert_eq!(track.image64.as_deref(), Some("https://img/64.jpg"));
    }

    #[test]
    fn missing_album_images_falls_back_none() {
        let body = json!({"tracks": {"items": [{
            "id": "t", "uri": "spotify:track:t", "name": "x",
            "artists": [], "album": {"name": "a"}
        }]}});
        let tracks = parse_search(&body);
        assert_eq!(tracks.len(), 1);
        assert!(tracks[0].image640.is_none());
    }

    #[test]
    fn degradable_errors_are_not_fatal() {
        assert!(is_degradable(&AppError::offline("x")));
        assert!(is_degradable(&AppError::quota("x")));
        assert!(!is_degradable(&AppError::internal("x")));
    }
}
