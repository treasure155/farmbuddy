//! Items, categories, payment methods, stock movements, expiry batches,
//! recipes and barcodes.

use crate::{auth::{self, Role}, db, AppState};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

pub const KINDS: [&str; 3] = ["product", "service", "material"];

#[derive(Serialize, Clone)]
pub struct Product {
    id: i64,
    sku: String,
    barcode: String,
    name: String,
    category_id: Option<i64>,
    category_name: String,
    kind: String,
    unit: String,
    selling_price_kobo: i64,
    /// Hidden (0) for cashiers.
    cost_price_kobo: i64,
    track_stock: bool,
    stock_qty: f64,
    reorder_level: f64,
    description: String,
    active: bool,
    pack_unit: String,
    pack_size: f64,
    pack_price_kobo: i64,
    wholesale_price_kobo: i64,
    wholesale_min_qty: f64,
    track_expiry: bool,
    has_recipe: bool,
    /// Cost of one unit from its recipe (managers only), 0 when no recipe.
    recipe_cost_kobo: i64,
}

#[tauri::command]
pub fn list_products(state: tauri::State<'_, AppState>, session_token: String, include_inactive: Option<bool>) -> Result<Vec<Product>, String> {
    let me = auth::require(&state, &session_token, Role::Cashier)?;
    let conn = state.db()?;
    let mut stmt = conn
        .prepare(
            "SELECT p.id,p.sku,p.barcode,p.name,p.category_id,COALESCE(c.name,''),p.kind,p.unit,p.selling_price_kobo,p.cost_price_kobo,
                    p.track_stock,p.stock_qty,p.reorder_level,p.description,p.active,
                    p.pack_unit,p.pack_size,p.pack_price_kobo,p.wholesale_price_kobo,p.wholesale_min_qty,p.track_expiry,
                    (SELECT COUNT(*) FROM recipe_items ri WHERE ri.product_id=p.id),
                    (SELECT COALESCE(SUM(ROUND(ri.quantity*m.cost_price_kobo)),0) FROM recipe_items ri JOIN products m ON m.id=ri.material_id WHERE ri.product_id=p.id)
             FROM products p LEFT JOIN categories c ON c.id=p.category_id
             WHERE (?1=1 OR p.active=1)
             ORDER BY COALESCE(c.sort_order,9999), c.name COLLATE NOCASE, p.name COLLATE NOCASE",
        )
        .map_err(|e| e.to_string())?;
    let show_cost = me.role >= Role::Manager;
    let rows = stmt
        .query_map([include_inactive.unwrap_or(false) && show_cost], |r| {
            Ok(Product {
                id: r.get(0)?, sku: r.get(1)?, barcode: r.get(2)?, name: r.get(3)?, category_id: r.get(4)?, category_name: r.get(5)?,
                kind: r.get(6)?, unit: r.get(7)?, selling_price_kobo: r.get(8)?, cost_price_kobo: if show_cost { r.get(9)? } else { 0 },
                track_stock: r.get::<_, i64>(10)? == 1, stock_qty: r.get(11)?, reorder_level: r.get(12)?, description: r.get(13)?, active: r.get::<_, i64>(14)? == 1,
                pack_unit: r.get(15)?, pack_size: r.get(16)?, pack_price_kobo: r.get(17)?, wholesale_price_kobo: r.get(18)?, wholesale_min_qty: r.get(19)?,
                track_expiry: r.get::<_, i64>(20)? == 1, has_recipe: r.get::<_, i64>(21)? > 0, recipe_cost_kobo: if show_cost { r.get::<_, f64>(22)? as i64 } else { 0 },
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct ProductInput {
    pub id: Option<i64>,
    pub sku: String,
    pub barcode: String,
    pub name: String,
    pub category_id: Option<i64>,
    pub kind: String,
    pub unit: String,
    pub selling_price_kobo: i64,
    pub cost_price_kobo: i64,
    pub track_stock: bool,
    /// Opening stock for new items only; use stock adjustments afterwards.
    pub stock_qty: f64,
    pub reorder_level: f64,
    pub description: String,
    pub active: bool,
    #[serde(default)] pub pack_unit: String,
    #[serde(default)] pub pack_size: f64,
    #[serde(default)] pub pack_price_kobo: i64,
    #[serde(default)] pub wholesale_price_kobo: i64,
    #[serde(default)] pub wholesale_min_qty: f64,
    #[serde(default)] pub track_expiry: bool,
}

pub fn save_product_tx(conn: &Connection, input: &ProductInput, user: &str) -> Result<i64, String> {
    let sku = input.sku.trim();
    let name = input.name.trim();
    let unit = input.unit.trim();
    let barcode = input.barcode.trim();
    let pack_unit = input.pack_unit.trim();
    if sku.is_empty() || name.is_empty() || unit.is_empty() {
        return Err("Code (SKU), name and unit are required".into());
    }
    if sku.len() > 40 || name.chars().count() > 120 || unit.chars().count() > 24 || pack_unit.chars().count() > 24 || barcode.len() > 64 || input.description.chars().count() > 500 {
        return Err("One of the fields is too long".into());
    }
    if !KINDS.contains(&input.kind.as_str()) {
        return Err("Type must be product, service or material".into());
    }
    let nums_ok = [input.stock_qty, input.reorder_level, input.pack_size, input.wholesale_min_qty].iter().all(|v| v.is_finite() && *v >= 0.0);
    if input.selling_price_kobo < 0 || input.cost_price_kobo < 0 || input.pack_price_kobo < 0 || input.wholesale_price_kobo < 0 || !nums_ok {
        return Err("Prices and stock values must be zero or more".into());
    }
    if input.kind != "material" && input.selling_price_kobo == 0 {
        return Err("Set a selling price for items you sell".into());
    }
    // A pack needs a name and more than one unit inside it.
    let (pack_unit, pack_size, pack_price) = if pack_unit.is_empty() || input.pack_size <= 1.0 { (String::new(), 0.0, 0) } else { (pack_unit.to_string(), input.pack_size, input.pack_price_kobo) };
    let (wholesale_price, wholesale_min) = if input.wholesale_price_kobo > 0 && input.wholesale_min_qty > 0.0 { (input.wholesale_price_kobo, input.wholesale_min_qty) } else { (0, 0.0) };
    // Services never carry stock.
    let track = input.track_stock && input.kind != "service";
    let track_expiry = track && input.track_expiry;
    if let Some(cid) = input.category_id {
        let ok: Option<i64> = conn.query_row("SELECT id FROM categories WHERE id=?1", [cid], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
        if ok.is_none() { return Err("Selected category no longer exists".into()); }
    }
    let map_unique = |e: rusqlite::Error| {
        let s = e.to_string();
        if s.contains("products.sku") { "Another item already uses that code (SKU)".to_string() }
        else if s.contains("barcode") { "Another item already uses that barcode".to_string() }
        else { s }
    };
    match input.id {
        Some(id) => {
            conn.execute(
                "UPDATE products SET sku=?1,barcode=?2,name=?3,category_id=?4,kind=?5,unit=?6,selling_price_kobo=?7,cost_price_kobo=?8,
                 track_stock=?9,reorder_level=?10,description=?11,active=?12,pack_unit=?13,pack_size=?14,pack_price_kobo=?15,
                 wholesale_price_kobo=?16,wholesale_min_qty=?17,track_expiry=?18,updated_at=CURRENT_TIMESTAMP WHERE id=?19",
                params![sku, barcode, name, input.category_id, input.kind, unit, input.selling_price_kobo, input.cost_price_kobo, track as i64, input.reorder_level, input.description.trim(), input.active as i64,
                        pack_unit, pack_size, pack_price, wholesale_price, wholesale_min, track_expiry as i64, id],
            )
            .map_err(map_unique)?;
            Ok(id)
        }
        None => {
            let opening = if track { input.stock_qty } else { 0.0 };
            conn.execute(
                "INSERT INTO products(sku,barcode,name,category_id,kind,unit,selling_price_kobo,cost_price_kobo,track_stock,stock_qty,reorder_level,description,active,
                                      pack_unit,pack_size,pack_price_kobo,wholesale_price_kobo,wholesale_min_qty,track_expiry)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
                params![sku, barcode, name, input.category_id, input.kind, unit, input.selling_price_kobo, input.cost_price_kobo, track as i64, opening, input.reorder_level, input.description.trim(), input.active as i64,
                        pack_unit, pack_size, pack_price, wholesale_price, wholesale_min, track_expiry as i64],
            )
            .map_err(map_unique)?;
            let id = conn.last_insert_rowid();
            if opening > 0.0 {
                conn.execute(
                    "INSERT INTO stock_movements(product_id,movement_type,quantity_delta,unit_cost_kobo,note,user_name) VALUES(?1,'opening',?2,?3,'Opening stock',?4)",
                    params![id, opening, input.cost_price_kobo, user],
                )
                .map_err(|e| e.to_string())?;
            }
            Ok(id)
        }
    }
}

#[tauri::command]
pub fn save_product(state: tauri::State<'_, AppState>, session_token: String, input: ProductInput) -> Result<i64, String> {
    let me = auth::require(&state, &session_token, Role::Manager)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let id = save_product_tx(&tx, &input, &me.display_name())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(id)
}

/// Deletes an item that has never been sold; otherwise archives it so receipt
/// history stays intact.
#[tauri::command]
pub fn remove_product(state: tauri::State<'_, AppState>, session_token: String, product_id: i64) -> Result<String, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let conn = state.db()?;
    let used: i64 = conn.query_row(
        "SELECT (SELECT COUNT(*) FROM receipt_items WHERE product_id=?1)+(SELECT COUNT(*) FROM purchase_items WHERE product_id=?1)+(SELECT COUNT(*) FROM recipe_items WHERE material_id=?1)",
        [product_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    if used > 0 {
        conn.execute("UPDATE products SET active=0,updated_at=CURRENT_TIMESTAMP WHERE id=?1", [product_id]).map_err(|e| e.to_string())?;
        Ok("archived".into())
    } else {
        conn.execute("DELETE FROM products WHERE id=?1", [product_id]).map_err(|e| e.to_string())?;
        Ok("deleted".into())
    }
}

// ------------------------------------------------------------------ stock helpers shared by purchases, sales and returns

/// Adds stock, updates the weighted average cost and, when given an expiry
/// date, records a batch. `unit_cost` is per base unit; 0 keeps the current cost.
#[allow(clippy::too_many_arguments)]
pub fn receive_stock(conn: &Connection, product_id: i64, base_qty: f64, unit_cost: i64, movement: &str, note: &str, user: &str, batch: Option<(&str, &str)>) -> Result<f64, String> {
    let (current, cost): (f64, i64) = conn.query_row("SELECT stock_qty,cost_price_kobo FROM products WHERE id=?1", [product_id], |r| Ok((r.get(0)?, r.get(1)?))).map_err(|e| e.to_string())?;
    if unit_cost > 0 {
        // Weighted average cost keeps profit reports honest when buying prices change.
        let base = current.max(0.0);
        let avg = if base + base_qty > 0.0 { ((base * cost as f64) + (base_qty * unit_cost as f64)) / (base + base_qty) } else { unit_cost as f64 };
        conn.execute("UPDATE products SET cost_price_kobo=?1 WHERE id=?2", params![avg.round() as i64, product_id]).map_err(|e| e.to_string())?;
    }
    conn.execute("UPDATE products SET stock_qty=stock_qty+?1,updated_at=CURRENT_TIMESTAMP WHERE id=?2", params![base_qty, product_id]).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO stock_movements(product_id,movement_type,quantity_delta,unit_cost_kobo,note,user_name) VALUES(?1,?2,?3,?4,?5,?6)",
        params![product_id, movement, base_qty, if unit_cost > 0 { unit_cost } else { cost }, note, user]).map_err(|e| e.to_string())?;
    if let Some((batch_no, expiry)) = batch {
        if !expiry.is_empty() {
            if chrono::NaiveDate::parse_from_str(expiry, "%Y-%m-%d").is_err() { return Err("Expiry date is not valid".into()); }
            conn.execute("INSERT INTO stock_batches(product_id,batch_no,expiry_date,quantity_received,quantity_left) VALUES(?1,?2,?3,?4,?4)", params![product_id, batch_no.trim(), expiry, base_qty]).map_err(|e| e.to_string())?;
        }
    }
    Ok(current + base_qty)
}

/// Takes stock out of expiry batches, earliest expiry first. Stock that was
/// never recorded in a batch is simply not tracked by date.
pub fn consume_batches(conn: &Connection, product_id: i64, mut qty: f64) -> Result<(), String> {
    let mut stmt = conn.prepare("SELECT id,quantity_left FROM stock_batches WHERE product_id=?1 AND quantity_left>0 ORDER BY expiry_date, id").map_err(|e| e.to_string())?;
    let batches = stmt.query_map([product_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, f64>(1)?))).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    drop(stmt);
    for (id, left) in batches {
        if qty <= 1e-9 { break; }
        let take = left.min(qty);
        conn.execute("UPDATE stock_batches SET quantity_left=MAX(0,quantity_left-?1) WHERE id=?2", params![take, id]).map_err(|e| e.to_string())?;
        qty -= take;
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct StockAdjustInput {
    pub product_id: i64,
    /// stock_in | stock_out | count
    pub mode: String,
    pub quantity: f64,
    pub unit_cost_kobo: Option<i64>,
    pub note: String,
    /// Receive whole packs (cartons); quantity and cost are then per pack.
    #[serde(default)] pub as_pack: bool,
    #[serde(default)] pub batch_no: String,
    #[serde(default)] pub expiry_date: String,
    /// stock_out from a specific expiry batch (e.g. removing expired stock).
    #[serde(default)] pub batch_id: Option<i64>,
}

pub fn adjust_stock_tx(conn: &Connection, input: &StockAdjustInput, user: &str, allow_negative: bool) -> Result<f64, String> {
    if !input.quantity.is_finite() || input.quantity < 0.0 || (input.mode != "count" && input.quantity == 0.0) {
        return Err("Enter a quantity greater than zero".into());
    }
    let row: Option<(String, i64, f64, i64, f64)> = conn
        .query_row("SELECT name,track_stock,stock_qty,cost_price_kobo,pack_size FROM products WHERE id=?1 AND active=1", [input.product_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
        .optional()
        .map_err(|e| e.to_string())?;
    let (name, track, current, cost, pack_size) = row.ok_or("Item was not found or is archived")?;
    if track == 0 {
        return Err(format!("{name} does not track stock"));
    }
    let note = input.note.trim();
    if note.chars().count() > 200 || input.batch_no.chars().count() > 40 { return Err("Note or batch number is too long".into()); }
    let factor = if input.as_pack && pack_size > 1.0 { pack_size } else { 1.0 };
    let qty = input.quantity * factor;
    match input.mode.as_str() {
        "stock_in" => {
            let unit_cost = input.unit_cost_kobo.unwrap_or(0).max(0);
            let per_base = if unit_cost > 0 { (unit_cost as f64 / factor).round() as i64 } else { 0 };
            receive_stock(conn, input.product_id, qty, per_base, "stock_in", note, user, Some((&input.batch_no, &input.expiry_date)))
        }
        "stock_out" | "count" => {
            let (movement, delta) = if input.mode == "stock_out" {
                if note.is_empty() { return Err("Give a reason for removing stock (e.g. damaged, expired, mortality)".into()); }
                ("stock_out", -qty)
            } else { ("adjustment", qty - current) };
            if delta.abs() < 1e-9 { return Ok(current); }
            let new_qty = current + delta;
            if new_qty < -1e-9 && !allow_negative {
                return Err(format!("Only {} in stock for {name}", db::format_qty(current)));
            }
            conn.execute("UPDATE products SET stock_qty=?1,updated_at=CURRENT_TIMESTAMP WHERE id=?2", params![new_qty, input.product_id]).map_err(|e| e.to_string())?;
            let note = if note.is_empty() && movement == "adjustment" { "Stock count".to_string() } else { note.to_string() };
            conn.execute("INSERT INTO stock_movements(product_id,movement_type,quantity_delta,unit_cost_kobo,note,user_name) VALUES(?1,?2,?3,?4,?5,?6)",
                params![input.product_id, movement, delta, cost, note, user]).map_err(|e| e.to_string())?;
            if delta < 0.0 {
                match input.batch_id {
                    Some(bid) => { conn.execute("UPDATE stock_batches SET quantity_left=MAX(0,quantity_left-?1) WHERE id=?2 AND product_id=?3", params![-delta, bid, input.product_id]).map_err(|e| e.to_string())?; }
                    None => consume_batches(conn, input.product_id, -delta)?,
                }
            }
            Ok(new_qty)
        }
        _ => Err("Unknown stock action".into()),
    }
}

#[tauri::command]
pub fn adjust_stock(state: tauri::State<'_, AppState>, session_token: String, input: StockAdjustInput) -> Result<f64, String> {
    let me = auth::require(&state, &session_token, Role::Manager)?;
    let mut conn = state.db()?;
    let allow_negative = db::setting_bool(&conn, "allow_negative_stock")?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let q = adjust_stock_tx(&tx, &input, &me.display_name(), allow_negative)?;
    crate::audit::log(&tx, &me.display_name(), "stock_adjusted", "product", Some(input.product_id), &format!("new quantity {q}"));
    tx.commit().map_err(|e| e.to_string())?;
    Ok(q)
}

// ------------------------------------------------------------------ expiry batches

#[derive(Serialize)]
pub struct Batch { id: i64, product_id: i64, product_name: String, unit: String, batch_no: String, expiry_date: String, quantity_left: f64, days_left: i64 }

#[tauri::command]
pub fn list_batches(state: tauri::State<'_, AppState>, session_token: String, product_id: Option<i64>, within_days: Option<i64>) -> Result<Vec<Batch>, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let conn = state.db()?;
    let mut stmt = conn.prepare(
        "SELECT b.id,b.product_id,p.name,p.unit,b.batch_no,b.expiry_date,b.quantity_left,CAST(julianday(b.expiry_date)-julianday(date('now','localtime')) AS INTEGER)
         FROM stock_batches b JOIN products p ON p.id=b.product_id
         WHERE b.quantity_left>0 AND p.active=1 AND (?1 IS NULL OR b.product_id=?1)
           AND (?2 IS NULL OR julianday(b.expiry_date)-julianday(date('now','localtime'))<=?2)
         ORDER BY b.expiry_date, p.name LIMIT 1000").map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![product_id, within_days], |r| Ok(Batch { id: r.get(0)?, product_id: r.get(1)?, product_name: r.get(2)?, unit: r.get(3)?, batch_no: r.get(4)?, expiry_date: r.get(5)?, quantity_left: r.get(6)?, days_left: r.get(7)? })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ recipes

#[derive(Serialize, Deserialize)]
pub struct RecipeLine { material_id: i64, #[serde(default)] material_name: String, #[serde(default)] unit: String, quantity: f64, #[serde(default)] cost_kobo: i64 }

#[tauri::command]
pub fn get_recipe(state: tauri::State<'_, AppState>, session_token: String, product_id: i64) -> Result<Vec<RecipeLine>, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let conn = state.db()?;
    let mut stmt = conn.prepare("SELECT ri.material_id,m.name,m.unit,ri.quantity,ROUND(ri.quantity*m.cost_price_kobo) FROM recipe_items ri JOIN products m ON m.id=ri.material_id WHERE ri.product_id=?1 ORDER BY m.name").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([product_id], |r| Ok(RecipeLine { material_id: r.get(0)?, material_name: r.get(1)?, unit: r.get(2)?, quantity: r.get(3)?, cost_kobo: r.get::<_, f64>(4)? as i64 })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn save_recipe_tx(conn: &Connection, product_id: i64, lines: &[RecipeLine]) -> Result<(), String> {
    let kind: String = conn.query_row("SELECT kind FROM products WHERE id=?1", [product_id], |r| r.get(0)).optional().map_err(|e| e.to_string())?.ok_or("Item not found")?;
    if kind == "material" { return Err("Internal materials cannot have a recipe".into()); }
    if lines.len() > 40 { return Err("A recipe can have at most 40 ingredients".into()); }
    conn.execute("DELETE FROM recipe_items WHERE product_id=?1", [product_id]).map_err(|e| e.to_string())?;
    for l in lines {
        if !l.quantity.is_finite() || l.quantity <= 0.0 { return Err("Each ingredient needs a quantity greater than zero".into()); }
        if l.material_id == product_id { return Err("An item cannot be an ingredient of itself".into()); }
        let ok: Option<i64> = conn.query_row("SELECT id FROM products WHERE id=?1 AND track_stock=1", [l.material_id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
        ok.ok_or("Ingredients must be stocked items")?;
        conn.execute("INSERT INTO recipe_items(product_id,material_id,quantity) VALUES(?1,?2,?3) ON CONFLICT(product_id,material_id) DO UPDATE SET quantity=excluded.quantity", params![product_id, l.material_id, l.quantity]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn save_recipe(state: tauri::State<'_, AppState>, session_token: String, product_id: i64, lines: Vec<RecipeLine>) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    save_recipe_tx(&tx, product_id, &lines)?;
    tx.commit().map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ barcodes

/// EAN-13 check digit for the first 12 digits.
pub fn ean13_check(d12: &str) -> u32 {
    let sum: u32 = d12.chars().filter_map(|c| c.to_digit(10)).enumerate().map(|(i, d)| if i % 2 == 0 { d } else { d * 3 }).sum();
    (10 - sum % 10) % 10
}

/// Gives items without a barcode an in-store EAN-13 code ("20" prefix, which
/// retail scanners treat as shop-internal), unique per item.
#[tauri::command]
pub fn generate_barcodes(state: tauri::State<'_, AppState>, session_token: String, product_ids: Vec<i64>) -> Result<Vec<(i64, String)>, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    if product_ids.len() > 2000 { return Err("Too many items at once".into()); }
    let conn = state.db()?;
    let mut out = Vec::new();
    for id in product_ids {
        let current: Option<String> = conn.query_row("SELECT barcode FROM products WHERE id=?1", [id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
        let Some(current) = current else { continue };
        if !current.is_empty() { out.push((id, current)); continue; }
        let mut n = id;
        loop {
            let d12 = format!("20{:010}", n);
            let code = format!("{d12}{}", ean13_check(&d12));
            let taken: Option<i64> = conn.query_row("SELECT id FROM products WHERE barcode=?1", [&code], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
            if taken.is_none() {
                conn.execute("UPDATE products SET barcode=?1,updated_at=CURRENT_TIMESTAMP WHERE id=?2", params![code, id]).map_err(|e| e.to_string())?;
                out.push((id, code));
                break;
            }
            n += 1_000_000;
        }
    }
    Ok(out)
}

#[derive(Serialize)]
pub struct Movement {
    id: i64,
    product_id: i64,
    product_name: String,
    unit: String,
    movement_type: String,
    quantity_delta: f64,
    unit_cost_kobo: i64,
    note: String,
    receipt_no: String,
    user_name: String,
    created_at: String,
}

#[tauri::command]
pub fn list_movements(state: tauri::State<'_, AppState>, session_token: String, product_id: Option<i64>, from: Option<String>, to: Option<String>) -> Result<Vec<Movement>, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let conn = state.db()?;
    let mut stmt = conn
        .prepare(
            "SELECT m.id,m.product_id,p.name,p.unit,m.movement_type,m.quantity_delta,m.unit_cost_kobo,m.note,COALESCE(r.receipt_no,''),m.user_name,datetime(m.created_at,'localtime')
             FROM stock_movements m JOIN products p ON p.id=m.product_id LEFT JOIN receipts r ON r.id=m.receipt_id
             WHERE (?1 IS NULL OR m.product_id=?1)
               AND (?2 IS NULL OR date(m.created_at,'localtime')>=?2)
               AND (?3 IS NULL OR date(m.created_at,'localtime')<=?3)
             ORDER BY m.id DESC LIMIT 1000",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![product_id, from.filter(|s| !s.is_empty()), to.filter(|s| !s.is_empty())], |r| {
            Ok(Movement { id: r.get(0)?, product_id: r.get(1)?, product_name: r.get(2)?, unit: r.get(3)?, movement_type: r.get(4)?, quantity_delta: r.get(5)?, unit_cost_kobo: r.get(6)?, note: r.get(7)?, receipt_no: r.get(8)?, user_name: r.get(9)?, created_at: r.get(10)? })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ categories

#[derive(Serialize)]
pub struct Category {
    id: i64,
    name: String,
    sort_order: i64,
    item_count: i64,
}

#[tauri::command]
pub fn list_categories(state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<Category>, String> {
    auth::require(&state, &session_token, Role::Cashier)?;
    let conn = state.db()?;
    let mut stmt = conn
        .prepare("SELECT c.id,c.name,c.sort_order,(SELECT COUNT(*) FROM products p WHERE p.category_id=c.id AND p.active=1) FROM categories c WHERE c.active=1 ORDER BY c.sort_order, c.name COLLATE NOCASE")
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |r| Ok(Category { id: r.get(0)?, name: r.get(1)?, sort_order: r.get(2)?, item_count: r.get(3)? })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_category(state: tauri::State<'_, AppState>, session_token: String, id: Option<i64>, name: String, sort_order: i64) -> Result<i64, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 60 { return Err("Category name must be 1 to 60 characters".into()); }
    let conn = state.db()?;
    let map = |e: rusqlite::Error| if e.to_string().contains("UNIQUE") { "A category with that name already exists".to_string() } else { e.to_string() };
    match id {
        Some(id) => { conn.execute("UPDATE categories SET name=?1,sort_order=?2,active=1 WHERE id=?3", params![name, sort_order, id]).map_err(map)?; Ok(id) }
        None => { conn.execute("INSERT INTO categories(name,sort_order) VALUES(?1,?2)", params![name, sort_order]).map_err(map)?; Ok(conn.last_insert_rowid()) }
    }
}

#[tauri::command]
pub fn delete_category(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let conn = state.db()?;
    // Items keep existing; they simply become uncategorised.
    conn.execute("UPDATE products SET category_id=NULL WHERE category_id=?1", [id]).map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM categories WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

// ------------------------------------------------------------------ payment methods

#[derive(Serialize)]
pub struct PaymentMethod {
    id: i64,
    name: String,
    active: bool,
    sort_order: i64,
}

#[tauri::command]
pub fn list_payment_methods(state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<PaymentMethod>, String> {
    auth::require(&state, &session_token, Role::Cashier)?;
    let conn = state.db()?;
    let mut stmt = conn.prepare("SELECT id,name,active,sort_order FROM payment_methods ORDER BY sort_order,name").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |r| Ok(PaymentMethod { id: r.get(0)?, name: r.get(1)?, active: r.get::<_, i64>(2)? == 1, sort_order: r.get(3)? })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_payment_method(state: tauri::State<'_, AppState>, session_token: String, id: Option<i64>, name: String, active: bool, sort_order: i64) -> Result<i64, String> {
    auth::require(&state, &session_token, Role::Owner)?;
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 40 { return Err("Payment method name must be 1 to 40 characters".into()); }
    let conn = state.db()?;
    if !active {
        let others: i64 = conn.query_row("SELECT COUNT(*) FROM payment_methods WHERE active=1 AND id<>?1", [id.unwrap_or(0)], |r| r.get(0)).map_err(|e| e.to_string())?;
        if others == 0 { return Err("Keep at least one payment method active".into()); }
    }
    let map = |e: rusqlite::Error| if e.to_string().contains("UNIQUE") { "That payment method already exists".to_string() } else { e.to_string() };
    match id {
        Some(id) => { conn.execute("UPDATE payment_methods SET name=?1,active=?2,sort_order=?3 WHERE id=?4", params![name, active as i64, sort_order, id]).map_err(map)?; Ok(id) }
        None => { conn.execute("INSERT INTO payment_methods(name,active,sort_order) VALUES(?1,?2,?3)", params![name, active as i64, sort_order]).map_err(map)?; Ok(conn.last_insert_rowid()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: &str, stock: f64) -> ProductInput {
        ProductInput { id: None, sku: format!("T-{kind}"), barcode: String::new(), name: "Test".into(), category_id: None, kind: kind.into(), unit: "piece".into(),
            selling_price_kobo: 1000, cost_price_kobo: 600, track_stock: true, stock_qty: stock, reorder_level: 1.0, description: String::new(), active: true,
            pack_unit: String::new(), pack_size: 0.0, pack_price_kobo: 0, wholesale_price_kobo: 0, wholesale_min_qty: 0.0, track_expiry: false }
    }
    fn adj(id: i64, mode: &str, q: f64, cost: Option<i64>, note: &str) -> StockAdjustInput {
        StockAdjustInput { product_id: id, mode: mode.into(), quantity: q, unit_cost_kobo: cost, note: note.into(), as_pack: false, batch_no: String::new(), expiry_date: String::new(), batch_id: None }
    }

    #[test]
    fn stock_in_uses_weighted_average_cost_and_out_needs_reason() {
        let conn = db::test_conn();
        let id = save_product_tx(&conn, &item("product", 10.0), "t").unwrap();
        adjust_stock_tx(&conn, &adj(id, "stock_in", 10.0, Some(800), ""), "t", false).unwrap();
        let cost: i64 = conn.query_row("SELECT cost_price_kobo FROM products WHERE id=?1", [id], |r| r.get(0)).unwrap();
        assert_eq!(cost, 700);
        assert!(adjust_stock_tx(&conn, &adj(id, "stock_out", 1.0, None, ""), "t", false).is_err());
        assert!(adjust_stock_tx(&conn, &adj(id, "stock_out", 50.0, None, "damaged"), "t", false).is_err());
        let q = adjust_stock_tx(&conn, &adj(id, "count", 7.0, None, ""), "t", false).unwrap();
        assert_eq!(q, 7.0);
    }

    #[test]
    fn batches_are_used_earliest_expiry_first() {
        let conn = db::test_conn();
        let mut p = item("product", 0.0);
        p.track_expiry = true;
        let id = save_product_tx(&conn, &p, "t").unwrap();
        let mut a = adj(id, "stock_in", 5.0, None, "");
        a.expiry_date = "2030-06-01".into();
        adjust_stock_tx(&conn, &a, "t", false).unwrap();
        let mut b = adj(id, "stock_in", 5.0, None, "");
        b.expiry_date = "2030-01-01".into();
        adjust_stock_tx(&conn, &b, "t", false).unwrap();
        consume_batches(&conn, id, 6.0).unwrap();
        let left: Vec<(String, f64)> = conn.prepare("SELECT expiry_date,quantity_left FROM stock_batches ORDER BY expiry_date").unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap();
        assert_eq!(left, vec![("2030-01-01".into(), 0.0), ("2030-06-01".into(), 4.0)]);
    }

    #[test]
    fn packs_receive_in_base_units() {
        let conn = db::test_conn();
        let mut p = item("product", 0.0);
        p.pack_unit = "carton".into(); p.pack_size = 40.0; p.pack_price_kobo = 11500;
        let id = save_product_tx(&conn, &p, "t").unwrap();
        let mut a = adj(id, "stock_in", 2.0, Some(960000), "");
        a.as_pack = true;
        assert_eq!(adjust_stock_tx(&conn, &a, "t", false).unwrap(), 80.0);
        let cost: i64 = conn.query_row("SELECT cost_price_kobo FROM products WHERE id=?1", [id], |r| r.get(0)).unwrap();
        assert_eq!(cost, 24000);
    }

    #[test]
    fn ean13_check_digit() {
        assert_eq!(ean13_check("400638133393"), 1);
    }

    #[test]
    fn services_never_track_stock() {
        let conn = db::test_conn();
        let id = save_product_tx(&conn, &item("service", 5.0), "t").unwrap();
        let (track, qty): (i64, f64) = conn.query_row("SELECT track_stock,stock_qty FROM products WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!((track, qty), (0, 0.0));
    }
}
