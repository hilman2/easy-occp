//! OCPP 1.6J Server-Seite: akzeptiert Verbindungen, behandelt eingehende CALLs,
//! sendet Antworten, und kann RemoteStart/Stop/UnlockConnector etc. einleiten.
//!
//! Alle eingehenden Daten werden **sanity-gechecked** bevor sie die DB erreichen:
//!  - Duplicate StartTransaction / Heartbeat / StatusNotification werden idempotent behandelt.
//!  - Timestamps werden geparst; liegt ein Wert mehr als 24h in der Zukunft oder
//!    10 Jahre in der Vergangenheit, fällt der Server auf `Utc::now()` zurück.
//!  - Meter-Werte werden auf Plausibilität geprüft (nicht negativ, keine absurden
//!    Ausreißer); rückläufige Stop-Zählerstände werden auf den Startwert korrigiert.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{ConnectInfo, Path, State, WebSocketUpgrade};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::ocpp::hub::{Connection, Hub};
use crate::ocpp::wire::{OcppCallError, OcppMessage};
use crate::AppState;

const OCPP_SUBPROTOCOLS: &[&str] = &["ocpp2.0.1", "ocpp1.6"];

fn check_basic_auth(headers: &HeaderMap, expected_user: &str, pw_hash: &str) -> bool {
    use base64::Engine as _;
    let Some(auth) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    let Some(b64) = auth.strip_prefix("Basic ").or_else(|| auth.strip_prefix("basic ")) else {
        return false;
    };
    let Ok(raw) = base64::engine::general_purpose::STANDARD.decode(b64.trim()) else {
        return false;
    };
    let Ok(s) = std::str::from_utf8(&raw) else {
        return false;
    };
    let Some((user, pass)) = s.split_once(':') else {
        return false;
    };
    if user != expected_user {
        return false;
    }
    crate::db::verify_password(pass, pw_hash).unwrap_or(false)
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(cp_id): Path<String>,
    headers: HeaderMap,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
) -> axum::response::Response {
    if cp_id.is_empty() || cp_id.len() > 64 {
        return (StatusCode::BAD_REQUEST, "ChargePointId ungültig").into_response();
    }

    // OCPP Security Profile 1 — Basic Auth, pro Wallbox optional.
    // Wenn ein Passwort-Hash in der DB steht, muss die Wallbox sich mit
    // "Authorization: Basic base64(user:pass)" ausweisen. Username-Default = cp_id.
    let existing: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT auth_basic_user, auth_basic_pass FROM wallboxes WHERE charge_point_id = ?1",
    )
    .bind(&cp_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();
    if let Some((user_opt, Some(hash))) = existing {
        let expected_user = user_opt.as_deref().unwrap_or(cp_id.as_str());
        if !check_basic_auth(&headers, expected_user, &hash) {
            tracing::warn!("OCPP-Connect abgelehnt (Auth fehlt/falsch) für {cp_id} von {addr}");
            return (
                StatusCode::UNAUTHORIZED,
                [(axum::http::header::WWW_AUTHENTICATE, "Basic realm=\"OCPP\"")],
                "Unauthorized",
            )
                .into_response();
        }
    }

    // Sec-WebSocket-Protocol aushandeln. Wenn keines angeboten wird, akzeptieren
    // wir ebenfalls und behandeln als 1.6J (manche Clients senden keins).
    let requested: Vec<String> = headers
        .get_all(axum::http::header::SEC_WEBSOCKET_PROTOCOL)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|s| s.split(',').map(|p| p.trim().to_string()))
        .collect();

    let chosen = OCPP_SUBPROTOCOLS
        .iter()
        .find(|proto| requested.iter().any(|r| r.eq_ignore_ascii_case(proto)))
        .copied();

    tracing::info!(
        "OCPP-Connect von {addr} – chargePointId={cp_id}, subprotocols={requested:?}, chosen={chosen:?}"
    );

    let upgrade = if let Some(proto) = chosen {
        ws.protocols([proto])
    } else {
        ws
    };

    let version = chosen.unwrap_or("ocpp1.6").to_string();
    upgrade
        .on_upgrade(move |socket| handle_socket(socket, cp_id, version, state))
        .into_response()
}

