//! Farm profiles (several farms under one business), shared validation helpers
//! for the farm modules, and the farm overview shown on the dashboard.

use crate::{auth::{self, Role}, AppState};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

// ------------------------------------------------------------------ shared helpers

pub fn valid_date(d: &str) -> bool { chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok() }
pub fn today() -> chrono::NaiveDate { chrono::Local::now().date_naive() }
pub fn today_str() -> String { today().format("%Y-%m-%d").to_string() }
pub fn parse_date(d: &str) -> Option<chrono::NaiveDate> { chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok() }

pub fn check_len(v: &str, n: usize, label: &str) -> Result<(), String> {
    if v.trim().chars().count() > n { Err(format!("{label} must be {n} characters or fewer")) } else { Ok(()) }
}

/// A finite, non-negative quantity (kg, litres, hectares…).
pub fn check_qty(v: f64, label: &str) -> Result<(), String> {
    if v.is_finite() && v >= 0.0 { Ok(()) } else { Err(format!("{label} cannot be negative")) }
}

pub fn check_money(v: i64, label: &str) -> Result<(), String> {
    if v >= 0 { Ok(()) } else { Err(format!("{label} cannot be negative")) }
}

/// Optional date field: empty, or a valid YYYY-MM-DD date.
pub fn check_opt_date(d: &str, label: &str) -> Result<(), String> {
    if d.is_empty() || valid_date(d) { Ok(()) } else { Err(format!("{label} is not a valid date")) }
}

pub fn rows<T, F>(conn: &Connection, sql: &str, p: impl rusqlite::Params, f: F) -> Result<Vec<T>, String>
where F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let out = stmt.query_map(p, f).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string());
    out
}

// ------------------------------------------------------------------ farms

#[derive(Serialize, Debug)]
pub struct Farm {
    pub id: i64, pub name: String, pub state: String, pub lga: String, pub community: String, pub address: String, pub phone: String,
    pub activities: String, pub notes: String, pub status: String, pub is_demo: bool, pub created_at: String,
}

pub fn list_farms_tx(conn: &Connection, include_archived: bool) -> Result<Vec<Farm>, String> {
    rows(conn, "SELECT id,name,state,lga,community,address,phone,activities,notes,status,is_demo,created_at FROM farms WHERE ?1=1 OR status='active' ORDER BY status, is_demo, name", [include_archived as i64], |r| Ok(Farm {
        id: r.get(0)?, name: r.get(1)?, state: r.get(2)?, lga: r.get(3)?, community: r.get(4)?, address: r.get(5)?, phone: r.get(6)?,
        activities: r.get(7)?, notes: r.get(8)?, status: r.get(9)?, is_demo: r.get::<_, i64>(10)? == 1, created_at: r.get(11)?,
    }))
}

#[tauri::command]
pub fn list_farms(state: tauri::State<'_, AppState>, session_token: String, include_archived: Option<bool>) -> Result<Vec<Farm>, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let conn = state.db()?;
    let a = crate::farm_access::allowed(&conn, &me)?;
    Ok(list_farms_tx(&conn, include_archived.unwrap_or(false))?.into_iter().filter(|f| crate::farm_access::visible(&a, Some(f.id))).collect())
}

#[derive(Deserialize, Default)]
pub struct FarmInput { pub id: Option<i64>, pub name: String, pub state: String, pub lga: String, pub community: String, pub address: String, pub phone: String, pub activities: String, pub notes: String }

