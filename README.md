# easy-occp

🌐 [English](README.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md)

[![CI](https://github.com/hilman2/easy-occp/actions/workflows/ci.yml/badge.svg)](https://github.com/hilman2/easy-occp/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/hilman2/easy-occp)](https://github.com/hilman2/easy-occp/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Simple management tool for EV charging stations (wallboxes), built for **SMBs with 1–10 wallboxes**.

- **One binary, one SQLite file** – no external database, no message broker.
- **OCPP 1.6J** (complete) + **OCPP 2.0.1** (WebSocket scaffold, BootNotification / TransactionEvent) + **OCPP 1.5 SOAP** (scaffold for Boot/Heartbeat).
- **Modern web UI** (Askama + htmx), local users with Argon2 passwords.
- Active Directory (LDAP) and Microsoft Entra (OIDC) are prepared as configuration fields – the concrete bind/flow implementations will follow.
- Runs on **Windows** (primary focus) **and Linux**.

## Features

| Area            | Status |
|-----------------|--------|
| Wallbox inventory + status (online/offline, connectors, firmware) | ✅ |
| RFID chip management, enrollment via "learning window" (2 min, Authorize intercept) | ✅ |
| Chip → employee assignment, guest chips, validity expiry | ✅ |
| Remote unlocking for guests, billing to guest label | ✅ |
| Live values during charging (charged kWh, current power, SoC) | ✅ |
| Transaction list, filter by user | ✅ |
| Statistics by month / quarter / year | ✅ |
| User management (admin/user), local password | ✅ |
| Multilingual UI (Deutsch, English, Français, Español) | ✅ |
| Active Directory (LDAP) | 🟡 Config prepared |
| Entra ID (OIDC)        | 🟡 Config prepared |

## Getting started

**Prebuilt binaries** (Windows x64, Linux x64) are available under
[Releases](https://github.com/hilman2/easy-occp/releases/latest) — unpack,
run `easy-occp.exe` or `easy-occp`, done (see `INSTALL.md` in the package).

Or build it yourself:

```bash
# once: create a config (optional)
copy config.example.toml config.toml

# Build + start
cargo run --release
```

Then open the UI at <http://localhost:8080>. **Default login on first start:** `admin` / `admin` – please change the password right away under "Users".

Forgot the admin password?

```bash
cargo run --release -- --reset-admin "newPassword123"
```

## Configuring a wallbox

Wallboxes establish a WebSocket connection:

```
ws://<host>:8080/ocpp/<ChargePointId>
```

Subprotocols are negotiated automatically (`ocpp1.6` or `ocpp2.0.1`).

For older OCPP 1.5 devices (SOAP):

```
POST http://<host>:8080/ocpp15
```

### Live meter readings during charging

When an OCPP 1.6 wallbox connects, the server automatically configures it to
report the meter reading, power, and (if available) SoC every 30 seconds during
a charging session (`MeterValueSampleInterval`, `MeterValuesSampledData`).
The interval can be adjusted via `config.toml`; `0` disables the
auto-configuration:

```toml
[ocpp]
meter_interval_s = 30
```

The cockpit and the wallbox detail page automatically refresh active charging
sessions every 10 seconds (htmx polling). If a wallbox does not report power,
it is derived from the last two meter readings.

## Data storage

Everything lives in **one SQLite file** at `data/easy-occp.db` (changeable via `config.toml`). Migrations reside in `migrations/` and are applied automatically at startup.

### Sanity checks on incoming data

- **Timestamps**: >24 h in the future or >10 years in the past are discarded – fallback to the server clock.
- **StartTransaction / StopTransaction**: Idempotent against repeats; decreasing meter values are corrected.
- **StatusNotification**: UPSERT per (wallbox, connector) – no duplicates.
- **MeterValues**: negative values are discarded, SoC validated to 0–100 %, kWh → Wh normalized.
- **Enrollment**: A newly captured tag is assigned to exactly one open learning-window session.

## Project layout

```
src/
  main.rs           – entry point, Tokio runtime, SQLite pool
  config.rs         – TOML configuration
  db.rs             – bootstrap + helpers (Argon2, settings)
  error.rs          – AppError / IntoResponse
  auth/             – session cookies + local login
  domain/           – data models (FromRow)
  ocpp/
    wire.rs         – OCPP JSON frame parser
    hub.rs          – registry of all active connections
    ocpp16.rs       – OCPP 1.6J (complete)
    ocpp20.rs       – OCPP 2.0.1 (bootstrap)
    soap15.rs       – OCPP 1.5 SOAP endpoint
  web/              – axum router, Askama views
templates/          – HTML templates (Askama)
static/             – CSS + htmx shim (embedded via rust-embed)
migrations/         – SQLite migrations
```

## License

MIT