async fn handle_socket(socket: WebSocket, cp_id: String, version: String, state: AppState) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (out_tx, mut out_rx) = mpsc::channel::<String>(32);
    let conn = Arc::new(Connection::new(cp_id.clone(), version.clone(), out_tx));
    let conn_id = conn.id;

    if let Some(prev) = state.ocpp_hub.register(conn.clone()) {
        tracing::warn!(
            "Neue OCPP-Verbindung für {cp_id} ersetzt bestehende (vorherige Version: {})",
            prev.ocpp_version
        );
    }

    // Wallbox so konfigurieren, dass sie während einer Ladung regelmäßig
    // Zählerstand + Leistung meldet. Verzögert, damit die BootNotification-
    // Sequenz der Box zuerst durchläuft; mit Wiederholung, falls die Box beim
    // ersten Versuch noch nicht antwortet. (ChangeConfiguration ist 1.6-only.)
    if version != "ocpp2.0.1" && state.config.ocpp.meter_interval_s > 0 {
        let st = state.clone();
        let cp = cp_id.clone();
        tokio::spawn(async move {
            for delay_s in [10u64, 60, 240] {
                tokio::time::sleep(Duration::from_secs(delay_s)).await;
                if ensure_meter_config(&st, &cp).await {
                    break;
                }
            }
        });
    }

    // Sender-Task
    let send_task = tokio::spawn(async move {
        while let Some(msg) = out_rx.recv().await {
            if ws_tx.send(Message::Text(msg)).await.is_err() {
                break;
            }
        }
        let _ = ws_tx.close().await;
    });

    let hub = state.ocpp_hub.clone();

    while let Some(msg) = ws_rx.next().await {
        match msg {
            Ok(Message::Text(txt)) => {
                match OcppMessage::parse(&txt) {
                    Ok(OcppMessage::Call {
                        unique_id,
                        action,
                        payload,
                    }) => {
                        let reply = handle_call(&state, &cp_id, &version, &action, payload).await;
                        let out = match reply {
                            Ok(p) => OcppMessage::CallResult {
                                unique_id,
                                payload: p,
                            },
                            Err(e) => OcppMessage::CallError {
                                unique_id,
                                error_code: e.code,
                                error_description: e.description,
                                error_details: json!({}),
                            },
                        };
                        let _ = conn.tx.send(out.serialize()).await;
                    }
                    Ok(OcppMessage::CallResult { unique_id, payload }) => {
                        let pending = conn.pending.lock().unwrap().remove(&unique_id);
                        if let Some(p) = pending {
                            let _ = p.responder.send(Ok(payload));
                        } else {
                            tracing::warn!("CALLRESULT ohne passende uniqueId: {unique_id}");
                        }
                    }
                    Ok(OcppMessage::CallError {
                        unique_id,
                        error_code,
                        error_description,
                        ..
                    }) => {
                        let pending = conn.pending.lock().unwrap().remove(&unique_id);
                        if let Some(p) = pending {
                            let _ = p.responder.send(Err(OcppCallError {
                                code: error_code,
                                description: error_description,
                            }));
                        }
                    }
                    Err(e) => {
                        tracing::warn!("OCPP-Parse-Fehler von {cp_id}: {e} – rohe Nachricht: {txt}");
                    }
                }
            }
            // Pings beantwortet der WebSocket-Stack selbst mit Pong-Frames;
            // eine eigene Antwort wäre kein gültiges OCPP-J.
            Ok(Message::Ping(_)) => {}
            Ok(Message::Close(_)) | Err(_) => break,
            _ => {}
        }
    }

    hub.unregister(&cp_id, conn_id);
    drop(conn);
    let _ = send_task.await;
    tracing::info!("OCPP-Verbindung zu {cp_id} beendet");
}

// -----------------------------------------------------------------------------
// Meter-Konfiguration (MeterValues aktivieren)
// -----------------------------------------------------------------------------

