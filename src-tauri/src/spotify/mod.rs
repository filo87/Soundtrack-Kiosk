pub mod api;
pub mod auth;
pub mod pkce;
pub mod token;

pub const API_BASE: &str = "https://api.spotify.com";
pub const AUTHORIZE_URL: &str = "https://accounts.spotify.com/authorize";
pub const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
