//! Crop farming: plots, crop cycles (one planting of one crop on one plot),
//! field activities with labour and input costs, and harvests.
//!
//! Inputs issued from stock (seed, fertiliser, herbicide) are deducted in the
//! same transaction as the activity. Harvested produce can be added to stock.
//! Revenue only counts money actually recorded as received; an estimated value
//! of produce in store is never shown as income.

use crate::{auth::{self, Role}, farms::{check_len, check_money, check_opt_date, check_qty, rows, today, valid_date}, AppState};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

pub const ACTIVITY_TYPES: [&str; 17] = ["land_clearing", "land_preparation", "ploughing", "ridging", "planting", "transplanting", "fertiliser", "weeding", "irrigation",
    "pest_observation", "disease_observation", "spraying", "pruning", "mulching", "harvesting", "post_harvest", "other"];

// ------------------------------------------------------------------ plots

#[derive(Serialize)]
pub struct Plot {
    id: i64, farm_id: Option<i64>, farm_name: String, code: String, name: String, area: f64, area_unit: String, location: String, soil_notes: String,
    irrigation: String, status: String, is_demo: bool, current_crop: String, cycles: i64, field_id: Option<i64>, field_name: String,
}

pub fn list_plots_tx(conn: &Connection, include_archived: bool) -> Result<Vec<Plot>, String> {
    rows(conn, "SELECT p.id,p.farm_id,COALESCE(f.name,''),p.code,p.name,p.area,p.area_unit,p.location,p.soil_notes,p.irrigation,p.status,p.is_demo,
        COALESCE((SELECT c.crop FROM crop_cycles c WHERE c.plot_id=p.id AND c.status='growing' ORDER BY c.planting_date DESC LIMIT 1),''),
        (SELECT COUNT(*) FROM crop_cycles c WHERE c.plot_id=p.id), p.field_id, COALESCE(fl.name,'')
        FROM farm_plots p LEFT JOIN farms f ON f.id=p.farm_id LEFT JOIN farm_fields fl ON fl.id=p.field_id WHERE ?1=1 OR p.status='active' ORDER BY p.status, p.code", [include_archived as i64], |r| Ok(Plot {
        id: r.get(0)?, farm_id: r.get(1)?, farm_name: r.get(2)?, code: r.get(3)?, name: r.get(4)?, area: r.get(5)?, area_unit: r.get(6)?, location: r.get(7)?,
        soil_notes: r.get(8)?, irrigation: r.get(9)?, status: r.get(10)?, is_demo: r.get::<_, i64>(11)? == 1, current_crop: r.get(12)?, cycles: r.get(13)?, field_id: r.get(14)?, field_name: r.get(15)?,
    }))
}

#[tauri::command]
pub fn list_plots(state: tauri::State<'_, AppState>, session_token: String, include_archived: Option<bool>) -> Result<Vec<Plot>, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let conn = state.db()?;
    let a = crate::farm_access::allowed(&conn, &me)?;
    Ok(list_plots_tx(&conn, include_archived.unwrap_or(false))?.into_iter().filter(|p| crate::farm_access::visible(&a, p.farm_id)).collect())
}

#[derive(Deserialize, Default)]
pub struct PlotInput { pub id: Option<i64>, pub farm_id: Option<i64>, pub code: String, pub name: String, pub area: f64, pub area_unit: String, pub location: String, pub soil_notes: String, pub irrigation: String }

