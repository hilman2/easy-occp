use anyhow::{Context, Result};
use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHasher};
use sqlx::SqlitePool;

/// Legt beim ersten Start einen Admin-Benutzer `admin/admin` an.
/// Wenn `reset_password` gesetzt ist, wird das Passwort des Admins überschrieben.
pub async fn bootstrap_admin(db: &SqlitePool, reset_password: Option<&str>) -> Result<()> {
    let existing: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM users WHERE username = 'admin' AND auth_source = 'local'")
            .fetch_optional(db)
            .await?;

    match (existing, reset_password) {
        (None, reset) => {
            let pw = reset.unwrap_or("admin");
            let hash = hash_password(pw)?;
            sqlx::query(
                "INSERT INTO users (username, display_name, role, auth_source, password_hash)
                 VALUES ('admin', 'Administrator', 'admin', 'local', ?1)",
            )
            .bind(&hash)
            .execute(db)
            .await?;
            if reset.is_none() {
                tracing::warn!(
                    "Admin-Benutzer angelegt (admin/admin) – Passwort bitte sofort ändern."
                );
            } else {
                tracing::info!("Admin-Benutzer angelegt mit gesetztem Passwort.");
            }
        }
        (Some((id,)), Some(pw)) => {
            let hash = hash_password(pw)?;
            sqlx::query("UPDATE users SET password_hash = ?1, disabled = 0 WHERE id = ?2")
                .bind(&hash)
                .bind(id)
                .execute(db)
                .await?;
            tracing::info!("Admin-Passwort zurückgesetzt.");
        }
        _ => {}
    }
    Ok(())
}

pub fn hash_password(plain: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    let hash = argon
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("argon2-Hash: {e}"))?
        .to_string();
    Ok(hash)
}

pub fn verify_password(plain: &str, hash: &str) -> Result<bool> {
    use argon2::PasswordVerifier;
    let parsed = argon2::PasswordHash::new(hash)
        .map_err(|e| anyhow::anyhow!("argon2-Parse: {e}"))?;
    Ok(Argon2::default()
        .verify_password(plain.as_bytes(), &parsed)
        .is_ok())
}

#[allow(dead_code)]
pub async fn setting_get(db: &SqlitePool, key: &str) -> Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?1")
        .bind(key)
        .fetch_optional(db)
        .await
        .with_context(|| format!("setting_get {key}"))?;
    Ok(row.map(|r| r.0))
}

#[allow(dead_code)]
pub async fn setting_set(db: &SqlitePool, key: &str, value: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO settings(key,value) VALUES(?1,?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(db)
    .await?;
    Ok(())
}

/// Repariert Prüfsummen in `_sqlx_migrations`, die sich nur durch die
/// Zeilenenden der Migrationsdatei unterscheiden.
///
/// Hintergrund: `sqlx::migrate!` bettet den Dateiinhalt zur Compile-Zeit ein und
/// prüft beim Start eine SHA-384-Summe darüber. Wird dieselbe Migration einmal
/// aus einem LF- und einmal aus einem CRLF-Checkout gebaut (Windows-Buildrunner
/// ohne `eol=lf`), unterscheiden sich die Summen und sqlx bricht mit
/// "migration N was previously applied but has been modified" ab, obwohl das
/// SQL identisch ist. In diesem – und nur in diesem – Fall wird die gespeicherte
/// Prüfsumme auf den aktuellen Wert gehoben.
pub async fn repair_line_ending_checksums(
    db: &SqlitePool,
    migrator: &sqlx::migrate::Migrator,
) -> Result<()> {
    let table: Option<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_optional(db)
    .await
    .context("Migrationstabelle konnte nicht gelesen werden")?;
    if table.is_none() {
        return Ok(());
    }

    let applied: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT version, checksum FROM _sqlx_migrations")
            .fetch_all(db)
            .await
            .context("Angewandte Migrationen konnten nicht gelesen werden")?;

    for migration in migrator.iter() {
        let Some((_, stored)) = applied.iter().find(|(v, _)| *v == migration.version) else {
            continue;
        };
        if stored.as_slice() == migration.checksum.as_ref() {
            continue;
        }

        // Gegenstück mit umgedrehten Zeilenenden bilden und prüfen.
        let normalized = migration.sql.replace("\r\n", "\n");
        let variant = if migration.sql.contains("\r\n") {
            normalized
        } else {
            normalized.replace('\n', "\r\n")
        };
        if stored.as_slice() != sha384(variant.as_bytes()).as_slice() {
            continue; // Echte inhaltliche Abweichung – sqlx soll abbrechen.
        }

        sqlx::query("UPDATE _sqlx_migrations SET checksum = ?1 WHERE version = ?2")
            .bind(migration.checksum.as_ref())
            .bind(migration.version)
            .execute(db)
            .await
            .with_context(|| {
                format!(
                    "Prüfsumme der Migration {} konnte nicht aktualisiert werden",
                    migration.version
                )
            })?;
        tracing::warn!(
            "Migration {} ({}): Prüfsumme wich nur in den Zeilenenden ab und wurde korrigiert.",
            migration.version,
            migration.description
        );
    }

    Ok(())
}

fn sha384(data: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha384};
    let mut hasher = Sha384::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}
