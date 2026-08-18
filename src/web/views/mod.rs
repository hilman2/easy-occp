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
use crate::i18n::Lang;

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
    pub lang: Lang,
}

impl LayoutCtx {
    pub fn new(active: &'static str, user: Option<User>, lang: Lang) -> Self {
        Self {
            active,
            user,
            flash: None,
            lang,
        }
    }

    /// Übersetzung für Templates: `{{ layout.t("key") }}`.
    pub fn t(&self, key: &'static str) -> &'static str {
        self.lang.t(key)
    }

    /// Alle verfügbaren Sprachen — für den Umschalter in der Topbar.
    pub fn langs(&self) -> [Lang; 4] {
        Lang::ALL
    }
}
