use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Datenbank: {0}")]
    Db(#[from] sqlx::Error),
    #[error("Nicht gefunden")]
    NotFound,
    #[error("Nicht autorisiert")]
    Unauthorized,
    #[error("Verboten")]
    Forbidden,
    #[error("Ungültige Eingabe: {0}")]
    BadRequest(String),
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, self.to_string()),
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            AppError::Conflict(_) => (StatusCode::CONFLICT, self.to_string()),
            AppError::Db(e) => {
                tracing::error!("DB-Fehler: {e:#}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Interner Fehler".into())
            }
            AppError::Other(e) => {
                tracing::error!("Fehler: {e:#}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Interner Fehler".into())
            }
        };
        (status, msg).into_response()
    }
}
