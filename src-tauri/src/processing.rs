//! Agricultural processing as costed batches: palm oil, garri, flour, feed
//! milling and any other process.
//!
//! A batch collects raw materials (bought or taken from stock) and processing
//! costs, then records what came out: main products, by-products and waste.
//! The batch cost is split between the main products by percentage, giving a
//! cost per litre / kg / bag. Closing the batch adds the outputs to stock at
//! that cost, in one transaction. No fixed yield is ever assumed.

use crate::{auth::{self, Role}, farms::{check_len, check_money, check_qty, rows, valid_date}, AppState};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

pub const PROCESS_TYPES: [&str; 10] = ["palm_oil", "palm_kernel", "cassava_garri", "cassava_fufu", "cassava_flour", "cassava_starch", "grain_milling", "feed_milling", "groundnut", "other"];

#[derive(Serialize, Debug, Clone)]
pub struct Input { pub id: i64, pub entry_date: String, pub kind: String, pub name: String, pub quantity: f64, pub unit: String, pub unit_cost_kobo: i64, pub amount_kobo: i64, pub product_id: Option<i64>, pub supplier: String, pub notes: String, pub user_name: String }

#[derive(Serialize, Debug, Clone)]
pub struct Output {
    pub id: i64, pub entry_date: String, pub output_type: String, pub name: String, pub quantity: f64, pub unit: String, pub cost_share_pct: f64,
    pub product_id: Option<i64>, pub product_name: String, pub added_to_stock: bool, pub sold_qty: f64, pub sale_amount_kobo: i64, pub buyer: String, pub notes: String, pub user_name: String,
    /// Batch cost carried by this output ÷ its quantity. 0 for by-products and waste.
    pub unit_cost_kobo: i64,
    /// Sale amount − cost of the quantity sold (main products only).
    pub profit_on_sold_kobo: i64,
}

#[derive(Serialize, Debug, Default)]
pub struct BatchSummary {
    pub id: i64, pub batch_no: String, pub process_type: String, pub title: String, pub farm_id: Option<i64>, pub farm_name: String, pub start_date: String, pub end_date: String,
    pub stage: String, pub status: String, pub notes: String, pub is_demo: bool,
    pub raw_cost_kobo: i64, pub processing_cost_kobo: i64, pub total_cost_kobo: i64,
    pub raw_qty: f64, pub raw_unit: String,
    pub main_share_pct: f64, pub output_count: i64,
    pub revenue_kobo: i64, pub profit_on_sold_kobo: i64,
    /// Main output quantity ÷ raw input quantity × 100, only when both use the same unit.
    pub yield_pct: Option<f64>,
    pub waste_pct: Option<f64>,
    pub cost_breakdown: Vec<(String, i64)>,
}

#[derive(Serialize, Debug)]
pub struct BatchDetail { pub batch: BatchSummary, pub inputs: Vec<Input>, pub outputs: Vec<Output> }

fn load_inputs(conn: &Connection, id: i64) -> Result<Vec<Input>, String> {
    rows(conn, "SELECT id,entry_date,kind,name,quantity,unit,unit_cost_kobo,amount_kobo,product_id,supplier,notes,user_name FROM processing_inputs WHERE batch_id=?1 ORDER BY kind DESC, id", [id], |r| Ok(Input {
        id: r.get(0)?, entry_date: r.get(1)?, kind: r.get(2)?, name: r.get(3)?, quantity: r.get(4)?, unit: r.get(5)?, unit_cost_kobo: r.get(6)?, amount_kobo: r.get(7)?, product_id: r.get(8)?,
        supplier: r.get(9)?, notes: r.get(10)?, user_name: r.get(11)?,
    }))
}

