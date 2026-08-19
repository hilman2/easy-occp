# easy-occp v0.3.1 – Instalación y primer arranque (Windows)

🌐 [English](INSTALL.md) · [Deutsch](ANLEITUNG.md) · [Français](INSTALL.fr.md) · [Español](INSTALL.es.md)

Herramienta de gestión para cargadores (OCPP 1.5/1.6/2.0.1) — un solo binario, un solo archivo SQLite.

## Novedades de la v0.3.0

- **Interfaz web multilingüe:** la interfaz está ahora disponible en Deutsch,
  English, Français y Español. El idioma se detecta automáticamente a partir de
  la configuración del navegador y puede cambiarse en cualquier momento con el
  selector de la cabecera.
- **Repositorio de GitHub con CI y releases automáticas:** el código fuente
  está ahora en <https://github.com/hilman2/easy-occp>. Cada versión se compila
  automáticamente mediante CI y se publica como release.

## Novedades de la v0.2.0

- **Valores en vivo durante la carga:** el panel y la página de detalle del
  cargador muestran, para las cargas en curso, los **kWh** cargados hasta
  el momento, la **potencia (kW)** actual y — si el cargador lo comunica — el
  **SoC** del vehículo. La pantalla se actualiza automáticamente cada 10
  segundos.
- **Configuración automática del cargador:** al conectarse, el servidor
  configura el cargador para que envíe lecturas del contador cada 30 segundos
  (`MeterValueSampleInterval`, `MeterValuesSampledData`) — ajustable mediante
  la nueva sección `[ocpp]` del `config.toml`.
- Diversas correcciones de robustez en el procesamiento de las lecturas
  (valores por fase, valores erróneos, indicaciones obsoletas).

**Actualización desde la v0.1.0:** basta con copiar `easy-occp.exe` sobre el
archivo existente y reiniciar el servicio. La base de datos
(`data\easy-occp.db`) permanece sin cambios; no hay migraciones nuevas. Los
valores en vivo aparecen a partir de la siguiente carga, una vez que el
cargador se haya vuelto a conectar.

## Contenido de esta carpeta

```
easy-occp.exe          Programa principal (Windows x64, todo incluido)
config.example.toml    Plantilla de configuración
README.md              Resumen del proyecto
INSTALL.es.md          Esta guía
ANLEITUNG.md           Versión alemana de esta guía
```

El `.exe` contiene el esquema de la base de datos, la interfaz web y los recursos estáticos — **no hace falta distribuir nada más**.

## 1. Instalación

1. Copie esta carpeta a una ubicación fija, p. ej. `C:\easy-occp\`.
2. Opcional: copie `config.example.toml` como `config.toml` y ajústelo (véase más abajo). Sin `config.toml` se aplican los valores por defecto (puerto 8080, directorio de datos `./data`).
3. Asegúrese de que el puerto **8080** esté libre en el host y permitido en entrada en el Firewall de Windows — de lo contrario, los cargadores no podrán alcanzar el servidor.

## 2. Primer arranque

Abra un **PowerShell** en la carpeta e inicie:

```powershell
.\easy-occp.exe
```

En el primer arranque, la aplicación se encarga de todo por sí misma:

- crea el directorio `data\`,
- crea el archivo SQLite `data\easy-occp.db`,
- aplica el esquema completo (las migraciones están integradas en el binario),
- crea el usuario admin.

Después, abra en el navegador:

<http://localhost:8080>

**Credenciales por defecto:** `admin` / `admin` — cambie la contraseña de inmediato en «Usuarios».

Detenga con `Ctrl+C` en la ventana de PowerShell.

## 3. Configuración (`config.toml`)

Con este mínimo es suficiente:

```toml
[http]
bind = "0.0.0.0:8080"
public_base_url = "http://<ip-o-nombre-del-servidor>:8080"

[storage]
data_dir = "data"
db_file  = "easy-occp.db"

