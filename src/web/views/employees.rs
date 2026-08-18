use askama::Template;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;

use super::{render, LayoutCtx};
use crate::auth::AdminUser;
use crate::domain::employee::Employee;
use crate::{AppError, AppResult, AppState};

pub struct EmployeeRow {
    pub emp: Employee,
    pub chip_count: i64,
    pub tx_count: i64,
    pub total_wh: i64,
}

#[derive(Template)]
#[template(path = "employees.html")]
struct ListTpl {
    layout: LayoutCtx,
    employees: Vec<EmployeeRow>,
}

pub async fn list(
    State(state): State<AppState>,
    AdminUser(user): AdminUser,
    lang: crate::i18n::Lang,
) -> AppResult<Response> {
    // Rollup pro Mitarbeiter: Chip-Anzahl + Lade-Summen.
    let rows: Vec<(i64, String, Option<String>, Option<String>, i64, String, i64, i64, i64)> =
        sqlx::query_as(
            "SELECT e.id, e.display_name, e.email, e.department, e.active, e.created_at,
                    (SELECT COUNT(*) FROM chips c WHERE c.employee_id = e.id),
                    (SELECT COUNT(*) FROM transactions t WHERE t.employee_id = e.id),
                    COALESCE((SELECT SUM(stop_meter_wh - start_meter_wh)
                              FROM transactions t
                              WHERE t.employee_id = e.id AND t.stop_meter_wh IS NOT NULL), 0)
             FROM employees e
             ORDER BY e.display_name",
        )
        .fetch_all(&state.db)
        .await?;

    let employees = rows
        .into_iter()
        .map(|r| EmployeeRow {
            emp: Employee {
                id: r.0,
                display_name: r.1,
                email: r.2,
                department: r.3,
                active: r.4,
                created_at: r.5,
            },
            chip_count: r.6,
            tx_count: r.7,
            total_wh: r.8.max(0),
        })
        .collect();

    Ok(render(&ListTpl {
        layout: LayoutCtx::new("employees", Some(user), lang),
        employees,
    })?
    .into_response())
}

#[derive(Deserialize)]
pub struct CreateForm {
    pub display_name: String,
    pub email: Option<String>,
    pub department: Option<String>,
}

pub async fn create(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    lang: crate::i18n::Lang,
    Form(form): Form<CreateForm>,
) -> AppResult<Response> {
    let name = form.display_name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest(lang.t("err.name_required").into()));
    }
    sqlx::query("INSERT INTO employees (display_name, email, department) VALUES (?1, ?2, ?3)")
        .bind(name)
        .bind(form.email.as_deref().map(str::trim).filter(|s| !s.is_empty()))
        .bind(form.department.as_deref().map(str::trim).filter(|s| !s.is_empty()))
        .execute(&state.db)
        .await?;
    Ok(Redirect::to("/employees").into_response())
}

#[derive(Template)]
#[template(path = "employee_detail.html")]
struct DetailTpl {
    layout: LayoutCtx,
    emp: Employee,
    chips: Vec<crate::domain::chip::Chip>,
    recent_tx: Vec<RecentTx>,
}

pub struct RecentTx {
    pub start_time: String,
    pub stop_time: Option<String>,
    pub wallbox: String,
    pub id_tag: String,
    pub energy_wh: i64,
}

pub async fn detail(
    State(state): State<AppState>,
    AdminUser(user): AdminUser,
    Path(id): Path<i64>,
    lang: crate::i18n::Lang,
) -> AppResult<Response> {
    let emp: Employee = sqlx::query_as::<_, Employee>("SELECT * FROM employees WHERE id = ?1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let chips: Vec<crate::domain::chip::Chip> = sqlx::query_as::<_, crate::domain::chip::Chip>(
        "SELECT * FROM chips WHERE employee_id = ?1 ORDER BY created_at DESC",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let tx_rows: Vec<(String, Option<String>, String, String, Option<i64>, i64)> = sqlx::query_as(
        "SELECT t.start_time, t.stop_time, w.name, t.id_tag, t.stop_meter_wh, t.start_meter_wh
         FROM transactions t
         JOIN wallboxes w ON w.id = t.wallbox_id
         WHERE t.employee_id = ?1
         ORDER BY t.start_time DESC
         LIMIT 100",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    let recent_tx = tx_rows
        .into_iter()
        .map(|(st, et, wb, tag, stop_m, start_m)| RecentTx {
            start_time: st,
            stop_time: et,
            wallbox: wb,
            id_tag: tag,
            energy_wh: stop_m.map(|s| (s - start_m).max(0)).unwrap_or(0),
        })
        .collect();

    Ok(render(&DetailTpl {
        layout: LayoutCtx::new("employees", Some(user), lang),
        emp,
        chips,
        recent_tx,
    })?
    .into_response())
}

#[derive(Deserialize)]
pub struct UpdateForm {
    pub display_name: String,
    pub email: Option<String>,
    pub department: Option<String>,
    pub active: Option<String>,
}

pub async fn update(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
    lang: crate::i18n::Lang,
    Form(form): Form<UpdateForm>,
) -> AppResult<Response> {
    let name = form.display_name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest(lang.t("err.name_required").into()));
    }
    let active = if form.active.as_deref() == Some("1") { 1 } else { 0 };
    sqlx::query(
        "UPDATE employees SET display_name = ?1, email = ?2, department = ?3, active = ?4
         WHERE id = ?5",
    )
    .bind(name)
    .bind(form.email.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(form.department.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(active)
    .bind(id)
    .execute(&state.db)
    .await?;
    Ok(Redirect::to(&format!("/employees/{id}")).into_response())
}

pub async fn delete(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    // FK ON DELETE SET NULL bei chips/transactions/users → historische Daten bleiben,
    // Verknüpfung wird gelöst.
    sqlx::query("DELETE FROM employees WHERE id = ?1")
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(Redirect::to("/employees").into_response())
}
