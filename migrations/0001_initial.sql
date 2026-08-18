-- Initial schema

CREATE TABLE users (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    username        TEXT    NOT NULL,
    display_name    TEXT    NOT NULL,
    email           TEXT,
    role            TEXT    NOT NULL CHECK(role IN ('admin','user')),
    auth_source     TEXT    NOT NULL CHECK(auth_source IN ('local','ldap','oidc')),
    password_hash   TEXT,
    external_id     TEXT,
    disabled        INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT    NOT NULL DEFAULT (datetime('now'))
);
CREATE UNIQUE INDEX idx_users_username ON users(username);
CREATE UNIQUE INDEX idx_users_external ON users(auth_source, external_id) WHERE external_id IS NOT NULL;

CREATE TABLE sessions (
    token       TEXT    PRIMARY KEY,
    user_id     INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
    expires_at  TEXT    NOT NULL
);
CREATE INDEX idx_sessions_user ON sessions(user_id);

CREATE TABLE wallboxes (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    charge_point_id  TEXT    NOT NULL UNIQUE,
    name             TEXT    NOT NULL,
    location         TEXT,
    vendor           TEXT,
    model            TEXT,
    firmware         TEXT,
    serial_number    TEXT,
    ocpp_version     TEXT,
    auth_basic_user  TEXT,
    auth_basic_pass  TEXT,
    last_heartbeat   TEXT,
    last_boot        TEXT,
    connector_count  INTEGER NOT NULL DEFAULT 1,
    created_at       TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE connectors (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    wallbox_id   INTEGER NOT NULL REFERENCES wallboxes(id) ON DELETE CASCADE,
    connector_id INTEGER NOT NULL,
    status       TEXT,
    error_code   TEXT,
    info         TEXT,
    updated_at   TEXT    NOT NULL DEFAULT (datetime('now')),
    UNIQUE(wallbox_id, connector_id)
);

CREATE TABLE chips (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    id_tag      TEXT    NOT NULL UNIQUE,
    label       TEXT,
    user_id     INTEGER REFERENCES users(id) ON DELETE SET NULL,
    kind        TEXT    NOT NULL CHECK(kind IN ('employee','guest')),
    enabled     INTEGER NOT NULL DEFAULT 1,
    expires_at  TEXT,
    created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_chips_user ON chips(user_id);

CREATE TABLE enrollment_sessions (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    started_by       INTEGER NOT NULL REFERENCES users(id),
    wallbox_id       INTEGER REFERENCES wallboxes(id) ON DELETE SET NULL,
    started_at       TEXT    NOT NULL DEFAULT (datetime('now')),
    expires_at       TEXT    NOT NULL,
    consumed         INTEGER NOT NULL DEFAULT 0,
    captured_id_tag  TEXT,
    captured_at      TEXT
);

CREATE TABLE transactions (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    ocpp_transaction_id  INTEGER,
    wallbox_id           INTEGER NOT NULL REFERENCES wallboxes(id),
    connector_id         INTEGER NOT NULL,
    id_tag               TEXT    NOT NULL,
    chip_id              INTEGER REFERENCES chips(id),
    user_id              INTEGER REFERENCES users(id),
    guest_label          TEXT,
    start_time           TEXT    NOT NULL,
    start_meter_wh       INTEGER NOT NULL,
    stop_time            TEXT,
    stop_meter_wh        INTEGER,
    stop_reason          TEXT,
    started_remote       INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_tx_wallbox ON transactions(wallbox_id);
CREATE INDEX idx_tx_user ON transactions(user_id);
CREATE INDEX idx_tx_start ON transactions(start_time);
CREATE UNIQUE INDEX idx_tx_ocpp ON transactions(wallbox_id, ocpp_transaction_id)
    WHERE ocpp_transaction_id IS NOT NULL;

CREATE TABLE meter_values (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    transaction_id INTEGER NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    timestamp      TEXT    NOT NULL,
    energy_wh      INTEGER,
    power_w        INTEGER,
    soc_percent    INTEGER
);
CREATE INDEX idx_mv_tx ON meter_values(transaction_id);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
