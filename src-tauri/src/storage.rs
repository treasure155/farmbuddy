//! Storage locations: farm stores, warehouses, cold rooms, oil drums, grain
//! stores. A location holds part of the business stock (products.stock_qty);
//! putting stock away or issuing it never changes the business total, a
//! transfer moves it between locations, and spoilage, damage or loss reduce
//! both the location and the business stock in one transaction.

use crate::{auth::{self, Role}, farms::{check_len, rows, valid_date}, AppState};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

pub const KINDS: [&str; 9] = ["store", "warehouse", "cold_room", "fish_pond", "palm_oil_storage", "grain_store", "processing_store", "retail_shop", "other"];

#[derive(Serialize, Debug)]
pub struct Warehouse { id: i64, farm_id: Option<i64>, name: String, kind: String, notes: String, status: String, is_demo: bool, items: i64, value_kobo: i64 }

pub fn list_warehouses_tx(conn: &Connection, include_archived: bool) -> Result<Vec<Warehouse>, String> {
    rows(conn, "SELECT w.id,w.farm_id,w.name,w.kind,w.notes,w.status,w.is_demo,
        (SELECT COUNT(*) FROM stock_locations l WHERE l.warehouse_id=w.id AND l.quantity>0),
        (SELECT COALESCE(SUM(CAST(ROUND(l.quantity*p.cost_price_kobo) AS INTEGER)),0) FROM stock_locations l JOIN products p ON p.id=l.product_id WHERE l.warehouse_id=w.id)
        FROM warehouses w WHERE ?1=1 OR w.status='active' ORDER BY w.status, w.name", [include_archived as i64], |r| Ok(Warehouse {
        id: r.get(0)?, farm_id: r.get(1)?, name: r.get(2)?, kind: r.get(3)?, notes: r.get(4)?, status: r.get(5)?, is_demo: r.get::<_, i64>(6)? == 1, items: r.get(7)?, value_kobo: r.get(8)?,
    }))
}

/// Money fields are blanked for roles below Manager (workers, advisers, cashiers).
#[tauri::command]
pub fn list_warehouses(state: tauri::State<'_, AppState>, session_token: String, include_archived: Option<bool>) -> Result<serde_json::Value, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let out = list_warehouses_inner(state, session_token, include_archived)?;
    crate::auth::money_view(&me, &out)
}

fn list_warehouses_inner(state: tauri::State<'_, AppState>, session_token: String, include_archived: Option<bool>) -> Result<Vec<Warehouse>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    list_warehouses_tx(&*state.db()?, include_archived.unwrap_or(false))
}

