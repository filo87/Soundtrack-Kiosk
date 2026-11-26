use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

use super::itunes::ItunesCandidate;

pub fn normalize(input: &str) -> String {
    let decomposed: String = input.nfkd().filter(|c| !is_combining_mark(*c)).collect();
    let lowered = decomposed.to_lowercase();
    let stripped = strip_bracketed(&lowered);
    let stem = strip_suffixes(&stripped);
    let mut out = String::with_capacity(stem.len());
    let mut last_space = true;
    for ch in stem.chars() {
        if ch.is_alphanumeric() {
            out.push(ch);
            last_space = false;
        } else if !last_space {
            out.push(' ');
            last_space = true;
        }
    }
    out.trim().to_string()
}

fn strip_bracketed(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut depth = 0i32;
    for ch in input.chars() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out
}

fn strip_suffixes(input: &str) -> String {
    let markers = [
        "- remaster",
        "- live",
        "- single",
        "- radio edit",
        "- album version",
        "- mono",
        "- stereo",
    ];
    let mut current = input.trim().to_string();
    loop {
        let before = current.clone();
        for marker in markers {
            if let Some(index) = current.find(marker) {
                current = current[..index].trim_end().to_string();
            }
        }
        if current == before {
            break;
        }
    }
    current
}

pub fn song_identity(title: &str, artist: &str) -> String {
    format!("{}|{}", normalize(title), normalize(artist))
}

pub fn title_matches(a: &str, b: &str) -> bool {
    let a = normalize(a);
    let b = normalize(b);
    if a.is_empty() || b.is_empty() {
        return false;
    }
    a == b || (a.len() >= 3 && b.len() >= 3 && (safe_prefix(&a, &b) || safe_prefix(&b, &a)))
}

fn safe_prefix(shorter: &str, longer: &str) -> bool {
    let Some(rest) = longer.strip_prefix(shorter) else {
        return false;
    };
    let next = rest.trim_start().chars().next();
    next.map_or(true, |ch| !ch.is_ascii_digit())
}

pub fn artist_matches(candidate: &str, wanted: &str) -> bool {
    let mut wanted_norms = vec![normalize(wanted)];
    wanted_norms.extend(
        artist_parts(wanted)
            .into_iter()
            .map(|part| normalize(&part)),
    );
    wanted_norms.retain(|value| !value.is_empty());
    if wanted_norms.is_empty() {
        return false;
    }
    artist_parts(candidate).into_iter().any(|part| {
        let part = normalize(&part);
        !part.is_empty()
            && wanted_norms.iter().any(|wanted| {
                part == *wanted
                    || (part.len() >= 4
                        && wanted.len() >= 4
                        && (part.starts_with(wanted) || wanted.starts_with(&part)))
            })
    })
}

fn artist_parts(input: &str) -> Vec<String> {
    let markers = ["feat.", "feat ", " with ", " x "];
    let mut parts = Vec::new();
    for raw in input.split([',', '&', '/', ';']) {
        let lower = raw.to_ascii_lowercase();
        let cut = markers.iter().filter_map(|m| lower.find(m)).min();
        let part = match cut {
            Some(index) => &raw[..index],
            None => raw,
        };
        let trimmed = part.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_string());
        }
    }
    parts
}

pub fn is_strict_match(candidate: &ItunesCandidate, title: &str, artist: &str) -> bool {
    title_matches(&candidate.track_name, title) && artist_matches(&candidate.artist_name, artist)
}

pub fn select_best<'a>(
    candidates: &'a [ItunesCandidate],
    title: &str,
    artist: &str,
    album: Option<&str>,
) -> Option<&'a ItunesCandidate> {
    let mut matches: Vec<&ItunesCandidate> = candidates
        .iter()
        .filter(|c| is_strict_match(c, title, artist))
        .collect();
    if let Some(album) = album {
        let album_norm = normalize(album);
        matches.sort_by_key(|c| normalize(&c.collection_name) != album_norm);
    }
    matches.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(track: &str, artist: &str, album: &str) -> ItunesCandidate {
        ItunesCandidate {
            track_name: track.to_string(),
            artist_name: artist.to_string(),
            collection_name: album.to_string(),
            artwork_url: "https://x/100x100bb.jpg".to_string(),
        }
    }

    #[test]
    fn normalizes_diacritics_and_suffixes() {
        assert_eq!(normalize("Beyoncé - Halo (feat. X)"), "beyonce halo");
        assert_eq!(normalize("Song - Remastered 2011"), "song");
        assert_eq!(normalize("Song [Live]"), "song");
        assert_eq!(normalize("  Café   del  Mar "), "cafe del mar");
    }

    #[test]
    fn song_identity_ignores_case_and_remaster_suffixes() {
        assert_eq!(
            song_identity("Wonderwall - Remastered", "Oasis"),
            song_identity("wonderwall", "OASIS")
        );
        assert_ne!(
            song_identity("Wonderwall", "Oasis"),
            song_identity("Wonderwall", "Ryan Adams")
        );
    }

    #[test]
    fn title_matching_handles_prefixes_and_features() {
        assert!(title_matches("Halo", "Halo (feat. Beyoncé)"));
        assert!(title_matches("Halo - Remastered", "Halo"));
        assert!(!title_matches("Halo", "Halo 2"));
        assert!(!title_matches("", "Halo"));
    }

    #[test]
    fn artist_matching_handles_lists() {
        assert!(artist_matches("Beyoncé", "Beyonce"));
        assert!(artist_matches("Jay-Z & Beyoncé", "Beyonce"));
        assert!(artist_matches("Beyoncé feat. Jay-Z", "Beyonce"));
        assert!(artist_matches("Jay-Z", "Beyoncé, Jay-Z"));
        assert!(!artist_matches("Adele", "Beyonce"));
    }

    #[test]
    fn strict_match_requires_both_and_prefers_album() {
        let candidates = vec![
            candidate("Halo", "Beyoncé", "Other Album"),
            candidate("Halo", "Beyoncé", "I Am... Sasha Fierce"),
            candidate("Halo", "Adele", "I Am... Sasha Fierce"),
        ];
        let best = select_best(&candidates, "Halo", "Beyonce", Some("I Am... Sasha Fierce"))
            .expect("match");
        assert_eq!(best.artist_name, "Beyoncé");
        assert_eq!(best.collection_name, "I Am... Sasha Fierce");
    }

    #[test]
    fn wrong_artist_is_rejected() {
        let candidates = vec![candidate("Halo", "Adele", "Album")];
        assert!(select_best(&candidates, "Halo", "Beyonce", None).is_none());
    }

    #[test]
    fn non_latin_titles_match() {
        assert!(title_matches("夜に駆ける", "夜に駆ける - Remastered"));
        assert!(artist_matches("YOASOBI", "YOASOBI"));
    }
}
