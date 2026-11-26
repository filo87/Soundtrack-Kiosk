use std::collections::HashMap;
use std::time::Duration;

use tauri_plugin_opener::OpenerExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

use crate::error::{AppError, AppResult};
use crate::spotify::pkce;
use crate::spotify::token::{exchange_code, TokenManager};

pub const CALLBACK_PORT: u16 = 43821;
pub const CALLBACK_PATH: &str = "/callback";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const SCOPES: &str = "playlist-modify-public playlist-modify-private playlist-read-private";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallbackParse {
    Callback(Vec<(String, String)>),
    Ignore,
}

pub fn callback_redirect_uri() -> String {
    format!("http://127.0.0.1:{CALLBACK_PORT}{CALLBACK_PATH}")
}

pub fn parse_request_line(line: &str) -> CallbackParse {
    let mut parts = line.split_whitespace();
    let Some(method) = parts.next() else {
        return CallbackParse::Ignore;
    };
    let Some(target) = parts.next() else {
        return CallbackParse::Ignore;
    };
    if !method.eq_ignore_ascii_case("GET") {
        return CallbackParse::Ignore;
    }
    let (path, query) = match target.split_once('?') {
        Some((path, query)) => (path, query),
        None => (target, ""),
    };
    if path != CALLBACK_PATH {
        return CallbackParse::Ignore;
    }
    let pairs = url::form_urlencoded::parse(query.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    CallbackParse::Callback(pairs)
}

pub fn build_authorize_url(
    authorize_base: &str,
    client_id: &str,
    challenge: &str,
    state: &str,
) -> AppResult<String> {
    let mut url = url::Url::parse(authorize_base)
        .map_err(|e| AppError::internal(format!("bad authorize URL: {e}")))?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", &callback_redirect_uri())
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", challenge)
        .append_pair("state", state)
        .append_pair("scope", SCOPES);
    Ok(url.to_string())
}

pub struct LoginEndpoints {
    pub authorize_base: String,
    pub token_url: String,
}

pub async fn run_login(
    app: &tauri::AppHandle,
    http: &reqwest::Client,
    client_id: &str,
    endpoints: &LoginEndpoints,
    tokens: &TokenManager,
) -> AppResult<()> {
    if client_id.trim().is_empty() {
        return Err(AppError::invalid_input("Set a Spotify client id first."));
    }
    let pair = pkce::generate();
    let state = pkce::random_state();
    let listener = std::net::TcpListener::bind(("127.0.0.1", CALLBACK_PORT))
        .map_err(|e| AppError::internal(format!("Could not open the login callback port: {e}")))?;
    listener.set_nonblocking(true)?;
    let listener = TcpListener::from_std(listener)?;

    let authorize_url = build_authorize_url(
        &endpoints.authorize_base,
        client_id,
        &pair.challenge,
        &state,
    )?;
    app.opener()
        .open_url(authorize_url, None::<&str>)
        .map_err(|e| AppError::internal(format!("Could not open the browser: {e}")))?;

    let code = timeout(LOGIN_TIMEOUT, accept_callback(listener, &state))
        .await
        .map_err(|_| AppError::auth_required("Spotify login timed out."))??;

    let tokens_value = exchange_code(
        http,
        &endpoints.token_url,
        client_id,
        &code,
        &pair.verifier,
        &callback_redirect_uri(),
    )
    .await?;
    tokens.store(tokens_value).await?;
    Ok(())
}

async fn accept_callback(listener: TcpListener, expected_state: &str) -> AppResult<String> {
    loop {
        let (mut socket, _) = listener.accept().await?;
        let mut buffer = [0u8; 8192];
        let read = match timeout(Duration::from_secs(5), socket.read(&mut buffer)).await {
            Ok(Ok(read)) => read,
            _ => {
                let _ = respond(&mut socket, 408, "Timed out").await;
                continue;
            }
        };
        let request = String::from_utf8_lossy(&buffer[..read]).to_string();
        let first_line = request.lines().next().unwrap_or_default();
        match parse_request_line(first_line) {
            CallbackParse::Ignore => {
                let _ = respond(&mut socket, 404, "Not found").await;
            }
            CallbackParse::Callback(pairs) => {
                let params: HashMap<String, String> = pairs.into_iter().collect();
                if let Some(error) = params.get("error") {
                    let _ = respond(&mut socket, 400, "Login was cancelled.").await;
                    return Err(AppError::auth_required(format!(
                        "Spotify login failed: {error}"
                    )));
                }
                if params.get("state").map(String::as_str) != Some(expected_state) {
                    let _ = respond(&mut socket, 400, "Invalid login state.").await;
                    return Err(AppError::auth_required("Login state did not match."));
                }
                let Some(code) = params.get("code").filter(|c| !c.is_empty()) else {
                    let _ = respond(&mut socket, 400, "Missing code.").await;
                    return Err(AppError::auth_required("Spotify returned no code."));
                };
                let code = code.clone();
                let _ = respond(
                    &mut socket,
                    200,
                    "Done, you can close this tab and return to the kiosk.",
                )
                .await;
                return Ok(code);
            }
        }
    }
}

async fn respond(socket: &mut TcpStream, status: u16, message: &str) -> AppResult<()> {
    let body = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Spotify login</title>\
         <body style=\"font-family:system-ui;padding:3rem;background:#FBF8F4;color:#1E1B18\">\
         <h1 style=\"font-weight:500\">{message}</h1></body>"
    );
    let response = format!(
        "HTTP/1.1 {status} OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    socket.write_all(response.as_bytes()).await?;
    socket.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_callback_request() {
        match parse_request_line("GET /callback?code=abc&state=xyz HTTP/1.1") {
            CallbackParse::Callback(pairs) => {
                let map: HashMap<_, _> = pairs.into_iter().collect();
                assert_eq!(map.get("code").map(String::as_str), Some("abc"));
                assert_eq!(map.get("state").map(String::as_str), Some("xyz"));
            }
            CallbackParse::Ignore => panic!("should parse"),
        }
    }

    #[test]
    fn ignores_favicon() {
        assert_eq!(
            parse_request_line("GET /favicon.ico HTTP/1.1"),
            CallbackParse::Ignore
        );
        assert_eq!(
            parse_request_line("POST /callback?code=1 HTTP/1.1"),
            CallbackParse::Ignore
        );
        assert_eq!(parse_request_line("garbage"), CallbackParse::Ignore);
    }

    #[test]
    fn detects_denied_and_mismatched_state() {
        match parse_request_line("GET /callback?error=access_denied&state=abc HTTP/1.1") {
            CallbackParse::Callback(pairs) => {
                let map: HashMap<_, _> = pairs.into_iter().collect();
                assert_eq!(map.get("error").map(String::as_str), Some("access_denied"));
            }
            CallbackParse::Ignore => panic!("should parse"),
        }
    }

    #[test]
    fn authorize_url_contains_pkce_and_scopes() {
        let url = build_authorize_url(
            "https://accounts.spotify.com/authorize",
            "client",
            "challenge",
            "state",
        )
        .unwrap();
        assert!(url.contains("code_challenge=challenge"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A43821%2Fcallback"));
        assert!(url.contains("playlist-modify-public"));
    }
}
