use crate::db::verify_password;
use crate::domain::user::User;
use crate::{AppError, AppResult, AppState};

pub async fn try_login(
    state: &AppState,
    username: &str,
    password: &str,
) -> AppResult<Option<User>> {
    let user: Option<User> = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE username = ?1 AND auth_source = 'local' AND disabled = 0",
    )
    .bind(username)
    .fetch_optional(&state.db)
    .await?;

    let Some(user) = user else { return Ok(None) };
    let Some(hash) = user.password_hash.as_deref() else {
        return Ok(None);
    };
    match verify_password(password, hash) {
        Ok(true) => Ok(Some(user)),
        Ok(false) => Ok(None),
        Err(e) => Err(AppError::Other(e)),
    }
}