pub fn save_plot_tx(conn: &Connection, i: &PlotInput) -> Result<i64, String> {
    let code = i.code.trim();
    if code.is_empty() { return Err("Give the plot a short code, such as PLOT-A".into()); }
    check_len(code, 30, "Plot code")?;
    check_len(&i.name, 80, "Plot name")?;
    check_qty(i.area, "Area")?;
    if i.area_unit.trim().is_empty() { return Err("Choose the unit of land area".into()); }
    check_len(&i.area_unit, 30, "Area unit")?;
    check_len(&i.location, 200, "Location")?;
    check_len(&i.soil_notes, 500, "Soil notes")?;
    check_len(&i.irrigation, 100, "Irrigation")?;
    let dup: Option<i64> = conn.query_row("SELECT id FROM farm_plots WHERE code=?1 AND id<>COALESCE(?2,-1)", params![code, i.id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    if dup.is_some() { return Err("Another plot already uses that code".into()); }
    match i.id {
        Some(id) => {
            let n = conn.execute("UPDATE farm_plots SET farm_id=?1,code=?2,name=?3,area=?4,area_unit=?5,location=?6,soil_notes=?7,irrigation=?8 WHERE id=?9",
                params![i.farm_id, code, i.name.trim(), i.area, i.area_unit.trim(), i.location.trim(), i.soil_notes.trim(), i.irrigation.trim(), id]).map_err(|e| e.to_string())?;
            if n == 0 { return Err("Plot not found".into()); }
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO farm_plots(farm_id,code,name,area,area_unit,location,soil_notes,irrigation) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![i.farm_id, code, i.name.trim(), i.area, i.area_unit.trim(), i.location.trim(), i.soil_notes.trim(), i.irrigation.trim()]).map_err(|e| e.to_string())?;
            Ok(conn.last_insert_rowid())
        }
    }
}

#[tauri::command]
pub fn save_plot(state: tauri::State<'_, AppState>, session_token: String, input: PlotInput) -> Result<i64, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    save_plot_tx(&*state.db()?, &input)
}

#[tauri::command]
pub fn archive_plot(state: tauri::State<'_, AppState>, session_token: String, id: i64, restore: Option<bool>) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let conn = state.db()?;
    if !restore.unwrap_or(false) {
        let growing: i64 = conn.query_row("SELECT COUNT(*) FROM crop_cycles WHERE plot_id=?1 AND status='growing'", [id], |r| r.get(0)).map_err(|e| e.to_string())?;
        if growing > 0 { return Err("Close the crop growing on this plot first".into()); }
    }
    let n = conn.execute("UPDATE farm_plots SET status=?1 WHERE id=?2", params![if restore.unwrap_or(false) { "active" } else { "archived" }, id]).map_err(|e| e.to_string())?;
    if n == 0 { return Err("Plot not found".into()); }
    Ok(())
}

// ------------------------------------------------------------------ crop cycles

#[derive(Serialize, Default, Debug, Clone)]
pub struct HarvestTotal { pub unit: String, pub quantity: f64, pub damaged: f64, pub stored: f64, pub sold: f64 }

#[derive(Serialize, Default, Debug)]
pub struct CycleSummary {
    pub id: i64, pub plot_id: i64, pub plot_code: String, pub plot_name: String, pub area: f64, pub area_unit: String,
    pub crop: String, pub variety: String, pub planting_date: String, pub quantity_planted: f64, pub planting_unit: String, pub material_source: String,
    pub planting_cost_kobo: i64, pub method: String, pub expected_harvest_from: String, pub expected_harvest_to: String, pub purpose: String,
    pub status: String, pub closed_date: String, pub notes: String, pub is_demo: bool,
    pub days_since_planting: i64, pub activity_count: i64, pub last_activity_date: String, pub harvest_count: i64,
    pub activity_cost_kobo: i64, pub harvest_cost_kobo: i64, pub total_cost_kobo: i64, pub revenue_kobo: i64, pub margin_kobo: i64,
    pub harvests: Vec<HarvestTotal>,
    /// Main harvest unit per unit of land (first harvest unit), 0 when not known.
    pub yield_per_area: f64,
    pub loss_pct: f64,
}

pub fn cycle_summary(conn: &Connection, id: i64) -> Result<CycleSummary, String> {
    let mut c = conn.query_row("SELECT c.id,c.plot_id,p.code,p.name,CASE WHEN c.area_planted>0 THEN c.area_planted ELSE p.area END,p.area_unit,c.crop,c.variety,c.planting_date,c.quantity_planted,c.planting_unit,c.material_source,
        c.planting_cost_kobo,c.method,c.expected_harvest_from,c.expected_harvest_to,c.purpose,c.status,c.closed_date,c.notes,c.is_demo
        FROM crop_cycles c JOIN farm_plots p ON p.id=c.plot_id WHERE c.id=?1", [id], |r| Ok(CycleSummary {
        id: r.get(0)?, plot_id: r.get(1)?, plot_code: r.get(2)?, plot_name: r.get(3)?, area: r.get(4)?, area_unit: r.get(5)?, crop: r.get(6)?, variety: r.get(7)?,
        planting_date: r.get(8)?, quantity_planted: r.get(9)?, planting_unit: r.get(10)?, material_source: r.get(11)?, planting_cost_kobo: r.get(12)?, method: r.get(13)?,
        expected_harvest_from: r.get(14)?, expected_harvest_to: r.get(15)?, purpose: r.get(16)?, status: r.get(17)?, closed_date: r.get(18)?, notes: r.get(19)?,
        is_demo: r.get::<_, i64>(20)? == 1, ..Default::default()
    })).optional().map_err(|e| e.to_string())?.ok_or("Crop not found")?;
    let end = if c.status == "closed" { crate::farms::parse_date(&c.closed_date).unwrap_or_else(today) } else { today() };
    c.days_since_planting = crate::farms::parse_date(&c.planting_date).map(|d| (end - d).num_days().max(0)).unwrap_or(0);
    let (n, last, cost): (i64, String, i64) = conn.query_row("SELECT COUNT(*),COALESCE(MAX(activity_date),''),COALESCE(SUM(labour_kobo+other_cost_kobo+input_cost_kobo),0) FROM crop_activities WHERE cycle_id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(|e| e.to_string())?;
    c.activity_count = n;
    c.last_activity_date = last;
    c.activity_cost_kobo = cost;
    let (hn, hcost, revenue): (i64, i64, i64) = conn.query_row("SELECT COUNT(*),COALESCE(SUM(labour_kobo+transport_kobo+other_cost_kobo),0),COALESCE(SUM(sale_amount_kobo),0) FROM harvest_records WHERE cycle_id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(|e| e.to_string())?;
    c.harvest_count = hn;
    c.harvest_cost_kobo = hcost;
    c.revenue_kobo = revenue;
    c.total_cost_kobo = c.planting_cost_kobo + c.activity_cost_kobo + c.harvest_cost_kobo;
    c.margin_kobo = c.revenue_kobo - c.total_cost_kobo;
    c.harvests = rows(conn, "SELECT unit,SUM(quantity),SUM(damaged_qty),SUM(stored_qty),SUM(sold_qty),MIN(id) FROM harvest_records WHERE cycle_id=?1 GROUP BY lower(unit) ORDER BY MIN(id)", [id], |r| Ok(HarvestTotal {
        unit: r.get(0)?, quantity: r.get(1)?, damaged: r.get(2)?, stored: r.get(3)?, sold: r.get(4)?,
    }))?;
    if let Some(h) = c.harvests.first() {
        if c.area > 0.0 { c.yield_per_area = h.quantity / c.area; }
        // Loss is damaged produce as a share of everything harvested in that unit.
        if h.quantity > 0.0 { c.loss_pct = h.damaged * 100.0 / h.quantity; }
    }
    Ok(c)
}

/// Money fields are blanked for roles below Manager (workers, advisers, cashiers).
#[tauri::command]
pub fn list_crop_cycles(state: tauri::State<'_, AppState>, session_token: String, include_closed: Option<bool>, plot_id: Option<i64>) -> Result<serde_json::Value, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let out = list_crop_cycles_inner(state, session_token, include_closed, plot_id)?;
    crate::auth::money_view(&me, &out)
}

fn list_crop_cycles_inner(state: tauri::State<'_, AppState>, session_token: String, include_closed: Option<bool>, plot_id: Option<i64>) -> Result<Vec<CycleSummary>, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let conn = state.db()?;
    if let Some(ids) = crate::farm_access::allowed(&conn, &me)? {
        let list = ids.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
        let ids: Vec<i64> = rows(&conn, &format!("SELECT c.id FROM crop_cycles c JOIN farm_plots p ON p.id=c.plot_id WHERE (?1=1 OR c.status='growing') AND (?2 IS NULL OR c.plot_id=?2) AND (p.farm_id IS NULL OR p.farm_id IN ({list})) ORDER BY c.status DESC, c.planting_date DESC LIMIT 300"), params![include_closed.unwrap_or(false) as i64, plot_id], |r| r.get(0))?;
        return ids.into_iter().map(|id| cycle_summary(&conn, id)).collect();
    }
    let ids: Vec<i64> = rows(&conn, "SELECT id FROM crop_cycles WHERE (?1=1 OR status='growing') AND (?2 IS NULL OR plot_id=?2) ORDER BY status DESC, planting_date DESC LIMIT 300", params![include_closed.unwrap_or(false) as i64, plot_id], |r| r.get(0))?;
    ids.into_iter().map(|id| cycle_summary(&conn, id)).collect()
}

#[derive(Serialize)]
pub struct CropActivity {
    id: i64, activity_date: String, activity_type: String, description: String, area_done: f64, labour_kobo: i64, other_cost_kobo: i64,
    product_id: Option<i64>, product_name: String, input_qty: f64, input_unit: String, input_cost_kobo: i64, user_name: String,
}

#[derive(Serialize)]
pub struct Harvest {
    id: i64, harvest_date: String, quantity: f64, unit: String, grade: String, damaged_qty: f64, product_id: Option<i64>, product_name: String, stored_qty: f64,
    sold_qty: f64, sale_amount_kobo: i64, buyer: String, labour_kobo: i64, transport_kobo: i64, other_cost_kobo: i64, notes: String, user_name: String,
}

#[derive(Serialize)]
pub struct CycleDetail { cycle: CycleSummary, activities: Vec<CropActivity>, harvests: Vec<Harvest> }

pub fn cycle_detail(conn: &Connection, id: i64) -> Result<CycleDetail, String> {
    let cycle = cycle_summary(conn, id)?;
    let activities = rows(conn, "SELECT a.id,a.activity_date,a.activity_type,a.description,a.area_done,a.labour_kobo,a.other_cost_kobo,a.product_id,COALESCE(p.name,''),a.input_qty,COALESCE(p.unit,''),a.input_cost_kobo,a.user_name
        FROM crop_activities a LEFT JOIN products p ON p.id=a.product_id WHERE a.cycle_id=?1 ORDER BY a.activity_date DESC, a.id DESC", [id], |r| Ok(CropActivity {
        id: r.get(0)?, activity_date: r.get(1)?, activity_type: r.get(2)?, description: r.get(3)?, area_done: r.get(4)?, labour_kobo: r.get(5)?, other_cost_kobo: r.get(6)?,
        product_id: r.get(7)?, product_name: r.get(8)?, input_qty: r.get(9)?, input_unit: r.get(10)?, input_cost_kobo: r.get(11)?, user_name: r.get(12)?,
    }))?;
    let harvests = rows(conn, "SELECT h.id,h.harvest_date,h.quantity,h.unit,h.grade,h.damaged_qty,h.product_id,COALESCE(p.name,''),h.stored_qty,h.sold_qty,h.sale_amount_kobo,h.buyer,h.labour_kobo,h.transport_kobo,h.other_cost_kobo,h.notes,h.user_name
        FROM harvest_records h LEFT JOIN products p ON p.id=h.product_id WHERE h.cycle_id=?1 ORDER BY h.harvest_date DESC, h.id DESC", [id], |r| Ok(Harvest {
        id: r.get(0)?, harvest_date: r.get(1)?, quantity: r.get(2)?, unit: r.get(3)?, grade: r.get(4)?, damaged_qty: r.get(5)?, product_id: r.get(6)?, product_name: r.get(7)?,
        stored_qty: r.get(8)?, sold_qty: r.get(9)?, sale_amount_kobo: r.get(10)?, buyer: r.get(11)?, labour_kobo: r.get(12)?, transport_kobo: r.get(13)?, other_cost_kobo: r.get(14)?,
        notes: r.get(15)?, user_name: r.get(16)?,
    }))?;
    Ok(CycleDetail { cycle, activities, harvests })
}

