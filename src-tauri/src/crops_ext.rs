//! More crop records: fields (groups of plots), the crop variety catalogue,
//! crop health observations (pest, disease, weed, nutrient) with their review
//! status, and the crop calendar built from each crop's own dates.
//!
//! The calendar never assumes one planting date for the whole country: every
//! date comes from the farmer's records or from a seasonal outlook the farmer
//! entered, shown with its source.

use crate::{auth::{self, Role}, farms::{check_len, check_money, check_qty, rows, valid_date}, AppState};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

// ------------------------------------------------------------------ fields

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Field { #[serde(default)] pub id: Option<i64>, #[serde(default)] pub farm_id: Option<i64>, pub name: String, #[serde(default)] pub area: f64, #[serde(default)] pub area_unit: String, #[serde(default)] pub notes: String, #[serde(default)] pub plots: i64 }

pub fn list_fields_tx(conn: &Connection) -> Result<Vec<Field>, String> {
    rows(conn, "SELECT f.id,f.farm_id,f.name,f.area,f.area_unit,f.notes,(SELECT COUNT(*) FROM farm_plots p WHERE p.field_id=f.id) FROM farm_fields f ORDER BY f.name", [], |r| Ok(Field {
        id: r.get(0)?, farm_id: r.get(1)?, name: r.get(2)?, area: r.get(3)?, area_unit: r.get(4)?, notes: r.get(5)?, plots: r.get(6)?,
    }))
}

#[tauri::command]
pub fn list_fields(state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<Field>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    list_fields_tx(&*state.db()?)
}

#[tauri::command]
pub fn save_field(state: tauri::State<'_, AppState>, session_token: String, input: Field) -> Result<i64, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let conn = state.db()?;
    let name = input.name.trim();
    if name.is_empty() { return Err("Name the field".into()); }
    check_len(name, 80, "Name")?;
    check_qty(input.area, "Area")?;
    check_len(&input.notes, 500, "Notes")?;
    let unit = if input.area_unit.trim().is_empty() { "hectare" } else { input.area_unit.trim() };
    let dup: Option<i64> = conn.query_row("SELECT id FROM farm_fields WHERE name=?1 AND id<>COALESCE(?2,-1)", params![name, input.id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    if dup.is_some() { return Err("Another field already has that name".into()); }
    match input.id {
        Some(id) => { conn.execute("UPDATE farm_fields SET farm_id=?1,name=?2,area=?3,area_unit=?4,notes=?5 WHERE id=?6", params![input.farm_id, name, input.area, unit, input.notes.trim(), id]).map_err(|e| e.to_string())?; Ok(id) }
        None => { conn.execute("INSERT INTO farm_fields(farm_id,name,area,area_unit,notes) VALUES(?1,?2,?3,?4,?5)", params![input.farm_id, name, input.area, unit, input.notes.trim()]).map_err(|e| e.to_string())?; Ok(conn.last_insert_rowid()) }
    }
}

#[tauri::command]
pub fn set_plot_field(state: tauri::State<'_, AppState>, session_token: String, plot_id: i64, field_id: Option<i64>) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let n = state.db()?.execute("UPDATE farm_plots SET field_id=?1 WHERE id=?2", params![field_id, plot_id]).map_err(|e| e.to_string())?;
    if n == 0 { return Err("Plot not found".into()); }
    Ok(())
}

/// Every crop grown on a plot, newest first: the plot's crop history.
#[tauri::command]
pub fn plot_history(state: tauri::State<'_, AppState>, session_token: String, plot_id: i64) -> Result<Vec<crate::crops::CycleSummary>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    let conn = state.db()?;
    let ids: Vec<i64> = rows(&conn, "SELECT id FROM crop_cycles WHERE plot_id=?1 ORDER BY planting_date DESC", [plot_id], |r| r.get(0))?;
    ids.into_iter().map(|id| crate::crops::cycle_summary(&conn, id)).collect()
}

// ------------------------------------------------------------------ varieties

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Variety { #[serde(default)] pub id: Option<i64>, pub crop: String, pub variety: String, #[serde(default)] pub maturity_days: Option<i64>, #[serde(default)] pub notes: String }

pub fn list_varieties_tx(conn: &Connection) -> Result<Vec<Variety>, String> {
    rows(conn, "SELECT id,crop,variety,maturity_days,notes FROM crop_varieties ORDER BY crop, variety", [], |r| Ok(Variety { id: r.get(0)?, crop: r.get(1)?, variety: r.get(2)?, maturity_days: r.get(3)?, notes: r.get(4)? }))
}

#[tauri::command]
pub fn list_varieties(state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<Variety>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    list_varieties_tx(&*state.db()?)
}

#[tauri::command]
pub fn save_variety(state: tauri::State<'_, AppState>, session_token: String, input: Variety) -> Result<i64, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let conn = state.db()?;
    if input.crop.trim().is_empty() || input.variety.trim().is_empty() { return Err("Enter the crop and the variety".into()); }
    check_len(&input.crop, 60, "Crop")?;
    check_len(&input.variety, 80, "Variety")?;
    check_len(&input.notes, 500, "Notes")?;
    if let Some(d) = input.maturity_days { if !(1..=3650).contains(&d) { return Err("Days to maturity must be between 1 and 3650".into()); } }
    match input.id {
        Some(id) => conn.execute("UPDATE crop_varieties SET crop=?1,variety=?2,maturity_days=?3,notes=?4 WHERE id=?5", params![input.crop.trim(), input.variety.trim(), input.maturity_days, input.notes.trim(), id]).map(|_| id),
        None => conn.execute("INSERT INTO crop_varieties(crop,variety,maturity_days,notes) VALUES(?1,?2,?3,?4)", params![input.crop.trim(), input.variety.trim(), input.maturity_days, input.notes.trim()]).map(|_| conn.last_insert_rowid()),
    }.map_err(|e| if e.to_string().contains("UNIQUE") { "That variety is already listed".into() } else { e.to_string() })
}

#[tauri::command]
pub fn delete_variety(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    state.db()?.execute("DELETE FROM crop_varieties WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

// ------------------------------------------------------------------ crop health

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct CropHealth {
    #[serde(default)] pub id: Option<i64>, pub cycle_id: i64, #[serde(default)] pub crop: String, #[serde(default)] pub plot_code: String, pub observed_date: String, pub kind: String, pub observation: String,
    #[serde(default)] pub affected_area: f64, #[serde(default)] pub action_taken: String, #[serde(default)] pub product_applied: String, #[serde(default)] pub quantity_applied: String,
    #[serde(default)] pub cost_kobo: i64, #[serde(default)] pub advice_source: String, #[serde(default)] pub review_status: String, #[serde(default)] pub reviewer: String, #[serde(default)] pub notes: String, #[serde(default)] pub user_name: String,
}

pub fn list_crop_health_tx(conn: &Connection, cycle_id: Option<i64>) -> Result<Vec<CropHealth>, String> {
    rows(conn, "SELECT h.id,h.cycle_id,c.crop,p.code,h.observed_date,h.kind,h.observation,h.affected_area,h.action_taken,h.product_applied,h.quantity_applied,h.cost_kobo,h.advice_source,h.review_status,h.reviewer,h.notes,h.user_name
        FROM crop_health_records h JOIN crop_cycles c ON c.id=h.cycle_id JOIN farm_plots p ON p.id=c.plot_id WHERE ?1 IS NULL OR h.cycle_id=?1 ORDER BY h.observed_date DESC, h.id DESC LIMIT 500", [cycle_id], |r| Ok(CropHealth {
        id: r.get(0)?, cycle_id: r.get(1)?, crop: r.get(2)?, plot_code: r.get(3)?, observed_date: r.get(4)?, kind: r.get(5)?, observation: r.get(6)?, affected_area: r.get(7)?, action_taken: r.get(8)?,
        product_applied: r.get(9)?, quantity_applied: r.get(10)?, cost_kobo: r.get(11)?, advice_source: r.get(12)?, review_status: r.get(13)?, reviewer: r.get(14)?, notes: r.get(15)?, user_name: r.get(16)?,
    }))
}

/// Money fields are blanked for roles below Manager (workers, advisers, cashiers).
#[tauri::command]
pub fn list_crop_health(state: tauri::State<'_, AppState>, session_token: String, cycle_id: Option<i64>) -> Result<serde_json::Value, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let out = list_crop_health_inner(state, session_token, cycle_id)?;
    crate::auth::money_view(&me, &out)
}

fn list_crop_health_inner(state: tauri::State<'_, AppState>, session_token: String, cycle_id: Option<i64>) -> Result<Vec<CropHealth>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    list_crop_health_tx(&*state.db()?, cycle_id)
}

/// A product and quantity may only be recorded together with where the advice
/// came from; FarmBuddy never suggests a product or a rate.
pub fn save_crop_health_tx(conn: &Connection, h: &CropHealth, who: &str) -> Result<i64, String> {
    if !valid_date(&h.observed_date) { return Err("Choose the date you saw it".into()); }
    if !["pest", "disease", "weed", "nutrient", "other"].contains(&h.kind.as_str()) { return Err("Choose pest, disease, weed, nutrient or other".into()); }
    if h.observation.trim().is_empty() { return Err("Describe what you saw".into()); }
    for (v, n, l) in [(&h.observation, 500, "Observation"), (&h.action_taken, 500, "Action"), (&h.product_applied, 150, "Product"), (&h.quantity_applied, 120, "Quantity"), (&h.advice_source, 150, "Advice source"), (&h.reviewer, 120, "Reviewer"), (&h.notes, 500, "Notes")] { check_len(v, n, l)?; }
    check_qty(h.affected_area, "Affected area")?;
    check_money(h.cost_kobo, "Cost")?;
    if !h.product_applied.trim().is_empty() && h.advice_source.trim().is_empty() { return Err("Say who advised this product (extension officer, agronomist, product label)".into()); }
    let status = if h.review_status == "confirmed" { "confirmed" } else { "unconfirmed" };
    if status == "confirmed" && h.reviewer.trim().is_empty() { return Err("Enter who confirmed the problem".into()); }
    let exists: Option<i64> = conn.query_row("SELECT id FROM crop_cycles WHERE id=?1", [h.cycle_id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    if exists.is_none() { return Err("Crop not found".into()); }
    match h.id {
        Some(id) => {
            conn.execute("UPDATE crop_health_records SET observed_date=?1,kind=?2,observation=?3,affected_area=?4,action_taken=?5,product_applied=?6,quantity_applied=?7,cost_kobo=?8,advice_source=?9,review_status=?10,reviewer=?11,notes=?12 WHERE id=?13",
                params![h.observed_date, h.kind, h.observation.trim(), h.affected_area, h.action_taken.trim(), h.product_applied.trim(), h.quantity_applied.trim(), h.cost_kobo, h.advice_source.trim(), status, h.reviewer.trim(), h.notes.trim(), id]).map_err(|e| e.to_string())?;
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO crop_health_records(cycle_id,observed_date,kind,observation,affected_area,action_taken,product_applied,quantity_applied,cost_kobo,advice_source,review_status,reviewer,notes,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
                params![h.cycle_id, h.observed_date, h.kind, h.observation.trim(), h.affected_area, h.action_taken.trim(), h.product_applied.trim(), h.quantity_applied.trim(), h.cost_kobo, h.advice_source.trim(), status, h.reviewer.trim(), h.notes.trim(), who]).map_err(|e| e.to_string())?;
            Ok(conn.last_insert_rowid())
        }
    }
}

#[tauri::command]
pub fn save_crop_health(state: tauri::State<'_, AppState>, session_token: String, input: CropHealth) -> Result<i64, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    if input.cost_kobo > 0 && me.role < Role::Manager { return Err("Only managers can record costs".into()); }
    if input.review_status == "confirmed" && !(me.role == Role::Adviser || me.role >= Role::Manager) { return Err("Only a manager or adviser can confirm a problem".into()); }
    save_crop_health_tx(&*state.db()?, &input, &me.display_name())
}

#[tauri::command]
pub fn delete_crop_health(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    state.db()?.execute("DELETE FROM crop_health_records WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

// ------------------------------------------------------------------ calendar

#[derive(Serialize, Debug, Clone)]
pub struct CalendarEvent { pub date: String, pub end_date: String, pub kind: String, pub title: String, pub cycle_id: Option<i64>, pub source: String }

/// Planting, expected harvest windows, field work, harvests, crop health
/// observations, crop reminders and rainfall outlooks between two dates.
pub fn calendar_tx(conn: &Connection, from: &str, to: &str) -> Result<Vec<CalendarEvent>, String> {
    if !valid_date(from) || !valid_date(to) || to < from { return Err("Choose a valid date range".into()); }
    let mut ev: Vec<CalendarEvent> = Vec::new();
    let cycles: Vec<(i64, String, String, String, String, String)> = rows(conn, "SELECT c.id,c.crop||' on '||p.code,c.planting_date,c.expected_harvest_from,c.expected_harvest_to,c.status FROM crop_cycles c JOIN farm_plots p ON p.id=c.plot_id", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?;
    for (id, label, planted, hfrom, hto, status) in cycles {
        if planted.as_str() >= from && planted.as_str() <= to { ev.push(CalendarEvent { date: planted.clone(), end_date: String::new(), kind: "planting".into(), title: format!("Planted {label}"), cycle_id: Some(id), source: "Your records".into() }); }
        if !hfrom.is_empty() && status == "growing" {
            let end = if hto.is_empty() { hfrom.clone() } else { hto.clone() };
            if !(end.as_str() < from || hfrom.as_str() > to) { ev.push(CalendarEvent { date: hfrom.clone(), end_date: end, kind: "harvest_window".into(), title: format!("Expected harvest: {label}"), cycle_id: Some(id), source: "Your expected dates".into() }); }
        }
    }
    let acts: Vec<(i64, String, String, String)> = rows(conn, "SELECT a.cycle_id,a.activity_date,a.activity_type,c.crop||' on '||p.code FROM crop_activities a JOIN crop_cycles c ON c.id=a.cycle_id JOIN farm_plots p ON p.id=c.plot_id WHERE a.activity_date BETWEEN ?1 AND ?2", params![from, to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    for (id, d, t, label) in acts { ev.push(CalendarEvent { date: d, end_date: String::new(), kind: "activity".into(), title: format!("{}: {label}", t.replace('_', " ")), cycle_id: Some(id), source: "Your records".into() }); }
    let hv: Vec<(i64, String, f64, String, String)> = rows(conn, "SELECT h.cycle_id,h.harvest_date,h.quantity,h.unit,c.crop||' on '||p.code FROM harvest_records h JOIN crop_cycles c ON c.id=h.cycle_id JOIN farm_plots p ON p.id=c.plot_id WHERE h.harvest_date BETWEEN ?1 AND ?2", params![from, to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
    for (id, d, q, u, label) in hv { ev.push(CalendarEvent { date: d, end_date: String::new(), kind: "harvest".into(), title: format!("Harvested {} {u}: {label}", crate::db::format_qty(q)), cycle_id: Some(id), source: "Your records".into() }); }
    let ch: Vec<(i64, String, String, String)> = rows(conn, "SELECT h.cycle_id,h.observed_date,h.kind,c.crop FROM crop_health_records h JOIN crop_cycles c ON c.id=h.cycle_id WHERE h.observed_date BETWEEN ?1 AND ?2", params![from, to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    for (id, d, k, crop) in ch { ev.push(CalendarEvent { date: d, end_date: String::new(), kind: "crop_health".into(), title: format!("{k} seen on {crop}"), cycle_id: Some(id), source: "Your records".into() }); }
    let rem: Vec<(String, String, Option<i64>)> = rows(conn, "SELECT due_date,title,related_id FROM farm_reminders WHERE status='open' AND related_kind='crops' AND due_date BETWEEN ?1 AND ?2", params![from, to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    for (d, t, id) in rem { ev.push(CalendarEvent { date: d, end_date: String::new(), kind: "reminder".into(), title: t, cycle_id: id, source: "Your reminders".into() }); }
    let outl: Vec<(String, String, String, String, i64)> = rows(conn, "SELECT onset_date,cessation_date,state||CASE WHEN lga<>'' THEN ' · '||lga ELSE '' END,source,year FROM climate_outlooks", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
    for (on, off, place, src, year) in outl {
        if !on.is_empty() && on.as_str() >= from && on.as_str() <= to { ev.push(CalendarEvent { date: on, end_date: String::new(), kind: "rain_onset".into(), title: format!("Expected rainfall onset {year}: {place}"), cycle_id: None, source: src.clone() }); }
        if !off.is_empty() && off.as_str() >= from && off.as_str() <= to { ev.push(CalendarEvent { date: off, end_date: String::new(), kind: "rain_cessation".into(), title: format!("Expected rainfall cessation {year}: {place}"), cycle_id: None, source: src }); }
    }
    ev.sort_by(|a, b| a.date.cmp(&b.date).then(a.title.cmp(&b.title)));
    Ok(ev)
}

#[tauri::command]
pub fn crop_calendar(state: tauri::State<'_, AppState>, session_token: String, from: String, to: String) -> Result<Vec<CalendarEvent>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    calendar_tx(&*state.db()?, &from, &to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_health_needs_advice_source_for_products() {
        let conn = crate::db::test_conn();
        conn.execute("INSERT INTO farm_plots(code,area) VALUES('P1',1)", []).unwrap();
        conn.execute("INSERT INTO crop_cycles(plot_id,crop,planting_date,expected_harvest_from,expected_harvest_to) VALUES(1,'Maize','2026-04-01','2026-07-01','2026-07-20')", []).unwrap();
        let base = CropHealth { cycle_id: 1, observed_date: "2026-05-01".into(), kind: "pest".into(), observation: "Holes in leaves".into(), ..Default::default() };
        save_crop_health_tx(&conn, &base, "W").unwrap();
        assert!(save_crop_health_tx(&conn, &CropHealth { product_applied: "Product X".into(), ..CropHealth { cycle_id: 1, observed_date: "2026-05-02".into(), kind: "pest".into(), observation: "More holes".into(), ..Default::default() } }, "W").is_err());
        let cal = calendar_tx(&conn, "2026-04-01", "2026-07-31").unwrap();
        let kinds: Vec<&str> = cal.iter().map(|e| e.kind.as_str()).collect();
        assert!(kinds.contains(&"planting") && kinds.contains(&"harvest_window") && kinds.contains(&"crop_health"));
        let keys: Vec<String> = crate::advisory::recommendations(&conn).unwrap().into_iter().map(|r| r.key).collect();
        assert!(keys.iter().any(|k| k.starts_with("crop-health:")));
    }
}
