//! Monatsbericht als PDF: je Mitarbeiter eine Seite mit allen abgeschlossenen
//! Ladungen des Monats.

use std::io::BufWriter;

use axum::extract::{Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use printpdf::{BuiltinFont, Mm, PdfDocument};
use serde::Deserialize;

use crate::auth::AdminUser;
use crate::i18n::Lang;
use crate::{AppError, AppResult, AppState};

#[derive(Deserialize)]
pub struct Filter {
    pub year: Option<i32>,
    pub month: Option<u32>,
}

struct Row {
    start: String,
    stop: String,
    wallbox: String,
    id_tag: String,
    energy_wh: i64,
    duration_min: i64,
}

struct Employee {
    id: i64,
    name: String,
    email: Option<String>,
    rows: Vec<Row>,
    total_wh: i64,
}

pub async fn monthly_pdf(
    State(state): State<AppState>,
    AdminUser(_): AdminUser,
    lang: Lang,
    Query(q): Query<Filter>,
) -> AppResult<Response> {
    let now = Utc::now();
    let year = q.year.unwrap_or(now.year());
    let month = q.month.unwrap_or(now.month());
    if !(1..=12).contains(&month) {
        return Err(AppError::BadRequest(lang.t("err.month_range").into()));
    }

    let start = NaiveDate::from_ymd_opt(year, month, 1)
        .ok_or_else(|| AppError::BadRequest(lang.t("err.invalid_date").into()))?;
    let end = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .unwrap();
    let start_iso = Utc
        .from_utc_datetime(&start.and_hms_opt(0, 0, 0).unwrap())
        .to_rfc3339();
    let end_iso = Utc
        .from_utc_datetime(&end.and_hms_opt(0, 0, 0).unwrap())
        .to_rfc3339();

    // Alle abgeschlossenen Transaktionen im Monat, die einem Benutzer zugeordnet sind.
    // Alles in einer Query — Mitarbeiter, Wallbox, Start + Stop, Energie, Dauer.
    let rows: Vec<(i64, String, Option<String>, String, String, String, Option<String>, i64, i64)> =
        sqlx::query_as(
            "SELECT e.id, e.display_name, e.email, w.name, t.id_tag,
                    t.start_time, t.stop_time,
                    COALESCE(t.stop_meter_wh - t.start_meter_wh, 0),
                    strftime('%s', COALESCE(t.stop_time, t.start_time)) - strftime('%s', t.start_time)
             FROM transactions t
             JOIN employees e ON e.id = t.employee_id
             JOIN wallboxes w ON w.id = t.wallbox_id
             WHERE t.stop_meter_wh IS NOT NULL
               AND t.start_time >= ?1 AND t.start_time < ?2
             ORDER BY e.display_name, t.start_time",
        )
        .bind(&start_iso)
        .bind(&end_iso)
        .fetch_all(&state.db)
        .await?;

    // Gruppieren nach Mitarbeiter-ID.
    let mut groups: Vec<Employee> = Vec::new();
    for (emp_id, name, email, wb, tag, start_time, stop_time, energy_wh, dur_sec) in rows {
        let is_same = matches!(groups.last(), Some(e) if e.id == emp_id);
        if !is_same {
            groups.push(Employee {
                id: emp_id,
                name,
                email,
                rows: Vec::new(),
                total_wh: 0,
            });
        }
        let emp = groups.last_mut().unwrap();
        emp.rows.push(Row {
            start: fmt_iso(&start_time, lang),
            stop: stop_time.as_deref().map(|s| fmt_iso(s, lang)).unwrap_or_default(),
            wallbox: wb,
            id_tag: tag,
            energy_wh,
            duration_min: dur_sec.max(0) / 60,
        });
        emp.total_wh += energy_wh.max(0);
    }

    if groups.is_empty() {
        return Err(AppError::NotFound);
    }

    let pdf_bytes = render_pdf(year, month, &groups, lang)
        .map_err(|e| AppError::Other(anyhow::anyhow!("PDF: {e}")))?;

    let filename = format!("{}_{year:04}-{month:02}.pdf", lang.t("pdf.filename"));
    let mut resp = (StatusCode::OK, pdf_bytes).into_response();
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/pdf"),
    );
    resp.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")).unwrap(),
    );
    Ok(resp)
}

fn fmt_iso(s: &str, lang: Lang) -> String {
    let fmt = match lang {
        Lang::De => "%d.%m.%Y %H:%M",
        Lang::En => "%Y-%m-%d %H:%M",
        Lang::Fr | Lang::Es => "%d/%m/%Y %H:%M",
    };
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc).format(fmt).to_string())
        .unwrap_or_else(|_| s.to_string())
}

fn month_name(m: u32, lang: Lang) -> &'static str {
    match m {
        1 => lang.t("month.1"),
        2 => lang.t("month.2"),
        3 => lang.t("month.3"),
        4 => lang.t("month.4"),
        5 => lang.t("month.5"),
        6 => lang.t("month.6"),
        7 => lang.t("month.7"),
        8 => lang.t("month.8"),
        9 => lang.t("month.9"),
        10 => lang.t("month.10"),
        11 => lang.t("month.11"),
        12 => lang.t("month.12"),
        _ => "",
    }
}

