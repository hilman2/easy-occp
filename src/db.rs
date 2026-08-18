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
