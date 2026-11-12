//! Feed management: feed guides (expected use, entered from a named source and
//! marked reviewed or not), expected versus actual feed per batch, feed use
//! compared across batches, and the feed formulation calculator.
//!
//! No feed requirement is built in. Expected figures only appear when the farm
//! has entered a guide for that animal and age, and the guide's source and
//! review status are shown with every comparison. The formulation calculator
//! works out cost and proportions; it never claims a mix is balanced.

use crate::{auth::{self, Role}, farms::{check_len, parse_date, rows, today}, flocks::{species_group, summary}, AppState};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct FeedGuide { #[serde(default)] pub id: Option<i64>, pub species: String, pub age_from_day: i64, pub age_to_day: i64, pub grams_per_animal_day: f64, pub source: String, #[serde(default)] pub review_status: String, #[serde(default)] pub reviewer: String, #[serde(default)] pub notes: String }

pub fn list_guides_tx(conn: &Connection) -> Result<Vec<FeedGuide>, String> {
    rows(conn, "SELECT id,species,age_from_day,age_to_day,grams_per_animal_day,source,review_status,reviewer,notes FROM feed_guides ORDER BY species, age_from_day", [], |r| Ok(FeedGuide {
        id: r.get(0)?, species: r.get(1)?, age_from_day: r.get(2)?, age_to_day: r.get(3)?, grams_per_animal_day: r.get(4)?, source: r.get(5)?, review_status: r.get(6)?, reviewer: r.get(7)?, notes: r.get(8)?,
    }))
}

#[tauri::command]
pub fn list_feed_guides(state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<FeedGuide>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    list_guides_tx(&*state.db()?)
}

pub fn save_guide_tx(conn: &Connection, g: &FeedGuide) -> Result<i64, String> {
    if species_group(&g.species).is_none() { return Err("Choose the type of livestock".into()); }
    if g.age_from_day < 0 || g.age_to_day < g.age_from_day || g.age_to_day > 5000 { return Err("Check the age range in days".into()); }
    if !(g.grams_per_animal_day.is_finite() && g.grams_per_animal_day > 0.0 && g.grams_per_animal_day < 100_000.0) { return Err("Enter grams of feed per animal per day".into()); }
    if g.source.trim().chars().count() < 3 { return Err("Name the source of this guide (feed label, vet, extension guide…)".into()); }
    check_len(&g.source, 200, "Source")?;
    check_len(&g.notes, 500, "Notes")?;
    let status = if g.review_status == "approved" { "approved" } else { "pending" };
    if status == "approved" && g.reviewer.trim().chars().count() < 3 { return Err("Enter who reviewed this guide".into()); }
    check_len(&g.reviewer, 120, "Reviewer")?;
    match g.id {
        Some(id) => { conn.execute("UPDATE feed_guides SET species=?1,age_from_day=?2,age_to_day=?3,grams_per_animal_day=?4,source=?5,review_status=?6,reviewer=?7,notes=?8 WHERE id=?9",
            params![g.species, g.age_from_day, g.age_to_day, g.grams_per_animal_day, g.source.trim(), status, g.reviewer.trim(), g.notes.trim(), id]).map_err(|e| e.to_string())?; Ok(id) }
        None => { conn.execute("INSERT INTO feed_guides(species,age_from_day,age_to_day,grams_per_animal_day,source,review_status,reviewer,notes) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![g.species, g.age_from_day, g.age_to_day, g.grams_per_animal_day, g.source.trim(), status, g.reviewer.trim(), g.notes.trim()]).map_err(|e| e.to_string())?; Ok(conn.last_insert_rowid()) }
    }
}

#[tauri::command]
pub fn save_feed_guide(state: tauri::State<'_, AppState>, session_token: String, input: FeedGuide) -> Result<i64, String> {
    auth::require_adviser_or_manager(&state, &session_token)?;
    save_guide_tx(&*state.db()?, &input)
}

#[tauri::command]
pub fn delete_feed_guide(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    state.db()?.execute("DELETE FROM feed_guides WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Serialize, Debug)]
pub struct DayFeed { pub date: String, pub age_day: i64, pub actual_kg: f64, pub wasted_kg: f64, pub expected_kg: Option<f64>, pub guide_source: String, pub guide_reviewed: bool }

#[derive(Serialize, Debug)]
pub struct BatchFeed {
    pub flock_id: i64, pub code: String, pub species: String, pub status: String, pub age_days: i64, pub alive: i64,
    pub feed_kg: f64, pub wasted_kg: f64, pub wastage_pct: f64, pub feed_per_animal_kg: f64, pub feed_cost_kobo: i64, pub feed_cost_per_animal_kobo: i64, pub fcr: f64,
    pub days: Vec<DayFeed>,
}

/// Feed for one batch, day by day, with the expected amount wherever a guide covers that age.
pub fn batch_feed(conn: &Connection, flock_id: i64, with_days: bool) -> Result<BatchFeed, String> {
    let f = summary(conn, flock_id)?;
    let feed_cost: i64 = conn.query_row("SELECT COALESCE(SUM(amount_kobo),0) FROM flock_costs WHERE flock_id=?1 AND lower(category)='feed'", [flock_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    let animals = (f.sold + f.alive).max(1);
    let mut out = BatchFeed {
        flock_id, code: f.code.clone(), species: f.species.clone(), status: f.status.clone(), age_days: f.age_days, alive: f.alive,
        feed_kg: f.feed_kg, wasted_kg: f.feed_wasted_kg, wastage_pct: if f.feed_kg > 0.0 { f.feed_wasted_kg * 100.0 / f.feed_kg } else { 0.0 },
        feed_per_animal_kg: f.feed_kg / animals as f64, feed_cost_kobo: feed_cost, feed_cost_per_animal_kobo: feed_cost / animals, fcr: f.fcr, days: vec![],
    };
    if with_days {
        let guides: Vec<FeedGuide> = list_guides_tx(conn)?.into_iter().filter(|g| g.species == f.species).collect();
        let placed = parse_date(&f.placed_date).unwrap_or_else(today);
        let logs: Vec<(String, f64, f64)> = rows(conn, "SELECT log_date,SUM(feed_kg),SUM(feed_wasted_kg) FROM flock_logs WHERE flock_id=?1 GROUP BY log_date ORDER BY log_date", [flock_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        // Animals alive on each date: everyone in, minus those who left on or before that date.
        for (date, kg, wasted) in logs {
            let age = parse_date(&date).map(|d| (d - placed).num_days()).unwrap_or(0);
            let guide = guides.iter().find(|g| age >= g.age_from_day && age <= g.age_to_day);
            let alive_then = alive_on(conn, flock_id, f.chicks_placed, &date)?;
            out.days.push(DayFeed {
                date, age_day: age, actual_kg: kg, wasted_kg: wasted,
                expected_kg: guide.map(|g| g.grams_per_animal_day * alive_then as f64 / 1000.0),
                guide_source: guide.map(|g| g.source.clone()).unwrap_or_default(), guide_reviewed: guide.map(|g| g.review_status == "approved").unwrap_or(false),
            });
        }
    }
    Ok(out)
}

fn alive_on(conn: &Connection, id: i64, placed: i64, date: &str) -> Result<i64, String> {
    let q = |sql: &str| conn.query_row(sql, params![id, date], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string());
    let inn = q("SELECT COALESCE(SUM(quantity),0) FROM flock_receipts WHERE flock_id=?1 AND receipt_date<=?2")? + q("SELECT COALESCE(SUM(quantity),0) FROM flock_transfers WHERE to_flock_id=?1 AND transfer_date<=?2")?
        + q("SELECT COALESCE(SUM(born_alive),0) FROM birth_records WHERE flock_id=?1 AND birth_date<=?2")?;
    let out = q("SELECT COALESCE(SUM(mortality+culls),0) FROM flock_logs WHERE flock_id=?1 AND log_date<?2")? + q("SELECT COALESCE(SUM(birds),0) FROM flock_sales WHERE flock_id=?1 AND sale_date<?2")?
        + q("SELECT COALESCE(SUM(quantity),0) FROM flock_transfers WHERE from_flock_id=?1 AND transfer_date<?2")?;
    Ok((placed + inn - out).max(0))
}

/// Money fields are blanked for roles below Manager (workers, advisers, cashiers).
#[tauri::command]
pub fn get_batch_feed(state: tauri::State<'_, AppState>, session_token: String, flock_id: i64) -> Result<serde_json::Value, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let out = get_batch_feed_inner(state, session_token, flock_id)?;
    crate::auth::money_view(&me, &out)
}

fn get_batch_feed_inner(state: tauri::State<'_, AppState>, session_token: String, flock_id: i64) -> Result<BatchFeed, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    batch_feed(&*state.db()?, flock_id, true)
}

pub fn compare_tx(conn: &Connection, species: Option<String>) -> Result<Vec<BatchFeed>, String> {
    let sp = species.unwrap_or_default();
    let ids: Vec<i64> = rows(conn, "SELECT id FROM flocks WHERE ?1='' OR species=?1 ORDER BY placed_date DESC LIMIT 50", [&sp], |r| r.get(0))?;
    ids.into_iter().map(|id| batch_feed(conn, id, false)).collect()
}

/// Money fields are blanked for roles below Manager (workers, advisers, cashiers).
#[tauri::command]
pub fn compare_batch_feed(state: tauri::State<'_, AppState>, session_token: String, species: Option<String>) -> Result<serde_json::Value, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let out = compare_batch_feed_inner(state, session_token, species)?;
    crate::auth::money_view(&me, &out)
}

fn compare_batch_feed_inner(state: tauri::State<'_, AppState>, session_token: String, species: Option<String>) -> Result<Vec<BatchFeed>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    let conn = state.db()?;
    compare_tx(&conn, species)
}

// ------------------------------------------------------------------ formulation calculator

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct FormulaItem { pub ingredient: String, pub quantity_kg: f64, #[serde(default)] pub unit_cost_kobo: i64, #[serde(default)] pub protein_pct: Option<f64>, #[serde(default)] pub energy_kcal_kg: Option<f64> }

#[derive(Serialize, Debug, Default)]
pub struct FormulaResult {
    pub total_kg: f64, pub total_cost_kobo: i64, pub cost_per_kg_kobo: i64,
    pub shares: Vec<(String, f64)>,
    /// Weighted only over ingredients whose nutrient value was entered; None when any is missing.
    pub protein_pct: Option<f64>, pub energy_kcal_kg: Option<f64>,
    pub warning: String,
}

pub fn calculate(items: &[FormulaItem]) -> Result<FormulaResult, String> {
    if items.is_empty() { return Err("Add at least one ingredient".into()); }
    for i in items {
        if i.ingredient.trim().is_empty() { return Err("Name every ingredient".into()); }
        if !(i.quantity_kg.is_finite() && i.quantity_kg > 0.0) { return Err(format!("Enter the quantity of {}", i.ingredient)); }
        if i.unit_cost_kobo < 0 { return Err("Cost cannot be negative".into()); }
        if let Some(p) = i.protein_pct { if !(0.0..=100.0).contains(&p) { return Err(format!("Protein for {} must be 0–100%", i.ingredient)); } }
        if let Some(e) = i.energy_kcal_kg { if !(0.0..=10_000.0).contains(&e) { return Err(format!("Energy for {} looks wrong", i.ingredient)); } }
    }
    let total: f64 = items.iter().map(|i| i.quantity_kg).sum();
    let cost: i64 = items.iter().map(|i| ((i.unit_cost_kobo as f64) * i.quantity_kg).round() as i64).sum();
    let weighted = |f: fn(&FormulaItem) -> Option<f64>| -> Option<f64> {
        if items.iter().all(|i| f(i).is_some()) { Some(items.iter().map(|i| f(i).unwrap_or(0.0) * i.quantity_kg).sum::<f64>() / total) } else { None }
    };
    Ok(FormulaResult {
        total_kg: total, total_cost_kobo: cost, cost_per_kg_kobo: (cost as f64 / total).round() as i64,
        shares: items.iter().map(|i| (i.ingredient.trim().to_string(), i.quantity_kg * 100.0 / total)).collect(),
        protein_pct: weighted(|i| i.protein_pct), energy_kcal_kg: weighted(|i| i.energy_kcal_kg),
        warning: "Proportions adding up to 100% do not make a balanced feed. Have the formula checked by an animal nutritionist or your feed supplier, using real nutrient analysis, before feeding it.".into(),
    })
}

#[tauri::command]
pub fn calculate_formulation(state: tauri::State<'_, AppState>, session_token: String, items: Vec<FormulaItem>) -> Result<FormulaResult, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    calculate(&items)
}

#[derive(Serialize, Debug)]
pub struct Formulation { id: i64, name: String, species: String, notes: String, user_name: String, created_at: String, items: Vec<FormulaItem>, result: FormulaResult }

pub fn list_formulations_tx(conn: &Connection) -> Result<Vec<Formulation>, String> {
    let heads: Vec<(i64, String, String, String, String, String)> = rows(conn, "SELECT id,name,species,notes,user_name,created_at FROM feed_formulations ORDER BY created_at DESC LIMIT 200", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?;
    heads.into_iter().map(|(id, name, species, notes, user_name, created_at)| {
        let items: Vec<FormulaItem> = rows(conn, "SELECT ingredient,quantity_kg,unit_cost_kobo,protein_pct,energy_kcal_kg FROM feed_formulation_items WHERE formulation_id=?1 ORDER BY id", [id], |r| Ok(FormulaItem {
            ingredient: r.get(0)?, quantity_kg: r.get(1)?, unit_cost_kobo: r.get(2)?, protein_pct: r.get(3)?, energy_kcal_kg: r.get(4)?,
        }))?;
        let result = calculate(&items).unwrap_or_default();
        Ok(Formulation { id, name, species, notes, user_name, created_at, items, result })
    }).collect()
}

#[tauri::command]
pub fn list_formulations(state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<Formulation>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    list_formulations_tx(&*state.db()?)
}

pub fn save_formulation_tx(conn: &Connection, name: &str, species: &str, notes: &str, items: &[FormulaItem], who: &str) -> Result<i64, String> {
    if name.trim().is_empty() { return Err("Name the formula".into()); }
    check_len(name, 80, "Name")?;
    check_len(notes, 500, "Notes")?;
    calculate(items)?;
    conn.execute("INSERT INTO feed_formulations(name,species,notes,user_name) VALUES(?1,?2,?3,?4)", params![name.trim(), species, notes.trim(), who]).map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    for i in items {
        conn.execute("INSERT INTO feed_formulation_items(formulation_id,ingredient,quantity_kg,unit_cost_kobo,protein_pct,energy_kcal_kg) VALUES(?1,?2,?3,?4,?5,?6)",
            params![id, i.ingredient.trim(), i.quantity_kg, i.unit_cost_kobo, i.protein_pct, i.energy_kcal_kg]).map_err(|e| e.to_string())?;
    }
    Ok(id)
}

#[tauri::command]
pub fn save_formulation(state: tauri::State<'_, AppState>, session_token: String, name: String, species: String, notes: String, items: Vec<FormulaItem>) -> Result<i64, String> {
    let me = auth::require(&state, &session_token, Role::Manager)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let id = save_formulation_tx(&tx, &name, &species, &notes, &items, &me.display_name())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn delete_formulation(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    state.db()?.execute("DELETE FROM feed_formulations WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formulation_costs_and_honest_nutrients() {
        let items = vec![
            FormulaItem { ingredient: "Maize".into(), quantity_kg: 60.0, unit_cost_kobo: 40_000, protein_pct: Some(9.0), energy_kcal_kg: None },
            FormulaItem { ingredient: "Soybean meal".into(), quantity_kg: 40.0, unit_cost_kobo: 80_000, protein_pct: Some(44.0), energy_kcal_kg: None },
        ];
        let r = calculate(&items).unwrap();
        assert_eq!(r.total_cost_kobo, 5_600_000);
        assert_eq!(r.cost_per_kg_kobo, 56_000);
        assert!((r.protein_pct.unwrap() - 23.0).abs() < 1e-9);
        assert!(r.energy_kcal_kg.is_none(), "not every energy value was entered");
        assert!(r.warning.contains("do not make a balanced feed"));
        assert!(calculate(&[]).is_err());
    }

    #[test]
    fn expected_feed_only_from_an_entered_guide() {
        let conn = crate::db::test_conn();
        conn.execute("INSERT INTO flocks(code,placed_date,chicks_placed,species) VALUES('BR','2026-01-01',100,'broiler')", []).unwrap();
        conn.execute("INSERT INTO flock_logs(flock_id,log_date,feed_kg,feed_wasted_kg) VALUES(1,'2026-01-11',10,0.5)", []).unwrap();
        let before = batch_feed(&conn, 1, true).unwrap();
        assert!(before.days[0].expected_kg.is_none());
        assert!(save_guide_tx(&conn, &FeedGuide { species: "broiler".into(), age_from_day: 8, age_to_day: 21, grams_per_animal_day: 80.0, source: "x".into(), ..Default::default() }).is_err(), "source required");
        save_guide_tx(&conn, &FeedGuide { species: "broiler".into(), age_from_day: 8, age_to_day: 21, grams_per_animal_day: 80.0, source: "Feed bag label".into(), ..Default::default() }).unwrap();
        let after = batch_feed(&conn, 1, true).unwrap();
        assert!((after.days[0].expected_kg.unwrap() - 8.0).abs() < 1e-9);
        assert!(!after.days[0].guide_reviewed);
        assert!((after.wastage_pct - 5.0).abs() < 1e-9);
    }
}