fn load_outputs(conn: &Connection, id: i64) -> Result<Vec<Output>, String> {
    rows(conn, "SELECT o.id,o.entry_date,o.output_type,o.name,o.quantity,o.unit,o.cost_share_pct,o.product_id,COALESCE(p.name,''),o.added_to_stock,o.sold_qty,o.sale_amount_kobo,o.buyer,o.notes,o.user_name
        FROM processing_outputs o LEFT JOIN products p ON p.id=o.product_id WHERE o.batch_id=?1 ORDER BY CASE o.output_type WHEN 'main' THEN 0 WHEN 'byproduct' THEN 1 ELSE 2 END, o.id", [id], |r| Ok(Output {
        id: r.get(0)?, entry_date: r.get(1)?, output_type: r.get(2)?, name: r.get(3)?, quantity: r.get(4)?, unit: r.get(5)?, cost_share_pct: r.get(6)?, product_id: r.get(7)?,
        product_name: r.get(8)?, added_to_stock: r.get::<_, i64>(9)? == 1, sold_qty: r.get(10)?, sale_amount_kobo: r.get(11)?, buyer: r.get(12)?, notes: r.get(13)?, user_name: r.get(14)?,
        unit_cost_kobo: 0, profit_on_sold_kobo: 0,
    }))
}

/// Effective share of each main output: when there is a single main output
/// with no share entered, it carries the whole batch cost.
fn effective_shares(outputs: &[Output]) -> Vec<f64> {
    let mains = outputs.iter().filter(|o| o.output_type == "main").count();
    outputs.iter().map(|o| if o.output_type != "main" { 0.0 } else if mains == 1 && o.cost_share_pct == 0.0 { 100.0 } else { o.cost_share_pct }).collect()
}

pub fn batch_detail(conn: &Connection, id: i64) -> Result<BatchDetail, String> {
    let mut b = conn.query_row("SELECT b.id,b.batch_no,b.process_type,b.title,b.farm_id,COALESCE(f.name,''),b.start_date,b.end_date,b.stage,b.status,b.notes,b.is_demo
        FROM processing_batches b LEFT JOIN farms f ON f.id=b.farm_id WHERE b.id=?1", [id], |r| Ok(BatchSummary {
        id: r.get(0)?, batch_no: r.get(1)?, process_type: r.get(2)?, title: r.get(3)?, farm_id: r.get(4)?, farm_name: r.get(5)?, start_date: r.get(6)?, end_date: r.get(7)?,
        stage: r.get(8)?, status: r.get(9)?, notes: r.get(10)?, is_demo: r.get::<_, i64>(11)? == 1, ..Default::default()
    })).optional().map_err(|e| e.to_string())?.ok_or("Processing batch not found")?;
    let inputs = load_inputs(conn, id)?;
    let mut outputs = load_outputs(conn, id)?;
    for i in &inputs {
        if i.kind == "raw" { b.raw_cost_kobo += i.amount_kobo; } else { b.processing_cost_kobo += i.amount_kobo; }
        let key = if i.kind == "raw" { format!("Raw material: {}", i.name) } else { i.name.clone() };
        match b.cost_breakdown.iter_mut().find(|(k, _)| *k == key) { Some(e) => e.1 += i.amount_kobo, None => b.cost_breakdown.push((key, i.amount_kobo)) }
    }
    b.total_cost_kobo = b.raw_cost_kobo + b.processing_cost_kobo;
    let raws: Vec<&Input> = inputs.iter().filter(|i| i.kind == "raw").collect();
    if let Some(first) = raws.first() {
        if raws.iter().all(|i| i.unit.eq_ignore_ascii_case(&first.unit)) {
            b.raw_unit = first.unit.clone();
            b.raw_qty = raws.iter().map(|i| i.quantity).sum();
        }
    }
    let shares = effective_shares(&outputs);
    b.main_share_pct = shares.iter().sum();
    b.output_count = outputs.len() as i64;
    let mut main_same_unit = 0.0;
    let mut waste_same_unit = 0.0;
    for (o, share) in outputs.iter_mut().zip(shares) {
        if o.output_type == "main" && o.quantity > 0.0 {
            o.unit_cost_kobo = ((b.total_cost_kobo as f64) * share / 100.0 / o.quantity).round() as i64;
            o.profit_on_sold_kobo = o.sale_amount_kobo - ((o.unit_cost_kobo as f64) * o.sold_qty).round() as i64;
            b.profit_on_sold_kobo += o.profit_on_sold_kobo;
        }
        b.revenue_kobo += o.sale_amount_kobo;
        if !b.raw_unit.is_empty() && o.unit.eq_ignore_ascii_case(&b.raw_unit) {
            if o.output_type == "main" { main_same_unit += o.quantity; }
            if o.output_type == "waste" { waste_same_unit += o.quantity; }
        }
    }
    if b.raw_qty > 0.0 {
        if main_same_unit > 0.0 { b.yield_pct = Some(main_same_unit * 100.0 / b.raw_qty); }
        if waste_same_unit > 0.0 { b.waste_pct = Some(waste_same_unit * 100.0 / b.raw_qty); }
    }
    Ok(BatchDetail { batch: b, inputs, outputs })
}

