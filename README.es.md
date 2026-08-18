# easy-occp

🌐 [English](README.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md)

[![CI](https://github.com/hilman2/easy-occp/actions/workflows/ci.yml/badge.svg)](https://github.com/hilman2/easy-occp/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/hilman2/easy-occp)](https://github.com/hilman2/easy-occp/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Herramienta sencilla de gestión de estaciones de carga (cargadores) para **pymes con 1–10 cargadores**.

- **Un solo binario, un solo archivo SQLite** – sin base de datos externa, sin message broker.
- **OCPP 1.6J** (completo) + **OCPP 2.0.1** (esqueleto WebSocket, BootNotification / TransactionEvent) + **OCPP 1.5 SOAP** (esqueleto para Boot/Heartbeat).
- **Interfaz web moderna** (Askama + htmx), usuarios locales con contraseñas Argon2.
- Active Directory (LDAP) y Microsoft Entra (OIDC) están preparados como campos de configuración – las implementaciones concretas de bind/flow llegarán más adelante.
- Funciona en **Windows** (prioridad) **y Linux**.

## Funcionalidades

| Área            | Estado |
|-----------------|--------|
| Inventario de cargadores + estado (online/offline, conectores, firmware) | ✅ |
| Gestión de tarjetas RFID, alta mediante «ventana de aprendizaje» (2 min, intercepción de Authorize) | ✅ |
| Asignación tarjeta → empleado, tarjetas de invitado, caducidad de validez | ✅ |
| Desbloqueo remoto para invitados, imputación a una etiqueta de invitado | ✅ |
| Valores en vivo durante la carga (kWh cargados, potencia actual, SoC) | ✅ |
| Lista de transacciones, filtro por usuario | ✅ |
| Estadísticas por mes / trimestre / año | ✅ |
| Gestión de usuarios (admin/user), contraseña local | ✅ |
| Interfaz multilingüe (Deutsch, English, Français, Español) | ✅ |
| Active Directory (LDAP) | 🟡 Config preparada |
| Entra ID (OIDC)        | 🟡 Config preparada |

## Puesta en marcha

**Binarios listos para usar** (Windows x64, Linux x64) disponibles en
[Releases](https://github.com/hilman2/easy-occp/releases/latest) — descomprimir,
ejecutar `easy-occp.exe` o `easy-occp`, y listo (ver `INSTALL.es.md` en el paquete).

O compilarlo uno mismo:

```bash
# una sola vez: crear la configuración (opcional)
copy config.example.toml config.toml

# Build + inicio
cargo run --release
```

Después abrir la interfaz en <http://localhost:8080>. **Credenciales por defecto en el primer inicio:** `admin` / `admin` – cambie la contraseña de inmediato en «Usuarios».

¿Olvidó la contraseña de admin?

```bash
cargo run --release -- --reset-admin "nuevaContrasena123"
```

## Configurar un cargador

Los cargadores establecen una conexión WebSocket:

```
ws://<host>:8080/ocpp/<ChargePointId>
```

Los subprotocolos se negocian automáticamente (`ocpp1.6` u `ocpp2.0.1`).

Para dispositivos OCPP 1.5 más antiguos (SOAP):

```
POST http://<host>:8080/ocpp15
```

### Mediciones en vivo durante la carga

Al conectarse un cargador OCPP 1.6, el servidor lo configura automáticamente
para que durante una carga informe cada 30 segundos la lectura del contador, la
potencia y (si está disponible) el SoC (`MeterValueSampleInterval`,
`MeterValuesSampledData`). El intervalo puede ajustarse mediante `config.toml`;
`0` desactiva la configuración automática:

```toml
[ocpp]
meter_interval_s = 30
```

El panel y la página de detalle del cargador actualizan automáticamente las
cargas en curso cada 10 segundos (polling con htmx). Si un cargador no informa
la potencia, esta se deriva de las dos últimas lecturas del contador.

## Almacenamiento de datos

Todo se guarda en **un solo archivo SQLite** en `data/easy-occp.db` (modificable mediante `config.toml`). Las migraciones se encuentran en `migrations/` y se aplican automáticamente al iniciar.

### Comprobaciones de coherencia al recibir datos

- **Timestamps**: >24 h en el futuro o >10 años en el pasado se descartan – se recurre al reloj del servidor.
- **StartTransaction / StopTransaction**: idempotentes frente a repeticiones; los valores de contador decrecientes se corrigen.
- **StatusNotification**: UPSERT por (cargador, conector) – sin duplicados.
- **MeterValues**: los valores negativos se descartan, el SoC se valida entre 0 y 100 %, kWh → Wh normalizados.
- **Enrollment**: un tag recién capturado se asigna exactamente a una sesión abierta de ventana de aprendizaje.

## Estructura del proyecto

```
src/
  main.rs           – punto de entrada, runtime de Tokio, pool de SQLite
  config.rs         – configuración TOML
  db.rs             – bootstrap + helpers (Argon2, ajustes)
  error.rs          – AppError / IntoResponse
  auth/             – cookies de sesión + login local
  domain/           – modelos de datos (FromRow)
  ocpp/
    wire.rs         – parser de tramas JSON de OCPP
    hub.rs          – registro de todas las conexiones activas
    ocpp16.rs       – OCPP 1.6J (completo)
    ocpp20.rs       – OCPP 2.0.1 (bootstrap)
    soap15.rs       – endpoint OCPP 1.5 SOAP
  web/              – router axum, vistas Askama
templates/          – plantillas HTML (Askama)
static/             – CSS + shim de htmx (embebido vía rust-embed)
migrations/         – migraciones SQLite
```

## Licencia

MIT
