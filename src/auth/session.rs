use axum::http::request::Parts;
use chrono::Utc;
use rand::RngCore;

use crate::domain::user::{session_expires_at, User};
use crate::{AppError, AppResult, AppState};

pub const COOKIE_NAME: &str = "easy_occp_sid";

pub fn new_token() -> String {
    let mut buf = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut buf);
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf)
}

pub async fn create(state: &AppState, user_id: i64) -> AppResult<String> {
    let token = new_token();
    let expires = session_expires_at();
    sqlx::query(
        "INSERT INTO sessions (token, user_id, expires_at) VALUES (?1, ?2, ?3)",
    )
    .bind(&token)
    .bind(user_id)
    .bind(expires.to_rfc3339())
    .execute(&state.db)
    .await?;
    Ok(token)
}

pub async fn destroy(state: &AppState, token: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM sessions WHERE token = ?1")
        .bind(token)
        .execute(&state.db)
        .await?;
    Ok(())
}

pub async fn resolve(state: &AppState, parts: &Parts) -> AppResult<Option<User>> {
    let Some(cookie_header) = parts.headers.get(axum::http::header::COOKIE) else {
        return Ok(None);
    };
    let s = cookie_header.to_str().map_err(|_| AppError::Unauthorized)?;
    let token = s
        .split(';')
        .filter_map(|part| {
            let part = part.trim();
            part.strip_prefix(&format!("{COOKIE_NAME}="))
        })
        .next();
    let Some(token) = token else {
        return Ok(None);
    };

    let row: Option<(i64, String)> =
        sqlx::query_as("SELECT user_id, expires_at FROM sessions WHERE token = ?1")
            .bind(token)
            .fetch_optional(&state.db)
            .await?;
    let Some((uid, exp)) = row else {
        return Ok(None);
    };
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&exp) {
        if dt < Utc::now() {
            let _ = sqlx::query("DELETE FROM sessions WHERE token = ?1")
                .bind(token)
                .execute(&state.db)
                .await;
            return Ok(None);
        }
    }
    let user: Option<User> = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE id = ?1 AND disabled = 0",
    )
    .bind(uid)
    .fetch_optional(&state.db)
    .await?;
    Ok(user)
}

pub fn cookie_header(token: &str, secure: bool) -> String {
    let mut parts = vec![
        format!("{COOKIE_NAME}={token}"),
        "HttpOnly".into(),
        "SameSite=Lax".into(),
        "Path=/".into(),
        format!("Max-Age={}", 12 * 60 * 60),
    ];
    if secure {
        parts.push("Secure".into());
    }
    parts.join("; ")
}

pub fn clear_cookie_header() -> String {
    format!("{COOKIE_NAME}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0")
}
