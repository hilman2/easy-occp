use anyhow::Result;
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize)]
pub struct PeriodStat {
    pub bucket: String,
    pub sessions: i64,
    pub energy_wh: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NamedStat {
    pub label: String,
    pub sessions: i64,
    pub energy_wh: i64,
}

#[derive(Debug, Clone, Copy)]
pub enum Granularity {
    Month,
    Quarter,
    Year,
}

impl Granularity {
    /// SQLite strftime-Formatstring für die jeweilige Periode.
    pub fn bucket_expr(&self) -> &'static str {
        match self {
            // strftime liefert '2026-04'
            Granularity::Month => "strftime('%Y-%m', start_time)",
            Granularity::Year => "strftime('%Y', start_time)",
            // Quartal erfordert Arithmetik – wir berechnen manuell in Rust
            Granularity::Quarter => "strftime('%Y-%m', start_time)",
        }
    }
}

pub async fn overview(db: &SqlitePool, gran: Granularity) -> Result<Vec<PeriodStat>> {
    let expr = gran.bucket_expr();
    let sql = format!(
        "SELECT {expr} AS bucket,
                COUNT(*) AS sessions,
                COALESCE(SUM(COALESCE(stop_meter_wh,0) - start_meter_wh), 0) AS energy_wh
         FROM transactions
         WHERE stop_meter_wh IS NOT NULL
         GROUP BY bucket
         ORDER BY bucket DESC
         LIMIT 60"
    );
    let rows: Vec<(String, i64, i64)> = sqlx::query_as(&sql).fetch_all(db).await?;

    let mut out: Vec<PeriodStat> = rows
        .into_iter()
        .map(|(bucket, sessions, energy_wh)| PeriodStat {
            bucket,
            sessions,
            energy_wh,
        })
        .collect();

    if let Granularity::Quarter = gran {
        use std::collections::BTreeMap;
        let mut acc: BTreeMap<String, (i64, i64)> = BTreeMap::new();
        for p in out.drain(..) {
            // bucket = YYYY-MM
            let year = &p.bucket[..4];
            let month: u32 = p.bucket[5..7].parse().unwrap_or(1);
            let q = (month - 1) / 3 + 1;
            let key = format!("{year}-Q{q}");
            let e = acc.entry(key).or_default();
            e.0 += p.sessions;
            e.1 += p.energy_wh;
        }
        out = acc
            .into_iter()
            .rev()
            .map(|(bucket, (sessions, energy_wh))| PeriodStat {
                bucket,
                sessions,
                energy_wh,
            })
            .collect();
    }

    Ok(out)
}

pub async fn by_employee(db: &SqlitePool, since: Option<&str>) -> Result<Vec<NamedStat>> {
    let (sql, bind_since) = match since {
        Some(_) => (
            "SELECT COALESCE(e.display_name, t.guest_label, '(Gast)') AS label,
                    COUNT(*) AS sessions,
                    COALESCE(SUM(stop_meter_wh - start_meter_wh), 0) AS energy_wh
             FROM transactions t
             LEFT JOIN employees e ON e.id = t.employee_id
             WHERE stop_meter_wh IS NOT NULL AND t.start_time >= ?1
             GROUP BY label
             ORDER BY energy_wh DESC
             LIMIT 100",
            true,
        ),
        None => (
            "SELECT COALESCE(e.display_name, t.guest_label, '(Gast)') AS label,
                    COUNT(*) AS sessions,
                    COALESCE(SUM(stop_meter_wh - start_meter_wh), 0) AS energy_wh
             FROM transactions t
             LEFT JOIN employees e ON e.id = t.employee_id
             WHERE stop_meter_wh IS NOT NULL
             GROUP BY label
             ORDER BY energy_wh DESC
             LIMIT 100",
            false,
        ),
    };
    let mut q = sqlx::query_as::<_, (String, i64, i64)>(sql);
    if bind_since {
        q = q.bind(since.unwrap());
    }
    let rows = q.fetch_all(db).await?;
    Ok(rows
        .into_iter()
        .map(|(label, sessions, energy_wh)| NamedStat {
            label,
            sessions,
            energy_wh,
        })
        .collect())
}

pub async fn by_wallbox(db: &SqlitePool, since: Option<&str>) -> Result<Vec<NamedStat>> {
    let (sql, bind_since) = match since {
        Some(_) => (
            "SELECT w.name AS label,
                    COUNT(*) AS sessions,
                    COALESCE(SUM(stop_meter_wh - start_meter_wh), 0) AS energy_wh
             FROM transactions t
             JOIN wallboxes w ON w.id = t.wallbox_id
             WHERE stop_meter_wh IS NOT NULL AND t.start_time >= ?1
             GROUP BY w.id
             ORDER BY energy_wh DESC",
            true,
        ),
        None => (
            "SELECT w.name AS label,
                    COUNT(*) AS sessions,
                    COALESCE(SUM(stop_meter_wh - start_meter_wh), 0) AS energy_wh
             FROM transactions t
             JOIN wallboxes w ON w.id = t.wallbox_id
             WHERE stop_meter_wh IS NOT NULL
             GROUP BY w.id
             ORDER BY energy_wh DESC",
            false,
        ),
    };
    let mut q = sqlx::query_as::<_, (String, i64, i64)>(sql);
    if bind_since {
        q = q.bind(since.unwrap());
    }
    let rows = q.fetch_all(db).await?;
    Ok(rows
        .into_iter()
        .map(|(label, sessions, energy_wh)| NamedStat {
            label,
            sessions,
            energy_wh,
        })
        .collect())
}

#[derive(Debug, Clone, Serialize)]
pub struct GuestSplit {
    pub employee_sessions: i64,
    pub employee_wh: i64,
    pub guest_sessions: i64,
    pub guest_wh: i64,
}

pub async fn employee_vs_guest(db: &SqlitePool, since: Option<&str>) -> Result<GuestSplit> {
    // Gast = kein user_id ODER chip.kind='guest'. Wir approximieren über user_id IS NULL.
    let sql = "SELECT
                 SUM(CASE WHEN t.employee_id IS NOT NULL THEN 1 ELSE 0 END) AS emp_s,
                 SUM(CASE WHEN t.employee_id IS NOT NULL THEN (stop_meter_wh - start_meter_wh) ELSE 0 END) AS emp_e,
                 SUM(CASE WHEN t.employee_id IS NULL THEN 1 ELSE 0 END) AS g_s,
                 SUM(CASE WHEN t.employee_id IS NULL THEN (stop_meter_wh - start_meter_wh) ELSE 0 END) AS g_e
               FROM transactions t
               WHERE stop_meter_wh IS NOT NULL";
    let row: (Option<i64>, Option<i64>, Option<i64>, Option<i64>) = match since {
        Some(s) => sqlx::query_as(&format!("{sql} AND t.start_time >= ?1"))
            .bind(s)
            .fetch_one(db)
            .await?,
        None => sqlx::query_as(sql).fetch_one(db).await?,
    };
    Ok(GuestSplit {
        employee_sessions: row.0.unwrap_or(0),
        employee_wh: row.1.unwrap_or(0).max(0),
        guest_sessions: row.2.unwrap_or(0),
        guest_wh: row.3.unwrap_or(0).max(0),
    })
}
