//! Livestock batch tracking for poultry, other livestock and fish (the table is
//! still called `flocks` so eStock data upgrades in place): daily mortality,
//! culls, feed, water, eggs and weight logs, costs, sales and the key numbers a
//! farmer watches: animals alive, mortality %, feed conversion ratio (FCR),
//! cost per animal and profit. Deaths, culls and sales are separate events, so
//! the same animal is never deducted twice.

use crate::{auth::{self, Role}, AppState};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::farms::{today, valid_date};

pub const POULTRY: [&str; 9] = ["broiler", "layer", "cockerel", "noiler", "turkey", "guinea_fowl", "duck", "goose", "other_poultry"];
pub const LIVESTOCK: [&str; 7] = ["goat", "sheep", "pig", "cattle", "rabbit", "snail", "other_livestock"];
pub const FISH: [&str; 3] = ["catfish", "tilapia", "other_fish"];

/// poultry | livestock | fish
pub fn species_group(s: &str) -> Option<&'static str> {
    if POULTRY.contains(&s) { Some("poultry") } else if LIVESTOCK.contains(&s) { Some("livestock") } else if FISH.contains(&s) { Some("fish") } else { None }
}

#[derive(Serialize, Default)]
pub struct FlockSummary {
    pub id: i64, pub code: String, pub breed: String, pub house: String, pub placed_date: String, pub chicks_placed: i64, pub chick_cost_kobo: i64, pub status: String, pub closed_date: String, pub notes: String,
    pub species: String, pub group: String, pub purpose: String, pub source: String, pub farm_id: Option<i64>, pub farm_name: String, pub is_demo: bool,
    pub age_days: i64, pub dead: i64, pub culled: i64, pub sold: i64, pub alive: i64, pub mortality_pct: f64, pub survival_pct: f64,
    pub feed_kg: f64, pub water_l: f64, pub eggs: i64, pub eggs_damaged: i64, pub latest_weight_kg: f64, pub sold_weight_kg: f64, pub fcr: f64,
    pub last_log_date: String,
    pub received: i64, pub transferred_in: i64, pub transferred_out: i64, pub born: i64, pub total_in: i64,
    pub eggs_sold: i64, pub eggs_used: i64, pub eggs_lost: i64, pub egg_stock: i64, pub egg_revenue_kobo: i64, pub laying_pct: f64,
    pub feed_wasted_kg: f64, pub pond_id: Option<i64>, pub pond_code: String, pub animals_registered: i64,
    pub costs_kobo: i64, pub revenue_kobo: i64, pub profit_kobo: i64, pub cost_per_bird_kobo: i64, pub cost_per_kg_kobo: i64,
}

