use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Transaction {
    pub id: i64,
    pub ocpp_transaction_id: Option<i64>,
    pub wallbox_id: i64,
    pub connector_id: i64,
    pub id_tag: String,
    pub chip_id: Option<i64>,
    pub user_id: Option<i64>,
    pub guest_label: Option<String>,
    pub start_time: String,
    pub start_meter_wh: i64,
    pub stop_time: Option<String>,
    pub stop_meter_wh: Option<i64>,
    pub stop_reason: Option<String>,
    pub started_remote: i64,
}

impl Transaction {
    pub fn energy_wh(&self) -> Option<i64> {
        self.stop_meter_wh.map(|s| (s - self.start_meter_wh).max(0))
    }
}

/// Aktuelle Messwerte einer laufenden Transaktion, abgeleitet aus den zuletzt
/// gemeldeten MeterValues.
#[derive(Debug, Clone, Default)]
pub struct LiveMeter {
    /// Bisher geladene Energie (letzter Zählerstand − Startzählerstand).
    pub energy_wh: Option<i64>,
    /// Aktuelle Ladeleistung in Watt.
    pub power_w: Option<i64>,
    /// Ladestand des Fahrzeugs, falls die Wallbox ihn meldet.
    pub soc_percent: Option<i64>,
}

/// Holt die zuletzt gemeldeten Messwerte einer Transaktion. Meldet die Wallbox
/// keine Leistung, wird sie aus den letzten beiden Zählerständen abgeleitet
/// (ΔWh / Δt), sofern die beiden Messungen höchstens eine Stunde auseinanderliegen.
/// Leistung wird nur zurückgegeben, wenn die Messung frisch ist — sonst würde
/// nach einem Verbindungsabriss dauerhaft eine veraltete Leistung angezeigt.
pub async fn live_meter(
    db: &sqlx::SqlitePool,
    tx_id: i64,
    start_meter_wh: i64,
) -> sqlx::Result<LiveMeter> {
    let energy_rows: Vec<(i64, String)> = sqlx::query_as(
        "SELECT energy_wh, timestamp FROM meter_values
         WHERE transaction_id = ?1 AND energy_wh IS NOT NULL
         ORDER BY timestamp DESC, id DESC LIMIT 2",
    )
    .bind(tx_id)
    .fetch_all(db)
    .await?;
    let power_row: Option<(i64, String)> = sqlx::query_as(
        "SELECT power_w, timestamp FROM meter_values
         WHERE transaction_id = ?1 AND power_w IS NOT NULL
         ORDER BY timestamp DESC, id DESC LIMIT 1",
    )
    .bind(tx_id)
    .fetch_optional(db)
    .await?;
    let soc_row: Option<(i64,)> = sqlx::query_as(
        "SELECT soc_percent FROM meter_values
         WHERE transaction_id = ?1 AND soc_percent IS NOT NULL
         ORDER BY timestamp DESC, id DESC LIMIT 1",
    )
    .bind(tx_id)
    .fetch_optional(db)
    .await?;

    let energy_wh = energy_rows
        .first()
        .map(|(wh, _)| (wh - start_meter_wh).max(0));

    let mut power_w = power_row
        .filter(|(_, ts)| is_fresh(ts))
        .map(|(w, _)| w);
    if power_w.is_none() && energy_rows.len() == 2 && is_fresh(&energy_rows[0].1) {
        let (wh_new, ts_new) = &energy_rows[0];
        let (wh_old, ts_old) = &energy_rows[1];
        if let (Ok(new), Ok(old)) = (
            chrono::DateTime::parse_from_rfc3339(ts_new),
            chrono::DateTime::parse_from_rfc3339(ts_old),
        ) {
            let dt_s = (new - old).num_seconds();
            if (1..=3600).contains(&dt_s) {
                power_w = (wh_new - wh_old).max(0).checked_mul(3600).map(|x| x / dt_s);
            }
        }
    }

    Ok(LiveMeter {
        energy_wh,
        power_w,
        soc_percent: soc_row.map(|(s,)| s),
    })
}

/// Messwert gilt als frisch, wenn er höchstens 5 Minuten alt ist
/// (10× das Standard-Meldeintervall von 30 s).
fn is_fresh(ts: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(ts)
        .map(|t| chrono::Utc::now() - t.with_timezone(&chrono::Utc) <= chrono::Duration::minutes(5))
        .unwrap_or(false)
}

/// Formatiert Wh als kWh mit einer Nachkommastelle und deutschem Komma („12,3“).
pub fn fmt_kwh(wh: i64) -> String {
    format!("{:.1}", wh as f64 / 1000.0).replace('.', ",")
}

/// Formatiert W als kW mit einer Nachkommastelle („7,4“).
pub fn fmt_kw(w: i64) -> String {
    format!("{:.1}", w as f64 / 1000.0).replace('.', ",")
}
