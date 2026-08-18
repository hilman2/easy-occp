use std::fmt::Write as _;

use askama::Template;
use axum::extract::{Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use super::{render, LayoutCtx};
use crate::auth::AuthUser;
use crate::domain::transaction::{fmt_kw, fmt_kwh, live_meter};
use crate::{AppResult, AppState};

pub struct TxRow {
    pub id: i64,
    pub wallbox_name: String,
    pub connector_id: i64,
    pub id_tag: String,
    pub employee_name: Option<String>,
    pub start_time: String,
    pub stop_time: Option<String>,
    /// Geladene Energie, formatiert („12,3“). Bei laufenden Transaktionen der
    /// aktuelle Stand aus den zuletzt gemeldeten MeterValues — None, wenn
    /// (noch) keine Messung vorliegt.
    pub energy_kwh: Option<String>,
    /// Aktuelle Ladeleistung, formatiert — nur bei laufenden Transaktionen gesetzt.
    pub power_kw: Option<String>,
}

#[derive(Template)]
#[template(path = "transactions.html")]
struct ListTpl {
    layout: LayoutCtx,
    rows: Vec<TxRow>,
    filter_employee: Option<String>,
}

#[derive(Deserialize)]
pub struct Filter {
    pub employee: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Query(filter): Query<Filter>,
) -> AppResult<Response> {
    // Nicht-Admins sehen nur ihre eigenen Transaktionen — d.h. die Transaktionen
    // des Mitarbeiters, der mit ihrem Login verknüpft ist.
    let mine_only = !user.is_admin();
    let rows: Vec<(i64, String, i64, String, Option<String>, String, Option<String>, Option<i64>, i64)> = if mine_only {
        sqlx::query_as(
            "SELECT t.id, w.name, t.connector_id, t.id_tag, e.display_name,
                    t.start_time, t.stop_time, t.stop_meter_wh, t.start_meter_wh
             FROM transactions t
             JOIN wallboxes w ON w.id = t.wallbox_id
             LEFT JOIN employees e ON e.id = t.employee_id
             WHERE t.employee_id = (SELECT employee_id FROM users WHERE id = ?1)
             ORDER BY t.start_time DESC
             LIMIT 500",
        )
        .bind(user.id)
        .fetch_all(&state.db)
        .await?
    } else if let Some(name) = filter.employee.as_deref().filter(|s| !s.is_empty()) {
        sqlx::query_as(
            "SELECT t.id, w.name, t.connector_id, t.id_tag, e.display_name,
                    t.start_time, t.stop_time, t.stop_meter_wh, t.start_meter_wh
             FROM transactions t
             JOIN wallboxes w ON w.id = t.wallbox_id
             LEFT JOIN employees e ON e.id = t.employee_id
             WHERE e.display_name LIKE ?1 OR t.guest_label LIKE ?1
             ORDER BY t.start_time DESC
             LIMIT 500",
        )
        .bind(format!("%{name}%"))
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as(
            "SELECT t.id, w.name, t.connector_id, t.id_tag, e.display_name,
                    t.start_time, t.stop_time, t.stop_meter_wh, t.start_meter_wh
             FROM transactions t
             JOIN wallboxes w ON w.id = t.wallbox_id
             LEFT JOIN employees e ON e.id = t.employee_id
             ORDER BY t.start_time DESC
             LIMIT 500",
        )
        .fetch_all(&state.db)
        .await?
    };

    let mut out = Vec::with_capacity(rows.len());
    for (id, wn, cid, tag, un, st, et, stop_m, start_m) in rows {
        // Laufende Transaktionen: aktuellen Stand aus den MeterValues holen.
        let (energy_wh, power_kw) = if et.is_none() {
            let live = live_meter(&state.db, id, start_m).await?;
            (live.energy_wh, live.power_w.map(fmt_kw))
        } else {
            (stop_m.map(|s| (s - start_m).max(0)), None)
        };
        out.push(TxRow {
            id,
            wallbox_name: wn,
            connector_id: cid,
            id_tag: tag,
            employee_name: un,
            start_time: st,
            stop_time: et,
            energy_kwh: energy_wh.map(fmt_kwh),
            power_kw,
        });
    }
    let rows = out;

    Ok(render(&ListTpl {
        layout: LayoutCtx::new("transactions", Some(user)),
        rows,
        filter_employee: filter.employee,
    })?
    .into_response())
}

pub async fn export_csv(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Query(filter): Query<Filter>,
) -> AppResult<Response> {
    let mine_only = !user.is_admin();
    let rows: Vec<(i64, String, i64, String, Option<String>, String, Option<String>, Option<i64>, i64, Option<String>)> = if mine_only {
        sqlx::query_as(
            "SELECT t.id, w.name, t.connector_id, t.id_tag, e.display_name,
                    t.start_time, t.stop_time, t.stop_meter_wh, t.start_meter_wh, t.stop_reason
             FROM transactions t
             JOIN wallboxes w ON w.id = t.wallbox_id
             LEFT JOIN employees e ON e.id = t.employee_id
             WHERE t.employee_id = (SELECT employee_id FROM users WHERE id = ?1)
             ORDER BY t.start_time DESC",
        )
        .bind(user.id)
        .fetch_all(&state.db)
        .await?
    } else if let Some(name) = filter.employee.as_deref().filter(|s| !s.is_empty()) {
        sqlx::query_as(
            "SELECT t.id, w.name, t.connector_id, t.id_tag, e.display_name,
                    t.start_time, t.stop_time, t.stop_meter_wh, t.start_meter_wh, t.stop_reason
             FROM transactions t
             JOIN wallboxes w ON w.id = t.wallbox_id
             LEFT JOIN employees e ON e.id = t.employee_id
             WHERE e.display_name LIKE ?1 OR t.guest_label LIKE ?1
             ORDER BY t.start_time DESC",
        )
        .bind(format!("%{name}%"))
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as(
            "SELECT t.id, w.name, t.connector_id, t.id_tag, e.display_name,
                    t.start_time, t.stop_time, t.stop_meter_wh, t.start_meter_wh, t.stop_reason
             FROM transactions t
             JOIN wallboxes w ON w.id = t.wallbox_id
             LEFT JOIN employees e ON e.id = t.employee_id
             ORDER BY t.start_time DESC",
        )
        .fetch_all(&state.db)
        .await?
    };

    let mut out = String::with_capacity(256 + rows.len() * 120);
    out.push('\u{FEFF}');
    out.push_str("id;wallbox;connector;id_tag;mitarbeiter;start;ende;energie_wh;grund\n");
    for (id, wname, cid, tag, uname, st, et, stop_m, start_m, reason) in rows {
        // Laufende Transaktionen: aktuellen Stand aus den MeterValues exportieren,
        // konsistent zur HTML-Liste.
        let energy = if et.is_none() {
            live_meter(&state.db, id, start_m).await?.energy_wh.unwrap_or(0)
        } else {
            stop_m.map(|s| (s - start_m).max(0)).unwrap_or(0)
        };
        let _ = writeln!(
            out,
            "{id};{w};{cid};{tag};{u};{st};{et};{energy};{reason}",
            w = csv_escape(&wname),
            tag = csv_escape(&tag),
            u = csv_escape(uname.as_deref().unwrap_or("")),
            et = et.as_deref().unwrap_or(""),
            reason = csv_escape(reason.as_deref().unwrap_or("")),
        );
    }

    let filename = format!(
        "transaktionen_{}.csv",
        chrono::Utc::now().format("%Y%m%d_%H%M%S")
    );
    let mut resp = (StatusCode::OK, out).into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/csv; charset=utf-8"),
    );
    resp.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")).unwrap(),
    );
    Ok(resp)
}

fn csv_escape(s: &str) -> String {
    if s.contains([';', '"', '\n', '\r']) {
        let escaped = s.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        s.to_string()
    }
}
