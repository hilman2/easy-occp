# easy-occp v0.2.0 – Installation & Erststart (Windows)

Management-Tool für Wallboxen (OCPP 1.5/1.6/2.0.1) — ein Binary, eine SQLite-Datei.

## Was ist neu in v0.2.0

- **Live-Werte während der Ladung:** Cockpit und Wallbox-Detailseite zeigen für
  laufende Ladungen die bisher geladenen **kWh**, die aktuelle **Leistung (kW)**
  und — falls die Wallbox ihn meldet — den **SoC** des Fahrzeugs. Die Anzeige
  aktualisiert sich alle 10 Sekunden automatisch.
- **Automatische Wallbox-Konfiguration:** Beim Verbinden richtet der Server die
  Wallbox so ein, dass sie alle 30 Sekunden Messwerte meldet
  (`MeterValueSampleInterval`, `MeterValuesSampledData`) — einstellbar über die
  neue `[ocpp]`-Sektion in der `config.toml`.
- Diverse Robustheits-Fixes bei der Messwert-Verarbeitung (Phasenwerte,
  fehlerhafte Werte, veraltete Anzeigen).

**Update von v0.1.0:** Einfach die `easy-occp.exe` über die bestehende Datei
kopieren und den Dienst neu starten. Die Datenbank (`data\easy-occp.db`) bleibt
unverändert; es gibt keine neuen Migrationen. Die Live-Werte erscheinen ab der
nächsten Ladung, nachdem sich die Wallbox neu verbunden hat.

## Inhalt dieses Ordners

```
easy-occp.exe          Hauptprogramm (Windows x64, alles inkl.)
config.example.toml    Vorlage für die Konfiguration
README.md              Projekt-Übersicht
ANLEITUNG.md           Diese Datei
```

Die `.exe` enthält Datenbank-Schema, Web-UI und statische Assets — **nichts weiter muss mitgeliefert werden**.

## 1. Installation

1. Diesen Ordner an einen festen Ort kopieren, z. B. `C:\easy-occp\`.
2. Optional: `config.example.toml` nach `config.toml` kopieren und anpassen (siehe unten). Ohne `config.toml` laufen die Defaults (Port 8080, Datenverzeichnis `./data`).
3. Sicherstellen, dass Port **8080** auf dem Host frei ist und in der Windows-Firewall eingehend freigegeben ist — sonst erreichen die Wallboxen den Server nicht.

## 2. Erststart

Im Ordner eine **PowerShell** öffnen und starten:

```powershell
.\easy-occp.exe
```

Beim ersten Start erledigt die Anwendung alles selbst:

- legt das Verzeichnis `data\` an,
- erzeugt die SQLite-Datei `data\easy-occp.db`,
- spielt das komplette Schema ein (Migrationen sind in die Binary eingebettet),
- legt den Admin-Benutzer an.

Danach im Browser öffnen:

<http://localhost:8080>

**Default-Login:** `admin` / `admin` — Passwort bitte sofort unter „Benutzer" ändern.

Beenden mit `Strg+C` im PowerShell-Fenster.

## 3. Konfiguration (`config.toml`)

Minimal reicht Folgendes:

```toml
[http]
bind = "0.0.0.0:8080"
public_base_url = "http://<server-ip-oder-hostname>:8080"

[storage]
data_dir = "data"
db_file  = "easy-occp.db"

