# easy-occp v0.3.0 – Installation & First Start (Windows)

🌐 [English](INSTALL.md) · [Deutsch](ANLEITUNG.md) · [Français](INSTALL.fr.md) · [Español](INSTALL.es.md)

Management tool for wallboxes (OCPP 1.5/1.6/2.0.1) — one binary, one SQLite file.

## What's new in v0.3.0

- **Multilingual web UI:** The interface is now available in Deutsch, English,
  Français and Español. The language is detected automatically from your
  browser settings and can be changed at any time via the switcher in the
  header.
- **GitHub repository with CI and automatic releases:** The source code now
  lives at <https://github.com/hilman2/easy-occp>. Every version is built
  automatically by CI and published as a release.

## What's new in v0.2.0

- **Live values during charging:** The cockpit and the wallbox detail page show
  the **kWh** charged so far, the current **power (kW)** and — if the wallbox
  reports it — the vehicle's **SoC** for running charging sessions. The display
  refreshes automatically every 10 seconds.
- **Automatic wallbox configuration:** On connect, the server configures the
  wallbox to report meter values every 30 seconds
  (`MeterValueSampleInterval`, `MeterValuesSampledData`) — adjustable via the
  new `[ocpp]` section in `config.toml`.
- Various robustness fixes in meter-value processing (phase values, invalid
  values, stale displays).

**Updating from v0.1.0:** Simply copy `easy-occp.exe` over the existing file
and restart the service. The database (`data\easy-occp.db`) remains unchanged;
there are no new migrations. Live values appear from the next charging session
onward, once the wallbox has reconnected.

## Contents of this folder

```
easy-occp.exe          Main program (Windows x64, all-in-one)
config.example.toml    Configuration template
README.md              Project overview
INSTALL.md             This file
ANLEITUNG.md           German version of this guide
```

The `.exe` contains the database schema, the web UI and all static assets — **nothing else needs to be shipped**.

## 1. Installation

1. Copy this folder to a permanent location, e.g. `C:\easy-occp\`.
2. Optional: copy `config.example.toml` to `config.toml` and adjust it (see below). Without a `config.toml` the defaults apply (port 8080, data directory `./data`).
3. Make sure port **8080** is free on the host and allowed inbound in the Windows Firewall — otherwise the wallboxes cannot reach the server.

## 2. First start

Open a **PowerShell** in the folder and start:

```powershell
.\easy-occp.exe
```

On first start the application takes care of everything itself:

- creates the `data\` directory,
- creates the SQLite file `data\easy-occp.db`,
- applies the complete schema (migrations are embedded in the binary),
- creates the admin user.

Then open in your browser:

<http://localhost:8080>

**Default login:** `admin` / `admin` — please change the password immediately under "Users".

Stop with `Ctrl+C` in the PowerShell window.

## 3. Configuration (`config.toml`)

The following minimum is sufficient:

```toml
[http]
bind = "0.0.0.0:8080"
public_base_url = "http://<server-ip-or-hostname>:8080"

[storage]
data_dir = "data"
db_file  = "easy-occp.db"

[ocpp]
# Interval in seconds at which wallboxes report meter reading and power during
# a charging session. Set automatically in the wallbox on connect.
# 0 = disable auto-configuration.
meter_interval_s = 30
```

- `bind = "0.0.0.0:8080"` — listens on all network adapters (required so the wallboxes can connect).
- `public_base_url` — public address at which the server is reachable from the wallboxes' point of view.
- `meter_interval_s` — reporting interval for the live meter values (default 30 s).

The LDAP / Entra ID sections are commented out in the example — currently field stubs, not active.

## 4. Setting up a wallbox

Enter the wallbox backend URL in the device configuration (vendor portal or the box's web UI):

**OCPP 1.6 / 2.0.1 (WebSocket):**
```
ws://<server-ip>:8080/ocpp/<ChargePointId>
```

`<ChargePointId>` is the wallbox's unique ID (freely choosable, e.g. `WB-Halle-01`). The subprotocol (`ocpp1.6` / `ocpp2.0.1`) is negotiated automatically.

**OCPP 1.5 (SOAP, legacy devices only):**
```
http://<server-ip>:8080/ocpp15
```

As soon as the wallbox connects, it appears under **Wallboxes** with status *online*. Shortly after connecting, the server automatically configures the box for the live meter values (OCPP 1.6 only).

## 5. Enrolling RFID chips

1. In the UI → **Chips → Open learning window** (active for 2 minutes).
2. Authenticate at the wallbox with the new RFID chip.
3. The chip appears in the list and can be assigned to an employee or marked as a guest chip (incl. expiry date).

## 6. Running permanently as a Windows service (optional)

`easy-occp.exe` is an ordinary console program. For autostart/service:

**Option A: Task Scheduler**

- Task Scheduler → *Create Task* → Trigger *At system startup* → Action *Start a program* → `C:\easy-occp\easy-occp.exe` → *Start in:* `C:\easy-occp\`.
- Enable *"Run whether user is logged on or not"*.

**Option B: NSSM (Non-Sucking Service Manager)**

```powershell
nssm install easy-occp "C:\easy-occp\easy-occp.exe"
nssm set easy-occp AppDirectory "C:\easy-occp"
nssm start easy-occp
```

## 7. Resetting the admin password

If the admin password is lost:

```powershell
.\easy-occp.exe --reset-admin "newPassword123"
```

Resets the password of the `admin` account and exits.

## 8. Backup

Everything relevant lives in **a single file**: `data\easy-occp.db`.

For a consistent backup, briefly stop the service and copy the file (incl. any `-wal` / `-shm`) somewhere safe — done.

## 9. Troubleshooting

| Problem | Cause / solution |
|---------|------------------|
| Browser shows "page unreachable" | Port 8080 in use or blocked by the firewall. Check with `netstat -ano \| findstr :8080`. |
| Wallbox does not connect | Check `public_base_url` / firewall / `ChargePointId` URL. The console logs show incoming WS connections. |
| No live values during charging | Let the wallbox reconnect (the configuration is applied on connect). Check the console log: `MeterValueSampleInterval` should appear as "set". Some boxes need a restart, others do not support power measurement — in that case the power is derived from the meter readings. |
| "database is locked" | Do not start a second `easy-occp.exe` on the same DB at the same time. |
| More logs wanted | Before starting: set `$env:RUST_LOG="debug"`. |

## 10. Uninstallation

Remove the service/task, delete the folder — there are no registry entries and no external dependencies.

---

Version 0.3.0 · License: MIT
