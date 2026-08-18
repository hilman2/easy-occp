-- Mitarbeiter sind echte Personen mit 1..n Chips — unabhängig vom Login-Konto.
-- Ein users-Eintrag ist nur ein Login; er kann optional einem Mitarbeiter zugeordnet sein.

CREATE TABLE employees (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    display_name  TEXT    NOT NULL,
    email         TEXT,
    department    TEXT,
    active        INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT    NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_employees_name ON employees(display_name);

-- Nicht-Admin-Logins automatisch als Mitarbeiter übernehmen, damit der
-- bestehende Datenbestand nicht verwaist.
INSERT INTO employees (display_name, email)
    SELECT display_name, email FROM users WHERE role = 'user';

ALTER TABLE users ADD COLUMN employee_id INTEGER REFERENCES employees(id) ON DELETE SET NULL;
UPDATE users SET employee_id = (
    SELECT e.id FROM employees e WHERE e.display_name = users.display_name LIMIT 1
) WHERE role = 'user';

ALTER TABLE chips ADD COLUMN employee_id INTEGER REFERENCES employees(id) ON DELETE SET NULL;
UPDATE chips SET employee_id = (
    SELECT u.employee_id FROM users u WHERE u.id = chips.user_id
) WHERE chips.user_id IS NOT NULL;
CREATE INDEX idx_chips_employee ON chips(employee_id);

ALTER TABLE transactions ADD COLUMN employee_id INTEGER REFERENCES employees(id) ON DELETE SET NULL;
UPDATE transactions SET employee_id = (
    SELECT c.employee_id FROM chips c WHERE c.id_tag = transactions.id_tag
);
UPDATE transactions SET employee_id = COALESCE(
    employee_id,
    (SELECT u.employee_id FROM users u WHERE u.id = transactions.user_id)
);
CREATE INDEX idx_tx_employee ON transactions(employee_id);

-- chips.user_id und transactions.user_id bleiben aus SQLite-ALTER-Gründen
-- weiterhin in der Tabelle, werden aber vom Anwendungscode nicht mehr geschrieben
-- und nicht mehr gelesen. Die Semantik „gehört zu Person" läuft ab jetzt
-- ausschließlich über employee_id.
