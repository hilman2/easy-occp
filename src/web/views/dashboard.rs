use std::collections::HashSet;

use askama::Template;
use axum::extract::State;
use axum::response::{IntoResponse, Redirect, Response};

use super::{render, LayoutCtx};
use crate::auth::{AuthUser, MaybeAuth};
use crate::domain::transaction::{fmt_kw, fmt_kwh, live_meter};
use crate::domain::wallbox::Wallbox;
use crate::{AppResult, AppState};

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashTpl {
    layout: LayoutCtx,
    wallboxes_total: i64,
    wallboxes_online: usize,
    active_tx_count: i64,
    energy_month_kwh: i64,
    sessions_month: i64,
    cards: Vec<WallboxCard>,
    active_sessions: Vec<ActiveSession>,
    top_employees: Vec<TopEmployee>,
    /// Remote-Stop ist AdminUser-only — der Button wird nur Admins gezeigt.
    can_stop: bool,
}

pub struct WallboxCard {
    pub id: i64,
    pub name: String,
    pub charge_point_id: String,
    pub location: Option<String>,
    pub ocpp_version: Option<String>,
    pub online: bool,
    pub active_tx: i64,
    pub connector_statuses: Vec<String>,
    pub auth_locked: bool,
}

impl WallboxCard {
    /// Überlagert OCPP-Status + Online + laufende Tx zu einer einzigen
    /// Cockpit-Aussage ("lädt", "bereit", "offline", "fehler").
    pub fn state(&self) -> &'static str {
        if !self.online {
            return "offline";
        }
        if self.active_tx > 0 {
            return "charging";
        }
        if self
            .connector_statuses
            .iter()
            .any(|s| s.eq_ignore_ascii_case("Faulted"))
        {
            return "error";
        }
        if self
            .connector_statuses
            .iter()
            .any(|s| s.eq_ignore_ascii_case("Unavailable"))
        {
            return "warn";
        }
        "ready"
    }
    pub fn state_label(&self) -> &'static str {
        match self.state() {
            "charging" => "Lädt",
            "ready" => "Bereit",
            "offline" => "Offline",
            "error" => "Fehler",
            "warn" => "Nicht verfügbar",
            _ => "—",
        }
    }
}

pub struct ActiveSession {
    pub tx_id: i64,
    pub wallbox_id: i64,
    pub wallbox_name: String,
    pub connector_id: i64,
    pub id_tag: String,
    pub employee_name: Option<String>,
    pub start_time: String,
    /// Bisher geladene Energie, formatiert („12,3“) — None, wenn (noch) keine Messung vorliegt.
    pub energy_kwh: Option<String>,
    /// Aktuelle Ladeleistung, formatiert („7,4“) — None, wenn (noch) keine frische Messung vorliegt.
    pub power_kw: Option<String>,
    pub soc_percent: Option<i64>,
}

/// Laufende Ladungen inkl. Live-Messwerten aus den zuletzt gemeldeten MeterValues.
async fn load_active_sessions(state: &AppState) -> AppResult<Vec<ActiveSession>> {
    let rows: Vec<(i64, i64, String, i64, String, Option<String>, String, i64)> = sqlx::query_as(
        "SELECT t.id, t.wallbox_id, w.name, t.connector_id, t.id_tag,
                e.display_name, t.start_time, t.start_meter_wh
         FROM transactions t
         JOIN wallboxes w ON w.id = t.wallbox_id
         LEFT JOIN employees e ON e.id = t.employee_id
         WHERE t.stop_time IS NULL
         ORDER BY t.start_time DESC",
    )
    .fetch_all(&state.db)
    .await?;

    let mut sessions = Vec::with_capacity(rows.len());
    for (tx_id, wb_id, wb_name, conn, id_tag, emp, start, start_wh) in rows {
        let live = live_meter(&state.db, tx_id, start_wh).await?;
        sessions.push(ActiveSession {
            tx_id,
            wallbox_id: wb_id,
            wallbox_name: wb_name,
            connector_id: conn,
            id_tag,
            employee_name: emp,
            start_time: start,
            energy_kwh: live.energy_wh.map(fmt_kwh),
            power_kw: live.power_w.map(fmt_kw),
            soc_percent: live.soc_percent,
        });
    }
    Ok(sessions)
}

