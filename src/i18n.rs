//! Mehrsprachigkeit der Web-UI (Deutsch, Englisch, Französisch, Spanisch).
//!
//! Sprachwahl: Cookie `lang` (über den Umschalter in der Topbar gesetzt),
//! sonst `Accept-Language` des Browsers, sonst Englisch.
//! Alle Texte liegen als statische Tabelle in `Lang::t` — ein unbekannter
//! Key wird unverändert zurückgegeben (fällt beim Entwickeln sofort auf).

use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{header, HeaderMap};

pub const LANG_COOKIE: &str = "lang";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    De,
    #[default]
    En,
    Fr,
    Es,
}

impl Lang {
    pub const ALL: [Lang; 4] = [Lang::De, Lang::En, Lang::Fr, Lang::Es];

    pub fn code(self) -> &'static str {
        match self {
            Lang::De => "de",
            Lang::En => "en",
            Lang::Fr => "fr",
            Lang::Es => "es",
        }
    }

    /// Eigenbezeichnung für den Sprach-Umschalter.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::De => "Deutsch",
            Lang::En => "English",
            Lang::Fr => "Français",
            Lang::Es => "Español",
        }
    }

    pub fn from_code(s: &str) -> Option<Lang> {
        match s.to_ascii_lowercase().as_str() {
            "de" => Some(Lang::De),
            "en" => Some(Lang::En),
            "fr" => Some(Lang::Fr),
            "es" => Some(Lang::Es),
            _ => None,
        }
    }

    /// Sprache aus den Request-Headern bestimmen: Cookie → Accept-Language → En.
    pub fn from_headers(headers: &HeaderMap) -> Lang {
        if let Some(cookie) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
            for part in cookie.split(';') {
                if let Some(v) = part.trim().strip_prefix("lang=") {
                    if let Some(l) = Lang::from_code(v.trim()) {
                        return l;
                    }
                }
            }
        }
        if let Some(al) = headers
            .get(header::ACCEPT_LANGUAGE)
            .and_then(|v| v.to_str().ok())
        {
            // Einträge kommen bereits grob nach Präferenz sortiert; erster Treffer gewinnt.
            for entry in al.split(',') {
                let tag = entry.split(';').next().unwrap_or("").trim();
                if let Some(l) = Lang::from_code(tag.get(..2).unwrap_or("")) {
                    return l;
                }
            }
        }
        Lang::En
    }

    /// Übersetzten Text für einen Key liefern.
    pub fn t(self, key: &'static str) -> &'static str {
        let l = self;
        // (de, en, fr, es)
        macro_rules! tr {
            ($de:expr, $en:expr, $fr:expr, $es:expr) => {
                match l {
                    Lang::De => $de,
                    Lang::En => $en,
                    Lang::Fr => $fr,
                    Lang::Es => $es,
                }
            };
        }
        match key {
            // ---- Allgemein ------------------------------------------------
            "common.save" => tr!("Speichern", "Save", "Enregistrer", "Guardar"),
            "common.delete" => tr!("Löschen", "Delete", "Supprimer", "Eliminar"),
            "common.create" => tr!("Anlegen", "Create", "Créer", "Crear"),
            "common.name" => tr!("Name", "Name", "Nom", "Nombre"),
            "common.email" => tr!("E-Mail", "Email", "E-mail", "Correo electrónico"),
            "common.yes" => tr!("ja", "yes", "oui", "sí"),
            "common.no" => tr!("nein", "no", "non", "no"),
            "common.active" => tr!("Aktiv", "Active", "Actif", "Activo"),
            "common.online" => tr!("online", "online", "en ligne", "en línea"),
            "common.offline" => tr!("offline", "offline", "hors ligne", "desconectado"),
            "common.running" => tr!("läuft", "running", "en cours", "en curso"),
            "common.energy" => tr!("Energie", "Energy", "Énergie", "Energía"),
            "common.energy_kwh" => tr!("Energie (kWh)", "Energy (kWh)", "Énergie (kWh)", "Energía (kWh)"),
            "common.employee" => tr!("Mitarbeiter", "Employee", "Employé", "Empleado"),
            "common.employees" => tr!("Mitarbeiter", "Employees", "Employés", "Empleados"),
            "common.guest" => tr!("Gast", "Guest", "Invité", "Invitado"),
            "common.guests" => tr!("Gäste", "Guests", "Invités", "Invitados"),
            "common.start" => tr!("Start", "Start", "Début", "Inicio"),
            "common.end" => tr!("Ende", "End", "Fin", "Fin"),
            "common.status" => tr!("Status", "Status", "Statut", "Estado"),
            "common.location" => tr!("Standort", "Location", "Emplacement", "Ubicación"),
            "common.category" => tr!("Kategorie", "Category", "Catégorie", "Categoría"),
            "common.label" => tr!("Label", "Label", "Libellé", "Etiqueta"),
            "common.expiry" => tr!("Ablauf", "Expiry", "Expiration", "Vencimiento"),
            "common.password" => tr!("Passwort", "Password", "Mot de passe", "Contraseña"),
            "common.username" => tr!("Benutzername", "Username", "Nom d'utilisateur", "Nombre de usuario"),
            "common.role" => tr!("Rolle", "Role", "Rôle", "Rol"),
            "common.back" => tr!("zurück", "back", "retour", "volver"),
            "common.sessions" => tr!("Ladungen", "Sessions", "Recharges", "Cargas"),
            "common.no_data" => tr!("Keine Daten.", "No data.", "Aucune donnée.", "Sin datos."),
            "common.charged" => tr!("Geladen", "Charged", "Chargé", "Cargado"),
            "common.power" => tr!("Leistung", "Power", "Puissance", "Potencia"),
            "common.stop" => tr!("Stopp", "Stop", "Arrêter", "Detener"),
            "common.total" => tr!("Gesamt", "Total", "Total", "Total"),
            "common.wallbox" => tr!("Wallbox", "Wallbox", "Borne", "Cargador"),
            "common.wallboxes" => tr!("Wallboxen", "Wallboxes", "Bornes", "Cargadores"),

            // ---- Navigation / Layout -------------------------------------
            "nav.cockpit" => tr!("Cockpit", "Dashboard", "Tableau de bord", "Panel"),
            "nav.chips" => tr!("Chips", "Chips", "Badges", "Chips"),
            "nav.transactions" => tr!("Ladungen", "Sessions", "Recharges", "Cargas"),
            "nav.stats" => tr!("Auswertung", "Reports", "Statistiques", "Estadísticas"),
            "nav.users" => tr!("Benutzer", "Users", "Utilisateurs", "Usuarios"),
            "nav.logout" => tr!("Logout", "Log out", "Déconnexion", "Cerrar sesión"),
            "brand.tagline" => tr!("Wallbox-Management", "Wallbox management", "Gestion de bornes", "Gestión de cargadores"),

            // ---- Login ----------------------------------------------------
            "login.title" => tr!("Anmelden", "Sign in", "Connexion", "Iniciar sesión"),
            "login.hint" => tr!(
                "Standard-Zugang beim ersten Start:",
                "Default login on first start:",
                "Identifiants par défaut au premier démarrage :",
                "Acceso predeterminado en el primer inicio:"
            ),
            "login.failed" => tr!(
                "Benutzername oder Passwort falsch.",
                "Wrong username or password.",
                "Nom d'utilisateur ou mot de passe incorrect.",
                "Usuario o contraseña incorrectos."
            ),

            // ---- Cockpit --------------------------------------------------
            "dash.sub" => tr!(
                "Live-Überblick über alle Wallboxen.",
                "Live overview of all wallboxes.",
                "Vue d'ensemble en direct de toutes les bornes.",
                "Vista en vivo de todos los cargadores."
            ),
            "dash.add_wallbox" => tr!("Wallbox hinzufügen", "Add wallbox", "Ajouter une borne", "Añadir cargador"),
            "dash.monthly_report" => tr!("Monatsbericht", "Monthly report", "Rapport mensuel", "Informe mensual"),
            "dash.online_now" => tr!("Jetzt online", "Online now", "En ligne", "En línea"),
            "dash.online_foot" => tr!("Wallboxen verbunden", "wallboxes connected", "bornes connectées", "cargadores conectados"),
            "dash.charging_now" => tr!("Läuft gerade", "Charging now", "En charge", "Cargando ahora"),
            "dash.active_foot" => tr!("aktive Ladungen", "active sessions", "recharges actives", "cargas activas"),
            "dash.month_sessions" => tr!("Ladungen diesen Monat", "Sessions this month", "Recharges ce mois-ci", "Cargas este mes"),
            "dash.completed" => tr!("abgeschlossen", "completed", "terminées", "completadas"),
            "dash.month_energy" => tr!("Energie diesen Monat", "Energy this month", "Énergie ce mois-ci", "Energía este mes"),
            "dash.energy_foot" => tr!(
                "summiert über alle Wallboxen",
                "total across all wallboxes",
                "cumulée sur toutes les bornes",
                "total de todos los cargadores"
            ),
            "dash.no_wallbox_title" => tr!(
                "Noch keine Wallbox registriert",
                "No wallbox registered yet",
                "Aucune borne enregistrée",
                "Aún no hay cargadores registrados"
            ),
            "dash.no_wallbox_pre" => tr!(
                "Lege unter",
                "Create your first wallbox under",
                "Créez votre première borne sous",
                "Crea tu primer cargador en"
            ),
            "dash.no_wallbox_post" => tr!(
                "die erste Ladestation an.",
                ".",
                ".",
                "."
            ),
            "state.charging" => tr!("Lädt", "Charging", "En charge", "Cargando"),
            "state.ready" => tr!("Bereit", "Ready", "Prête", "Listo"),
            "state.offline" => tr!("Offline", "Offline", "Hors ligne", "Desconectado"),
            "state.error" => tr!("Fehler", "Fault", "Défaut", "Fallo"),
            "state.warn" => tr!("Nicht verfügbar", "Unavailable", "Indisponible", "No disponible"),
            "dash.secured" => tr!("gesichert", "secured", "sécurisée", "protegido"),
            "dash.secured_tip" => tr!("Basic-Auth aktiv", "Basic auth enabled", "Authentification Basic active", "Autenticación Basic activa"),
            "dash.active_one" => tr!("aktive Ladung", "active session", "recharge active", "carga activa"),
            "dash.details" => tr!("Details", "Details", "Détails", "Detalles"),
            "dash.running_sessions" => tr!("Laufende Ladungen", "Active sessions", "Recharges en cours", "Cargas en curso"),
            "dash.all" => tr!("Alle", "All", "Toutes", "Todas"),
            "dash.nobody" => tr!(
                "Aktuell lädt niemand.",
                "Nothing is charging right now.",
                "Aucune recharge en cours.",
                "No hay cargas en curso."
            ),
            "dash.since" => tr!("Seit", "Since", "Depuis", "Desde"),
            "dash.top_employees" => tr!(
                "Top-Mitarbeiter diesen Monat",
                "Top employees this month",
                "Meilleurs employés ce mois-ci",
                "Principales empleados este mes"
            ),
            "dash.no_completed" => tr!(
                "Noch keine abgeschlossenen Ladungen in diesem Monat.",
                "No completed sessions this month yet.",
                "Aucune recharge terminée ce mois-ci.",
                "Aún no hay cargas completadas este mes."
            ),

            // ---- Wallboxen (Liste) ---------------------------------------
            "wb.new" => tr!("Neue Wallbox anlegen", "Add new wallbox", "Créer une nouvelle borne", "Crear nuevo cargador"),
            "wb.hint_connect" => tr!(
                "Die Wallbox verbindet sich anschließend nach",
                "The wallbox then connects to",
                "La borne se connecte ensuite à",
                "El cargador se conecta después a"
            ),
            "wb.last_heartbeat" => tr!("Letzter Heartbeat", "Last heartbeat", "Dernier heartbeat", "Último heartbeat"),
            "wb.none" => tr!(
                "Noch keine Wallboxen angelegt.",
                "No wallboxes yet.",
                "Aucune borne créée.",
                "Aún no hay cargadores."
            ),
            "wb.delete_confirm" => tr!("Wallbox löschen?", "Delete wallbox?", "Supprimer la borne ?", "¿Eliminar cargador?"),

            // ---- Wallbox-Detail ------------------------------------------
            "wbd.manufacturer" => tr!("Hersteller", "Manufacturer", "Fabricant", "Fabricante"),
            "wbd.connectors" => tr!("Connectors", "Connectors", "Connecteurs", "Conectores"),
            "wbd.updated" => tr!("Stand", "Updated", "Mis à jour", "Actualizado"),
            "wbd.no_connectors" => tr!(
                "Keine Connector-Meldungen bisher.",
                "No connector reports yet.",
                "Aucune notification de connecteur pour l'instant.",
                "Sin informes de conectores todavía."
            ),
            "wbd.auth_title" => tr!(
                "OCPP-Zugangsschutz (Profile 1)",
                "OCPP access protection (profile 1)",
                "Protection d'accès OCPP (profil 1)",
                "Protección de acceso OCPP (perfil 1)"
            ),
            "wbd.pw_set" => tr!("Neues Passwort gesetzt.", "New password set.", "Nouveau mot de passe défini.", "Nueva contraseña establecida."),
            "wbd.pw_show_once" => tr!(
                "Trage in der Wallbox folgenden Zugang ein — wird nur genau jetzt angezeigt:",
                "Enter these credentials in the wallbox — shown only this once:",
                "Saisissez ces identifiants dans la borne — affichés une seule fois :",
                "Introduce estas credenciales en el cargador — se muestran solo esta vez:"
            ),
            "wbd.user" => tr!("Benutzer", "User", "Utilisateur", "Usuario"),
            "wbd.basic_auth_is" => tr!("Basic Auth ist", "Basic auth is", "L'authentification Basic est", "La autenticación Basic está"),
            "wbd.active_word" => tr!("aktiv", "enabled", "active", "activada"),
            "wbd.disabled_word" => tr!("deaktiviert", "disabled", "désactivée", "desactivada"),
            "wbd.auth_reject" => tr!(
                "Verbindungen ohne gültigen Authorization-Header werden mit 401 abgelehnt.",
                "Connections without a valid Authorization header are rejected with 401.",
                "Les connexions sans en-tête Authorization valide sont rejetées avec un 401.",
                "Las conexiones sin una cabecera Authorization válida se rechazan con 401."
            ),
            "wbd.auth_open" => tr!(
                "jede Verbindung mit dieser ChargePointId wird akzeptiert.",
                "any connection with this ChargePointId is accepted.",
                "toute connexion avec ce ChargePointId est acceptée.",
                "se acepta cualquier conexión con este ChargePointId."
            ),
            "wbd.user_empty_hint" => tr!("(leer = ChargePointId)", "(empty = ChargePointId)", "(vide = ChargePointId)", "(vacío = ChargePointId)"),
            "wbd.pw_min" => tr!("min. 8 Zeichen", "min. 8 characters", "min. 8 caractères", "mín. 8 caracteres"),
            "wbd.set_password" => tr!("Passwort setzen", "Set password", "Définir le mot de passe", "Establecer contraseña"),
            "wbd.generate" => tr!("Generieren", "Generate", "Générer", "Generar"),
            // Wird in confirm('…') verwendet — Übersetzungen ohne Apostroph halten!
            "wbd.disable_auth_confirm" => tr!(
                "Basic-Auth deaktivieren?",
                "Disable basic auth?",
                "Désactiver la protection Basic ?",
                "¿Desactivar la autenticación Basic?"
            ),
            "wbd.disable_auth" => tr!("Auth deaktivieren", "Disable auth", "Désactiver l'auth", "Desactivar autenticación"),
            "wbd.guest_title" => tr!(
                "Gast remote freischalten",
                "Remotely authorize a guest",
                "Autoriser un invité à distance",
                "Autorizar invitado remotamente"
            ),
            "wbd.guest_idtag" => tr!("idTag (Gast)", "idTag (guest)", "idTag (invité)", "idTag (invitado)"),
            "wbd.guest_label" => tr!("Gast-Label", "Guest label", "Libellé invité", "Etiqueta de invitado"),
            "wbd.guest_label_ph" => tr!("z. B. Firma Mustermann", "e.g. Acme Corp", "p. ex. Société Dupont", "p. ej. Empresa Ejemplo"),
            "wbd.unlock_now" => tr!("Jetzt freischalten", "Authorize now", "Autoriser maintenant", "Autorizar ahora"),
            "wbd.offline_hint" => tr!(
                "Wallbox ist offline — Remote-Start nicht möglich.",
                "Wallbox is offline — remote start not possible.",
                "La borne est hors ligne — démarrage à distance impossible.",
                "El cargador está desconectado — no es posible el inicio remoto."
            ),
            "wbd.no_active" => tr!("Keine aktive Ladung.", "No active session.", "Aucune recharge active.", "Ninguna carga activa."),

            // ---- Transaktionen -------------------------------------------
            "tx.title" => tr!("Transaktionen", "Transactions", "Transactions", "Transacciones"),
            "tx.csv" => tr!("CSV-Export", "CSV export", "Export CSV", "Exportar CSV"),
            "tx.filter_employee" => tr!("Filter Mitarbeiter", "Filter by employee", "Filtrer par employé", "Filtrar por empleado"),
            "tx.search" => tr!("Suchen", "Search", "Rechercher", "Buscar"),
            "tx.none" => tr!(
                "Keine Transaktionen gefunden.",
                "No transactions found.",
                "Aucune transaction trouvée.",
                "No se encontraron transacciones."
            ),
            "csv.header" => tr!(
                "id;wallbox;connector;id_tag;mitarbeiter;start;ende;energie_wh;grund",
                "id;wallbox;connector;id_tag;employee;start;end;energy_wh;reason",
                "id;borne;connecteur;id_tag;employe;debut;fin;energie_wh;raison",
                "id;cargador;conector;id_tag;empleado;inicio;fin;energia_wh;motivo"
            ),
            "csv.filename" => tr!("transaktionen", "transactions", "transactions", "transacciones"),

            // ---- Chips ----------------------------------------------------
            "chips.title" => tr!("Chips & RFID-Karten", "Chips & RFID cards", "Badges & cartes RFID", "Chips y tarjetas RFID"),
            "chips.sub" => tr!(
                "Ladechips werden über das Lernfenster einer Wallbox erfasst und hier Mitarbeitern zugeordnet.",
                "Charging chips are captured via a wallbox learning window and assigned to employees here.",
                "Les badges sont enregistrés via la fenêtre d'apprentissage d'une borne et affectés ici aux employés.",
                "Los chips se registran mediante la ventana de aprendizaje de un cargador y aquí se asignan a los empleados."
            ),
            "chips.enroll_new" => tr!("Neuen Chip anlernen", "Enroll new chip", "Enregistrer un nouveau badge", "Registrar nuevo chip"),
            "chips.window_until" => tr!(
                "Lernfenster läuft bis",
                "Learning window open until",
                "Fenêtre d'apprentissage ouverte jusqu'à",
                "Ventana de aprendizaje abierta hasta"
            ),
            "chips.hold_now" => tr!(
                "Chip jetzt an eine Wallbox halten.",
                "Hold the chip to a wallbox now.",
                "Présentez le badge à une borne maintenant.",
                "Acerca el chip a un cargador ahora."
            ),
            "chips.to_window" => tr!("Zum Lernfenster", "To learning window", "Vers la fenêtre d'apprentissage", "A la ventana de aprendizaje"),
            "chips.restrict" => tr!(
                "Auf Wallbox beschränken (optional)",
                "Restrict to wallbox (optional)",
                "Limiter à une borne (optionnel)",
                "Limitar a un cargador (opcional)"
            ),
            "chips.any" => tr!("— egal —", "— any —", "— indifférent —", "— cualquiera —"),
            "chips.start_window" => tr!(
                "Lernfenster starten (2 min)",
                "Start learning window (2 min)",
                "Démarrer la fenêtre (2 min)",
                "Iniciar ventana (2 min)"
            ),
            "chips.all" => tr!("Alle Chips", "All chips", "Tous les badges", "Todos los chips"),
            "chips.none_title" => tr!(
                "Noch keine Chips registriert",
                "No chips registered yet",
                "Aucun badge enregistré",
                "Aún no hay chips registrados"
            ),
            "chips.none_body" => tr!(
                "Starte das Lernfenster oben und halte den Chip an eine Wallbox.",
                "Start the learning window above and hold the chip to a wallbox.",
                "Démarrez la fenêtre ci-dessus et présentez le badge à une borne.",
                "Inicia la ventana de arriba y acerca el chip a un cargador."
            ),
            "chips.unassigned" => tr!("— unzugeordnet —", "— unassigned —", "— non affecté —", "— sin asignar —"),
            "chips.delete_confirm" => tr!("Chip löschen?", "Delete chip?", "Supprimer le badge ?", "¿Eliminar chip?"),
            "chips.row_hint" => tr!(
                "Änderungen pro Zeile mit „Speichern“ übernehmen.",
                "Apply changes per row with “Save”.",
                "Validez les modifications de chaque ligne avec « Enregistrer ».",
                "Aplica los cambios de cada fila con «Guardar»."
            ),

            // ---- Chip anlernen -------------------------------------------
            "enroll.title" => tr!("Chip anlernen", "Enroll chip", "Enregistrer un badge", "Registrar chip"),
            "enroll.detected" => tr!("Chip erkannt:", "Chip detected:", "Badge détecté :", "Chip detectado:"),
            "enroll.label_ph" => tr!("z. B. Karte Büro-01", "e.g. card office-01", "p. ex. carte bureau-01", "p. ej. tarjeta oficina-01"),
            "enroll.employee_optional" => tr!(
                "Mitarbeiter (optional — bei Gäste-Chip leer lassen)",
                "Employee (optional — leave empty for guest chips)",
                "Employé (optionnel — laisser vide pour un badge invité)",
                "Empleado (opcional — dejar vacío para chips de invitado)"
            ),
            "enroll.none_opt" => tr!("— kein —", "— none —", "— aucun —", "— ninguno —"),
            "enroll.valid_until" => tr!(
                "Gültig bis (optional, RFC3339)",
                "Valid until (optional, RFC3339)",
                "Valable jusqu'au (optionnel, RFC3339)",
                "Válido hasta (opcional, RFC3339)"
            ),
            "enroll.hold_hint" => tr!(
                "Bitte Chip an eine der Wallboxen halten. Das Fenster läuft bis",
                "Hold the chip to one of the wallboxes. The window is open until",
                "Présentez le badge à l'une des bornes. La fenêtre est ouverte jusqu'à",
                "Acerca el chip a uno de los cargadores. La ventana está abierta hasta"
            ),
            "enroll.autorefresh" => tr!(
                "Diese Seite aktualisiert sich alle 2 Sekunden automatisch.",
                "This page refreshes automatically every 2 seconds.",
                "Cette page s'actualise automatiquement toutes les 2 secondes.",
                "Esta página se actualiza automáticamente cada 2 segundos."
            ),

            // ---- Mitarbeiter ---------------------------------------------
            "emp.new" => tr!("Neuen Mitarbeiter anlegen", "Add new employee", "Créer un nouvel employé", "Crear nuevo empleado"),
            "emp.department" => tr!("Abteilung", "Department", "Service", "Departamento"),
            "emp.none" => tr!("Noch keine Mitarbeiter angelegt.", "No employees yet.", "Aucun employé créé.", "Aún no hay empleados."),
            // Wird in confirm('…') verwendet — Übersetzungen ohne Apostroph halten!
            "emp.delete_confirm" => tr!(
                "Mitarbeiter löschen? Chips und Transaktionen bleiben erhalten, verlieren aber die Verknüpfung.",
                "Delete employee? Chips and transactions are kept but lose their assignment.",
                "Supprimer cet employé ? Les badges et transactions sont conservés mais perdent leur affectation.",
                "¿Eliminar empleado? Los chips y transacciones se conservan pero pierden su asignación."
            ),
            "emp.recent" => tr!("Letzte Ladungen", "Recent sessions", "Dernières recharges", "Últimas cargas"),
            "emp.no_chips" => tr!("Noch keine Chips zugeordnet.", "No chips assigned yet.", "Aucun badge affecté.", "Aún no hay chips asignados."),
            "emp.chips_hint_pre" => tr!(
                "Neue Chips werden unter",
                "New chips are enrolled under",
                "Les nouveaux badges s'enregistrent sous",
                "Los nuevos chips se registran en"
            ),
            "emp.chips_hint_post" => tr!(
                "über das Lernfenster einer Wallbox eingelesen und hier zugeordnet.",
                "via a wallbox learning window and assigned here.",
                "via la fenêtre d'apprentissage d'une borne, puis sont affectés ici.",
                "mediante la ventana de aprendizaje de un cargador y se asignan aquí."
            ),
            "emp.no_sessions" => tr!("Keine Ladungen.", "No sessions.", "Aucune recharge.", "Sin cargas."),

            // ---- Benutzer -------------------------------------------------
            "users.new" => tr!("Neuen Benutzer anlegen", "Add new user", "Créer un nouvel utilisateur", "Crear nuevo usuario"),
            "users.user_role" => tr!("Benutzer", "User", "Utilisateur", "Usuario"),
            "users.admin_role" => tr!("Administrator", "Administrator", "Administrateur", "Administrador"),
            "users.source" => tr!("Quelle", "Source", "Source", "Origen"),
            "users.new_pw_ph" => tr!("neues Passwort", "new password", "nouveau mot de passe", "nueva contraseña"),
            "users.set" => tr!("Setzen", "Set", "Définir", "Establecer"),
            // Wird in confirm('…') verwendet — Übersetzungen ohne Apostroph halten!
            "users.delete_confirm" => tr!("Benutzer löschen?", "Delete user?", "Supprimer cet utilisateur ?", "¿Eliminar usuario?"),

            // ---- Statistik ------------------------------------------------
            "stats.title" => tr!("Statistik", "Statistics", "Statistiques", "Estadísticas"),
            "stats.monthly_pdf" => tr!("Monatsbericht (PDF)", "Monthly report (PDF)", "Rapport mensuel (PDF)", "Informe mensual (PDF)"),
            "stats.year" => tr!("Jahr", "Year", "Année", "Año"),
            "stats.month" => tr!("Monat", "Month", "Mois", "Mes"),
            "stats.create_report" => tr!("Bericht erstellen", "Generate report", "Générer le rapport", "Generar informe"),
            "stats.report_hint" => tr!(
                "Eine Seite pro Mitarbeiter mit ≥1 abgeschlossener Ladung im gewählten Monat.",
                "One page per employee with ≥1 completed session in the selected month.",
                "Une page par employé ayant ≥ 1 recharge terminée dans le mois choisi.",
                "Una página por empleado con ≥1 carga completada en el mes elegido."
            ),
            "stats.history" => tr!("Verlauf", "History", "Historique", "Historial"),
            "stats.quarter" => tr!("Quartal", "Quarter", "Trimestre", "Trimestre"),
            "stats.period" => tr!("Periode", "Period", "Période", "Período"),
            "stats.none_range" => tr!(
                "Keine abgeschlossenen Ladungen im Zeitraum.",
                "No completed sessions in this period.",
                "Aucune recharge terminée sur la période.",
                "Sin cargas completadas en el período."
            ),
            "stats.rollups" => tr!("Rollups", "Rollups", "Cumuls", "Acumulados"),
            "stats.days" => tr!("Tage", "days", "jours", "días"),
            "stats.emp_vs_guests" => tr!("Mitarbeiter vs. Gäste", "Employees vs. guests", "Employés vs invités", "Empleados vs. invitados"),
            "stats.group" => tr!("Gruppe", "Group", "Groupe", "Grupo"),
            "stats.by_employee" => tr!(
                "Nach Mitarbeiter / Gast-Label",
                "By employee / guest label",
                "Par employé / libellé invité",
                "Por empleado / etiqueta de invitado"
            ),
            "stats.by_wallbox" => tr!("Nach Wallbox", "By wallbox", "Par borne", "Por cargador"),

            // ---- Monatsnamen ---------------------------------------------
            "month.1" => tr!("Januar", "January", "janvier", "enero"),
            "month.2" => tr!("Februar", "February", "février", "febrero"),
            "month.3" => tr!("März", "March", "mars", "marzo"),
            "month.4" => tr!("April", "April", "avril", "abril"),
            "month.5" => tr!("Mai", "May", "mai", "mayo"),
            "month.6" => tr!("Juni", "June", "juin", "junio"),
            "month.7" => tr!("Juli", "July", "juillet", "julio"),
            "month.8" => tr!("August", "August", "août", "agosto"),
            "month.9" => tr!("September", "September", "septembre", "septiembre"),
            "month.10" => tr!("Oktober", "October", "octobre", "octubre"),
            "month.11" => tr!("November", "November", "novembre", "noviembre"),
            "month.12" => tr!("Dezember", "December", "décembre", "diciembre"),

            // ---- PDF-Bericht ---------------------------------------------
            "pdf.title" => tr!("Monatsbericht", "Monthly report", "Rapport mensuel", "Informe mensual"),
            "pdf.continued" => tr!("Fortsetzung:", "continued:", "suite :", "continuación:"),
            "pdf.duration" => tr!("Dauer", "Duration", "Durée", "Duración"),
            "pdf.chip" => tr!("Chip", "Chip", "Badge", "Chip"),
            "pdf.filename" => tr!("monatsbericht", "monthly-report", "rapport-mensuel", "informe-mensual"),

            // ---- Fehlermeldungen -----------------------------------------
            "err.id_name_required" => tr!(
                "ID und Name sind Pflicht.",
                "ID and name are required.",
                "L'ID et le nom sont obligatoires.",
                "El ID y el nombre son obligatorios."
            ),
            "err.cp_exists" => tr!(
                "ChargePointId existiert bereits:",
                "ChargePointId already exists:",
                "Ce ChargePointId existe déjà :",
                "El ChargePointId ya existe:"
            ),
            "err.idtag_missing" => tr!("idTag fehlt.", "idTag is missing.", "idTag manquant.", "Falta el idTag."),
            "err.remote_start_rejected" => tr!(
                "Wallbox hat RemoteStart abgelehnt:",
                "Wallbox rejected RemoteStart:",
                "La borne a refusé le RemoteStart :",
                "El cargador rechazó RemoteStart:"
            ),
            "err.remote_stop_rejected" => tr!(
                "Wallbox hat RemoteStop abgelehnt:",
                "Wallbox rejected RemoteStop:",
                "La borne a refusé le RemoteStop :",
                "El cargador rechazó RemoteStop:"
            ),
            "err.pw_min8" => tr!(
                "Passwort muss mindestens 8 Zeichen haben (oder 'Generieren' nutzen).",
                "Password must be at least 8 characters (or use 'Generate').",
                "Le mot de passe doit contenir au moins 8 caractères (ou utilisez « Générer »).",
                "La contraseña debe tener al menos 8 caracteres (o usa «Generar»)."
            ),
            "err.user_fields_required" => tr!(
                "Benutzername, Name und Passwort (≥6 Zeichen) sind Pflicht.",
                "Username, name, and password (≥6 characters) are required.",
                "Nom d'utilisateur, nom et mot de passe (≥ 6 caractères) sont obligatoires.",
                "Nombre de usuario, nombre y contraseña (≥6 caracteres) son obligatorios."
            ),
            "err.invalid_role" => tr!("Ungültige Rolle.", "Invalid role.", "Rôle non valide.", "Rol no válido."),
            "err.user_exists" => tr!(
                "Benutzername existiert bereits:",
                "Username already exists:",
                "Ce nom d'utilisateur existe déjà :",
                "El nombre de usuario ya existe:"
            ),
            "err.self_delete" => tr!(
                "Sie können sich nicht selbst löschen.",
                "You cannot delete yourself.",
                "Vous ne pouvez pas vous supprimer vous-même.",
                "No puedes eliminarte a ti mismo."
            ),
            "err.pw_min6" => tr!(
                "Passwort muss mindestens 6 Zeichen haben.",
                "Password must be at least 6 characters.",
                "Le mot de passe doit contenir au moins 6 caractères.",
                "La contraseña debe tener al menos 6 caracteres."
            ),
            "err.name_required" => tr!("Name ist Pflicht.", "Name is required.", "Le nom est obligatoire.", "El nombre es obligatorio."),
            "err.invalid_category" => tr!("Ungültige Kategorie.", "Invalid category.", "Catégorie non valide.", "Categoría no válida."),
            "err.no_chip_captured" => tr!(
                "Bisher wurde kein Chip erkannt – bitte an die Wallbox halten.",
                "No chip detected yet — please hold it to the wallbox.",
                "Aucun badge détecté pour l'instant — présentez-le à la borne.",
                "Aún no se ha detectado ningún chip — acércalo al cargador."
            ),
            "err.enroll_done" => tr!(
                "Enrollment bereits abgeschlossen.",
                "Enrollment already completed.",
                "Enregistrement déjà terminé.",
                "Registro ya completado."
            ),
            "err.chip_exists" => tr!(
                "Chip-Tag ist bereits registriert:",
                "Chip tag is already registered:",
                "Ce badge est déjà enregistré :",
                "El chip ya está registrado:"
            ),
            "err.month_range" => tr!(
                "Monat muss 1..12 sein.",
                "Month must be 1..12.",
                "Le mois doit être compris entre 1 et 12.",
                "El mes debe estar entre 1 y 12."
            ),
            "err.invalid_date" => tr!("ungültiges Datum", "invalid date", "date non valide", "fecha no válida"),

            // Unbekannter Key: unverändert zurückgeben, fällt sofort auf.
            other => {
                tracing::warn!("i18n: unbekannter Key '{other}'");
                other
            }
        }
    }
}

/// Axum-Extractor: bestimmt die Sprache aus Cookie bzw. Accept-Language.
#[async_trait]
impl<S: Send + Sync> FromRequestParts<S> for Lang {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Lang::from_headers(&parts.headers))
    }
}
