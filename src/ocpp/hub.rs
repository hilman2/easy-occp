//! Zentrale Registry aller aktiven OCPP-Verbindungen. Ermöglicht das Senden
//! von CALL-Nachrichten (z.B. RemoteStartTransaction) vom Webserver an die Wallbox.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use dashmap::DashMap;
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

static CONN_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub struct PendingCall {
    pub responder: oneshot::Sender<Result<Value, crate::ocpp::OcppCallError>>,
    pub action: String,
}

pub struct Connection {
    pub id: u64,
    pub charge_point_id: String,
    pub ocpp_version: String,
    pub tx: mpsc::Sender<String>,
    pub pending: Mutex<std::collections::HashMap<String, PendingCall>>,
}

impl Connection {
    pub fn new(charge_point_id: String, ocpp_version: String, tx: mpsc::Sender<String>) -> Self {
        Self {
            id: CONN_ID.fetch_add(1, Ordering::Relaxed),
            charge_point_id,
            ocpp_version,
            tx,
            pending: Mutex::new(Default::default()),
        }
    }
}

#[derive(Default)]
pub struct Hub {
    connections: DashMap<String, Arc<Connection>>,
}

impl Hub {
    pub fn register(&self, conn: Arc<Connection>) -> Option<Arc<Connection>> {
        // Wenn bereits eine Verbindung besteht, wird die alte verdrängt (OCPP-üblich).
        self.connections.insert(conn.charge_point_id.clone(), conn)
    }

    pub fn unregister(&self, cp_id: &str, conn_id: u64) {
        // Nur entfernen, wenn es noch genau diese Verbindung ist – verhindert
        // Race-Conditions bei Reconnects.
        if let Some(entry) = self.connections.get(cp_id) {
            if entry.value().id == conn_id {
                drop(entry);
                self.connections.remove(cp_id);
            }
        }
    }

    pub fn get(&self, cp_id: &str) -> Option<Arc<Connection>> {
        self.connections.get(cp_id).map(|e| e.value().clone())
    }

    pub fn list_online(&self) -> Vec<String> {
        self.connections.iter().map(|e| e.key().clone()).collect()
    }

    /// Sendet einen CALL an die Wallbox und wartet auf die Antwort.
    pub async fn call(
        &self,
        cp_id: &str,
        action: &str,
        payload: Value,
        timeout: Duration,
    ) -> Result<Value> {
        let conn = self
            .get(cp_id)
            .ok_or_else(|| anyhow!("Wallbox {cp_id} ist nicht verbunden"))?;

        let unique_id = uuid::Uuid::new_v4().to_string();
        let (resp_tx, resp_rx) = oneshot::channel();
        {
            let mut pending = conn.pending.lock().unwrap();
            pending.insert(
                unique_id.clone(),
                PendingCall {
                    responder: resp_tx,
                    action: action.to_string(),
                },
            );
        }

        let msg = crate::ocpp::OcppMessage::Call {
            unique_id: unique_id.clone(),
            action: action.to_string(),
            payload,
        };
        conn.tx
            .send(msg.serialize())
            .await
            .context("WebSocket-Queue geschlossen")?;

        match tokio::time::timeout(timeout, resp_rx).await {
            Ok(Ok(Ok(v))) => Ok(v),
            Ok(Ok(Err(e))) => Err(anyhow!("Wallbox-Fehler: {e}")),
            Ok(Err(_)) => Err(anyhow!("Antwort-Kanal geschlossen")),
            Err(_) => {
                conn.pending.lock().unwrap().remove(&unique_id);
                Err(anyhow!("Timeout beim Warten auf Antwort von {cp_id}"))
            }
        }
    }
}