/// Prüft per GetConfiguration, ob die Wallbox MeterValues im gewünschten
/// Intervall mit Energie + Leistung sendet, und setzt andernfalls
/// `MeterValueSampleInterval` bzw. ergänzt `MeterValuesSampledData`.
/// Fehler werden nur geloggt — nicht jede Box unterstützt jeden Schlüssel.
/// Rückgabe: true, wenn die Box auf GetConfiguration geantwortet hat
/// (dann ist kein weiterer Versuch nötig).
async fn ensure_meter_config(state: &AppState, cp_id: &str) -> bool {
    let interval = state.config.ocpp.meter_interval_s.to_string();

    let mut cur_interval: Option<String> = None;
    let mut cur_data: Option<String> = None;
    match state
        .ocpp_hub
        .call(
            cp_id,
            "GetConfiguration",
            json!({"key": ["MeterValueSampleInterval", "MeterValuesSampledData"]}),
            Duration::from_secs(15),
        )
        .await
    {
        Ok(v) => {
            for e in v
                .get("configurationKey")
                .and_then(|k| k.as_array())
                .unwrap_or(&Vec::new())
            {
                let key = e.get("key").and_then(|k| k.as_str()).unwrap_or("");
                let val = e.get("value").and_then(|k| k.as_str()).map(str::to_string);
                match key {
                    "MeterValueSampleInterval" => cur_interval = val,
                    "MeterValuesSampledData" => cur_data = val,
                    _ => {}
                }
            }
        }
        Err(e) => {
            // Ohne Kenntnis der aktuellen Konfiguration nichts überschreiben —
            // die Box antwortet evtl. erst nach der Boot-Sequenz (Retry folgt).
            tracing::warn!("GetConfiguration bei {cp_id} fehlgeschlagen: {e}");
            return false;
        }
    }

    if cur_interval.as_deref() != Some(interval.as_str()) {
        change_config(state, cp_id, "MeterValueSampleInterval", &interval).await;
    }

    // Fehlende Messgrößen an die bestehende Liste anhängen, statt sie zu
    // ersetzen — vom Betreiber konfigurierte Messgrößen bleiben erhalten.
    // Bei leerer Liste SoC mit anfordern; lehnt die Box ab, Rückfall auf
    // Energie + Leistung.
    let has_power = cur_data
        .as_deref()
        .is_some_and(|d| d.contains("Power.Active.Import"));
    let has_energy = cur_data
        .as_deref()
        .is_some_and(|d| d.contains("Energy.Active.Import.Register"));
    if !(has_power && has_energy) {
        let minimal = "Energy.Active.Import.Register,Power.Active.Import";
        let desired = match cur_data.as_deref().filter(|d| !d.trim().is_empty()) {
            Some(existing) => {
                let mut list = existing.to_string();
                if !has_energy {
                    list.push_str(",Energy.Active.Import.Register");
                }
                if !has_power {
                    list.push_str(",Power.Active.Import");
                }
                list
            }
            None => format!("{minimal},SoC"),
        };
        if !change_config(state, cp_id, "MeterValuesSampledData", &desired).await
            && desired != minimal
        {
            change_config(state, cp_id, "MeterValuesSampledData", minimal).await;
        }
    }
    true
}

/// Setzt einen Konfigurationsschlüssel per ChangeConfiguration.
/// Rückgabe: true, wenn die Wallbox akzeptiert hat.
async fn change_config(state: &AppState, cp_id: &str, key: &str, value: &str) -> bool {
    match state
        .ocpp_hub
        .call(
            cp_id,
            "ChangeConfiguration",
            json!({"key": key, "value": value}),
            Duration::from_secs(15),
        )
        .await
    {
        Ok(resp) => {
            let status = resp.get("status").and_then(|v| v.as_str()).unwrap_or("Unknown");
            if status == "Accepted" || status == "RebootRequired" {
                tracing::info!("{cp_id}: {key}={value} gesetzt ({status})");
                true
            } else {
                tracing::warn!("{cp_id}: ChangeConfiguration {key}={value} → {status}");
                false
            }
        }
        Err(e) => {
            tracing::warn!("{cp_id}: ChangeConfiguration {key} fehlgeschlagen: {e}");
            false
        }
    }
}

// -----------------------------------------------------------------------------
// Sanity Helpers
// -----------------------------------------------------------------------------

