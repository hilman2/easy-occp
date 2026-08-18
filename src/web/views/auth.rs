use askama::Template;
use axum::extract::State;
use axum::http::header;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;

use super::{render, LayoutCtx};
use crate::auth::session::{clear_cookie_header, cookie_header, COOKIE_NAME};
use crate::auth::{authenticate_username_password, session};
use crate::{AppResult, AppState};

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate {
    layout: LayoutCtx,
    error: Option<String>,
}

pub async fn login_form() -> AppResult<Response> {
    let tpl = LoginTemplate {
        layout: LayoutCtx::new("login", None),
        error: None,
    };
    Ok(render(&tpl)?.into_response())
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

pub async fn login_post(
    State(state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> AppResult<Response> {
    match authenticate_username_password(&state, form.username.trim(), &form.password).await {
        Ok(user) => {
            let token = session::create(&state, user.id).await?;
            let cookie = cookie_header(&token, false);
            let mut resp = Redirect::to("/").into_response();
            resp.headers_mut().insert(
                header::SET_COOKIE,
                axum::http::HeaderValue::from_str(&cookie).unwrap(),
            );
            Ok(resp)
        }
        Err(_) => {
            let tpl = LoginTemplate {
                layout: LayoutCtx::new("login", None),
                error: Some("Benutzername oder Passwort falsch.".into()),
            };
            Ok(render(&tpl)?.into_response())
        }
    }
}

pub async fn logout(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> AppResult<Response> {
    if let Some(ck) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
        if let Some(token) = ck.split(';').find_map(|p| {
            p.trim().strip_prefix(&format!("{COOKIE_NAME}=")).map(str::to_string)
        }) {
            let _ = session::destroy(&state, &token).await;
        }
    }
    let mut resp = Redirect::to("/login").into_response();
    resp.headers_mut().insert(
        header::SET_COOKIE,
        axum::http::HeaderValue::from_str(&clear_cookie_header()).unwrap(),
    );
    Ok(resp)
}
