pub mod local;
pub mod session;

use crate::domain::user::User;
use crate::{AppError, AppResult, AppState};
use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

/// Authentifizierter Benutzer, aus Cookie aufgelöst.
#[derive(Debug, Clone)]
pub struct AuthUser(pub User);

/// Optionaler auth – wird in Templates verwendet, um Nav zu zeigen oder nicht.
#[derive(Debug, Clone)]
pub struct MaybeAuth(pub Option<User>);

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = session::resolve(state, parts).await?;
        user.map(AuthUser).ok_or(AppError::Unauthorized)
    }
}

#[async_trait]
impl FromRequestParts<AppState> for MaybeAuth {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = session::resolve(state, parts).await.unwrap_or(None);
        Ok(MaybeAuth(user))
    }
}

pub struct AdminUser(pub User);

#[async_trait]
impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let AuthUser(user) = AuthUser::from_request_parts(parts, state).await?;
        if !user.is_admin() {
            return Err(AppError::Forbidden);
        }
        Ok(AdminUser(user))
    }
}

pub async fn authenticate_username_password(
    state: &AppState,
    username: &str,
    password: &str,
) -> AppResult<User> {
    if let Some(user) = local::try_login(state, username, password).await? {
        return Ok(user);
    }
    // LDAP/OIDC-Integration folgt; Stubs sind in auth::local::ldap_stub dokumentiert.
    Err(AppError::Unauthorized)
}