pub fn save_farm_tx(conn: &Connection, input: &FarmInput) -> Result<i64, String> {
    let name = input.name.trim();
    if name.is_empty() { return Err("Enter the farm name".into()); }
    check_len(name, 120, "Farm name")?;
    check_len(&input.state, 60, "State")?;
    check_len(&input.lga, 80, "LGA")?;
    check_len(&input.community, 120, "Community")?;
    check_len(&input.address, 300, "Address")?;
    check_len(&input.phone, 40, "Phone")?;
    check_len(&input.activities, 400, "Activities")?;
    check_len(&input.notes, 1000, "Notes")?;
    let dup: Option<i64> = conn.query_row("SELECT id FROM farms WHERE lower(name)=lower(?1) AND id<>COALESCE(?2,-1) AND status='active'", params![name, input.id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    if dup.is_some() { return Err("Another farm already has that name".into()); }
    let v = params![name, input.state.trim(), input.lga.trim(), input.community.trim(), input.address.trim(), input.phone.trim(), input.activities.trim(), input.notes.trim()];
    match input.id {
        Some(id) => {
            let n = conn.execute("UPDATE farms SET name=?1,state=?2,lga=?3,community=?4,address=?5,phone=?6,activities=?7,notes=?8 WHERE id=?9",
                params![name, input.state.trim(), input.lga.trim(), input.community.trim(), input.address.trim(), input.phone.trim(), input.activities.trim(), input.notes.trim(), id]).map_err(|e| e.to_string())?;
            if n == 0 { return Err("Farm not found".into()); }
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO farms(name,state,lga,community,address,phone,activities,notes) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)", v).map_err(|e| e.to_string())?;
            Ok(conn.last_insert_rowid())
        }
    }
}

#[tauri::command]
pub fn save_farm(state: tauri::State<'_, AppState>, session_token: String, input: FarmInput) -> Result<i64, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    save_farm_tx(&*state.db()?, &input)
}

#[tauri::command]
pub fn archive_farm(state: tauri::State<'_, AppState>, session_token: String, id: i64, restore: Option<bool>) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Owner)?;
    let status = if restore.unwrap_or(false) { "active" } else { "archived" };
    // Archiving keeps every record for reports; nothing is deleted.
    let n = state.db()?.execute("UPDATE farms SET status=?1 WHERE id=?2", params![status, id]).map_err(|e| e.to_string())?;
    if n == 0 { return Err("Farm not found".into()); }
    Ok(())
}

// ------------------------------------------------------------------ farm overview

#[derive(Serialize, Default, Debug)]
pub struct SpeciesCount { pub species: String, pub batches: i64, pub alive: i64 }

#[derive(Serialize, Default, Debug)]
pub struct FarmOverview {
    pub farms: i64,
    pub active_batches: i64,
    pub live_animals: i64,
    pub by_species: Vec<SpeciesCount>,
    pub active_plots: i64,
    pub growing_cycles: i64,
    pub harvest_soon: i64,
    pub open_processing: i64,
    pub eggs_today: i64,
    pub vaccinations_due: i64,
    pub vaccinations_overdue: i64,
    pub vaccinations_need_review: i64,
    pub reminders: Vec<crate::advisory::Reminder>,
    pub suggestions: usize,
    pub low_inputs: Vec<LowInput>,
    pub demo_loaded: bool,
    pub onboarding_done: bool,
    /// Money cards, filled only for managers and owners.
    pub money: Option<OverviewMoney>,
}

#[derive(Serialize, Default, Debug)]
pub struct OverviewMoney {
    pub inventory_value_kobo: i64,
    pub owed_to_suppliers_kobo: i64,
    pub owed_by_customers_kobo: i64,
    pub expenses_this_month_kobo: i64,
    pub sales_this_month_kobo: i64,
    pub profit_estimate_kobo: i64,
    pub month_from: String,
}

/// This month so far, using the farm profit and loss rules (demo records excluded).
pub fn overview_money(conn: &Connection) -> Result<OverviewMoney, String> {
    let to = today_str();
    let from = format!("{}-01", &to[..7]);
    let pl = crate::finance::profit_loss(conn, &from, &to, false)?;
    Ok(OverviewMoney {
        inventory_value_kobo: pl.inventory_value_kobo,
        owed_to_suppliers_kobo: pl.owed_to_suppliers_kobo,
        owed_by_customers_kobo: pl.owed_by_customers_kobo,
        expenses_this_month_kobo: pl.expenses_kobo,
        sales_this_month_kobo: pl.revenue_kobo,
        profit_estimate_kobo: pl.net_profit_kobo,
        month_from: from,
    })
}

#[derive(Serialize, Debug)]
pub struct LowInput { pub id: i64, pub name: String, pub unit: String, pub stock_qty: f64, pub reorder_level: f64 }

pub fn overview_tx(conn: &Connection) -> Result<FarmOverview, String> {
    let mut o = FarmOverview { farms: conn.query_row("SELECT COUNT(*) FROM farms WHERE status='active'", [], |r| r.get(0)).map_err(|e| e.to_string())?, ..Default::default() };
    let active: Vec<i64> = rows(conn, "SELECT id FROM flocks WHERE status='active'", [], |r| r.get(0))?;
    let mut species: Vec<SpeciesCount> = Vec::new();
    for id in active {
        let f = crate::flocks::summary(conn, id)?;
        o.active_batches += 1;
        o.live_animals += f.alive;
        match species.iter_mut().find(|s| s.species == f.species) {
            Some(s) => { s.batches += 1; s.alive += f.alive; }
            None => species.push(SpeciesCount { species: f.species.clone(), batches: 1, alive: f.alive }),
        }
    }
    species.sort_by_key(|s| std::cmp::Reverse(s.alive));
    o.by_species = species;
    o.active_plots = conn.query_row("SELECT COUNT(*) FROM farm_plots WHERE status='active'", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    o.growing_cycles = conn.query_row("SELECT COUNT(*) FROM crop_cycles WHERE status='growing'", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    let soon = (today() + chrono::Duration::days(14)).format("%Y-%m-%d").to_string();
    o.harvest_soon = conn.query_row("SELECT COUNT(*) FROM crop_cycles WHERE status='growing' AND expected_harvest_from<>'' AND expected_harvest_from<=?1", [soon], |r| r.get(0)).map_err(|e| e.to_string())?;
    o.open_processing = conn.query_row("SELECT (SELECT COUNT(*) FROM processing_batches WHERE status='open') + (SELECT COUNT(*) FROM palm_runs WHERE status='open')", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    o.eggs_today = conn.query_row("SELECT COALESCE(SUM(l.eggs_collected),0) FROM flock_logs l JOIN flocks f ON f.id=l.flock_id WHERE f.status='active' AND l.log_date=?1", [today_str()], |r| r.get(0)).map_err(|e| e.to_string())?;
    let v = crate::health::vaccination_counts(conn)?;
    o.vaccinations_due = v.due;
    o.vaccinations_overdue = v.overdue;
    o.vaccinations_need_review = v.needs_review;
    o.reminders = crate::advisory::open_reminders(conn, 7)?;
    o.suggestions = crate::advisory::recommendations(conn)?.len();
    o.low_inputs = rows(conn, "SELECT id,name,unit,stock_qty,reorder_level FROM products WHERE active=1 AND track_stock=1 AND kind='material' AND reorder_level>0 AND stock_qty<=reorder_level ORDER BY (stock_qty/reorder_level), name LIMIT 8", [], |r| Ok(LowInput {
        id: r.get(0)?, name: r.get(1)?, unit: r.get(2)?, stock_qty: r.get(3)?, reorder_level: r.get(4)?,
    }))?;
    o.demo_loaded = crate::db::setting_bool(conn, "demo_loaded")?;
    o.onboarding_done = crate::db::setting_bool(conn, "onboarding_done")?;
    Ok(o)
}

#[tauri::command]
pub fn farm_overview(state: tauri::State<'_, AppState>, session_token: String) -> Result<FarmOverview, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let conn = state.db()?;
    let mut o = overview_tx(&conn)?;
    if me.role >= Role::Manager { o.money = Some(overview_money(&conn)?); }
    Ok(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn farm_names_are_unique_and_required() {
        let conn = crate::db::test_conn();
        let id = save_farm_tx(&conn, &FarmInput { name: "Ikot Farm".into(), state: "Akwa Ibom".into(), lga: "Uyo".into(), ..Default::default() }).unwrap();
        assert!(save_farm_tx(&conn, &FarmInput { name: "ikot farm".into(), ..Default::default() }).is_err());
        assert!(save_farm_tx(&conn, &FarmInput { name: " ".into(), ..Default::default() }).is_err());
        save_farm_tx(&conn, &FarmInput { id: Some(id), name: "Ikot Farm".into(), lga: "Itu".into(), ..Default::default() }).unwrap();
        let farms = list_farms_tx(&conn, false).unwrap();
        assert_eq!(farms.len(), 1);
        assert_eq!(farms[0].lga, "Itu");
    }

    #[test]
    fn overview_on_empty_database() {
        let conn = crate::db::test_conn();
        let o = overview_tx(&conn).unwrap();
        assert_eq!(o.active_batches, 0);
        assert!(!o.onboarding_done);
    }
}