pub fn summary(conn: &Connection, id: i64) -> Result<FlockSummary, String> {
    let mut f = conn.query_row("SELECT f.id,f.code,f.breed,f.house,f.placed_date,f.chicks_placed,f.chick_cost_kobo,f.status,f.closed_date,f.notes,f.species,f.purpose,f.source,f.farm_id,COALESCE(fm.name,''),f.is_demo,f.pond_id,COALESCE(p.code,'')
        FROM flocks f LEFT JOIN farms fm ON fm.id=f.farm_id LEFT JOIN fish_ponds p ON p.id=f.pond_id WHERE f.id=?1", [id], |r| Ok(FlockSummary {
        id: r.get(0)?, code: r.get(1)?, breed: r.get(2)?, house: r.get(3)?, placed_date: r.get(4)?, chicks_placed: r.get(5)?, chick_cost_kobo: r.get(6)?, status: r.get(7)?, closed_date: r.get(8)?, notes: r.get(9)?,
        species: r.get(10)?, purpose: r.get(11)?, source: r.get(12)?, farm_id: r.get(13)?, farm_name: r.get(14)?, is_demo: r.get::<_, i64>(15)? == 1, pond_id: r.get(16)?, pond_code: r.get(17)?, ..Default::default()
    })).optional().map_err(|e| e.to_string())?.ok_or("Batch not found")?;
    f.group = species_group(&f.species).unwrap_or("livestock").to_string();
    let (dead, culled, feed, water, eggs, eggs_damaged, last_log): (i64, i64, f64, f64, i64, i64, String) = conn.query_row("SELECT COALESCE(SUM(mortality),0),COALESCE(SUM(culls),0),COALESCE(SUM(feed_kg),0),COALESCE(SUM(water_l),0),COALESCE(SUM(eggs_collected),0),COALESCE(SUM(eggs_damaged),0),COALESCE(MAX(log_date),'') FROM flock_logs WHERE flock_id=?1", [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))).map_err(|e| e.to_string())?;
    f.water_l = water;
    let q = |sql: &str| conn.query_row(sql, [id], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string());
    f.received = q("SELECT COALESCE(SUM(quantity),0) FROM flock_receipts WHERE flock_id=?1")?;
    f.transferred_in = q("SELECT COALESCE(SUM(quantity),0) FROM flock_transfers WHERE to_flock_id=?1")?;
    f.transferred_out = q("SELECT COALESCE(SUM(quantity),0) FROM flock_transfers WHERE from_flock_id=?1")?;
    f.born = q("SELECT COALESCE(SUM(born_alive),0) FROM birth_records WHERE flock_id=?1")?;
    f.eggs_sold = q("SELECT COALESCE(SUM(eggs),0) FROM egg_movements WHERE flock_id=?1 AND kind='sold'")?;
    f.eggs_used = q("SELECT COALESCE(SUM(eggs),0) FROM egg_movements WHERE flock_id=?1 AND kind='used'")?;
    f.eggs_lost = q("SELECT COALESCE(SUM(eggs),0) FROM egg_movements WHERE flock_id=?1 AND kind='lost'")?;
    f.egg_revenue_kobo = q("SELECT COALESCE(SUM(amount_kobo),0) FROM egg_movements WHERE flock_id=?1 AND kind='sold'")?;
    f.animals_registered = q("SELECT COUNT(*) FROM livestock_animals WHERE flock_id=?1 AND status='active'")?;
    f.feed_wasted_kg = conn.query_row("SELECT COALESCE(SUM(feed_wasted_kg),0) FROM flock_logs WHERE flock_id=?1", [id], |r| r.get(0)).map_err(|e| e.to_string())?;
    let receipt_costs = q("SELECT COALESCE(SUM(cost_kobo),0) FROM flock_receipts WHERE flock_id=?1")?;
    f.eggs = eggs;
    f.eggs_damaged = eggs_damaged;
    f.last_log_date = last_log;
    let latest_weight: f64 = conn.query_row("SELECT avg_weight_kg FROM flock_logs WHERE flock_id=?1 AND avg_weight_kg>0 ORDER BY log_date DESC, id DESC LIMIT 1", [id], |r| r.get(0)).optional().map_err(|e| e.to_string())?.unwrap_or(0.0);
    let (sold, sold_kg, revenue): (i64, f64, i64) = conn.query_row("SELECT COALESCE(SUM(birds),0),COALESCE(SUM(live_weight_kg),0),COALESCE(SUM(amount_kobo),0) FROM flock_sales WHERE flock_id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(|e| e.to_string())?;
    let other_costs: i64 = conn.query_row("SELECT COALESCE(SUM(amount_kobo),0) FROM flock_costs WHERE flock_id=?1", [id], |r| r.get(0)).map_err(|e| e.to_string())?;
    let end = if f.status == "closed" && valid_date(&f.closed_date) { chrono::NaiveDate::parse_from_str(&f.closed_date, "%Y-%m-%d").unwrap() } else { today() };
    f.age_days = chrono::NaiveDate::parse_from_str(&f.placed_date, "%Y-%m-%d").map(|d| (end - d).num_days().max(0)).unwrap_or(0);
    f.dead = dead;
    f.culled = culled;
    f.sold = sold;
    f.total_in = f.chicks_placed + f.received + f.transferred_in + f.born;
    f.alive = (f.total_in - dead - culled - sold - f.transferred_out).max(0);
    f.egg_stock = (f.eggs - f.eggs_damaged - f.eggs_sold - f.eggs_used - f.eggs_lost).max(0);
    let last_eggs: i64 = conn.query_row("SELECT COALESCE(SUM(eggs_collected),0) FROM flock_logs WHERE flock_id=?1 AND log_date=(SELECT MAX(log_date) FROM flock_logs WHERE flock_id=?1 AND eggs_collected>0)", [id], |r| r.get(0)).map_err(|e| e.to_string())?;
    // Laying percentage: eggs on the last collection day ÷ birds alive.
    f.laying_pct = if f.alive > 0 && last_eggs > 0 { (last_eggs as f64) * 100.0 / (f.alive as f64) } else { 0.0 };
    // Rates are measured against every animal that came in (placed, received, born, moved in), never against the animals still alive.
    // Measured against every animal that entered the batch (placed, received, transferred in, born).
    f.mortality_pct = if f.total_in > 0 { (dead as f64) * 100.0 / (f.total_in as f64) } else { 0.0 };
    // Survival = did not die. Animals sold, culled or moved to another batch still count as survived.
    f.survival_pct = if f.total_in > 0 { 100.0 - f.mortality_pct } else { 0.0 };
    f.feed_kg = feed;
    f.latest_weight_kg = latest_weight;
    f.sold_weight_kg = sold_kg;
    // FCR = feed eaten ÷ live weight produced (sold weight + current birds × latest average weight).
    let produced = sold_kg + (f.alive as f64) * latest_weight;
    f.fcr = if produced > 0.0 { feed / produced } else { 0.0 };
    f.costs_kobo = f.chick_cost_kobo + other_costs + receipt_costs;
    f.revenue_kobo = revenue + f.egg_revenue_kobo;
    f.profit_kobo = revenue - f.costs_kobo;
    let birds_out = (sold + f.alive).max(1);
    f.cost_per_bird_kobo = f.costs_kobo / birds_out;
    f.cost_per_kg_kobo = if produced > 0.0 { ((f.costs_kobo as f64) / produced).round() as i64 } else { 0 };
    Ok(f)
}

