use serde_json::Value;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItunesCandidate {
    pub track_name: String,
    pub artist_name: String,
    pub collection_name: String,
    pub artwork_url: String,
}

pub fn upgrade_artwork_url(url: &str) -> Option<String> {
    let (prefix, file) = url.rsplit_once('/')?;
    let (stem, ext) = file.rsplit_once('.')?;
    let ext_lower = ext.to_ascii_lowercase();
    if !matches!(ext_lower.as_str(), "jpg" | "jpeg" | "png" | "webp") {
        return None;
    }
    let (width, height) = stem.strip_suffix("bb")?.split_once('x')?;
    if width.is_empty()
        || height.is_empty()
        || !width.chars().all(|c| c.is_ascii_digit())
        || !height.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    Some(format!("{prefix}/3000x3000bb.{ext}"))
}

pub async fn search(
    http: &reqwest::Client,
    base: &str,
    term: &str,
    country: &str,
) -> AppResult<Vec<ItunesCandidate>> {
    let response = http
        .get(format!("{}/search", base.trim_end_matches('/')))
        .query(&[
            ("term", term),
            ("entity", "song"),
            ("limit", "10"),
            ("country", country),
        ])
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(AppError::internal(format!(
            "iTunes search failed with {}",
            response.status()
        )));
    }
    let body: Value = response.json().await?;
    Ok(parse_results(&body))
}

pub fn parse_results(body: &Value) -> Vec<ItunesCandidate> {
    body.get("results")
        .and_then(Value::as_array)
        .map(|results| {
            results
                .iter()
                .filter_map(|item| {
                    let artwork = item.get("artworkUrl100")?.as_str()?;
                    Some(ItunesCandidate {
                        track_name: item
                            .get("trackName")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        artist_name: item
                            .get("artistName")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        collection_name: item
                            .get("collectionName")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        artwork_url: artwork.to_string(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_artwork_to_3000() {
        assert_eq!(
            upgrade_artwork_url("https://is1.mzstatic.com/image/thumb/x/100x100bb.jpg").as_deref(),
            Some("https://is1.mzstatic.com/image/thumb/x/3000x3000bb.jpg")
        );
        assert_eq!(
            upgrade_artwork_url("https://is1.mzstatic.com/image/thumb/x/600x600bb.webp").as_deref(),
            Some("https://is1.mzstatic.com/image/thumb/x/3000x3000bb.webp")
        );
        assert_eq!(
            upgrade_artwork_url("https://is1.mzstatic.com/image/thumb/x/100x100.jpg"),
            None
        );
        assert_eq!(upgrade_artwork_url("https://example.com/cover.jpg"), None);
    }

    #[test]
    fn parses_itunes_results() {
        let body = serde_json::json!({
            "results": [
                {"trackName": "Song", "artistName": "Artist", "collectionName": "Album", "artworkUrl100": "https://x/100x100bb.jpg"},
                {"trackName": "No art"}
            ]
        });
        let results = parse_results(&body);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].track_name, "Song");
    }
}
