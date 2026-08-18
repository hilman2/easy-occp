//! OCPP 2.0.1-spezifische Abweichungen.
//!
//! 2.0.1 hat ein anderes Datenmodell (z.B. `TransactionEvent` statt
//! StartTransaction/StopTransaction). Aktuell handhabt `ocpp16::handle_call`
//! gemeinsame Actions (BootNotification, Heartbeat, Authorize, StatusNotification,
//! MeterValues, DataTransfer) pragmatisch gemeinsam.
//!
//! Hier implementieren wir ausschließlich die 2.0.1-only Actions. Gibt diese
//! Funktion `Ok(None)` zurück, soll der gemeinsame Pfad (ocpp16::handle_call)
//! übernehmen.

use chrono::Utc;
use serde_json::{json, Value};

use crate::AppState;

pub async fn maybe_handle(
    state: &AppState,
    cp_id: &str,
    action: &str,
    payload: &Value,
) -> Result<Option<Value>, crate::ocpp::OcppCallError> {
    match action {
        "TransactionEvent" => {
            handle_transaction_event(state, cp_id, payload).await.map(Some)
        }
        "NotifyReport" | "NotifyEvent" | "NotifyMonitoringReport" => Ok(Some(json!({}))),
        _ => Ok(None),
    }
}

/// Sehr schlanke Implementierung von `TransactionEvent`.
/// Dokumentiert Startpunkt, Updates und Ende einer Transaktion.
async fn handle_transaction_event(
    state: &AppState,
    cp_id: &str,
    payload: &Value,
) -> Result<Value, crate::ocpp::OcppCallError> {
    let event_type = payload
        .get("eventType")
        .and_then(|v| v.as_str())
        .unwrap_or("Updated");
    let trigger = payload.get("triggerReason").and_then(|v| v.as_str()).unwrap_or("");

    tracing::info!(
        "OCPP 2.0.1 TransactionEvent von {cp_id}: eventType={event_type}, trigger={trigger}"
    );

    // Wir erlauben aktuell kostenloses Laden – eine vollständige 2.0.1-
    // Umsetzung folgt in einem späteren Schritt.
    let _ = state;
    let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    Ok(json!({
        "totalCost": 0,
        "chargingPriority": 0,
        "idTokenInfo": {"status": "Accepted"},
        "updatedPersonalMessage": null,
        "currentTime": now
    }))
}