[ocpp]
# Intervall in Sekunden, in dem Wallboxen während einer Ladung Zählerstand und
# Leistung melden. Wird beim Verbinden automatisch in der Wallbox gesetzt.
# 0 = Auto-Konfiguration deaktivieren.
meter_interval_s = 30
```

- `bind = "0.0.0.0:8080"` — hört auf allen Netzwerkadaptern (nötig, damit Wallboxen verbinden können).
- `public_base_url` — öffentliche Adresse, unter der der Server aus Sicht der Wallboxen erreichbar ist.
- `meter_interval_s` — Meldeintervall der Live-Messwerte (Standard 30 s).

LDAP / Entra-ID-Abschnitte sind im Beispiel auskommentiert — derzeit Feld-Stubs, nicht aktiv.

## 4. Wallbox einrichten

Wallbox-Backend-URL in der jeweiligen Geräte-Konfiguration (Herstellerportal oder Web-UI der Box) eintragen:

**OCPP 1.6 / 2.0.1 (WebSocket):**
```
ws://<server-ip>:8080/ocpp/<ChargePointId>
```

`<ChargePointId>` ist die eindeutige ID der Wallbox (frei wählbar, z. B. `WB-Halle-01`). Subprotokoll (`ocpp1.6` / `ocpp2.0.1`) wird automatisch ausgehandelt.

**OCPP 1.5 (SOAP, nur Altgeräte):**
```
http://<server-ip>:8080/ocpp15
```

Sobald die Wallbox verbindet, erscheint sie unter **Wallboxen** mit Status *online*. Kurz nach dem Verbinden konfiguriert der Server die Box automatisch für die Live-Messwerte (nur OCPP 1.6).

## 5. Chips anlernen

1. In der UI → **Chips → Lernfenster öffnen** (2 Minuten aktiv).
2. An der Wallbox mit dem neuen RFID-Chip authentifizieren.
3. Der Chip erscheint in der Liste und kann einem Mitarbeiter zugeordnet oder als Gast-Chip markiert werden (inkl. Ablaufdatum).

## 6. Als Windows-Dienst dauerhaft laufen lassen (optional)

`easy-occp.exe` ist ein gewöhnliches Konsolenprogramm. Für Autostart/Service:

**Variante A: Aufgabenplanung**

- Aufgabenplanung → *Aufgabe erstellen* → Trigger *Beim Systemstart* → Aktion *Programm starten* → `C:\easy-occp\easy-occp.exe` → *Starten in:* `C:\easy-occp\`.
- *„Unabhängig von der Benutzeranmeldung ausführen"* aktivieren.

**Variante B: NSSM (Non-Sucking Service Manager)**

```powershell
nssm install easy-occp "C:\easy-occp\easy-occp.exe"
nssm set easy-occp AppDirectory "C:\easy-occp"
nssm start easy-occp
```

## 7. Admin-Passwort zurücksetzen

Falls das Admin-Passwort verloren ist:

```powershell
.\easy-occp.exe --reset-admin "neuesPasswort123"
```

Setzt das Passwort des `admin`-Accounts neu und beendet sich.

## 8. Backup

Alles Relevante liegt in **einer einzigen Datei**: `data\easy-occp.db`.

Für ein konsistentes Backup den Dienst kurz stoppen und die Datei (inkl. evtl. `-wal` / `-shm`) wegsichern — fertig.

## 9. Fehlersuche

| Problem | Ursache / Lösung |
|---------|------------------|
| Browser zeigt „Seite nicht erreichbar" | Port 8080 belegt oder Firewall blockiert. `netstat -ano \| findstr :8080` prüfen. |
| Wallbox verbindet nicht | `public_base_url` / Firewall / `ChargePointId`-URL prüfen. Logs in der Konsole zeigen eingehende WS-Verbindungen. |
| Keine Live-Werte während der Ladung | Wallbox neu verbinden lassen (Konfiguration wird beim Connect gesetzt). Konsolen-Log prüfen: `MeterValueSampleInterval` sollte als „gesetzt" erscheinen. Manche Boxen brauchen einen Neustart, andere unterstützen keine Leistungsmessung — dann wird die Leistung aus den Zählerständen abgeleitet. |
| „database is locked" | Kein zweites `easy-occp.exe` gleichzeitig auf derselben DB starten. |
| Mehr Logs gewünscht | Vor dem Start: `$env:RUST_LOG="debug"` setzen. |

## 10. Deinstallation

Dienst/Aufgabe entfernen, Ordner löschen — es gibt keine Registry-Einträge und keine externen Abhängigkeiten.

---

Version 0.2.0 · Lizenz: MIT