/// Akzeptiert einen ISO-8601-Timestamp. Wenn er fehlt, leer ist oder unplausibel
/// (>24h Zukunft oder >10 Jahre Vergangenheit), wird `Utc::now()` zurückgegeben.
fn sane_timestamp(raw: Option<&str>) -> DateTime<Utc> {
    let now = Utc::now();
    let Some(s) = raw else {
        return now;
    };
    let Ok(parsed) = DateTime::parse_from_rfc3339(s) else {
        tracing::debug!("Timestamp nicht parsbar: '{s}' – verwende Server-Zeit");
        return now;
    };
    let parsed = parsed.with_timezone(&Utc);
    let max_future = now + ChronoDuration::hours(24);
    let max_past = now - ChronoDuration::days(365 * 10);
    if parsed > max_future || parsed < max_past {
        tracing::warn!("Timestamp unplausibel: '{s}' – verwende Server-Zeit");
        return now;
    }
    parsed
}

fn rfc3339(ts: DateTime<Utc>) -> String {
    ts.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

// -----------------------------------------------------------------------------
// Action-Dispatch
// -----------------------------------------------------------------------------

async fn handle_call(
    state: &AppState,
    cp_id: &str,
    version: &str,
    action: &str,
    payload: Value,
) -> Result<Value, OcppCallError> {
    // Für OCPP 2.0.1 gibt es viele gleich benannte Actions (BootNotification, Heartbeat,
    // Authorize, ...). Die Payloads unterscheiden sich aber. Wir routen grob:
    if version == "ocpp2.0.1" {
        if let Some(v) = crate::ocpp::ocpp20::maybe_handle(state, cp_id, action, &payload).await? {
            return Ok(v);
        }
    }

    match action {
        "BootNotification" => handle_boot(state, cp_id, version, &payload).await,
        "Heartbeat" => handle_heartbeat(state, cp_id).await,
        "StatusNotification" => handle_status(state, cp_id, &payload).await,
        "Authorize" => handle_authorize(state, cp_id, &payload).await,
        "StartTransaction" => handle_start(state, cp_id, &payload).await,
        "StopTransaction" => handle_stop(state, cp_id, &payload).await,
        "MeterValues" => handle_meter(state, cp_id, &payload).await,
        "DataTransfer" => Ok(json!({"status":"Rejected"})),
        "FirmwareStatusNotification" | "DiagnosticsStatusNotification" => Ok(json!({})),
        other => {
            tracing::warn!("Unbekannte OCPP-Action von {cp_id}: {other}");
            Err(OcppCallError {
                code: "NotImplemented".into(),
                description: format!("Action '{other}' nicht unterstützt"),
            })
        }
    }
}

async fn ensure_wallbox(state: &AppState, cp_id: &str) -> sqlx::Result<i64> {
    let row: Option<(i64,)> = sqlx::query_as("SELECT id FROM wallboxes WHERE charge_point_id = ?1")
        .bind(cp_id)
        .fetch_optional(&state.db)
        .await?;
    if let Some((id,)) = row {
        return Ok(id);
    }
    // Unbekannte Wallbox: selbsttätig anlegen, damit sie in der UI sichtbar wird.
    let res = sqlx::query(
        "INSERT INTO wallboxes (charge_point_id, name) VALUES (?1, ?1) ON CONFLICT DO NOTHING",
    )
    .bind(cp_id)
    .execute(&state.db)
    .await?;
    if res.rows_affected() == 0 {
        let (id,): (i64,) = sqlx::query_as("SELECT id FROM wallboxes WHERE charge_point_id = ?1")
            .bind(cp_id)
            .fetch_one(&state.db)
            .await?;
        Ok(id)
    } else {
        Ok(res.last_insert_rowid())
    }
}

async fn handle_boot(
    state: &AppState,
    cp_id: &str,
    version: &str,
    payload: &Value,
) -> Result<Value, OcppCallError> {
    let wb_id = ensure_wallbox(state, cp_id)
        .await
        .map_err(db_err("BootNotification"))?;

    // Felder je nach OCPP-Version leicht anders; wir picken was da ist.
    let vendor = payload
        .get("chargePointVendor")
        .or_else(|| payload.get("chargingStation").and_then(|v| v.get("vendorName")))
        .and_then(|v| v.as_str());
    let model = payload
        .get("chargePointModel")
        .or_else(|| payload.get("chargingStation").and_then(|v| v.get("model")))
        .and_then(|v| v.as_str());
    let firmware = payload
        .get("firmwareVersion")
        .or_else(|| payload.get("chargingStation").and_then(|v| v.get("firmwareVersion")))
        .and_then(|v| v.as_str());
    let serial = payload
        .get("chargePointSerialNumber")
        .or_else(|| payload.get("chargingStation").and_then(|v| v.get("serialNumber")))
        .and_then(|v| v.as_str());

    let now = rfc3339(Utc::now());
    sqlx::query(
        "UPDATE wallboxes
         SET vendor = COALESCE(?1, vendor),
             model = COALESCE(?2, model),
             firmware = COALESCE(?3, firmware),
             serial_number = COALESCE(?4, serial_number),
             ocpp_version = ?5,
             last_boot = ?6,
             last_heartbeat = ?6
         WHERE id = ?7",
    )
    .bind(vendor)
    .bind(model)
    .bind(firmware)
    .bind(serial)
    .bind(version)
    .bind(&now)
    .bind(wb_id)
    .execute(&state.db)
    .await
    .map_err(db_err("BootNotification update"))?;

    Ok(json!({
        "status": "Accepted",
        "currentTime": now,
        "interval": 60
    }))
}

async fn handle_heartbeat(state: &AppState, cp_id: &str) -> Result<Value, OcppCallError> {
    let now = rfc3339(Utc::now());
    sqlx::query("UPDATE wallboxes SET last_heartbeat = ?1 WHERE charge_point_id = ?2")
        .bind(&now)
        .bind(cp_id)
        .execute(&state.db)
        .await
        .map_err(db_err("Heartbeat"))?;
    Ok(json!({ "currentTime": now }))
}

async fn handle_status(
    state: &AppState,
    cp_id: &str,
    payload: &Value,
) -> Result<Value, OcppCallError> {
    let wb_id = ensure_wallbox(state, cp_id)
        .await
        .map_err(db_err("StatusNotification"))?;
    let connector_id = payload.get("connectorId").and_then(|v| v.as_i64()).unwrap_or(0);
    let status = payload.get("status").and_then(|v| v.as_str()).unwrap_or("");
    let error_code = payload.get("errorCode").and_then(|v| v.as_str());
    let info = payload.get("info").and_then(|v| v.as_str());
    let ts = sane_timestamp(payload.get("timestamp").and_then(|v| v.as_str()));
    let ts_s = rfc3339(ts);

    // Idempotentes Upsert pro (wallbox, connector). Gleicher Status erneut ist OK.
    sqlx::query(
        "INSERT INTO connectors (wallbox_id, connector_id, status, error_code, info, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(wallbox_id, connector_id) DO UPDATE SET
            status = excluded.status,
            error_code = excluded.error_code,
            info = excluded.info,
            updated_at = excluded.updated_at",
    )
    .bind(wb_id)
    .bind(connector_id)
    .bind(status)
    .bind(error_code)
    .bind(info)
    .bind(&ts_s)
    .execute(&state.db)
    .await
    .map_err(db_err("StatusNotification upsert"))?;

    Ok(json!({}))
}

async fn handle_authorize(
    state: &AppState,
    cp_id: &str,
    payload: &Value,
) -> Result<Value, OcppCallError> {
    let id_tag = payload
        .get("idTag")
        .or_else(|| payload.get("idToken").and_then(|v| v.get("idToken")))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if id_tag.is_empty() {
        return Ok(json!({"idTagInfo":{"status":"Invalid"}}));
    }

    // Enrollment-Session: erster unbekannter Tag wird für aktive Session festgehalten.
    let _ = try_capture_for_enrollment(state, cp_id, &id_tag).await;

    let status = authorize_status(state, &id_tag).await;
    Ok(json!({"idTagInfo":{"status": status}}))
}

async fn authorize_status(state: &AppState, id_tag: &str) -> &'static str {
    let row: sqlx::Result<Option<crate::domain::chip::Chip>> =
        sqlx::query_as::<_, crate::domain::chip::Chip>("SELECT * FROM chips WHERE id_tag = ?1")
            .bind(id_tag)
            .fetch_optional(&state.db)
            .await;
    match row {
        Ok(Some(chip)) => {
            if chip.enabled == 0 {
                "Blocked"
            } else if let Some(ts) = chip.expires_at.as_deref() {
                if let Ok(dt) = DateTime::parse_from_rfc3339(ts) {
                    if dt < Utc::now() {
                        "Expired"
                    } else {
                        "Accepted"
                    }
                } else {
                    "Accepted"
                }
            } else {
                "Accepted"
            }
        }
        Ok(None) => "Invalid",
        Err(e) => {
            tracing::error!("authorize db: {e}");
            "Invalid"
        }
    }
}