/// Money fields are blanked for roles below Manager (workers, advisers, cashiers).
#[tauri::command]
pub fn get_crop_cycle(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<serde_json::Value, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let out = get_crop_cycle_inner(state, session_token, id)?;
    crate::auth::money_view(&me, &out)
}

fn get_crop_cycle_inner(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<CycleDetail, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    cycle_detail(&*state.db()?, id)
}

#[derive(Deserialize, Default)]
pub struct CycleInput {
    pub id: Option<i64>, pub plot_id: i64, pub crop: String, #[serde(default)] pub variety: String, pub planting_date: String, #[serde(default)] pub area_planted: f64,
    #[serde(default)] pub quantity_planted: f64, #[serde(default)] pub planting_unit: String, #[serde(default)] pub material_source: String, #[serde(default)] pub planting_cost_kobo: i64,
    #[serde(default)] pub method: String, #[serde(default)] pub expected_harvest_from: String, #[serde(default)] pub expected_harvest_to: String, #[serde(default)] pub purpose: String,
    #[serde(default)] pub notes: String,
}

pub fn save_cycle_tx(conn: &Connection, i: &CycleInput) -> Result<i64, String> {
    let crop = i.crop.trim();
    if crop.is_empty() { return Err("Choose the crop".into()); }
    if !valid_date(&i.planting_date) { return Err("Choose the planting date".into()); }
    check_qty(i.area_planted, "Area planted")?;
    check_qty(i.quantity_planted, "Quantity planted")?;
    check_money(i.planting_cost_kobo, "Planting cost")?;
    for (v, n, l) in [(&i.crop, 60, "Crop"), (&i.variety, 80, "Variety"), (&i.planting_unit, 30, "Unit"), (&i.material_source, 150, "Source"), (&i.method, 80, "Method"), (&i.purpose, 80, "Purpose"), (&i.notes, 1000, "Notes")] { check_len(v, n, l)?; }
    check_opt_date(&i.expected_harvest_from, "Expected harvest start")?;
    check_opt_date(&i.expected_harvest_to, "Expected harvest end")?;
    if !i.expected_harvest_from.is_empty() && i.expected_harvest_from < i.planting_date { return Err("Expected harvest cannot be before planting".into()); }
    if !i.expected_harvest_from.is_empty() && !i.expected_harvest_to.is_empty() && i.expected_harvest_to < i.expected_harvest_from { return Err("The harvest window ends before it starts".into()); }
    let plot: Option<String> = conn.query_row("SELECT status FROM farm_plots WHERE id=?1", [i.plot_id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    match plot.as_deref() { None => return Err("Plot not found".into()), Some("archived") if i.id.is_none() => return Err("This plot is archived".into()), _ => {} }
    let p = params![i.plot_id, crop, i.variety.trim(), i.planting_date, i.area_planted, i.quantity_planted, i.planting_unit.trim(), i.material_source.trim(), i.planting_cost_kobo, i.method.trim(), i.expected_harvest_from, i.expected_harvest_to, i.purpose.trim(), i.notes.trim()];
    match i.id {
        Some(id) => {
            let n = conn.execute("UPDATE crop_cycles SET plot_id=?1,crop=?2,variety=?3,planting_date=?4,area_planted=?5,quantity_planted=?6,planting_unit=?7,material_source=?8,planting_cost_kobo=?9,method=?10,expected_harvest_from=?11,expected_harvest_to=?12,purpose=?13,notes=?14 WHERE id=?15",
                params![i.plot_id, crop, i.variety.trim(), i.planting_date, i.area_planted, i.quantity_planted, i.planting_unit.trim(), i.material_source.trim(), i.planting_cost_kobo, i.method.trim(), i.expected_harvest_from, i.expected_harvest_to, i.purpose.trim(), i.notes.trim(), id]).map_err(|e| e.to_string())?;
            if n == 0 { return Err("Crop not found".into()); }
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO crop_cycles(plot_id,crop,variety,planting_date,area_planted,quantity_planted,planting_unit,material_source,planting_cost_kobo,method,expected_harvest_from,expected_harvest_to,purpose,notes) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)", p).map_err(|e| e.to_string())?;
            Ok(conn.last_insert_rowid())
        }
    }
}

#[tauri::command]
pub fn save_crop_cycle(state: tauri::State<'_, AppState>, session_token: String, input: CycleInput) -> Result<i64, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    save_cycle_tx(&*state.db()?, &input)
}

#[tauri::command]
pub fn close_crop_cycle(state: tauri::State<'_, AppState>, session_token: String, id: i64, closed_date: String, reopen: Option<bool>) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let conn = state.db()?;
    let n = if reopen.unwrap_or(false) {
        conn.execute("UPDATE crop_cycles SET status='growing',closed_date='' WHERE id=?1", [id])
    } else {
        if !valid_date(&closed_date) { return Err("Choose the closing date".into()); }
        conn.execute("UPDATE crop_cycles SET status='closed',closed_date=?1 WHERE id=?2", params![closed_date, id])
    }.map_err(|e| e.to_string())?;
    if n == 0 { return Err("Crop not found".into()); }
    Ok(())
}

fn require_growing(conn: &Connection, id: i64) -> Result<String, String> {
    let (status, crop, plot): (String, String, String) = conn.query_row("SELECT c.status,c.crop,p.code FROM crop_cycles c JOIN farm_plots p ON p.id=c.plot_id WHERE c.id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .optional().map_err(|e| e.to_string())?.ok_or("Crop not found")?;
    if status != "growing" { return Err("This crop is closed. Reopen it to add records.".into()); }
    Ok(format!("{crop} on {plot}"))
}

#[derive(Deserialize, Default)]
pub struct ActivityInput {
    pub cycle_id: i64, pub activity_date: String, pub activity_type: String, #[serde(default)] pub description: String, #[serde(default)] pub area_done: f64,
    #[serde(default)] pub labour_kobo: i64, #[serde(default)] pub other_cost_kobo: i64, #[serde(default)] pub product_id: Option<i64>, #[serde(default)] pub input_qty: f64,
}

/// Records a field activity. An input taken from stock is deducted (earliest
/// expiry first) and costed at its average cost, all in one transaction.
pub fn add_activity_tx(tx: &Connection, i: &ActivityInput, who: &str) -> Result<i64, String> {
    let label = require_growing(tx, i.cycle_id)?;
    if !valid_date(&i.activity_date) { return Err("Choose the date".into()); }
    if !ACTIVITY_TYPES.contains(&i.activity_type.as_str()) { return Err("Choose the type of activity".into()); }
    check_len(&i.description, 300, "Description")?;
    check_qty(i.area_done, "Area")?;
    check_money(i.labour_kobo, "Labour")?;
    check_money(i.other_cost_kobo, "Other cost")?;
    let mut input_cost = 0i64;
    if let Some(pid) = i.product_id {
        if !(i.input_qty.is_finite() && i.input_qty > 0.0) { return Err("Enter the quantity taken from stock".into()); }
        let (name, unit, cost, stock, track): (String, String, i64, f64, i64) = tx.query_row("SELECT name,unit,cost_price_kobo,stock_qty,track_stock FROM products WHERE id=?1", [pid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
            .optional().map_err(|e| e.to_string())?.ok_or("Stock item not found")?;
        if track == 1 && stock + 1e-9 < i.input_qty { return Err(format!("Not enough {name} in stock. Available: {} {unit}", crate::db::format_qty(stock))); }
        input_cost = ((cost as f64) * i.input_qty).round() as i64;
        if track == 1 {
            tx.execute("UPDATE products SET stock_qty=stock_qty-?1,updated_at=CURRENT_TIMESTAMP WHERE id=?2", params![i.input_qty, pid]).map_err(|e| e.to_string())?;
            tx.execute("INSERT INTO stock_movements(product_id,movement_type,quantity_delta,unit_cost_kobo,note,user_name) VALUES(?1,'crop_issue',?2,?3,?4,?5)",
                params![pid, -i.input_qty, cost, format!("Used on {label}"), who]).map_err(|e| e.to_string())?;
            crate::inventory::consume_batches(tx, pid, i.input_qty)?;
        }
    } else if i.input_qty != 0.0 {
        return Err("Choose the stock item used".into());
    }
    if i.labour_kobo == 0 && i.other_cost_kobo == 0 && i.product_id.is_none() && i.description.trim().is_empty() { return Err("Add a description, a cost or an input used".into()); }
    tx.execute("INSERT INTO crop_activities(cycle_id,activity_date,activity_type,description,area_done,labour_kobo,other_cost_kobo,product_id,input_qty,input_cost_kobo,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![i.cycle_id, i.activity_date, i.activity_type, i.description.trim(), i.area_done, i.labour_kobo, i.other_cost_kobo, i.product_id, i.input_qty.max(0.0), input_cost, who]).map_err(|e| e.to_string())?;
    Ok(tx.last_insert_rowid())
}

#[tauri::command]
pub fn add_crop_activity(state: tauri::State<'_, AppState>, session_token: String, input: ActivityInput) -> Result<CycleSummary, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    if (input.labour_kobo > 0 || input.other_cost_kobo > 0 || input.product_id.is_some()) && me.role < Role::Manager { return Err("Only managers can record costs and issue stock".into()); }
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    add_activity_tx(&tx, &input, &me.display_name())?;
    let s = cycle_summary(&tx, input.cycle_id)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(s)
}

#[derive(Deserialize, Default)]
pub struct HarvestInput {
    pub cycle_id: i64, pub harvest_date: String, pub quantity: f64, pub unit: String, #[serde(default)] pub grade: String, #[serde(default)] pub damaged_qty: f64,
    #[serde(default)] pub product_id: Option<i64>, #[serde(default)] pub stored_qty: f64, #[serde(default)] pub sold_qty: f64, #[serde(default)] pub sale_amount_kobo: i64,
    #[serde(default)] pub buyer: String, #[serde(default)] pub labour_kobo: i64, #[serde(default)] pub transport_kobo: i64, #[serde(default)] pub other_cost_kobo: i64, #[serde(default)] pub notes: String,
}

/// Records a harvest. Damaged + stored + sold may not exceed what was
/// harvested. Stored produce linked to a stock item is added to inventory.
pub fn add_harvest_tx(tx: &Connection, i: &HarvestInput, who: &str) -> Result<i64, String> {
    let label = require_growing(tx, i.cycle_id)?;
    if !valid_date(&i.harvest_date) { return Err("Choose the harvest date".into()); }
    if !(i.quantity.is_finite() && i.quantity > 0.0) { return Err("Enter the quantity harvested".into()); }
    if i.unit.trim().is_empty() { return Err("Choose the unit, such as kg, bag or bunch".into()); }
    check_len(&i.unit, 30, "Unit")?;
    for (v, l) in [(i.damaged_qty, "Damaged"), (i.stored_qty, "Stored"), (i.sold_qty, "Sold")] { check_qty(v, l)?; }
    for (v, l) in [(i.sale_amount_kobo, "Sale amount"), (i.labour_kobo, "Labour"), (i.transport_kobo, "Transport"), (i.other_cost_kobo, "Other cost")] { check_money(v, l)?; }
    check_len(&i.grade, 40, "Grade")?;
    check_len(&i.buyer, 120, "Buyer")?;
    check_len(&i.notes, 500, "Notes")?;
    if i.damaged_qty + i.stored_qty + i.sold_qty > i.quantity + 1e-9 { return Err("Damaged, stored and sold together cannot be more than the quantity harvested".into()); }
    if i.sale_amount_kobo > 0 && i.sold_qty <= 0.0 { return Err("Enter the quantity sold for this amount".into()); }
    if let Some(pid) = i.product_id {
        if i.stored_qty <= 0.0 { return Err("Enter the quantity put into stock".into()); }
        let unit: String = tx.query_row("SELECT unit FROM products WHERE id=?1", [pid], |r| r.get(0)).optional().map_err(|e| e.to_string())?.ok_or("Stock item not found")?;
        if !unit.eq_ignore_ascii_case(i.unit.trim()) { return Err(format!("The stock item is counted in {unit}. Record the harvest in {unit} too.")); }
        crate::inventory::receive_stock(tx, pid, i.stored_qty, 0, "harvest", &format!("Harvest from {label}"), who, None)?;
    }
    tx.execute("INSERT INTO harvest_records(cycle_id,harvest_date,quantity,unit,grade,damaged_qty,product_id,stored_qty,sold_qty,sale_amount_kobo,buyer,labour_kobo,transport_kobo,other_cost_kobo,notes,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
        params![i.cycle_id, i.harvest_date, i.quantity, i.unit.trim(), i.grade.trim(), i.damaged_qty, i.product_id, i.stored_qty, i.sold_qty, i.sale_amount_kobo, i.buyer.trim(), i.labour_kobo, i.transport_kobo, i.other_cost_kobo, i.notes.trim(), who]).map_err(|e| e.to_string())?;
    Ok(tx.last_insert_rowid())
}

#[tauri::command]
pub fn add_harvest(state: tauri::State<'_, AppState>, session_token: String, input: HarvestInput) -> Result<CycleSummary, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    add_harvest_tx(&tx, &input, &me.display_name())?;
    let s = cycle_summary(&tx, input.cycle_id)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(s)
}

#[tauri::command]
pub fn delete_crop_entry(state: tauri::State<'_, AppState>, session_token: String, kind: String, id: i64) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let table = match kind.as_str() { "activity" => "crop_activities", "harvest" => "harvest_records", _ => return Err("Unknown entry type".into()) };
    // Stock already issued or received is not reversed automatically; use a stock adjustment if needed.
    let n = state.db()?.execute(&format!("DELETE FROM {table} WHERE id=?1"), [id]).map_err(|e| e.to_string())?;
    if n == 0 { return Err("Entry not found".into()); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Connection, i64) {
        let conn = crate::db::test_conn();
        let plot = save_plot_tx(&conn, &PlotInput { code: "PLOT-A".into(), area: 2.0, area_unit: "hectare".into(), ..Default::default() }).unwrap();
        let cycle = save_cycle_tx(&conn, &CycleInput { plot_id: plot, crop: "Cassava".into(), planting_date: "2026-04-01".into(), planting_cost_kobo: 5_000_000, ..Default::default() }).unwrap();
        (conn, cycle)
    }

    #[test]
    fn activity_issues_stock_and_costs_it() {
        let (conn, cycle) = setup();
        conn.execute("INSERT INTO products(sku,name,kind,unit,cost_price_kobo,stock_qty,track_stock) VALUES('NPK','NPK fertiliser','material','bag',2_000_000,5,1)", []).unwrap();
        let pid = conn.last_insert_rowid();
        let base = ActivityInput { cycle_id: cycle, activity_date: "2026-05-01".into(), activity_type: "fertiliser".into(), product_id: Some(pid), input_qty: 2.0, labour_kobo: 300_000, ..Default::default() };
        add_activity_tx(&conn, &base, "M").unwrap();
        let stock: f64 = conn.query_row("SELECT stock_qty FROM products WHERE id=?1", [pid], |r| r.get(0)).unwrap();
        assert!((stock - 3.0).abs() < 1e-9);
        assert!(add_activity_tx(&conn, &ActivityInput { input_qty: 4.0, ..base }, "M").is_err(), "cannot issue more than in stock");
        let s = cycle_summary(&conn, cycle).unwrap();
        assert_eq!(s.activity_cost_kobo, 4_300_000);
        assert_eq!(s.total_cost_kobo, 9_300_000);
    }

    #[test]
    fn harvest_yield_loss_and_stock() {
        let (conn, cycle) = setup();
        conn.execute("INSERT INTO products(sku,name,kind,unit,stock_qty,track_stock) VALUES('CAS','Cassava tubers','product','bag',0,1)", []).unwrap();
        let pid = conn.last_insert_rowid();
        let h = HarvestInput { cycle_id: cycle, harvest_date: "2026-12-01".into(), quantity: 100.0, unit: "bag".into(), damaged_qty: 5.0, product_id: Some(pid), stored_qty: 60.0, sold_qty: 35.0, sale_amount_kobo: 7_000_000, transport_kobo: 1_000_000, ..Default::default() };
        assert!(add_harvest_tx(&conn, &HarvestInput { sold_qty: 40.0, ..h }, "W").is_err(), "more out than harvested");
        let h = HarvestInput { cycle_id: cycle, harvest_date: "2026-12-01".into(), quantity: 100.0, unit: "bag".into(), damaged_qty: 5.0, product_id: Some(pid), stored_qty: 60.0, sold_qty: 35.0, sale_amount_kobo: 7_000_000, transport_kobo: 1_000_000, ..Default::default() };
        add_harvest_tx(&conn, &h, "W").unwrap();
        let stock: f64 = conn.query_row("SELECT stock_qty FROM products WHERE id=?1", [pid], |r| r.get(0)).unwrap();
        assert!((stock - 60.0).abs() < 1e-9);
        let s = cycle_summary(&conn, cycle).unwrap();
        assert!((s.yield_per_area - 50.0).abs() < 1e-9, "100 bags on 2 ha");
        assert!((s.loss_pct - 5.0).abs() < 1e-9);
        assert_eq!(s.revenue_kobo, 7_000_000);
        assert_eq!(s.total_cost_kobo, 6_000_000);
        assert_eq!(s.margin_kobo, 1_000_000);
    }

    #[test]
    fn closed_crop_takes_no_records() {
        let (conn, cycle) = setup();
        conn.execute("UPDATE crop_cycles SET status='closed',closed_date='2026-12-31' WHERE id=?1", [cycle]).unwrap();
        let a = ActivityInput { cycle_id: cycle, activity_date: "2026-05-01".into(), activity_type: "weeding".into(), description: "x".into(), ..Default::default() };
        assert!(add_activity_tx(&conn, &a, "M").is_err());
    }
}
