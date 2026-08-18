use std::collections::HashSet;

use askama::Template;
use axum::extract::{Path, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;

use super::{render, LayoutCtx};
use crate::auth::AdminUser;
use crate::domain::transaction::{fmt_kw, fmt_kwh, live_meter};
use crate::domain::wallbox::{Connector, Wallbox};
use crate::{AppError, AppResult, AppState};

#[derive(Template)]
#[template(path = "wallboxes.html")]
struct ListTpl {
    layout: LayoutCtx,
    wallboxes: Vec<WallboxRow>,
}

pub struct WallboxRow {
    pub wb: Wallbox,
    pub online: bool,
}

pub async fn list(State(state): State<AppState>, AdminUser(user): AdminUser) -> AppResult<Response> {
    let wallboxes: Vec<Wallbox> = sqlx::query_as::<_, Wallbox>(
        "SELECT * FROM wallboxes ORDER BY name",
    )
    .fetch_all(&state.db)
    .await?;
    let online: HashSet<String> = state.ocpp_hub.list_online().into_iter().collect();
    let rows = wallboxes
        .into_iter()
        .map(|wb| WallboxRow {
            online: online.contains(&wb.charge_point_id),
            wb,
        })
        .collect();

    let tpl = ListTpl {
        layout: LayoutCtx::new("wallboxes", Some(user)),
        wallboxes: rows,
    };
    Ok(render(&tpl)?.into_response())
}

#[derive(Deserialize)]
pub struct CreateForm {
    pub charge_point_id: String,
    pub name: String,
    pub location: Option<String>,
}

pub async fn create(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Form(form): Form<CreateForm>,
) -> AppResult<Response> {
    let cp = form.charge_point_id.trim();
    let name = form.name.trim();
    if cp.is_empty() || name.is_empty() {
        return Err(AppError::BadRequest("ID und Name sind Pflicht.".into()));
    }
    let result = sqlx::query(
        "INSERT INTO wallboxes (charge_point_id, name, location) VALUES (?1, ?2, ?3)",
    )
    .bind(cp)
    .bind(name)
    .bind(form.location.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .execute(&state.db)
    .await;
    if let Err(sqlx::Error::Database(db)) = &result {
        if db.is_unique_violation() {
            return Err(AppError::Conflict(format!(
                "ChargePointId '{cp}' existiert bereits."
            )));
        }
    }
    result?;
    Ok(Redirect::to("/wallboxes").into_response())
}

#[derive(Template)]
#[template(path = "wallbox_detail.html")]
struct DetailTpl {
    layout: LayoutCtx,
    wb: Wallbox,
    online: bool,
    connectors: Vec<Connector>,
    active_tx: Vec<ActiveTx>,
    new_password: Option<String>,
}

pub struct ActiveTx {
    pub id: i64,
    pub id_tag: String,
    pub start_time: String,
    pub connector_id: i64,
    /// Bisher geladene Energie, formatiert („12,3“) — None, wenn (noch) keine Messung vorliegt.
    pub energy_kwh: Option<String>,
    /// Aktuelle Ladeleistung, formatiert („7,4“) — None, wenn (noch) keine frische Messung vorliegt.
    pub power_kw: Option<String>,
    pub soc_percent: Option<i64>,
}

/// Laufende Ladungen einer Wallbox inkl. Live-Messwerten.
async fn load_active_tx(state: &AppState, wallbox_id: i64) -> AppResult<Vec<ActiveTx>> {
    let rows: Vec<(i64, String, String, i64, i64)> = sqlx::query_as(
        "SELECT id, id_tag, start_time, connector_id, start_meter_wh
         FROM transactions
         WHERE wallbox_id = ?1 AND stop_time IS NULL
         ORDER BY start_time DESC",
    )
    .bind(wallbox_id)
    .fetch_all(&state.db)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for (id, id_tag, start_time, connector_id, start_wh) in rows {
        let live = live_meter(&state.db, id, start_wh).await?;
        out.push(ActiveTx {
            id,
            id_tag,
            start_time,
            connector_id,
            energy_kwh: live.energy_wh.map(fmt_kwh),
            power_kw: live.power_w.map(fmt_kw),
            soc_percent: live.soc_percent,
        });
    }
    Ok(out)
}

#[derive(Template)]
#[template(path = "_wallbox_live.html")]
struct LiveTpl {
    wb: Wallbox,
    online: bool,
    active_tx: Vec<ActiveTx>,
}

/// htmx-Fragment: Tabelle der laufenden Ladungen, wird von der Detailseite
/// alle paar Sekunden nachgeladen.
pub async fn live_fragment(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    let wb: Wallbox = sqlx::query_as::<_, Wallbox>("SELECT * FROM wallboxes WHERE id = ?1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let online = state.ocpp_hub.get(&wb.charge_point_id).is_some();
    let active_tx = load_active_tx(&state, id).await?;
    Ok(render(&LiveTpl {
        wb,
        online,
        active_tx,
    })?
    .into_response())
}

#[derive(Deserialize)]
pub struct DetailQuery {
    pub pw: Option<String>,
}

pub async fn detail(
    State(state): State<AppState>,
    AdminUser(user): AdminUser,
    Path(id): Path<i64>,
    axum::extract::Query(q): axum::extract::Query<DetailQuery>,
) -> AppResult<Response> {
    let wb: Wallbox = sqlx::query_as::<_, Wallbox>("SELECT * FROM wallboxes WHERE id = ?1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let connectors: Vec<Connector> = sqlx::query_as::<_, Connector>(
        "SELECT * FROM connectors WHERE wallbox_id = ?1 ORDER BY connector_id",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    let active_tx = load_active_tx(&state, id).await?;

    let online = state.ocpp_hub.get(&wb.charge_point_id).is_some();
    let tpl = DetailTpl {
        layout: LayoutCtx::new("wallboxes", Some(user)),
        wb,
        online,
        connectors,
        active_tx,
        new_password: q.pw,
    };
    Ok(render(&tpl)?.into_response())
}

pub async fn delete(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    sqlx::query("DELETE FROM wallboxes WHERE id = ?1")
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(Redirect::to("/wallboxes").into_response())
}

#[derive(Deserialize)]
pub struct RemoteStartForm {
    pub id_tag: String,
    pub connector_id: Option<i64>,
    pub guest_label: Option<String>,
}

pub async fn remote_start(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
    Form(form): Form<RemoteStartForm>,
) -> AppResult<Response> {
    let wb: Wallbox = sqlx::query_as::<_, Wallbox>("SELECT * FROM wallboxes WHERE id = ?1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let id_tag = form.id_tag.trim();
    if id_tag.is_empty() {
        return Err(AppError::BadRequest("idTag fehlt.".into()));
    }

    // Falls der Tag unbekannt ist, legen wir einen Gast-Chip an (einmalig nutzbar),
    // damit Authorize durchgeht. Das `guest_label` dient der Buchhaltung.
    let existing: Option<crate::domain::chip::Chip> =
        sqlx::query_as::<_, crate::domain::chip::Chip>("SELECT * FROM chips WHERE id_tag = ?1")
            .bind(id_tag)
            .fetch_optional(&state.db)
            .await?;
    if existing.is_none() {
        sqlx::query(
            "INSERT INTO chips (id_tag, label, kind, enabled) VALUES (?1, ?2, 'guest', 1)",
        )
        .bind(id_tag)
        .bind(form.guest_label.as_deref())
        .execute(&state.db)
        .await?;
    }

    let connector = form.connector_id.unwrap_or(1);
    let status = crate::ocpp::ocpp16::remote_start(
        &state.ocpp_hub,
        &wb.charge_point_id,
        id_tag,
        connector,
    )
    .await
    .map_err(|e| AppError::Conflict(format!("RemoteStart: {e}")))?;

    if status != "Accepted" {
        return Err(AppError::Conflict(format!(
            "Wallbox hat RemoteStart abgelehnt: {status}"
        )));
    }
    Ok(Redirect::to(&format!("/wallboxes/{id}")).into_response())
}

#[derive(Deserialize)]
pub struct SetAuthForm {
    pub user: Option<String>,
    pub password: Option<String>,
    pub generate: Option<String>,
}

pub async fn set_auth(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
    Form(form): Form<SetAuthForm>,
) -> AppResult<Response> {
    let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM wallboxes WHERE id = ?1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    let generate = form.generate.as_deref() == Some("1");
    let (plain, source) = if generate {
        (random_password(24), "generiert")
    } else {
        let p = form.password.as_deref().unwrap_or("").trim().to_string();
        if p.len() < 8 {
            return Err(AppError::BadRequest(
                "Passwort muss mindestens 8 Zeichen haben (oder 'Generieren' nutzen).".into(),
            ));
        }
        (p, "gesetzt")
    };

    let hash = crate::db::hash_password(&plain).map_err(AppError::Other)?;
    let user = form
        .user
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    sqlx::query("UPDATE wallboxes SET auth_basic_user = ?1, auth_basic_pass = ?2 WHERE id = ?3")
        .bind(&user)
        .bind(&hash)
        .bind(id)
        .execute(&state.db)
        .await?;

    tracing::info!("OCPP Basic-Auth {source} für wallbox_id={id}");

    // Passwort einmalig an das Detail-Template durchreichen. Das Alphabet
    // enthält bewusst keine URL-Sonderzeichen, daher kein Escaping nötig.
    Ok(Redirect::to(&format!("/wallboxes/{id}?pw={plain}")).into_response())
}

pub async fn clear_auth(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
) -> AppResult<Response> {
    sqlx::query(
        "UPDATE wallboxes SET auth_basic_user = NULL, auth_basic_pass = NULL WHERE id = ?1",
    )
    .bind(id)
    .execute(&state.db)
    .await?;
    Ok(Redirect::to(&format!("/wallboxes/{id}")).into_response())
}

fn random_password(len: usize) -> String {
    use rand::Rng;
    // URL-sicheres Alphabet (ohne mehrdeutige 0/O, 1/l/I).
    const ALPH: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let i = rng.gen_range(0..ALPH.len());
            ALPH[i] as char
        })
        .collect()
}

#[derive(Deserialize)]
pub struct RemoteStopForm {
    pub transaction_id: i64,
}

pub async fn remote_stop(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    Path(id): Path<i64>,
    Form(form): Form<RemoteStopForm>,
) -> AppResult<Response> {
    let wb: Wallbox = sqlx::query_as::<_, Wallbox>("SELECT * FROM wallboxes WHERE id = ?1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let status =
        crate::ocpp::ocpp16::remote_stop(&state.ocpp_hub, &wb.charge_point_id, form.transaction_id)
            .await
            .map_err(|e| AppError::Conflict(format!("RemoteStop: {e}")))?;
    if status != "Accepted" {
        return Err(AppError::Conflict(format!(
            "Wallbox hat RemoteStop abgelehnt: {status}"
        )));
    }
    Ok(Redirect::to(&format!("/wallboxes/{id}")).into_response())
}