async fn try_capture_for_enrollment(state: &AppState, cp_id: &str, id_tag: &str) -> sqlx::Result<()> {
    // Offene (nicht konsumierte, nicht abgelaufene) Enrollment-Sessions
    let rows: Vec<(i64, Option<i64>)> = sqlx::query_as(
        "SELECT id, wallbox_id FROM enrollment_sessions
         WHERE consumed = 0
           AND captured_id_tag IS NULL
           AND datetime(expires_at) > datetime('now')",
    )
    .fetch_all(&state.db)
    .await?;

    if rows.is_empty() {
        return Ok(());
    }

    // Wenn die Wallbox bekannt ist, bevorzugen wir Sessions mit passender oder
    // ohne wallbox_id-Einschränkung.
    let wb_id: Option<i64> = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM wallboxes WHERE charge_point_id = ?1",
    )
    .bind(cp_id)
    .fetch_optional(&state.db)
    .await?
    .map(|r| r.0);

    let now = rfc3339(Utc::now());
    for (sess_id, sess_wb) in rows {
        if let (Some(req), Some(act)) = (sess_wb, wb_id) {
            if req != act {
                continue;
            }
        }
        sqlx::query(
            "UPDATE enrollment_sessions
             SET captured_id_tag = ?1, captured_at = ?2
             WHERE id = ?3 AND captured_id_tag IS NULL",
        )
        .bind(id_tag)
        .bind(&now)
        .bind(sess_id)
        .execute(&state.db)
        .await?;
        tracing::info!("Enrollment-Session {sess_id} hat Tag '{id_tag}' eingefangen");
        break;
    }
    Ok(())
}