/// Money fields are blanked for roles below Manager (workers, advisers, cashiers).
#[tauri::command]
pub fn list_processing_batches(state: tauri::State<'_, AppState>, session_token: String, include_closed: Option<bool>) -> Result<serde_json::Value, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let out = list_processing_batches_inner(state, session_token, include_closed)?;
    crate::auth::money_view(&me, &out)
}

fn list_processing_batches_inner(state: tauri::State<'_, AppState>, session_token: String, include_closed: Option<bool>) -> Result<Vec<BatchSummary>, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let conn = state.db()?;
    let allowed = crate::farm_access::allowed(&conn, &me)?;
    let ids: Vec<i64> = rows(&conn, "SELECT id FROM processing_batches WHERE ?1=1 OR status='open' ORDER BY status DESC, start_date DESC, id DESC LIMIT 300", [include_closed.unwrap_or(false) as i64], |r| r.get(0))?;
    Ok(ids.into_iter().map(|id| batch_detail(&conn, id).map(|d| d.batch)).collect::<Result<Vec<_>, _>>()?.into_iter().filter(|b| crate::farm_access::visible(&allowed, b.farm_id)).collect())
}

/// Money fields are blanked for roles below Manager (workers, advisers, cashiers).
#[tauri::command]
pub fn get_processing_batch(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<serde_json::Value, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let out = get_processing_batch_inner(state, session_token, id)?;
    crate::auth::money_view(&me, &out)
}

fn get_processing_batch_inner(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<BatchDetail, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    batch_detail(&*state.db()?, id)
}

