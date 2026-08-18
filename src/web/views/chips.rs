use askama::Template;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use chrono::{Duration, Utc};
use serde::Deserialize;

use super::{render, LayoutCtx};
use crate::auth::AdminUser;
use crate::domain::chip::{Chip, EnrollmentSession};
use crate::{AppError, AppResult, AppState};

pub struct ChipRow {
    pub chip: Chip,
    pub employee_name: Option<String>,
}

impl ChipRow {
    pub fn is_assigned_to(&self, eid: &i64) -> bool {
        self.chip.employee_id == Some(*eid)
    }
}

pub struct EmpOpt {
    pub id: i64,
    pub name: String,
}

#[derive(Template)]
#[template(path = "chips.html")]
struct ListTpl {
    layout: LayoutCtx,
    chips: Vec<ChipRow>,
    active_enrollment: Option<EnrollmentSession>,
    wallboxes: Vec<(i64, String)>,
    employees: Vec<EmpOpt>,
}

pub async fn list(State(state): State<AppState>, AdminUser(user): AdminUser) -> AppResult<Response> {
    let rows: Vec<(
        i64,
        String,
        Option<String>,
        Option<i64>,
        String,
        i64,
        Option<String>,
        String,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT c.id, c.id_tag, c.label, c.employee_id, c.kind, c.enabled, c.expires_at, c.created_at,
                e.display_name
         FROM chips c LEFT JOIN employees e ON e.id = c.employee_id
         ORDER BY c.created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;

    let chips: Vec<ChipRow> = rows
        .into_iter()
        .map(|r| ChipRow {
            chip: Chip {
                id: r.0,
                id_tag: r.1,
                label: r.2,
                employee_id: r.3,
                kind: r.4,
                enabled: r.5,
                expires_at: r.6,
                created_at: r.7,
            },
            employee_name: r.8,
        })
        .collect();

    let active: Option<EnrollmentSession> = sqlx::query_as::<_, EnrollmentSession>(
        "SELECT * FROM enrollment_sessions
         WHERE started_by = ?1
           AND consumed = 0
           AND datetime(expires_at) > datetime('now')
         ORDER BY id DESC LIMIT 1",
    )
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?;

    let wallboxes: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, name FROM wallboxes ORDER BY name")
            .fetch_all(&state.db)
            .await?;
    let emp_rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, display_name FROM employees WHERE active = 1 ORDER BY display_name",
    )
    .fetch_all(&state.db)
    .await?;
    let employees: Vec<EmpOpt> = emp_rows
        .into_iter()
        .map(|(id, name)| EmpOpt { id, name })
        .collect();

    Ok(render(&ListTpl {
        layout: LayoutCtx::new("chips", Some(user)),
        chips,
        active_enrollment: active,
        wallboxes,
        employees,
    })?
    .into_response())
}

#[derive(Deserialize)]
pub struct UpdateForm {
    pub employee_id: Option<String>,
    pub label: Option<String>,
    pub kind: Option<String>,
    pub enabled: Option<String>,
    pub expires_at: Option<String>,
}

pub async fn update(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
    Form(form): Form<UpdateForm>,
) -> AppResult<Response> {
    let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM chips WHERE id = ?1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    // Leerer Select-Wert => Zuordnung entfernen.
    let employee_id: Option<i64> = form
        .employee_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse().ok());

    let label = form
        .label
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let kind = form.kind.as_deref().unwrap_or("employee");
    if kind != "employee" && kind != "guest" {
        return Err(AppError::BadRequest("Ungültige Kategorie.".into()));
    }

    let enabled: i64 = if form.enabled.as_deref() == Some("1") { 1 } else { 0 };

    let expires_at = form
        .expires_at
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    sqlx::query(
        "UPDATE chips
            SET employee_id = ?1,
                label       = ?2,
                kind        = ?3,
                enabled     = ?4,
                expires_at  = ?5
          WHERE id = ?6",
    )
    .bind(employee_id)
    .bind(&label)
    .bind(kind)
    .bind(enabled)
    .bind(&expires_at)
    .bind(id)
    .execute(&state.db)
    .await?;

    Ok(Redirect::to("/chips").into_response())
}

#[derive(Deserialize)]
pub struct EnrollForm {
    pub wallbox_id: Option<i64>,
}

pub async fn enroll_start(
    State(state): State<AppState>,
    AdminUser(user): AdminUser,
    Form(form): Form<EnrollForm>,
) -> AppResult<Response> {
    let expires = Utc::now() + Duration::minutes(2);
    let res = sqlx::query(
        "INSERT INTO enrollment_sessions (started_by, wallbox_id, expires_at)
         VALUES (?1, ?2, ?3)",
    )
    .bind(user.id)
    .bind(form.wallbox_id)
    .bind(expires.to_rfc3339())
    .execute(&state.db)
    .await?;
    let id = res.last_insert_rowid();
    Ok(Redirect::to(&format!("/chips/enroll/{id}")).into_response())
}

#[derive(Template)]
#[template(path = "chip_enroll.html")]
struct EnrollTpl {
    layout: LayoutCtx,
    session: EnrollmentSession,
    employees: Vec<(i64, String)>,
}

pub async fn enroll_poll(
    State(state): State<AppState>,
    AdminUser(user): AdminUser,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let sess: EnrollmentSession = sqlx::query_as::<_, EnrollmentSession>(
        "SELECT * FROM enrollment_sessions WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let employees: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, display_name FROM employees WHERE active = 1 ORDER BY display_name",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(render(&EnrollTpl {
        layout: LayoutCtx::new("chips", Some(user)),
        session: sess,
        employees,
    })?
    .into_response())
}

#[derive(Deserialize)]
pub struct EnrollSave {
    pub label: Option<String>,
    pub employee_id: Option<i64>,
    pub kind: String,
    pub expires_at: Option<String>,
}

pub async fn enroll_save(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
    Form(form): Form<EnrollSave>,
) -> AppResult<Response> {
    let sess: EnrollmentSession = sqlx::query_as::<_, EnrollmentSession>(
        "SELECT * FROM enrollment_sessions WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let Some(tag) = sess.captured_id_tag.as_deref() else {
        return Err(AppError::BadRequest(
            "Bisher wurde kein Chip erkannt – bitte an die Wallbox halten.".into(),
        ));
    };
    if sess.consumed != 0 {
        return Err(AppError::BadRequest("Enrollment bereits abgeschlossen.".into()));
    }
    if form.kind != "employee" && form.kind != "guest" {
        return Err(AppError::BadRequest("Ungültige Kategorie.".into()));
    }
    let expires_at = form
        .expires_at
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM chips WHERE id_tag = ?1")
        .bind(tag)
        .fetch_optional(&state.db)
        .await?;
    if exists.is_some() {
        return Err(AppError::Conflict(format!(
            "Chip-Tag {tag} ist bereits registriert."
        )));
    }

    sqlx::query(
        "INSERT INTO chips (id_tag, label, employee_id, kind, enabled, expires_at)
         VALUES (?1, ?2, ?3, ?4, 1, ?5)",
    )
    .bind(tag)
    .bind(form.label.as_deref())
    .bind(form.employee_id)
    .bind(&form.kind)
    .bind(expires_at)
    .execute(&state.db)
    .await?;

    sqlx::query("UPDATE enrollment_sessions SET consumed = 1 WHERE id = ?1")
        .bind(id)
        .execute(&state.db)
        .await?;

    Ok(Redirect::to("/chips").into_response())
}

pub async fn delete(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    sqlx::query("DELETE FROM chips WHERE id = ?1")
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(Redirect::to("/chips").into_response())
}