async fn handle_start(
    state: &AppState,
    cp_id: &str,
    payload: &Value,
) -> Result<Value, OcppCallError> {
    let wb_id = ensure_wallbox(state, cp_id)
        .await
        .map_err(db_err("StartTransaction"))?;

    let connector_id = payload.get("connectorId").and_then(|v| v.as_i64()).unwrap_or(1);
    let id_tag = payload.get("idTag").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let meter_start = payload
        .get("meterStart")
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
        .max(0);
    let ts = sane_timestamp(payload.get("timestamp").and_then(|v| v.as_str()));

    let status = authorize_status(state, &id_tag).await;
    if status != "Accepted" {
        return Ok(json!({
            "transactionId": 0,
            "idTagInfo": {"status": status}
        }));
    }

    // Chip + Mitarbeiter auflösen
    let chip: Option<crate::domain::chip::Chip> = sqlx::query_as::<_, crate::domain::chip::Chip>(
        "SELECT * FROM chips WHERE id_tag = ?1",
    )
    .bind(&id_tag)
    .fetch_optional(&state.db)
    .await
    .map_err(db_err("StartTransaction chip lookup"))?;
    let (chip_id, employee_id) = chip
        .as_ref()
        .map(|c| (Some(c.id), c.employee_id))
        .unwrap_or((None, None));

    // Insert; OCPP-transactionId generieren wir selbst (unsere rowid).
    let res = sqlx::query(
        "INSERT INTO transactions (wallbox_id, connector_id, id_tag, chip_id, employee_id,
                                   start_time, start_meter_wh, started_remote)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
    )
    .bind(wb_id)
    .bind(connector_id)
    .bind(&id_tag)
    .bind(chip_id)
    .bind(employee_id)
    .bind(rfc3339(ts))
    .bind(meter_start)
    .execute(&state.db)
    .await
    .map_err(db_err("StartTransaction insert"))?;

    let tx_id = res.last_insert_rowid();
    sqlx::query("UPDATE transactions SET ocpp_transaction_id = ?1 WHERE id = ?1")
        .bind(tx_id)
        .execute(&state.db)
        .await
        .map_err(db_err("StartTransaction id"))?;

    // Sofort eine erste Messung anfordern, damit die UI nicht bis zum ersten
    // Sample-Intervall ohne Werte dasteht. Ablehnung (NotImplemented) ist ok.
    if state.config.ocpp.meter_interval_s > 0 {
        let st = state.clone();
        let cp = cp_id.to_string();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(3)).await;
            let payload = json!({"requestedMessage": "MeterValues", "connectorId": connector_id});
            if let Err(e) = st
                .ocpp_hub
                .call(&cp, "TriggerMessage", payload, Duration::from_secs(15))
                .await
            {
                tracing::debug!("{cp}: TriggerMessage MeterValues nicht möglich: {e}");
            }
        });
    }

    Ok(json!({
        "transactionId": tx_id,
        "idTagInfo": {"status": "Accepted"}
    }))
}

