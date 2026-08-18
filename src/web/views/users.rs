use askama::Template;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;

use super::{render, LayoutCtx};
use crate::auth::AdminUser;
use crate::db::hash_password;
use crate::domain::user::User;
use crate::{AppError, AppResult, AppState};

#[derive(Template)]
#[template(path = "users.html")]
struct ListTpl {
    layout: LayoutCtx,
    users: Vec<User>,
}

pub async fn list(State(state): State<AppState>, AdminUser(user): AdminUser) -> AppResult<Response> {
    let users: Vec<User> =
        sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY display_name")
            .fetch_all(&state.db)
            .await?;
    let tpl = ListTpl {
        layout: LayoutCtx::new("users", Some(user)),
        users,
    };
    Ok(render(&tpl)?.into_response())
}

#[derive(Deserialize)]
pub struct CreateForm {
    pub username: String,
    pub display_name: String,
    pub email: Option<String>,
    pub role: String,
    pub password: String,
}

pub async fn create(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Form(form): Form<CreateForm>,
) -> AppResult<Response> {
    let username = form.username.trim();
    let display = form.display_name.trim();
    if username.is_empty() || display.is_empty() || form.password.len() < 6 {
        return Err(AppError::BadRequest(
            "Benutzername, Name und Passwort (≥6 Zeichen) sind Pflicht.".into(),
        ));
    }
    if form.role != "admin" && form.role != "user" {
        return Err(AppError::BadRequest("Ungültige Rolle.".into()));
    }
    let hash = hash_password(&form.password).map_err(AppError::Other)?;
    let email = form.email.as_deref().map(str::trim).filter(|s| !s.is_empty());

    let res = sqlx::query(
        "INSERT INTO users (username, display_name, email, role, auth_source, password_hash)
         VALUES (?1, ?2, ?3, ?4, 'local', ?5)",
    )
    .bind(username)
    .bind(display)
    .bind(email)
    .bind(&form.role)
    .bind(hash)
    .execute(&state.db)
    .await;
    if let Err(sqlx::Error::Database(db)) = &res {
        if db.is_unique_violation() {
            return Err(AppError::Conflict(format!(
                "Benutzername '{username}' existiert bereits."
            )));
        }
    }
    res?;
    Ok(Redirect::to("/users").into_response())
}

pub async fn delete(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    if id == admin.id {
        return Err(AppError::BadRequest(
            "Sie können sich nicht selbst löschen.".into(),
        ));
    }
    sqlx::query("DELETE FROM users WHERE id = ?1")
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(Redirect::to("/users").into_response())
}

#[derive(Deserialize)]
pub struct PwForm {
    pub password: String,
}

pub async fn set_password(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
    Form(form): Form<PwForm>,
) -> AppResult<Response> {
    if form.password.len() < 6 {
        return Err(AppError::BadRequest(
            "Passwort muss mindestens 6 Zeichen haben.".into(),
        ));
    }
    let hash = hash_password(&form.password).map_err(AppError::Other)?;
    sqlx::query("UPDATE users SET password_hash = ?1 WHERE id = ?2 AND auth_source = 'local'")
        .bind(hash)
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(Redirect::to("/users").into_response())
}
