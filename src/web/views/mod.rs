pub mod auth;
pub mod chips;
pub mod dashboard;
pub mod employees;
pub mod reports;
pub mod stats;
pub mod transactions;
pub mod users;
pub mod wallboxes;

use askama::Template;
use axum::response::Html;

use crate::domain::user::User;

/// Rendert ein Askama-Template in eine HTML-Response und wandelt Rendering-Fehler
/// in `AppError::Other`. Kleine Helferschicht, die das globale
/// `askama_axum::IntoResponse` ersetzt (wir verzichten bewusst auf askama_axum,
/// um Dependency-Konflikte mit Askama 0.12 zu vermeiden).
pub fn render<T: Template>(t: &T) -> crate::AppResult<Html<String>> {
    Ok(Html(t.render().map_err(|e| anyhow::anyhow!("Template: {e}"))?))
}

#[derive(Clone)]
pub struct LayoutCtx {
    pub active: &'static str,
    pub user: Option<User>,
    pub flash: Option<String>,
}

impl LayoutCtx {
    pub fn new(active: &'static str, user: Option<User>) -> Self {
        Self {
            active,
            user,
            flash: None,
        }
    }
}