#[derive(Deserialize, Default)]
pub struct BatchInput { pub id: Option<i64>, pub batch_no: String, pub process_type: String, #[serde(default)] pub title: String, #[serde(default)] pub farm_id: Option<i64>, pub start_date: String, #[serde(default)] pub stage: String, #[serde(default)] pub notes: String }

pub fn save_batch_tx(conn: &Connection, i: &BatchInput) -> Result<i64, String> {
    let no = i.batch_no.trim();
    if no.is_empty() { return Err("Give the batch a number, such as PB-001".into()); }
    check_len(no, 30, "Batch number")?;
    if !PROCESS_TYPES.contains(&i.process_type.as_str()) { return Err("Choose the type of processing".into()); }
    if !valid_date(&i.start_date) { return Err("Choose the start date".into()); }
    check_len(&i.title, 120, "Title")?;
    check_len(&i.stage, 60, "Stage")?;
    check_len(&i.notes, 1000, "Notes")?;
    let dup: Option<i64> = conn.query_row("SELECT id FROM processing_batches WHERE batch_no=?1 AND id<>COALESCE(?2,-1)", params![no, i.id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    if dup.is_some() { return Err("Another batch already uses that number".into()); }
    match i.id {
        Some(id) => {
            let n = conn.execute("UPDATE processing_batches SET batch_no=?1,process_type=?2,title=?3,farm_id=?4,start_date=?5,stage=?6,notes=?7 WHERE id=?8",
                params![no, i.process_type, i.title.trim(), i.farm_id, i.start_date, i.stage.trim(), i.notes.trim(), id]).map_err(|e| e.to_string())?;
            if n == 0 { return Err("Processing batch not found".into()); }
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO processing_batches(batch_no,process_type,title,farm_id,start_date,stage,notes) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![no, i.process_type, i.title.trim(), i.farm_id, i.start_date, i.stage.trim(), i.notes.trim()]).map_err(|e| e.to_string())?;
            Ok(conn.last_insert_rowid())
        }
    }
}

#[tauri::command]
pub fn save_processing_batch(state: tauri::State<'_, AppState>, session_token: String, input: BatchInput) -> Result<i64, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    save_batch_tx(&*state.db()?, &input)
}

#[tauri::command]
pub fn set_processing_stage(state: tauri::State<'_, AppState>, session_token: String, id: i64, stage: String) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Worker)?;
    check_len(&stage, 60, "Stage")?;
    let n = state.db()?.execute("UPDATE processing_batches SET stage=?1 WHERE id=?2 AND status='open'", params![stage.trim(), id]).map_err(|e| e.to_string())?;
    if n == 0 { return Err("This batch is closed".into()); }
    Ok(())
}

fn require_open(conn: &Connection, id: i64) -> Result<String, String> {
    let (status, no): (String, String) = conn.query_row("SELECT status,batch_no FROM processing_batches WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(|e| e.to_string())?.ok_or("Processing batch not found")?;
    if status != "open" { return Err("This batch is closed. Reopen it to change it.".into()); }
    Ok(no)
}

#[derive(Deserialize, Default)]
pub struct InputEntry {
    pub batch_id: i64, pub entry_date: String, pub kind: String, pub name: String, #[serde(default)] pub quantity: f64, #[serde(default)] pub unit: String,
    #[serde(default)] pub unit_cost_kobo: i64, #[serde(default)] pub amount_kobo: i64, #[serde(default)] pub product_id: Option<i64>, #[serde(default)] pub supplier: String, #[serde(default)] pub notes: String,
}

/// Adds a raw material or a processing cost. A raw material taken from stock
/// is deducted at its average cost; otherwise amount = quantity × unit cost
/// unless a total is given.
pub fn add_input_tx(tx: &Connection, i: &InputEntry, who: &str) -> Result<i64, String> {
    let no = require_open(tx, i.batch_id)?;
    if !valid_date(&i.entry_date) { return Err("Choose the date".into()); }
    if !["raw", "cost"].contains(&i.kind.as_str()) { return Err("Choose raw material or cost".into()); }
    if i.name.trim().is_empty() { return Err(if i.kind == "raw" { "Name the raw material" } else { "Choose the cost category" }.into()); }
    check_len(&i.name, 80, "Name")?;
    check_len(&i.unit, 30, "Unit")?;
    check_len(&i.supplier, 120, "Supplier")?;
    check_len(&i.notes, 300, "Notes")?;
    check_qty(i.quantity, "Quantity")?;
    check_money(i.unit_cost_kobo, "Unit cost")?;
    check_money(i.amount_kobo, "Amount")?;
    let mut unit = i.unit.trim().to_string();
    let mut unit_cost = i.unit_cost_kobo;
    let amount;
    if let Some(pid) = i.product_id {
        if i.kind != "raw" { return Err("Only raw materials can be taken from stock".into()); }
        if i.quantity <= 0.0 { return Err("Enter the quantity taken from stock".into()); }
        let (name, punit, cost, stock, track): (String, String, i64, f64, i64) = tx.query_row("SELECT name,unit,cost_price_kobo,stock_qty,track_stock FROM products WHERE id=?1", [pid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
            .optional().map_err(|e| e.to_string())?.ok_or("Stock item not found")?;
        if track == 1 && stock + 1e-9 < i.quantity { return Err(format!("Not enough {name} in stock. Available: {} {punit}", crate::db::format_qty(stock))); }
        unit = punit;
        unit_cost = cost;
        amount = ((cost as f64) * i.quantity).round() as i64;
        if track == 1 {
            tx.execute("UPDATE products SET stock_qty=stock_qty-?1,updated_at=CURRENT_TIMESTAMP WHERE id=?2", params![i.quantity, pid]).map_err(|e| e.to_string())?;
            tx.execute("INSERT INTO stock_movements(product_id,movement_type,quantity_delta,unit_cost_kobo,note,user_name) VALUES(?1,'processing_issue',?2,?3,?4,?5)",
                params![pid, -i.quantity, cost, format!("Used in processing batch {no}"), who]).map_err(|e| e.to_string())?;
            crate::inventory::consume_batches(tx, pid, i.quantity)?;
        }
    } else if i.amount_kobo > 0 {
        amount = i.amount_kobo;
    } else {
        amount = ((unit_cost as f64) * i.quantity).round() as i64;
    }
    if i.kind == "raw" && i.quantity <= 0.0 { return Err("Enter the quantity of raw material".into()); }
    if i.kind == "raw" && unit.is_empty() { return Err("Choose the unit, such as bunch, kg or bag".into()); }
    if i.kind == "cost" && amount <= 0 { return Err("Enter the amount".into()); }
    tx.execute("INSERT INTO processing_inputs(batch_id,entry_date,kind,name,quantity,unit,unit_cost_kobo,amount_kobo,product_id,supplier,notes,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![i.batch_id, i.entry_date, i.kind, i.name.trim(), i.quantity, unit, unit_cost, amount, i.product_id, i.supplier.trim(), i.notes.trim(), who]).map_err(|e| e.to_string())?;
    Ok(tx.last_insert_rowid())
}

#[tauri::command]
pub fn add_processing_input(state: tauri::State<'_, AppState>, session_token: String, input: InputEntry) -> Result<i64, String> {
    let me = auth::require(&state, &session_token, Role::Manager)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let id = add_input_tx(&tx, &input, &me.display_name())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(id)
}

#[derive(Deserialize, Default)]
pub struct OutputEntry {
    pub batch_id: i64, pub entry_date: String, pub output_type: String, pub name: String, pub quantity: f64, pub unit: String, #[serde(default)] pub cost_share_pct: f64,
    #[serde(default)] pub product_id: Option<i64>, #[serde(default)] pub sold_qty: f64, #[serde(default)] pub sale_amount_kobo: i64, #[serde(default)] pub buyer: String, #[serde(default)] pub notes: String,
}

pub fn add_output_tx(conn: &Connection, o: &OutputEntry, who: &str) -> Result<i64, String> {
    require_open(conn, o.batch_id)?;
    if !valid_date(&o.entry_date) { return Err("Choose the date".into()); }
    if !["main", "byproduct", "waste"].contains(&o.output_type.as_str()) { return Err("Choose main product, by-product or waste".into()); }
    if o.name.trim().is_empty() { return Err("Name what came out, such as Palm oil".into()); }
    check_len(&o.name, 80, "Name")?;
    if o.unit.trim().is_empty() { return Err("Choose the unit, such as litre or kg".into()); }
    check_len(&o.unit, 30, "Unit")?;
    check_qty(o.quantity, "Quantity")?;
    check_qty(o.sold_qty, "Quantity sold")?;
    check_money(o.sale_amount_kobo, "Sale amount")?;
    check_len(&o.buyer, 120, "Buyer")?;
    check_len(&o.notes, 300, "Notes")?;
    if !(o.cost_share_pct.is_finite() && (0.0..=100.0).contains(&o.cost_share_pct)) { return Err("Cost share must be between 0 and 100%".into()); }
    if o.output_type != "main" && o.cost_share_pct > 0.0 { return Err("Only main products carry a share of the batch cost".into()); }
    if o.sold_qty > o.quantity + 1e-9 { return Err("Quantity sold cannot be more than the quantity produced".into()); }
    if o.output_type == "waste" && (o.sold_qty > 0.0 || o.product_id.is_some()) { return Err("Waste is not sold or put into stock".into()); }
    if let Some(pid) = o.product_id {
        let unit: String = conn.query_row("SELECT unit FROM products WHERE id=?1", [pid], |r| r.get(0)).optional().map_err(|e| e.to_string())?.ok_or("Stock item not found")?;
        if !unit.eq_ignore_ascii_case(o.unit.trim()) { return Err(format!("The stock item is counted in {unit}. Record this output in {unit} too.")); }
    }
    let other_share: f64 = conn.query_row("SELECT COALESCE(SUM(cost_share_pct),0) FROM processing_outputs WHERE batch_id=?1 AND output_type='main'", [o.batch_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    if other_share + o.cost_share_pct > 100.0 + 1e-6 { return Err(format!("The main products already carry {other_share:.0}% of the cost; the total cannot pass 100%")); }
    conn.execute("INSERT INTO processing_outputs(batch_id,entry_date,output_type,name,quantity,unit,cost_share_pct,product_id,sold_qty,sale_amount_kobo,buyer,notes,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![o.batch_id, o.entry_date, o.output_type, o.name.trim(), o.quantity, o.unit.trim(), o.cost_share_pct, o.product_id, o.sold_qty, o.sale_amount_kobo, o.buyer.trim(), o.notes.trim(), who]).map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn add_processing_output(state: tauri::State<'_, AppState>, session_token: String, input: OutputEntry) -> Result<i64, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    if (input.sale_amount_kobo > 0 || input.cost_share_pct > 0.0 || input.product_id.is_some()) && me.role < Role::Manager { return Err("Only managers can record sales, cost shares and stock".into()); }
    add_output_tx(&*state.db()?, &input, &me.display_name())
}

#[tauri::command]
pub fn delete_processing_entry(state: tauri::State<'_, AppState>, session_token: String, kind: String, id: i64) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let conn = state.db()?;
    let table = match kind.as_str() { "input" => "processing_inputs", "output" => "processing_outputs", _ => return Err("Unknown entry type".into()) };
    let batch: i64 = conn.query_row(&format!("SELECT batch_id FROM {table} WHERE id=?1"), [id], |r| r.get(0)).optional().map_err(|e| e.to_string())?.ok_or("Entry not found")?;
    require_open(&conn, batch)?;
    if kind == "output" {
        let added: i64 = conn.query_row("SELECT added_to_stock FROM processing_outputs WHERE id=?1", [id], |r| r.get(0)).map_err(|e| e.to_string())?;
        if added == 1 { return Err("This output is already in stock. Adjust the stock instead.".into()); }
    }
    // Raw material taken from stock is not put back automatically.
    conn.execute(&format!("DELETE FROM {table} WHERE id=?1"), [id]).map_err(|e| e.to_string())?;
    Ok(())
}

/// Closes a batch: main-product shares must total 100%, then every output
/// linked to a stock item (less what was sold straight from the batch) is
/// received into stock at its cost per unit. Everything happens in one
/// transaction; if any step fails nothing changes.
pub fn close_batch_tx(tx: &Connection, id: i64, end_date: &str, who: &str) -> Result<BatchDetail, String> {
    let no = require_open(tx, id)?;
    if !valid_date(end_date) { return Err("Choose the end date".into()); }
    let d = batch_detail(tx, id)?;
    if end_date < d.batch.start_date.as_str() { return Err("The end date is before the start date".into()); }
    if d.inputs.is_empty() { return Err("Record the raw materials and costs first".into()); }
    let mains: Vec<&Output> = d.outputs.iter().filter(|o| o.output_type == "main").collect();
    if mains.is_empty() { return Err("Record at least one main product, such as Palm oil".into()); }
    if (d.batch.main_share_pct - 100.0).abs() > 0.01 { return Err(format!("The main products carry {:.1}% of the batch cost. Make the shares add up to 100%.", d.batch.main_share_pct)); }
    for o in &d.outputs {
        if let (Some(pid), false) = (o.product_id, o.added_to_stock) {
            let qty = o.quantity - o.sold_qty;
            if qty > 1e-9 {
                // By-products enter stock without changing the item's cost.
                let cost = if o.output_type == "main" { o.unit_cost_kobo } else { 0 };
                crate::inventory::receive_stock(tx, pid, qty, cost, "processing_output", &format!("From processing batch {no}"), who, None)?;
            }
            tx.execute("UPDATE processing_outputs SET added_to_stock=1 WHERE id=?1", [o.id]).map_err(|e| e.to_string())?;
        }
    }
    tx.execute("UPDATE processing_batches SET status='closed',end_date=?1 WHERE id=?2", params![end_date, id]).map_err(|e| e.to_string())?;
    batch_detail(tx, id)
}

#[tauri::command]
pub fn close_processing_batch(state: tauri::State<'_, AppState>, session_token: String, id: i64, end_date: String, reopen: Option<bool>) -> Result<BatchDetail, String> {
    let me = auth::require(&state, &session_token, Role::Manager)?;
    let mut conn = state.db()?;
    if reopen.unwrap_or(false) {
        // Outputs already in stock stay there; new outputs are added at the next close.
        let n = conn.execute("UPDATE processing_batches SET status='open',end_date='' WHERE id=?1 AND status='closed'", [id]).map_err(|e| e.to_string())?;
        if n == 0 { return Err("Processing batch not found".into()); }
        return batch_detail(&conn, id);
    }
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let d = close_batch_tx(&tx, id, &end_date, &me.display_name())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The specification's illustrative palm fruit milling batch: ₦265,000 in total.
    fn palm_batch(conn: &Connection) -> i64 {
        let id = save_batch_tx(conn, &BatchInput { batch_no: "PB-07".into(), process_type: "palm_oil".into(), start_date: "2026-06-01".into(), ..Default::default() }).unwrap();
        let e = |kind: &str, name: &str, q: f64, unit: &str, uc: i64, amt: i64| InputEntry { batch_id: id, entry_date: "2026-06-01".into(), kind: kind.into(), name: name.into(), quantity: q, unit: unit.into(), unit_cost_kobo: uc, amount_kobo: amt, ..Default::default() };
        add_input_tx(conn, &e("raw", "Palm fruit bunches", 100.0, "bunch", 200_000, 0), "M").unwrap();
        add_input_tx(conn, &e("cost", "Milling fee", 100.0, "bunch", 50_000, 0), "M").unwrap();
        add_input_tx(conn, &e("cost", "Transport", 0.0, "", 0, 1_000_000), "M").unwrap();
        add_input_tx(conn, &e("cost", "Labour", 0.0, "", 0, 500_000), "M").unwrap();
        id
    }

    #[test]
    fn palm_batch_costs_match_the_specification() {
        let conn = crate::db::test_conn();
        let id = palm_batch(&conn);
        let d = batch_detail(&conn, id).unwrap();
        assert_eq!(d.batch.raw_cost_kobo, 20_000_000);
        assert_eq!(d.batch.processing_cost_kobo, 6_500_000);
        assert_eq!(d.batch.total_cost_kobo, 26_500_000, "₦265,000");
        assert_eq!(d.batch.raw_qty, 100.0);
    }

    #[test]
    fn close_allocates_costs_and_adds_outputs_to_stock() {
        let conn = crate::db::test_conn();
        let id = palm_batch(&conn);
        conn.execute("INSERT INTO products(sku,name,kind,unit,stock_qty,track_stock) VALUES('PO','Palm oil','product','litre',0,1)", []).unwrap();
        let oil = conn.last_insert_rowid();
        conn.execute("INSERT INTO products(sku,name,kind,unit,stock_qty,track_stock) VALUES('PK','Palm kernel','product','kg',0,1)", []).unwrap();
        let kernel = conn.last_insert_rowid();
        let out = |t: &str, n: &str, q: f64, u: &str, share: f64, pid: Option<i64>, sold: f64, amt: i64| OutputEntry { batch_id: id, entry_date: "2026-06-03".into(), output_type: t.into(), name: n.into(), quantity: q, unit: u.into(), cost_share_pct: share, product_id: pid, sold_qty: sold, sale_amount_kobo: amt, ..Default::default() };
        add_output_tx(&conn, &out("main", "Palm oil", 150.0, "litre", 85.0, Some(oil), 50.0, 10_000_000), "M").unwrap();
        // Shares above 100% are refused.
        assert!(add_output_tx(&conn, &out("main", "Palm kernel", 80.0, "kg", 20.0, Some(kernel), 0.0, 0), "M").is_err());
        assert!(close_batch_tx(&conn, id, "2026-06-03", "M").is_err(), "shares only total 85%");
        add_output_tx(&conn, &out("main", "Palm kernel", 80.0, "kg", 15.0, Some(kernel), 0.0, 0), "M").unwrap();
        add_output_tx(&conn, &out("waste", "Rejected fruit", 3.0, "bunch", 0.0, None, 0.0, 0), "M").unwrap();
        let d = close_batch_tx(&conn, id, "2026-06-03", "M").unwrap();
        let oil_out = d.outputs.iter().find(|o| o.name == "Palm oil").unwrap();
        assert_eq!(oil_out.unit_cost_kobo, 150_167, "₦265,000 × 85% ÷ 150 L");
        let kernel_out = d.outputs.iter().find(|o| o.name == "Palm kernel").unwrap();
        assert_eq!(kernel_out.unit_cost_kobo, 49_688, "₦265,000 × 15% ÷ 80 kg");
        assert_eq!(oil_out.profit_on_sold_kobo, 10_000_000 - 50 * 150_167);
        assert!((d.batch.waste_pct.unwrap() - 3.0).abs() < 1e-9);
        let (stock, cost): (f64, i64) = conn.query_row("SELECT stock_qty,cost_price_kobo FROM products WHERE id=?1", [oil], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert!((stock - 100.0).abs() < 1e-9, "150 L made − 50 L sold from the batch");
        assert_eq!(cost, 150_167);
        assert_eq!(d.batch.status, "closed");
        assert!(add_input_tx(&conn, &InputEntry { batch_id: id, entry_date: "2026-06-04".into(), kind: "cost".into(), name: "Fuel".into(), amount_kobo: 100, ..Default::default() }, "M").is_err());
    }

    #[test]
    fn raw_material_from_stock_is_deducted_at_average_cost() {
        let conn = crate::db::test_conn();
        let id = save_batch_tx(&conn, &BatchInput { batch_no: "FM-1".into(), process_type: "feed_milling".into(), start_date: "2026-06-01".into(), ..Default::default() }).unwrap();
        conn.execute("INSERT INTO products(sku,name,kind,unit,cost_price_kobo,stock_qty,track_stock) VALUES('MZ','Maize','material','kg',40_000,500,1)", []).unwrap();
        let maize = conn.last_insert_rowid();
        add_input_tx(&conn, &InputEntry { batch_id: id, entry_date: "2026-06-01".into(), kind: "raw".into(), name: "Maize".into(), quantity: 200.0, product_id: Some(maize), ..Default::default() }, "M").unwrap();
        let stock: f64 = conn.query_row("SELECT stock_qty FROM products WHERE id=?1", [maize], |r| r.get(0)).unwrap();
        assert!((stock - 300.0).abs() < 1e-9);
        let d = batch_detail(&conn, id).unwrap();
        assert_eq!(d.batch.raw_cost_kobo, 8_000_000);
        assert_eq!(d.inputs[0].unit, "kg");
    }
}