#[derive(Deserialize, Default)]
pub struct WarehouseInput { pub id: Option<i64>, pub farm_id: Option<i64>, pub name: String, pub kind: String, #[serde(default)] pub notes: String }

pub fn save_warehouse_tx(conn: &Connection, i: &WarehouseInput) -> Result<i64, String> {
    let name = i.name.trim();
    if name.is_empty() { return Err("Name the storage location".into()); }
    check_len(name, 80, "Name")?;
    if !KINDS.contains(&i.kind.as_str()) { return Err("Choose the type of store".into()); }
    check_len(&i.notes, 500, "Notes")?;
    let dup: Option<i64> = conn.query_row("SELECT id FROM warehouses WHERE name=?1 AND id<>COALESCE(?2,-1)", params![name, i.id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    if dup.is_some() { return Err("Another store already has that name".into()); }
    match i.id {
        Some(id) => { conn.execute("UPDATE warehouses SET farm_id=?1,name=?2,kind=?3,notes=?4 WHERE id=?5", params![i.farm_id, name, i.kind, i.notes.trim(), id]).map_err(|e| e.to_string())?; Ok(id) }
        None => { conn.execute("INSERT INTO warehouses(farm_id,name,kind,notes) VALUES(?1,?2,?3,?4)", params![i.farm_id, name, i.kind, i.notes.trim()]).map_err(|e| e.to_string())?; Ok(conn.last_insert_rowid()) }
    }
}

#[tauri::command]
pub fn save_warehouse(state: tauri::State<'_, AppState>, session_token: String, input: WarehouseInput) -> Result<i64, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    save_warehouse_tx(&*state.db()?, &input)
}

#[tauri::command]
pub fn archive_warehouse(state: tauri::State<'_, AppState>, session_token: String, id: i64, restore: Option<bool>) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let conn = state.db()?;
    if !restore.unwrap_or(false) {
        let held: f64 = conn.query_row("SELECT COALESCE(SUM(quantity),0) FROM stock_locations WHERE warehouse_id=?1", [id], |r| r.get(0)).map_err(|e| e.to_string())?;
        if held > 1e-9 { return Err("Move or issue the stock in this store first".into()); }
    }
    conn.execute("UPDATE warehouses SET status=?1 WHERE id=?2", params![if restore.unwrap_or(false) { "active" } else { "archived" }, id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Serialize, Debug)]
pub struct LocatedStock { pub product_id: i64, pub product_name: String, pub unit: String, pub quantity: f64, pub business_stock: f64, pub unallocated: f64, pub value_kobo: i64 }

/// Stock held in one location, with each item's business total and the part not yet in any store.
/// `unallocated` below zero means more was sold or used than was issued from stores.
pub fn location_stock_tx(conn: &Connection, warehouse_id: i64) -> Result<Vec<LocatedStock>, String> {
    rows(conn, "SELECT p.id,p.name,p.unit,l.quantity,p.stock_qty,p.stock_qty-(SELECT COALESCE(SUM(x.quantity),0) FROM stock_locations x WHERE x.product_id=p.id),CAST(ROUND(l.quantity*p.cost_price_kobo) AS INTEGER)
        FROM stock_locations l JOIN products p ON p.id=l.product_id WHERE l.warehouse_id=?1 AND l.quantity>0 ORDER BY p.name", [warehouse_id], |r| Ok(LocatedStock {
        product_id: r.get(0)?, product_name: r.get(1)?, unit: r.get(2)?, quantity: r.get(3)?, business_stock: r.get(4)?, unallocated: r.get(5)?, value_kobo: r.get(6)?,
    }))
}

/// Money fields are blanked for roles below Manager (workers, advisers, cashiers).
#[tauri::command]
pub fn location_stock(state: tauri::State<'_, AppState>, session_token: String, warehouse_id: i64) -> Result<serde_json::Value, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let out = location_stock_inner(state, session_token, warehouse_id)?;
    crate::auth::money_view(&me, &out)
}

fn location_stock_inner(state: tauri::State<'_, AppState>, session_token: String, warehouse_id: i64) -> Result<Vec<LocatedStock>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    location_stock_tx(&*state.db()?, warehouse_id)
}

fn at_location(conn: &Connection, w: i64, p: i64) -> Result<f64, String> {
    Ok(conn.query_row("SELECT quantity FROM stock_locations WHERE warehouse_id=?1 AND product_id=?2", params![w, p], |r| r.get(0)).optional().map_err(|e| e.to_string())?.unwrap_or(0.0))
}
fn unallocated(conn: &Connection, p: i64) -> Result<f64, String> {
    conn.query_row("SELECT stock_qty-(SELECT COALESCE(SUM(quantity),0) FROM stock_locations WHERE product_id=?1) FROM products WHERE id=?1", [p], |r| r.get(0)).optional().map_err(|e| e.to_string())?.ok_or_else(|| "Stock item not found".to_string())
}
fn add_at(conn: &Connection, w: i64, p: i64, delta: f64) -> Result<(), String> {
    conn.execute("INSERT INTO stock_locations(warehouse_id,product_id,quantity) VALUES(?1,?2,MAX(0,?3)) ON CONFLICT(warehouse_id,product_id) DO UPDATE SET quantity=MAX(0,quantity+?3)", params![w, p, delta]).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Deserialize, Default)]
pub struct StorageAction {
    pub warehouse_id: i64, pub product_id: i64, pub kind: String, pub date: String, #[serde(default)] pub quantity: f64,
    #[serde(default)] pub to_warehouse_id: Option<i64>, #[serde(default)] pub grade: String, #[serde(default)] pub moisture_pct: Option<f64>, #[serde(default)] pub cost_kobo: i64, #[serde(default)] pub note: String,
}

/// put_away | issue | transfer | spoilage | damage | loss | count | inspection
pub fn action_tx(conn: &Connection, a: &StorageAction, who: &str) -> Result<(), String> {
    let (status, wname): (String, String) = conn.query_row("SELECT status,name FROM warehouses WHERE id=?1", [a.warehouse_id], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(|e| e.to_string())?.ok_or("Store not found")?;
    if status != "active" { return Err("This store is archived".into()); }
    if !valid_date(&a.date) { return Err("Choose the date".into()); }
    check_len(&a.note, 300, "Note")?;
    check_len(&a.grade, 40, "Grade")?;
    if a.cost_kobo < 0 { return Err("Cost cannot be negative".into()); }
    if let Some(m) = a.moisture_pct { if !(m.is_finite() && (0.0..=100.0).contains(&m)) { return Err("Moisture must be between 0 and 100%".into()); } }
    let (pname, unit, cost, track): (String, String, i64, i64) = conn.query_row("SELECT name,unit,cost_price_kobo,track_stock FROM products WHERE id=?1", [a.product_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional().map_err(|e| e.to_string())?.ok_or("Stock item not found")?;
    if track != 1 { return Err(format!("{pname} is not a stocked item")); }
    let here = at_location(conn, a.warehouse_id, a.product_id)?;
    let q = a.quantity;
    let need_qty = |q: f64| if q.is_finite() && q > 0.0 { Ok(()) } else { Err("Enter the quantity".to_string()) };
    let log = |kind: &str, qty: f64, other: Option<i64>| conn.execute("INSERT INTO storage_movements(warehouse_id,product_id,move_date,kind,quantity,other_warehouse_id,grade,moisture_pct,cost_kobo,note,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![a.warehouse_id, a.product_id, a.date, kind, qty, other, a.grade.trim(), a.moisture_pct, a.cost_kobo, a.note.trim(), who]).map(|_| ()).map_err(|e| e.to_string());
    match a.kind.as_str() {
        "put_away" => {
            need_qty(q)?;
            let free = unallocated(conn, a.product_id)?;
            if q > free + 1e-9 { return Err(format!("Only {} {unit} of {pname} is not already in a store. Receive more stock in Inventory or Purchases first.", crate::db::format_qty(free.max(0.0)))); }
            add_at(conn, a.warehouse_id, a.product_id, q)?;
            log("put_away", q, None)?;
        }
        "issue" => {
            need_qty(q)?;
            if q > here + 1e-9 { return Err(format!("Only {} {unit} of {pname} is in {wname}", crate::db::format_qty(here))); }
            add_at(conn, a.warehouse_id, a.product_id, -q)?;
            log("issue", q, None)?;
        }
        "transfer" => {
            need_qty(q)?;
            let to = a.to_warehouse_id.ok_or("Choose the store to move it to")?;
            if to == a.warehouse_id { return Err("Choose a different store".into()); }
            let ok: Option<String> = conn.query_row("SELECT status FROM warehouses WHERE id=?1", [to], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
            if ok.as_deref() != Some("active") { return Err("The destination store is not active".into()); }
            if q > here + 1e-9 { return Err(format!("Only {} {unit} of {pname} is in {wname}", crate::db::format_qty(here))); }
            add_at(conn, a.warehouse_id, a.product_id, -q)?;
            add_at(conn, to, a.product_id, q)?;
            log("transfer_out", q, Some(to))?;
            conn.execute("INSERT INTO storage_movements(warehouse_id,product_id,move_date,kind,quantity,other_warehouse_id,note,user_name) VALUES(?1,?2,?3,'transfer_in',?4,?5,?6,?7)",
                params![to, a.product_id, a.date, q, a.warehouse_id, a.note.trim(), who]).map_err(|e| e.to_string())?;
        }
        "spoilage" | "damage" | "loss" => {
            need_qty(q)?;
            if q > here + 1e-9 { return Err(format!("Only {} {unit} of {pname} is in {wname}", crate::db::format_qty(here))); }
            add_at(conn, a.warehouse_id, a.product_id, -q)?;
            // The business loses this stock too, at its average cost.
            conn.execute("UPDATE products SET stock_qty=stock_qty-?1,updated_at=CURRENT_TIMESTAMP WHERE id=?2", params![q, a.product_id]).map_err(|e| e.to_string())?;
            conn.execute("INSERT INTO stock_movements(product_id,movement_type,quantity_delta,unit_cost_kobo,note,user_name) VALUES(?1,'stock_out',?2,?3,?4,?5)",
                params![a.product_id, -q, cost, format!("{} in {wname}{}", a.kind, if a.note.trim().is_empty() { String::new() } else { format!(": {}", a.note.trim()) }), who]).map_err(|e| e.to_string())?;
            crate::inventory::consume_batches(conn, a.product_id, q)?;
            log(&a.kind, q, None)?;
        }
        "count" => {
            if !(q.is_finite() && q >= 0.0) { return Err("Enter the quantity counted".into()); }
            let diff = q - here;
            if diff > 1e-9 {
                let free = unallocated(conn, a.product_id)?;
                if diff > free + 1e-9 { return Err(format!("The count is {} {unit} more than the business stock allows. Record the stock received first.", crate::db::format_qty(diff - free.max(0.0)))); }
                add_at(conn, a.warehouse_id, a.product_id, diff)?;
            } else if diff < -1e-9 {
                // Missing stock is a loss to the business.
                add_at(conn, a.warehouse_id, a.product_id, diff)?;
                conn.execute("UPDATE products SET stock_qty=stock_qty+?1,updated_at=CURRENT_TIMESTAMP WHERE id=?2", params![diff, a.product_id]).map_err(|e| e.to_string())?;
                conn.execute("INSERT INTO stock_movements(product_id,movement_type,quantity_delta,unit_cost_kobo,note,user_name) VALUES(?1,'adjustment',?2,?3,?4,?5)", params![a.product_id, diff, cost, format!("Store count in {wname}"), who]).map_err(|e| e.to_string())?;
                crate::inventory::consume_batches(conn, a.product_id, -diff)?;
            }
            log("count", q, None)?;
        }
        "inspection" => {
            if a.grade.trim().is_empty() && a.moisture_pct.is_none() && a.note.trim().is_empty() { return Err("Enter the grade, the moisture reading or a note".into()); }
            log("inspection", here, None)?;
        }
        _ => return Err("Unknown storage action".into()),
    }
    Ok(())
}

#[tauri::command]
pub fn storage_action(state: tauri::State<'_, AppState>, session_token: String, input: StorageAction) -> Result<(), String> {
    let me = auth::require(&state, &session_token, if ["spoilage", "damage", "loss", "count"].contains(&input.kind.as_str()) { Role::Manager } else { Role::Worker })?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    action_tx(&tx, &input, &me.display_name())?;
    if ["spoilage", "damage", "loss", "count"].contains(&input.kind.as_str()) { crate::audit::log(&tx, &me.display_name(), &format!("storage_{}", input.kind), "warehouse", Some(input.warehouse_id), &format!("product {} qty {}", input.product_id, input.quantity)); }
    tx.commit().map_err(|e| e.to_string())
}

#[derive(Serialize, Debug)]
pub struct StorageMove { id: i64, move_date: String, kind: String, product_name: String, unit: String, quantity: f64, other_store: String, grade: String, moisture_pct: Option<f64>, cost_kobo: i64, note: String, user_name: String }

pub fn storage_history_tx(conn: &Connection, warehouse_id: i64) -> Result<Vec<StorageMove>, String> {
    rows(conn, "SELECT m.id,m.move_date,m.kind,p.name,p.unit,m.quantity,COALESCE(o.name,''),m.grade,m.moisture_pct,m.cost_kobo,m.note,m.user_name FROM storage_movements m JOIN products p ON p.id=m.product_id LEFT JOIN warehouses o ON o.id=m.other_warehouse_id
        WHERE m.warehouse_id=?1 ORDER BY m.move_date DESC, m.id DESC LIMIT 500", [warehouse_id], |r| Ok(StorageMove {
        id: r.get(0)?, move_date: r.get(1)?, kind: r.get(2)?, product_name: r.get(3)?, unit: r.get(4)?, quantity: r.get(5)?, other_store: r.get(6)?, grade: r.get(7)?, moisture_pct: r.get(8)?, cost_kobo: r.get(9)?, note: r.get(10)?, user_name: r.get(11)?,
    }))
}

#[tauri::command]
pub fn storage_history(state: tauri::State<'_, AppState>, session_token: String, warehouse_id: i64) -> Result<Vec<StorageMove>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    let conn = state.db()?;
    storage_history_tx(&conn, warehouse_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locations_never_exceed_business_stock_and_transfers_keep_totals() {
        let conn = crate::db::test_conn();
        conn.execute("INSERT INTO products(sku,name,kind,unit,cost_price_kobo,stock_qty,track_stock) VALUES('GA','Garri','product','kg',50_000,100,1)", []).unwrap();
        let a = save_warehouse_tx(&conn, &WarehouseInput { name: "Main store".into(), kind: "store".into(), ..Default::default() }).unwrap();
        let b = save_warehouse_tx(&conn, &WarehouseInput { name: "Shop".into(), kind: "retail_shop".into(), ..Default::default() }).unwrap();
        let act = |w, kind: &str, q: f64, to: Option<i64>| action_tx(&conn, &StorageAction { warehouse_id: w, product_id: 1, kind: kind.into(), date: "2026-05-01".into(), quantity: q, to_warehouse_id: to, ..Default::default() }, "M");
        act(a, "put_away", 80.0, None).unwrap();
        assert!(act(b, "put_away", 30.0, None).is_err(), "only 20 left unallocated");
        act(a, "transfer", 30.0, Some(b)).unwrap();
        assert!(act(a, "transfer", 60.0, Some(b)).is_err());
        act(b, "spoilage", 5.0, None).unwrap();
        act(a, "count", 45.0, None).unwrap();
        let stock: f64 = conn.query_row("SELECT stock_qty FROM products WHERE id=1", [], |r| r.get(0)).unwrap();
        assert!((stock - 90.0).abs() < 1e-9, "100 − 5 spoilt − 5 missing at count");
        let (sa, sb) = (location_stock_tx(&conn, a).unwrap(), location_stock_tx(&conn, b).unwrap());
        assert!((sa[0].quantity - 45.0).abs() < 1e-9 && (sb[0].quantity - 25.0).abs() < 1e-9);
        assert!((sa[0].unallocated - 20.0).abs() < 1e-9);
        assert!(archive_warehouse_check(&conn, a));
    }

    fn archive_warehouse_check(conn: &Connection, id: i64) -> bool {
        let held: f64 = conn.query_row("SELECT COALESCE(SUM(quantity),0) FROM stock_locations WHERE warehouse_id=?1", [id], |r| r.get(0)).unwrap();
        held > 0.0
    }
}