async fn handle_stop(
    state: &AppState,
    cp_id: &str,
    payload: &Value,
) -> Result<Value, OcppCallError> {
    let tx_id = payload
        .get("transactionId")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| OcppCallError {
            code: "FormationViolation".into(),
            description: "transactionId fehlt".into(),
        })?;
    let meter_stop = payload
        .get("meterStop")
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
        .max(0);
    let ts = sane_timestamp(payload.get("timestamp").and_then(|v| v.as_str()));
    let reason = payload.get("reason").and_then(|v| v.as_str()).unwrap_or("Local");

    // Sanity: wenn bereits gestoppt, idempotent antworten.
    let existing: Option<(Option<String>, i64)> = sqlx::query_as(
        "SELECT stop_time, start_meter_wh FROM transactions
         WHERE id = ?1 AND wallbox_id = (SELECT id FROM wallboxes WHERE charge_point_id = ?2)",
    )
    .bind(tx_id)
    .bind(cp_id)
    .fetch_optional(&state.db)
    .await
    .map_err(db_err("StopTransaction lookup"))?;

    let Some((stop_time, start_meter)) = existing else {
        tracing::warn!("StopTransaction für unbekannte tx {tx_id} von {cp_id}");
        return Ok(json!({"idTagInfo":{"status":"Accepted"}}));
    };
    if stop_time.is_some() {
        return Ok(json!({"idTagInfo":{"status":"Accepted"}}));
    }

    // Rückläufige Meter-Werte korrigieren.
    let meter_stop = meter_stop.max(start_meter);

    sqlx::query(
        "UPDATE transactions
         SET stop_time = ?1, stop_meter_wh = ?2, stop_reason = ?3
         WHERE id = ?4",
    )
    .bind(rfc3339(ts))
    .bind(meter_stop)
    .bind(reason)
    .bind(tx_id)
    .execute(&state.db)
    .await
    .map_err(db_err("StopTransaction update"))?;

    Ok(json!({"idTagInfo":{"status":"Accepted"}}))
}

