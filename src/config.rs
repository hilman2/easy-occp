use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub http: HttpConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub ocpp: OcppConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OcppConfig {
    /// Intervall in Sekunden, in dem Wallboxen während einer Ladung Zählerstand
    /// und Leistung melden sollen (MeterValues). Wird beim Verbinden per
    /// ChangeConfiguration in der Wallbox gesetzt.
    /// 0 = Auto-Konfiguration deaktivieren, Wallbox-Einstellung bleibt unberührt.
    #[serde(default = "default_meter_interval_s")]
    pub meter_interval_s: u32,
}

impl Default for OcppConfig {
    fn default() -> Self {
        Self {
            meter_interval_s: default_meter_interval_s(),
        }
    }
}

fn default_meter_interval_s() -> u32 {
    30
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HttpConfig {
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_public_base_url")]
    pub public_base_url: String,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            public_base_url: default_public_base_url(),
        }
    }
}

fn default_bind() -> String {
    "0.0.0.0:8080".to_string()
}
fn default_public_base_url() -> String {
    "http://localhost:8080".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorageConfig {
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    #[serde(default = "default_db_file")]
    pub db_file: String,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            db_file: default_db_file(),
        }
    }
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("data")
}
fn default_db_file() -> String {
    "easy-occp.db".to_string()
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub ldap: Option<LdapConfig>,
    #[serde(default)]
    pub oidc: Option<OidcConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LdapConfig {
    pub url: String,
    pub bind_dn: String,
    pub bind_password: String,
    pub user_base_dn: String,
    pub user_filter: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OidcConfig {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            tracing::warn!(
                "Keine Konfigurationsdatei unter {:?} gefunden – verwende Defaults.",
                path
            );
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("Kann Konfigurationsdatei {:?} nicht lesen", path))?;
        let cfg: Config =
            toml::from_str(&raw).with_context(|| format!("TOML-Fehler in {:?}", path))?;
        Ok(cfg)
    }

    pub fn data_dir(&self) -> &Path {
        &self.storage.data_dir
    }

    pub fn db_path(&self) -> PathBuf {
        self.storage.data_dir.join(&self.storage.db_file)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            http: HttpConfig::default(),
            storage: StorageConfig::default(),
            auth: AuthConfig::default(),
            ocpp: OcppConfig::default(),
        }
    }
}