#[derive(Template)]
#[template(path = "_active_sessions.html")]
struct ActiveSessionsTpl {
    active_sessions: Vec<ActiveSession>,
    /// Remote-Stop ist AdminUser-only — der Button wird nur Admins gezeigt.
    can_stop: bool,
}

/// htmx-Fragment: wird vom Cockpit alle paar Sekunden nachgeladen.
pub async fn active_sessions_fragment(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> AppResult<Response> {
    let active_sessions = load_active_sessions(&state).await?;
    Ok(render(&ActiveSessionsTpl {
        active_sessions,
        can_stop: user.is_admin(),
    })?
    .into_response())
}

pub struct TopEmployee {
    pub name: String,
    pub sessions: i64,
    pub energy_kwh: i64,
}

pub async fn get(State(state): State<AppState>, MaybeAuth(user): MaybeAuth) -> AppResult<Response> {
    let Some(user) = user else {
        return Ok(Redirect::to("/login").into_response());
    };

    let wallboxes: Vec<Wallbox> =
        sqlx::query_as::<_, Wallbox>("SELECT * FROM wallboxes ORDER BY name")
            .fetch_all(&state.db)
            .await?;
    let online: HashSet<String> = state.ocpp_hub.list_online().into_iter().collect();
    let wallboxes_total = wallboxes.len() as i64;
    let wallboxes_online = online.len();

    let mut cards = Vec::with_capacity(wallboxes.len());
    for wb in &wallboxes {
        let (active_tx,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM transactions WHERE wallbox_id = ?1 AND stop_time IS NULL",
        )
        .bind(wb.id)
        .fetch_one(&state.db)
        .await?;
        let connector_statuses: Vec<(Option<String>,)> = sqlx::query_as(
            "SELECT status FROM connectors WHERE wallbox_id = ?1 ORDER BY connector_id",
        )
        .bind(wb.id)
        .fetch_all(&state.db)
        .await?;
        cards.push(WallboxCard {
            id: wb.id,
            name: wb.name.clone(),
            charge_point_id: wb.charge_point_id.clone(),
            location: wb.location.clone(),
            ocpp_version: wb.ocpp_version.clone(),
            online: online.contains(&wb.charge_point_id),
            active_tx,
            connector_statuses: connector_statuses
                .into_iter()
                .map(|(s,)| s.unwrap_or_default())
                .collect(),
            auth_locked: wb.auth_basic_pass.is_some(),
        });
    }

    let (active_tx_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM transactions WHERE stop_time IS NULL")
            .fetch_one(&state.db)
            .await?;

    let (energy_wh,): (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(COALESCE(stop_meter_wh,0) - start_meter_wh), 0)
         FROM transactions
         WHERE stop_meter_wh IS NOT NULL
           AND strftime('%Y-%m', start_time) = strftime('%Y-%m', 'now')",
    )
    .fetch_one(&state.db)
    .await?;

    let (sessions_month,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transactions
         WHERE stop_time IS NOT NULL
           AND strftime('%Y-%m', start_time) = strftime('%Y-%m', 'now')",
    )
    .fetch_one(&state.db)
    .await?;

    let active_sessions = load_active_sessions(&state).await?;

    // Top-3 Mitarbeiter im laufenden Monat
    let top_rows: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT COALESCE(e.display_name, 'Gast') AS name,
                COUNT(*) AS sessions,
                COALESCE(SUM(COALESCE(t.stop_meter_wh,0) - t.start_meter_wh), 0) AS wh
         FROM transactions t
         LEFT JOIN employees e ON e.id = t.employee_id
         WHERE t.stop_meter_wh IS NOT NULL
           AND strftime('%Y-%m', t.start_time) = strftime('%Y-%m', 'now')
         GROUP BY e.id
         ORDER BY wh DESC
         LIMIT 3",
    )
    .fetch_all(&state.db)
    .await?;
    let top_employees = top_rows
        .into_iter()
        .map(|(name, sessions, wh)| TopEmployee {
            name,
            sessions,
            energy_kwh: wh / 1000,
        })
        .collect();

    let can_stop = user.is_admin();
    let tpl = DashTpl {
        layout: LayoutCtx::new("dashboard", Some(user)),
        wallboxes_total,
        wallboxes_online,
        active_tx_count,
        energy_month_kwh: energy_wh / 1000,
        sessions_month,
        cards,
        active_sessions,
        top_employees,
        can_stop,
    };
    Ok(render(&tpl)?.into_response())
}