async fn handle_meter(
    state: &AppState,
    cp_id: &str,
    payload: &Value,
) -> Result<Value, OcppCallError> {
    let tx_id = payload.get("transactionId").and_then(|v| v.as_i64());
    let empty: Vec<Value> = Vec::new();
    let meter_value = payload
        .get("meterValue")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);

    // Wir ordnen MeterValues der Transaktion zu. Manche Boxen senden während
    // der Ladung keine transactionId, nur die connectorId — dann nehmen wir
    // die laufende Transaktion dieses Connectors. Ohne Zuordnung keine Speicherung.
    let tx_id = match tx_id {
        Some(id) => id,
        None => {
            let Some(conn_id) = payload.get("connectorId").and_then(|v| v.as_i64()) else {
                return Ok(json!({}));
            };
            // Nur Transaktionen der letzten 24 h — schützt davor, Messwerte einer
            // verwaisten offenen Transaktion (verlorene StopTransaction) zuzuordnen.
            let row: Option<(i64,)> = sqlx::query_as(
                "SELECT t.id FROM transactions t
                 JOIN wallboxes w ON w.id = t.wallbox_id
                 WHERE w.charge_point_id = ?1 AND t.connector_id = ?2 AND t.stop_time IS NULL
                   AND datetime(t.start_time) > datetime('now', '-24 hours')
                 ORDER BY t.start_time DESC LIMIT 1",
            )
            .bind(cp_id)
            .bind(conn_id)
            .fetch_optional(&state.db)
            .await
            .map_err(db_err("MeterValues connector lookup"))?;
            match row {
                Some((id,)) => id,
                None => return Ok(json!({})),
            }
        }
    };

    // Prüfen, dass die Transaktion existiert und zur Wallbox gehört.
    let tx_row: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM transactions
         WHERE id = ?1 AND wallbox_id = (SELECT id FROM wallboxes WHERE charge_point_id = ?2)",
    )
    .bind(tx_id)
    .bind(cp_id)
    .fetch_optional(&state.db)
    .await
    .map_err(db_err("MeterValues tx lookup"))?;
    if tx_row.is_none() {
        return Ok(json!({}));
    }

    for mv in meter_value {
        let ts = sane_timestamp(mv.get("timestamp").and_then(|v| v.as_str()));
        let mut energy_wh: Option<i64> = None;
        let mut power_w: Option<i64> = None;
        let mut soc: Option<i64> = None;
        if let Some(sampled) = mv.get("sampledValue").and_then(|v| v.as_array()) {
            for s in sampled {
                // Einzelphasen-Werte (L1/L2/L3/…) überspringen — nur der
                // Eintrag ohne "phase" repräsentiert laut Spec den Gesamtwert.
                if s.get("phase").and_then(|v| v.as_str()).is_some() {
                    continue;
                }
                let measurand = s
                    .get("measurand")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Energy.Active.Import.Register");
                let unit = s.get("unit").and_then(|v| v.as_str()).unwrap_or("Wh");
                // value ist laut Schema ein String; manche Boxen senden aber
                // JSON-Zahlen. Unparsbare Werte überspringen statt 0 zu erfinden.
                let Some(val) = s
                    .get("value")
                    .and_then(|v| {
                        v.as_str()
                            .and_then(|s| s.trim().parse::<f64>().ok())
                            .or_else(|| v.as_f64())
                    })
                    .filter(|v| v.is_finite())
                else {
                    continue;
                };
                let scaled = match unit {
                    "kWh" => val * 1000.0,
                    "kW" => val * 1000.0,
                    _ => val,
                };
                match measurand {
                    "Energy.Active.Import.Register" => energy_wh = Some(scaled as i64),
                    "Power.Active.Import" => power_w = Some(scaled as i64),
                    "SoC" => soc = Some(val as i64),
                    _ => {}
                }
            }
        }

        // Sanity: negative und absurd große Werte verwerfen
        // (Zählerstand < 100 GWh, Leistung < 10 MW).
        let energy_wh = energy_wh.filter(|&v| (0..100_000_000_000).contains(&v));
        let power_w = power_w.filter(|&v| (0..10_000_000).contains(&v));
        let soc = soc.filter(|&v| (0..=100).contains(&v));

        sqlx::query(
            "INSERT INTO meter_values (transaction_id, timestamp, energy_wh, power_w, soc_percent)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(tx_id)
        .bind(rfc3339(ts))
        .bind(energy_wh)
        .bind(power_w)
        .bind(soc)
        .execute(&state.db)
        .await
        .map_err(db_err("MeterValues insert"))?;
    }

    Ok(json!({}))
}

fn db_err(ctx: &'static str) -> impl Fn(sqlx::Error) -> OcppCallError {
    move |e| {
        tracing::error!("DB {ctx}: {e}");
        OcppCallError {
            code: "InternalError".into(),
            description: format!("DB-Fehler: {ctx}"),
        }
    }
}

// -----------------------------------------------------------------------------
// Remote-Commands (vom Webserver ausgelöst)
// -----------------------------------------------------------------------------

pub async fn remote_start(
    hub: &Hub,
    cp_id: &str,
    id_tag: &str,
    connector_id: i64,
) -> anyhow::Result<String> {
    let payload = json!({
        "connectorId": connector_id,
        "idTag": id_tag
    });
    let resp = hub
        .call(cp_id, "RemoteStartTransaction", payload, Duration::from_secs(15))
        .await?;
    Ok(resp
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string())
}

pub async fn remote_stop(hub: &Hub, cp_id: &str, transaction_id: i64) -> anyhow::Result<String> {
    let payload = json!({ "transactionId": transaction_id });
    let resp = hub
        .call(cp_id, "RemoteStopTransaction", payload, Duration::from_secs(15))
        .await?;
    Ok(resp
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string())
}