fn render_pdf(year: i32, month: u32, employees: &[Employee], lang: Lang) -> anyhow::Result<Vec<u8>> {
    let (doc, page1, layer1) = PdfDocument::new(
        format!("{} {:04}-{:02}", lang.t("pdf.title"), year, month),
        Mm(210.0),
        Mm(297.0),
        "Layer 1",
    );
    let font_regular = doc.add_builtin_font(BuiltinFont::Helvetica)?;
    let font_bold = doc.add_builtin_font(BuiltinFont::HelveticaBold)?;
    let font_mono = doc.add_builtin_font(BuiltinFont::Courier)?;

    let mut first = true;
    let mut current_page = page1;
    let mut current_layer = layer1;

    for emp in employees {
        if !first {
            let (np, nl) = doc.add_page(Mm(210.0), Mm(297.0), "Layer 1");
            current_page = np;
            current_layer = nl;
        }
        first = false;

        {
            let layer = doc.get_page(current_page).get_layer(current_layer);
            layer.use_text(
                format!("{} {} {}", lang.t("pdf.title"), month_name(month, lang), year),
                18.0,
                Mm(20.0),
                Mm(275.0),
                &font_bold,
            );
            layer.use_text(
                format!("{}: {}", lang.t("common.employee"), emp.name),
                13.0,
                Mm(20.0),
                Mm(263.0),
                &font_regular,
            );
            if let Some(mail) = &emp.email {
                layer.use_text(
                    format!("{}: {}", lang.t("common.email"), mail),
                    10.0,
                    Mm(20.0),
                    Mm(256.0),
                    &font_regular,
                );
            }

            let header_y = 242.0;
            layer.use_text(lang.t("common.start"),    10.0, Mm(20.0),  Mm(header_y), &font_bold);
            layer.use_text(lang.t("common.end"),      10.0, Mm(55.0),  Mm(header_y), &font_bold);
            layer.use_text(lang.t("common.wallbox"),  10.0, Mm(90.0),  Mm(header_y), &font_bold);
            layer.use_text(lang.t("pdf.chip"),        10.0, Mm(125.0), Mm(header_y), &font_bold);
            layer.use_text(lang.t("pdf.duration"),    10.0, Mm(160.0), Mm(header_y), &font_bold);
            layer.use_text("kWh",                     10.0, Mm(180.0), Mm(header_y), &font_bold);
        }

        let mut y = 236.0;
        let mut total_min: i64 = 0;
        for r in &emp.rows {
            if y < 25.0 {
                let (np, nl) = doc.add_page(Mm(210.0), Mm(297.0), "Layer 1");
                current_page = np;
                current_layer = nl;
                let layer = doc.get_page(current_page).get_layer(current_layer);
                layer.use_text(
                    format!("… {} {}", lang.t("pdf.continued"), emp.name),
                    12.0,
                    Mm(20.0),
                    Mm(280.0),
                    &font_bold,
                );
                y = 270.0;
            }
            let layer = doc.get_page(current_page).get_layer(current_layer);
            layer.use_text(r.start.clone(),           9.0, Mm(20.0),  Mm(y), &font_regular);
            layer.use_text(r.stop.clone(),            9.0, Mm(55.0),  Mm(y), &font_regular);
            layer.use_text(truncate(&r.wallbox, 18),  9.0, Mm(90.0),  Mm(y), &font_regular);
            layer.use_text(truncate(&r.id_tag, 18),   9.0, Mm(125.0), Mm(y), &font_mono);
            layer.use_text(format!("{} min", r.duration_min), 9.0, Mm(160.0), Mm(y), &font_regular);
            layer.use_text(
                format!("{:.3}", r.energy_wh as f64 / 1000.0),
                9.0, Mm(180.0), Mm(y), &font_regular,
            );
            total_min += r.duration_min.max(0);
            y -= 5.5;
        }

        let layer = doc.get_page(current_page).get_layer(current_layer);
        y -= 4.0;
        layer.use_text(
            format!("{} {}", emp.rows.len(), lang.t("common.sessions")),
            11.0, Mm(20.0), Mm(y), &font_bold,
        );
        layer.use_text(
            format!("{}: {} min", lang.t("common.total"), total_min),
            11.0, Mm(90.0), Mm(y), &font_bold,
        );
        layer.use_text(
            format!("{}: {:.3} kWh", lang.t("common.energy"), emp.total_wh as f64 / 1000.0),
            11.0, Mm(160.0), Mm(y), &font_bold,
        );
    }

    let mut buf = BufWriter::new(Vec::new());
    doc.save(&mut buf)?;
    Ok(buf.into_inner()?)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max - 1).collect();
        out.push('…');
        out
    }
}
