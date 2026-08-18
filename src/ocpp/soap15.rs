//! OCPP 1.5 (SOAP) Gerüst.
//!
//! OCPP 1.5 verwendet SOAP über HTTP (mit WS-Addressing). Der Endpunkt
//! empfängt POST-Requests mit XML-Body. Diese Implementierung liefert ein
//! minimales, gut konformes Antwortskelett für `BootNotification` und
//! `Heartbeat`; alle anderen Actions werden mit einem Soap-Fault
//! abgelehnt. Damit kann ein Bestand-Gerät minimal "grün" bleiben, bis die
//! SOAP-Seite vollständig ausgebaut ist.

use axum::body::Bytes;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::Utc;

use crate::AppState;

pub async fn soap_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let soap_action = headers
        .get("SOAPAction")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .trim_matches('"')
        .to_string();
    tracing::info!("OCPP 1.5 SOAP-Request – SOAPAction='{soap_action}' ({} bytes)", body.len());

    let body_str = String::from_utf8_lossy(&body);
    let cp_id = extract_header_value(&body_str, "chargeBoxIdentity").unwrap_or_default();

    let action_local = soap_action.rsplit('/').next().unwrap_or(&soap_action);

    let response_xml = match action_local {
        "BootNotification" => {
            if !cp_id.is_empty() {
                let _ = sqlx::query(
                    "INSERT INTO wallboxes (charge_point_id, name, ocpp_version, last_boot)
                     VALUES (?1, ?1, '1.5', ?2)
                     ON CONFLICT(charge_point_id) DO UPDATE SET
                       ocpp_version = excluded.ocpp_version,
                       last_boot = excluded.last_boot",
                )
                .bind(&cp_id)
                .bind(Utc::now().to_rfc3339())
                .execute(&state.db)
                .await;
            }
            boot_response(&cp_id)
        }
        "Heartbeat" => {
            if !cp_id.is_empty() {
                let _ = sqlx::query(
                    "UPDATE wallboxes SET last_heartbeat = ?1 WHERE charge_point_id = ?2",
                )
                .bind(Utc::now().to_rfc3339())
                .bind(&cp_id)
                .execute(&state.db)
                .await;
            }
            heartbeat_response()
        }
        _ => fault_response(action_local),
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/soap+xml; charset=utf-8")
        .body(response_xml)
        .unwrap()
}

fn extract_header_value(xml: &str, tag: &str) -> Option<String> {
    // Minimalparser – sucht <ns:tag>…</ns:tag>. Ausreichend für die Mini-Variante.
    let needle_open = format!(":{tag}>");
    let needle_alt = format!("<{tag}>");
    let start = xml.find(&needle_open).map(|i| i + needle_open.len()).or_else(|| {
        xml.find(&needle_alt).map(|i| i + needle_alt.len())
    })?;
    let rest = &xml[start..];
    let end = rest.find('<')?;
    Some(rest[..end].trim().to_string())
}

fn boot_response(_cp_id: &str) -> String {
    let now = Utc::now().to_rfc3339();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
               xmlns:cs="urn://Ocpp/Cs/2012/06/">
  <soap:Body>
    <cs:bootNotificationResponse>
      <cs:status>Accepted</cs:status>
      <cs:currentTime>{now}</cs:currentTime>
      <cs:heartbeatInterval>60</cs:heartbeatInterval>
    </cs:bootNotificationResponse>
  </soap:Body>
</soap:Envelope>"#
    )
}

fn heartbeat_response() -> String {
    let now = Utc::now().to_rfc3339();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope"
               xmlns:cs="urn://Ocpp/Cs/2012/06/">
  <soap:Body>
    <cs:heartbeatResponse>
      <cs:currentTime>{now}</cs:currentTime>
    </cs:heartbeatResponse>
  </soap:Body>
</soap:Envelope>"#
    )
}

fn fault_response(action: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<soap:Envelope xmlns:soap="http://www.w3.org/2003/05/soap-envelope">
  <soap:Body>
    <soap:Fault>
      <soap:Code><soap:Value>soap:Receiver</soap:Value></soap:Code>
      <soap:Reason><soap:Text xml:lang="de">Action '{action}' im 1.5-SOAP-Endpoint nicht implementiert.</soap:Text></soap:Reason>
    </soap:Fault>
  </soap:Body>
</soap:Envelope>"#
    )
}