[ocpp]
# Intervalo en segundos en el que los cargadores comunican la lectura del
# contador y la potencia durante una carga. Se establece automáticamente en el
# cargador al conectarse.
# 0 = desactivar la configuración automática.
meter_interval_s = 30
```

- `bind = "0.0.0.0:8080"` — escucha en todos los adaptadores de red (necesario para que los cargadores puedan conectarse).
- `public_base_url` — dirección pública en la que el servidor es accesible desde el punto de vista de los cargadores.
- `meter_interval_s` — intervalo de envío de las lecturas en vivo (30 s por defecto).

Las secciones LDAP / Entra ID están comentadas en el ejemplo — por ahora son esbozos, no están activas.

## 4. Configurar un cargador

Introduzca la URL del backend en la configuración del dispositivo (portal del fabricante o interfaz web del cargador):

**OCPP 1.6 / 2.0.1 (WebSocket):**
```
ws://<server-ip>:8080/ocpp/<ChargePointId>
```

`<ChargePointId>` es el identificador único del cargador (de libre elección, p. ej. `WB-Halle-01`). El subprotocolo (`ocpp1.6` / `ocpp2.0.1`) se negocia automáticamente.

**OCPP 1.5 (SOAP, solo dispositivos antiguos):**
```
http://<server-ip>:8080/ocpp15
```

En cuanto el cargador se conecta, aparece en **Cargadores** con estado *online*. Poco después de conectarse, el servidor configura automáticamente el cargador para las lecturas en vivo (solo OCPP 1.6).

## 5. Registrar tarjetas RFID

1. En la interfaz → **Tarjetas → Abrir ventana de aprendizaje** (activa durante 2 minutos).
2. Autentíquese en el cargador con la nueva tarjeta RFID.
3. La tarjeta aparece en la lista y puede asignarse a un empleado o marcarse como tarjeta de invitado (con fecha de caducidad).

## 6. Ejecución permanente como servicio de Windows (opcional)

`easy-occp.exe` es un programa de consola normal. Para arranque automático/servicio:

**Variante A: Programador de tareas**

- Programador de tareas → *Crear tarea* → Desencadenador *Al iniciar el sistema* → Acción *Iniciar un programa* → `C:\easy-occp\easy-occp.exe` → *Iniciar en:* `C:\easy-occp\`.
- Active *«Ejecutar tanto si el usuario ha iniciado sesión como si no»*.

**Variante B: NSSM (Non-Sucking Service Manager)**

```powershell
nssm install easy-occp "C:\easy-occp\easy-occp.exe"
nssm set easy-occp AppDirectory "C:\easy-occp"
nssm start easy-occp
```

## 7. Restablecer la contraseña de admin

Si se ha perdido la contraseña de admin:

```powershell
.\easy-occp.exe --reset-admin "nuevaContrasena123"
```

Restablece la contraseña de la cuenta `admin` y finaliza.

## 8. Copia de seguridad

Todo lo relevante está en **un único archivo**: `data\easy-occp.db`.

Para una copia de seguridad consistente, detenga brevemente el servicio y guarde el archivo (incluidos los posibles `-wal` / `-shm`) — listo.

## 9. Solución de problemas

| Problema | Causa / solución |
|---------|------------------|
| El navegador muestra «página no accesible» | Puerto 8080 ocupado o bloqueado por el firewall. Compruebe con `netstat -ano \| findstr :8080`. |
| El cargador no se conecta | Compruebe `public_base_url` / el firewall / la URL con el `ChargePointId`. Los logs de la consola muestran las conexiones WS entrantes. |
| Sin valores en vivo durante la carga | Deje que el cargador se vuelva a conectar (la configuración se aplica al conectar). Compruebe el log de la consola: `MeterValueSampleInterval` debería aparecer como «establecido». Algunos cargadores necesitan un reinicio, otros no admiten medición de potencia — en ese caso la potencia se deriva de las lecturas del contador. |
| «database is locked» | No inicie un segundo `easy-occp.exe` a la vez sobre la misma base de datos. |
| ¿Más logs? | Antes de iniciar: establezca `$env:RUST_LOG="debug"`. |

## 10. Desinstalación

Elimine el servicio/la tarea y borre la carpeta — no hay entradas en el registro ni dependencias externas.

---

Versión 0.3.1 · Licencia: MIT
