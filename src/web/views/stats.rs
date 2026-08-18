use askama::Template;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use chrono::{Duration, Utc};
use serde::Deserialize;

use super::{render, LayoutCtx};
use crate::auth::AdminUser;
use crate::domain::stats::{
    by_employee, by_wallbox, employee_vs_guest, overview, GuestSplit, Granularity, NamedStat,
    PeriodStat,
};
use crate::{AppResult, AppState};

#[derive(Template)]
#[template(path = "stats.html")]
struct StatsTpl {
    layout: LayoutCtx,
    granularity: String,
    range: String,
    rows: Vec<PeriodStat>,
    per_employee: Vec<NamedStat>,
    per_wallbox: Vec<NamedStat>,
    split: GuestSplit,
}

#[derive(Deserialize)]
pub struct Filter {
    pub g: Option<String>,
    /// Zeitraum für die Rollups: "30d", "90d", "365d", "all" (default 90d).
    pub r: Option<String>,
}

pub async fn show(
    State(state): State<AppState>,
    AdminUser(user): AdminUser,
    lang: crate::i18n::Lang,
    Query(filter): Query<Filter>,
) -> AppResult<Response> {
    let g = filter.g.as_deref().unwrap_or("month");
    let gran = match g {
        "quarter" => Granularity::Quarter,
        "year" => Granularity::Year,
        _ => Granularity::Month,
    };
    let range = filter.r.as_deref().unwrap_or("90d");
    let since = match range {
        "30d" => Some((Utc::now() - Duration::days(30)).to_rfc3339()),
        "365d" => Some((Utc::now() - Duration::days(365)).to_rfc3339()),
        "all" => None,
        _ => Some((Utc::now() - Duration::days(90)).to_rfc3339()),
    };
    let since_ref = since.as_deref();

    let rows = overview(&state.db, gran).await.map_err(crate::AppError::Other)?;
    let per_employee = by_employee(&state.db, since_ref)
        .await
        .map_err(crate::AppError::Other)?;
    let per_wallbox = by_wallbox(&state.db, since_ref)
        .await
        .map_err(crate::AppError::Other)?;
    let split = employee_vs_guest(&state.db, since_ref)
        .await
        .map_err(crate::AppError::Other)?;

    let tpl = StatsTpl {
        layout: LayoutCtx::new("stats", Some(user), lang),
        granularity: g.to_string(),
        range: range.to_string(),
        rows,
        per_employee,
        per_wallbox,
        split,
    };
    Ok(render(&tpl)?.into_response())
}
