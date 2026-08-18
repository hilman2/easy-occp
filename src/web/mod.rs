use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use rust_embed::RustEmbed;
use tower_http::trace::TraceLayer;

use crate::AppState;

mod assets;
mod views;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(views::dashboard::get))
        .route("/lang/:code", get(set_lang))
        .route(
            "/fragments/active-sessions",
            get(views::dashboard::active_sessions_fragment),
        )
        .route("/login", get(views::auth::login_form).post(views::auth::login_post))
        .route("/logout", post(views::auth::logout))
        .route("/wallboxes", get(views::wallboxes::list))
        .route("/wallboxes/new", post(views::wallboxes::create))
        .route("/wallboxes/:id", get(views::wallboxes::detail))
        .route("/wallboxes/:id/live", get(views::wallboxes::live_fragment))
        .route("/wallboxes/:id/delete", post(views::wallboxes::delete))
        .route("/wallboxes/:id/remote-start", post(views::wallboxes::remote_start))
        .route("/wallboxes/:id/remote-stop", post(views::wallboxes::remote_stop))
        .route("/wallboxes/:id/auth", post(views::wallboxes::set_auth))
        .route("/wallboxes/:id/auth/clear", post(views::wallboxes::clear_auth))
        .route("/chips", get(views::chips::list))
        .route("/chips/enroll", post(views::chips::enroll_start))
        .route("/chips/enroll/:id", get(views::chips::enroll_poll))
        .route("/chips/enroll/:id/save", post(views::chips::enroll_save))
        .route("/chips/:id/update", post(views::chips::update))
        .route("/chips/:id/delete", post(views::chips::delete))
        .route("/employees", get(views::employees::list))
        .route("/employees/new", post(views::employees::create))
        .route("/employees/:id", get(views::employees::detail))
        .route("/employees/:id/update", post(views::employees::update))
        .route("/employees/:id/delete", post(views::employees::delete))
        .route("/users", get(views::users::list))
        .route("/users/new", post(views::users::create))
        .route("/users/:id/delete", post(views::users::delete))
        .route("/users/:id/password", post(views::users::set_password))
        .route("/transactions", get(views::transactions::list))
        .route("/transactions.csv", get(views::transactions::export_csv))
        .route("/stats", get(views::stats::show))
        .route("/reports/monthly.pdf", get(views::reports::monthly_pdf))
        .route("/ocpp/:cp_id", get(crate::ocpp::ocpp16::ws_handler))
        .route("/ocpp15", post(crate::ocpp::soap15::soap_handler))
        .route("/static/*path", get(serve_asset))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Sprach-Umschalter: setzt das `lang`-Cookie und leitet zurück zur
/// aufrufenden Seite (Referer) bzw. zum Cockpit.
async fn set_lang(
    axum::extract::Path(code): axum::extract::Path<String>,
    headers: axum::http::HeaderMap,
) -> Response {
    let Some(lang) = crate::i18n::Lang::from_code(&code) else {
        return (axum::http::StatusCode::BAD_REQUEST, "unknown language").into_response();
    };
    let back = headers
        .get(header::REFERER)
        .and_then(|v| v.to_str().ok())
        // Nur lokale Pfade akzeptieren — kein Open-Redirect über fremde Referer.
        .and_then(|r| {
            let path = r.strip_prefix("http://").or_else(|| r.strip_prefix("https://"))
                .and_then(|rest| rest.find('/').map(|i| &rest[i..]))
                .unwrap_or(r);
            if path.starts_with('/') && !path.starts_with("//") {
                Some(path.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "/".to_string());
    let cookie = format!(
        "{}={}; Path=/; Max-Age=31536000; SameSite=Lax",
        crate::i18n::LANG_COOKIE,
        lang.code()
    );
    let mut resp = axum::response::Redirect::to(&back).into_response();
    resp.headers_mut().insert(
        header::SET_COOKIE,
        axum::http::HeaderValue::from_str(&cookie).unwrap(),
    );
    resp
}

#[derive(RustEmbed)]
#[folder = "static/"]
struct Assets;

async fn serve_asset(axum::extract::Path(path): axum::extract::Path<String>) -> Response {
    match Assets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            Response::builder()
                .header(header::CONTENT_TYPE, mime.as_ref())
                .header(header::CACHE_CONTROL, "public, max-age=3600")
                .body(axum::body::Body::from(content.data.into_owned()))
                .unwrap()
        }
        None => (axum::http::StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}