/// Money fields are blanked for roles below Manager (workers, advisers, cashiers).
#[tauri::command]
pub fn list_flocks(state: tauri::State<'_, AppState>, session_token: String, include_closed: Option<bool>, group: Option<String>) -> Result<serde_json::Value, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let out = list_flocks_inner(state, session_token, include_closed, group)?;
    crate::auth::money_view(&me, &out)
}

fn list_flocks_inner(state: tauri::State<'_, AppState>, session_token: String, include_closed: Option<bool>, group: Option<String>) -> Result<Vec<FlockSummary>, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let conn = state.db()?;
    let allowed = crate::farm_access::allowed(&conn, &me)?;
    let mut stmt = conn.prepare("SELECT id,species FROM flocks WHERE ?1=1 OR status='active' ORDER BY status, placed_date DESC, id DESC LIMIT 300").map_err(|e| e.to_string())?;
    let g = group.unwrap_or_default();
    let ids: Vec<i64> = stmt.query_map([include_closed.unwrap_or(false) as i64], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
        .into_iter().filter(|(_, sp)| g.is_empty() || species_group(sp) == Some(g.as_str())).map(|(id, _)| id).collect();
    Ok(ids.into_iter().map(|id| summary(&conn, id)).collect::<Result<Vec<_>, _>>()?.into_iter().filter(|f| crate::farm_access::visible(&allowed, f.farm_id)).collect())
}

#[derive(Serialize)]
pub struct FlockEntry { id: i64, kind: String, date: String, description: String, quantity: f64, amount_kobo: i64, mortality: i64, culls: i64, feed_kg: f64, avg_weight_kg: f64, eggs: i64, eggs_damaged: i64, water_l: f64, user_name: String }

#[derive(Serialize)]
pub struct FlockDetail { flock: FlockSummary, entries: Vec<FlockEntry> }

/// Money fields are blanked for roles below Manager (workers, advisers, cashiers).
#[tauri::command]
pub fn get_flock(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<serde_json::Value, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let out = get_flock_inner(state, session_token, id)?;
    crate::auth::money_view(&me, &out)
}

fn get_flock_inner(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<FlockDetail, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    flock_detail(&*state.db()?, id)
}

pub fn flock_detail(conn: &Connection, id: i64) -> Result<FlockDetail, String> {
    let flock = summary(conn, id)?;
    let sql = "
        SELECT id,'log',log_date,notes,0,0,mortality,culls,feed_kg,avg_weight_kg,eggs_collected,eggs_damaged,water_l,user_name FROM flock_logs WHERE flock_id=?1
        UNION ALL SELECT id,'cost',cost_date,category||CASE WHEN description<>'' THEN ': '||description ELSE '' END,quantity,amount_kobo,0,0,0,0,0,0,0,user_name FROM flock_costs WHERE flock_id=?1
        UNION ALL SELECT id,'sale',sale_date,'Sold '||birds||' animals'||CASE WHEN buyer<>'' THEN ' to '||buyer ELSE '' END,live_weight_kg,amount_kobo,0,0,0,0,0,0,0,user_name FROM flock_sales WHERE flock_id=?1
        UNION ALL SELECT id,'receive',receipt_date,'Received '||quantity||CASE WHEN source<>'' THEN ' from '||source ELSE '' END||CASE WHEN notes<>'' THEN ' · '||notes ELSE '' END,quantity,cost_kobo,0,0,0,0,0,0,0,user_name FROM flock_receipts WHERE flock_id=?1
        UNION ALL SELECT t.id,'transfer_out',t.transfer_date,'Moved '||t.quantity||' to '||COALESCE(f.code,'another batch'),t.quantity,0,0,0,0,0,0,0,0,t.user_name FROM flock_transfers t LEFT JOIN flocks f ON f.id=t.to_flock_id WHERE t.from_flock_id=?1
        UNION ALL SELECT t.id,'transfer_in',t.transfer_date,'Moved in '||t.quantity||' from '||COALESCE(f.code,'another batch'),t.quantity,0,0,0,0,0,0,0,0,t.user_name FROM flock_transfers t LEFT JOIN flocks f ON f.id=t.from_flock_id WHERE t.to_flock_id=?1
        UNION ALL SELECT id,'eggs',move_date,CASE kind WHEN 'sold' THEN 'Sold '||eggs||' eggs'||CASE WHEN buyer<>'' THEN ' to '||buyer ELSE '' END WHEN 'used' THEN 'Used '||eggs||' eggs on the farm' ELSE 'Lost '||eggs||' eggs' END,eggs,amount_kobo,0,0,0,0,0,0,0,user_name FROM egg_movements WHERE flock_id=?1
        UNION ALL SELECT id,'birth',birth_date,born_alive||' born alive'||CASE WHEN stillborn>0 THEN ', '||stillborn||' stillborn' ELSE '' END,born_alive,0,0,0,0,0,0,0,0,user_name FROM birth_records WHERE flock_id=?1
        ORDER BY 3 DESC, 1 DESC";
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let entries = stmt.query_map([id], |r| Ok(FlockEntry { id: r.get(0)?, kind: r.get(1)?, date: r.get(2)?, description: r.get(3)?, quantity: r.get(4)?, amount_kobo: r.get(5)?, mortality: r.get(6)?, culls: r.get(7)?, feed_kg: r.get(8)?, avg_weight_kg: r.get(9)?, eggs: r.get(10)?, eggs_damaged: r.get(11)?, water_l: r.get(12)?, user_name: r.get(13)? }))
        .map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(FlockDetail { flock, entries })
}

#[derive(Deserialize)]
pub struct FlockInput {
    pub id: Option<i64>, pub code: String, pub breed: String, pub house: String, pub placed_date: String, pub chicks_placed: i64, pub chick_cost_kobo: i64, pub notes: String,
    #[serde(default = "default_species")] pub species: String,
    #[serde(default)] pub purpose: String,
    #[serde(default)] pub source: String,
    #[serde(default)] pub farm_id: Option<i64>,
    #[serde(default)] pub pond_id: Option<i64>,
}
fn default_species() -> String { "broiler".into() }

#[tauri::command]
pub fn save_flock(state: tauri::State<'_, AppState>, session_token: String, input: FlockInput) -> Result<i64, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    save_flock_tx(&*state.db()?, &input)
}

pub fn save_flock_tx(conn: &Connection, input: &FlockInput) -> Result<i64, String> {
    let code = input.code.trim();
    if code.is_empty() || code.chars().count() > 40 { return Err("Give the batch a short code, such as BR-2026-07".into()); }
    if species_group(&input.species).is_none() { return Err("Choose the type of livestock".into()); }
    if !valid_date(&input.placed_date) { return Err("Choose the date the animals were placed or bought".into()); }
    if !(1..=10_000_000).contains(&input.chicks_placed) { return Err("Enter how many animals were placed".into()); }
    if input.chick_cost_kobo < 0 { return Err("Purchase cost cannot be negative".into()); }
    if input.breed.chars().count() > 60 || input.house.chars().count() > 60 || input.notes.chars().count() > 500 || input.purpose.chars().count() > 60 || input.source.chars().count() > 120 { return Err("One of the fields is too long".into()); }
    if let Some(fid) = input.farm_id {
        let ok: Option<i64> = conn.query_row("SELECT id FROM farms WHERE id=?1", [fid], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
        if ok.is_none() { return Err("Farm not found".into()); }
    }
    let dup: Option<i64> = conn.query_row("SELECT id FROM flocks WHERE code=?1 AND id<>COALESCE(?2,-1)", params![code, input.id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    if dup.is_some() { return Err("Another batch already uses that code".into()); }
    match input.id {
        Some(id) => {
            // Lowering the number placed below what has already left the batch would make counts negative.
            let s = summary(conn, id)?;
            let out = s.dead + s.culled + s.sold + s.transferred_out - s.received - s.transferred_in - s.born;
            if input.chicks_placed < out { return Err(format!("{out} animals have already died, been culled or been sold from this batch")); }
            conn.execute("UPDATE flocks SET code=?1,breed=?2,house=?3,placed_date=?4,chicks_placed=?5,chick_cost_kobo=?6,notes=?7,species=?8,purpose=?9,source=?10,farm_id=?11,pond_id=?12 WHERE id=?13",
                params![code, input.breed.trim(), input.house.trim(), input.placed_date, input.chicks_placed, input.chick_cost_kobo, input.notes.trim(), input.species, input.purpose.trim(), input.source.trim(), input.farm_id, input.pond_id, id]).map_err(|e| e.to_string())?;
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO flocks(code,breed,house,placed_date,chicks_placed,chick_cost_kobo,notes,species,purpose,source,farm_id,pond_id) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                params![code, input.breed.trim(), input.house.trim(), input.placed_date, input.chicks_placed, input.chick_cost_kobo, input.notes.trim(), input.species, input.purpose.trim(), input.source.trim(), input.farm_id, input.pond_id]).map_err(|e| e.to_string())?;
            Ok(conn.last_insert_rowid())
        }
    }
}

#[tauri::command]
pub fn close_flock(state: tauri::State<'_, AppState>, session_token: String, id: i64, closed_date: String, reopen: Option<bool>) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let conn = state.db()?;
    let n = if reopen.unwrap_or(false) {
        conn.execute("UPDATE flocks SET status='active',closed_date='' WHERE id=?1", [id])
    } else {
        if !valid_date(&closed_date) { return Err("Choose the closing date".into()); }
        conn.execute("UPDATE flocks SET status='closed',closed_date=?1 WHERE id=?2", params![closed_date, id])
    }.map_err(|e| e.to_string())?;
    if n == 0 { return Err("Batch not found".into()); }
    Ok(())
}

#[derive(Deserialize)]
pub struct FlockEntryInput {
    flock_id: i64,
    /// log | cost | sale
    kind: String,
    date: String,
    #[serde(default)] mortality: i64,
    #[serde(default)] culls: i64,
    #[serde(default)] feed_kg: f64,
    #[serde(default)] avg_weight_kg: f64,
    #[serde(default)] eggs_collected: i64,
    #[serde(default)] eggs_damaged: i64,
    #[serde(default)] water_l: f64,
    #[serde(default)] feed_wasted_kg: f64,
    #[serde(default)] category: String,
    #[serde(default)] description: String,
    /// For costs: take this stock item (e.g. feed, vaccine) out of inventory.
    #[serde(default)] product_id: Option<i64>,
    #[serde(default)] quantity: f64,
    #[serde(default)] amount_kobo: i64,
    #[serde(default)] birds: i64,
    #[serde(default)] live_weight_kg: f64,
    #[serde(default)] buyer: String,
    #[serde(default)] receipt_no: String,
}

#[tauri::command]
pub fn add_flock_entry(state: tauri::State<'_, AppState>, session_token: String, input: FlockEntryInput) -> Result<FlockSummary, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    if !valid_date(&input.date) { return Err("Choose a valid date".into()); }
    if input.description.chars().count() > 300 || input.buyer.chars().count() > 120 || input.category.chars().count() > 60 || input.receipt_no.chars().count() > 40 { return Err("One of the fields is too long".into()); }
    let nonneg = |v: f64| v.is_finite() && v >= 0.0;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let flock = summary(&tx, input.flock_id)?;
    if flock.status != "active" { return Err("This batch is closed. Reopen it to add entries.".into()); }
    let who = me.display_name();
    match input.kind.as_str() {
        "log" => {
            if input.mortality < 0 || input.culls < 0 || input.eggs_collected < 0 || input.eggs_damaged < 0 || !nonneg(input.feed_kg) || !nonneg(input.avg_weight_kg) || !nonneg(input.water_l) || !nonneg(input.feed_wasted_kg) { return Err("Numbers cannot be negative".into()); }
            if input.mortality + input.culls > flock.alive { return Err(format!("Only {} animals are alive in this batch", flock.alive)); }
            if input.eggs_damaged > input.eggs_collected { return Err("Damaged eggs cannot be more than eggs collected".into()); }
            if input.mortality == 0 && input.culls == 0 && input.feed_kg == 0.0 && input.avg_weight_kg == 0.0 && input.eggs_collected == 0 && input.water_l == 0.0 && input.feed_wasted_kg == 0.0 && input.description.trim().is_empty() { return Err("Enter at least one figure for the day".into()); }
            tx.execute("INSERT INTO flock_logs(flock_id,log_date,mortality,culls,feed_kg,avg_weight_kg,notes,user_name,eggs_collected,eggs_damaged,water_l,feed_wasted_kg) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                params![input.flock_id, input.date, input.mortality, input.culls, input.feed_kg, input.avg_weight_kg, input.description.trim(), who, input.eggs_collected, input.eggs_damaged, input.water_l, input.feed_wasted_kg]).map_err(|e| e.to_string())?;
        }
        "cost" => {
            if me.role < Role::Manager { return Err("Only managers can record batch costs".into()); }
            let category = input.category.trim();
            if category.is_empty() { return Err("Choose a cost category, such as Feed or Vaccine".into()); }
            let mut amount = input.amount_kobo;
            let mut feed_from_stock = 0.0;
            if let Some(pid) = input.product_id {
                if !(input.quantity.is_finite() && input.quantity > 0.0) { return Err("Enter the quantity taken from stock".into()); }
                let (name, unit, cost, stock, track): (String, String, i64, f64, i64) = tx.query_row("SELECT name,unit,cost_price_kobo,stock_qty,track_stock FROM products WHERE id=?1", [pid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).optional().map_err(|e| e.to_string())?.ok_or("Stock item not found")?;
                if track == 1 && stock + 1e-9 < input.quantity { return Err(format!("Not enough {name} in stock. Available: {} {unit}", crate::db::format_qty(stock))); }
                if amount <= 0 { amount = ((cost as f64) * input.quantity).round() as i64; }
                if track == 1 {
                    tx.execute("UPDATE products SET stock_qty=stock_qty-?1,updated_at=CURRENT_TIMESTAMP WHERE id=?2", params![input.quantity, pid]).map_err(|e| e.to_string())?;
                    tx.execute("INSERT INTO stock_movements(product_id,movement_type,quantity_delta,unit_cost_kobo,note,user_name) VALUES(?1,'flock_issue',?2,?3,?4,?5)", params![pid, -input.quantity, cost, format!("Issued to batch {}", flock.code), who]).map_err(|e| e.to_string())?;
                    crate::inventory::consume_batches(&tx, pid, input.quantity)?;
                }
                // Feed issued in kg counts toward the flock's feed total for FCR.
                if category.eq_ignore_ascii_case("feed") && unit.eq_ignore_ascii_case("kg") { feed_from_stock = input.quantity; }
            }
            if amount < 0 { return Err("Amount cannot be negative".into()); }
            tx.execute("INSERT INTO flock_costs(flock_id,cost_date,category,description,product_id,quantity,amount_kobo,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![input.flock_id, input.date, category, input.description.trim(), input.product_id, input.quantity.max(0.0), amount, who]).map_err(|e| e.to_string())?;
            if feed_from_stock > 0.0 {
                tx.execute("INSERT INTO flock_logs(flock_id,log_date,feed_kg,notes,user_name) VALUES(?1,?2,?3,'Feed issued from stock',?4)", params![input.flock_id, input.date, feed_from_stock, who]).map_err(|e| e.to_string())?;
            }
        }
        "sale" => {
            if input.birds <= 0 { return Err("Enter how many animals were sold".into()); }
            if input.birds > flock.alive { return Err(format!("Only {} animals are alive in this batch", flock.alive)); }
            if !nonneg(input.live_weight_kg) || input.amount_kobo < 0 { return Err("Numbers cannot be negative".into()); }
            tx.execute("INSERT INTO flock_sales(flock_id,sale_date,birds,live_weight_kg,amount_kobo,buyer,receipt_no,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![input.flock_id, input.date, input.birds, input.live_weight_kg, input.amount_kobo, input.buyer.trim(), input.receipt_no.trim(), who]).map_err(|e| e.to_string())?;
        }
        _ => return Err("Unknown entry type".into()),
    }
    let s = summary(&tx, input.flock_id)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(s)
}

fn me_name(state: &tauri::State<'_, AppState>, token: &str) -> String { auth::require(state, token, Role::Worker).map(|s| s.display_name()).unwrap_or_default() }

#[tauri::command]
pub fn delete_flock_entry(state: tauri::State<'_, AppState>, session_token: String, kind: String, id: i64) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let table = match kind.as_str() {
        "log" => "flock_logs", "cost" => "flock_costs", "sale" => "flock_sales", "receive" => "flock_receipts",
        "transfer_out" | "transfer_in" => "flock_transfers", "eggs" => "egg_movements", "birth" => "birth_records",
        _ => return Err("Unknown entry type".into()),
    };
    // Stock issued to a batch is not put back automatically; adjust stock if it was a mistake.
    let conn = state.db()?;
    let n = conn.execute(&format!("DELETE FROM {table} WHERE id=?1"), [id]).map_err(|e| e.to_string())?;
    if n == 0 { return Err("Entry not found".into()); }
    crate::audit::log(&conn, &me_name(&state, &session_token), "delete", table, Some(id), &format!("Deleted a {kind} entry"));
    Ok(())
}

#[tauri::command]
pub fn delete_flock(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<(), String> {
    let me = auth::require(&state, &session_token, Role::Owner)?;
    let conn = state.db()?;
    let code: String = conn.query_row("SELECT code FROM flocks WHERE id=?1", [id], |r| r.get(0)).optional().map_err(|e| e.to_string())?.ok_or("Batch not found")?;
    conn.execute("DELETE FROM flock_transfers WHERE to_flock_id=?1 OR from_flock_id=?1", [id]).map_err(|e| e.to_string())?;
    let n = conn.execute("DELETE FROM flocks WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    if n == 0 { return Err("Batch not found".into()); }
    crate::audit::log(&conn, &me.display_name(), "delete", "livestock_batch", Some(id), &format!("Deleted batch {code} and its records"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flock_metrics() {
        let conn = crate::db::test_conn();
        conn.execute("INSERT INTO flocks(code,placed_date,chicks_placed,chick_cost_kobo) VALUES('B1','2026-01-01',1000,60000000)", []).unwrap();
        conn.execute("INSERT INTO flock_logs(flock_id,log_date,mortality,feed_kg,avg_weight_kg) VALUES(1,'2026-01-10',30,1500,0.5)", []).unwrap();
        conn.execute("INSERT INTO flock_logs(flock_id,log_date,mortality,feed_kg,avg_weight_kg) VALUES(1,'2026-02-10',20,2500,2.0)", []).unwrap();
        conn.execute("INSERT INTO flock_costs(flock_id,cost_date,category,amount_kobo) VALUES(1,'2026-01-05','Feed',200000000)", []).unwrap();
        conn.execute("INSERT INTO flock_sales(flock_id,sale_date,birds,live_weight_kg,amount_kobo) VALUES(1,'2026-02-12',450,900,450000000)", []).unwrap();
        let f = summary(&conn, 1).unwrap();
        assert_eq!(f.alive, 500);
        assert!((f.mortality_pct - 5.0).abs() < 1e-9);
        // feed 4000 kg ÷ (900 kg sold + 500 × 2.0 kg) = 2.105
        assert!((f.fcr - 4000.0 / 1900.0).abs() < 1e-9);
        assert_eq!(f.costs_kobo, 260000000);
        assert_eq!(f.profit_kobo, 190000000);
    }

    #[test]
    fn livestock_batches_validate_species_and_counts() {
        let conn = crate::db::test_conn();
        let mut input = FlockInput { id: None, code: "GT-1".into(), breed: "West African Dwarf".into(), house: String::new(), placed_date: "2026-03-01".into(), chicks_placed: 12, chick_cost_kobo: 0, notes: String::new(), species: "goat".into(), purpose: "Meat".into(), source: String::new(), farm_id: None, pond_id: None };
        let id = save_flock_tx(&conn, &input).unwrap();
        let f = summary(&conn, id).unwrap();
        assert_eq!(f.group, "livestock");
        assert!((f.survival_pct - 100.0).abs() < 1e-9);
        input.species = "dragon".into();
        assert!(save_flock_tx(&conn, &input).is_err());
        // Cannot reduce the number placed below what has already left the batch.
        conn.execute("INSERT INTO flock_logs(flock_id,log_date,mortality) VALUES(?1,'2026-03-05',3)", [id]).unwrap();
        conn.execute("INSERT INTO flock_sales(flock_id,sale_date,birds) VALUES(?1,'2026-03-06',4)", [id]).unwrap();
        input.species = "goat".into();
        input.id = Some(id);
        input.chicks_placed = 6;
        assert!(save_flock_tx(&conn, &input).is_err());
        input.chicks_placed = 7;
        save_flock_tx(&conn, &input).unwrap();
        let f = summary(&conn, id).unwrap();
        assert_eq!(f.alive, 0);
        assert_eq!(species_group("catfish"), Some("fish"));
    }
}
