# easy-occp

[![CI](https://github.com/hilman2/easy-occp/actions/workflows/ci.yml/badge.svg)](https://github.com/hilman2/easy-occp/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/hilman2/easy-occp)](https://github.com/hilman2/easy-occp/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Einfaches Management-Tool für Wallboxen für **KMUs mit 1–10 Wallboxen**.

- **Ein Binary, eine SQLite-Datei** – keine externe Datenbank, kein Message-Broker.
- **OCPP 1.6J** (vollständig) + **OCPP 2.0.1** (WebSocket-Gerüst, BootNotification / TransactionEvent) + **OCPP 1.5 SOAP** (Gerüst für Boot/Heartbeat).
- **Moderne Web-UI** (Askama + htmx), lokale Benutzer mit Argon2-Passwörtern.
- Active Directory (LDAP) und Microsoft Entra (OIDC) sind als Konfigurations-Felder vorbereitet – die konkreten Bind-/Flow-Implementierungen folgen.
- Läuft auf **Windows** (Fokus) **und Linux**.

## Features

| Bereich         | Status |
|-----------------|--------|
| Wallbox-Inventar + Status (online/offline, Connectors, Firmware) | ✅ |
| Chip-Verwaltung, Anlernen via „Lernfenster" (2 min, Authorize-Intercept) | ✅ |
| Chip → Mitarbeiter-Zuordnung, Gast-Chips, Gültigkeits-Ablauf | ✅ |
| Remote-Freischalten für Gäste, Buchung auf Gast-Label | ✅ |
| Live-Werte während der Ladung (geladene kWh, aktuelle Leistung, SoC) | ✅ |
| Transaktionsliste, Filter nach Benutzer | ✅ |
| Statistik nach Monat / Quartal / Jahr | ✅ |
| Benutzerverwaltung (admin/user), lokales Passwort | ✅ |
| Active Directory (LDAP) | 🟡 Konfig vorbereitet |
| Entra ID (OIDC)        | 🟡 Konfig vorbereitet |

## Starten

**Fertige Binaries** (Windows x64, Linux x64) gibt es unter
[Releases](https://github.com/hilman2/easy-occp/releases/latest) — entpacken,
`easy-occp.exe` bzw. `easy-occp` starten, fertig (siehe `ANLEITUNG.md` im Paket).

Oder selbst bauen:

```bash
# einmalig: Konfig anlegen (optional)
copy config.example.toml config.toml

# Build + Start
cargo run --release
```

Danach die UI unter <http://localhost:8080> öffnen. **Default-Login beim ersten Start:** `admin` / `admin` – Passwort bitte direkt unter „Benutzer" ändern.

Admin-Passwort vergessen?

```bash
cargo run --release -- --reset-admin "neuesPasswort123"
```

## Wallbox konfigurieren

Wallboxen stellen eine WebSocket-Verbindung her:

```
ws://<host>:8080/ocpp/<ChargePointId>
```

Subprotokolle werden automatisch ausgehandelt (`ocpp1.6` oder `ocpp2.0.1`).

Für ältere OCPP-1.5-Geräte (SOAP):

```
POST http://<host>:8080/ocpp15
```

### Live-Messwerte während der Ladung

Beim Verbinden einer OCPP-1.6-Wallbox konfiguriert der Server sie automatisch so,
dass sie während einer Ladung alle 30 Sekunden Zählerstand, Leistung und (falls
verfügbar) SoC meldet (`MeterValueSampleInterval`, `MeterValuesSampledData`).
Das Intervall ist über `config.toml` einstellbar, `0` deaktiviert die
Auto-Konfiguration:

```toml
[ocpp]
meter_interval_s = 30
```

Cockpit und Wallbox-Detailseite aktualisieren die laufenden Ladungen alle
10 Sekunden automatisch (htmx-Polling). Meldet eine Wallbox keine Leistung,
wird sie aus den letzten beiden Zählerständen abgeleitet.

## Datenhaltung

Alles liegt in **einer SQLite-Datei** unter `data/easy-occp.db` (über `config.toml` änderbar). Migrationen liegen in `migrations/` und werden beim Start automatisch angewendet.

### Sanity-Checks beim Datenempfang

- **Timestamps**: >24 h in der Zukunft oder >10 Jahre in der Vergangenheit werden verworfen – Fallback auf die Server-Uhrzeit.
- **StartTransaction / StopTransaction**: Idempotent gegen Wiederholungen; rückläufige Meter-Werte werden korrigiert.
- **StatusNotification**: UPSERT pro (Wallbox, Connector) – keine Duplikate.
- **MeterValues**: negative Werte werden verworfen, SoC auf 0–100 % validiert, kWh → Wh normalisiert.
- **Enrollment**: Ein neu erfasster Tag wird genau einer offenen Lernfenster-Session zugeordnet.

## Projekt-Layout

```
src/
  main.rs           – Einstiegspunkt, Tokio-Runtime, SQLite-Pool
  config.rs         – TOML-Konfiguration
  db.rs             – Bootstrap + Helpers (Argon2, Settings)
  error.rs          – AppError / IntoResponse
  auth/             – Session-Cookies + lokaler Login
  domain/           – Datenmodelle (FromRow)
  ocpp/
    wire.rs         – OCPP-JSON-Frame-Parser
    hub.rs          – Registry aller aktiven Verbindungen
    ocpp16.rs       – OCPP 1.6J (vollständig)
    ocpp20.rs       – OCPP 2.0.1 (Bootstrap)
    soap15.rs       – OCPP 1.5 SOAP-Endpunkt
  web/              – axum-Router, Askama-Views
templates/          – HTML-Templates (Askama)
static/             – CSS + htmx-Shim (embedded via rust-embed)
migrations/         – SQLite-Migrationen
```

## Lizenz

MIT
