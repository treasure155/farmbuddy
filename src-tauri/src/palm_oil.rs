//! Runs and sales: from the farm or the market, to the barn, to the buyer.
//!
//! A **run** is one lot of produce, filled in with one form of plain questions:
//!   * a processing run: palm fruit → palm oil (+ kernel); cassava → garri,
//!     fufu or cassava flour. Raw material bought (measure, price of one,
//!     how many) or from the farm, transport, which mill and its fee, workers
//!     by name and pay, other expenses by name and price, how much came out.
//!   * a harvest run: cucumber, pepper, vegetables, melon (egusi), water yam
//!     (ete), yam. How much was harvested, workers, transport, other expenses.
//!
//! What came out goes into the barn (Farm stock) in the product's stock unit.
//! A **sale** is recorded with its own plain questions (what, from which run,
//! by which measure, how many, price of one, buyer, payment, transport, other
//! costs). It makes a receipt, takes the right amount out of stock using the
//! farmer's own measure sizes ("1 bag = 120 cups") and counts against its run.
//! When everything from a run is sold the run closes and its result is final:
//!
//!   total cost = raw material + transport + mill fee + workers + other
//!                expenses + selling transport and costs
//!   total sales = every sale from the run
//!   profit or loss = total sales − total cost
//!
//! Many runs can be open at once. Nothing is assumed: measure sizes, prices
//! and quantities are all the farmer's.

use crate::{auth::{self, Role, Session}, farms::{check_len, check_money, check_qty, rows, valid_date}, AppState};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ------------------------------------------------------------------ products

pub struct Product {
    pub key: &'static str,
    pub label: &'static str,
    /// process | harvest
    pub kind: &'static str,
    pub raw: &'static str,
    pub raw_units: &'static [&'static str],
    pub raw_stock_names: &'static [&'static str],
    pub main: &'static str,
    /// Measures this product is commonly sold by; the first is the suggested stock unit.
    /// (unit, how many make one of the first unit when that is fixed by definition, else 0)
    pub main_measures: &'static [(&'static str, f64)],
    pub second: Option<&'static str>,
    pub second_measures: &'static [(&'static str, f64)],
}

const CASSAVA_UNITS: &[&str] = &["bag", "basin", "heap", "pick-up load", "tonne", "kg"];
const CASSAVA_NAMES: &[&str] = &["Cassava", "Cassava tubers", "Cassava roots"];
const GARRI_MEASURES: &[(&str, f64)] = &[("bag", 1.0), ("half bag", 2.0), ("quarter bag", 4.0), ("paint bucket", 0.0), ("custard bucket", 0.0), ("derica", 0.0), ("cup", 0.0), ("kg", 0.0)];

pub const PRODUCTS: &[Product] = &[
    Product { key: "palm_oil", label: "Palm oil", kind: "process", raw: "palm fruit", raw_units: &["basin", "bag", "bunch", "head pan", "tonne", "pick-up load"],
        raw_stock_names: &["Palm fruit", "Palm fruits", "Palm fruit bunches", "Fresh fruit bunches"],
        main: "Palm oil", main_measures: &[("25-litre keg", 1.0), ("litre", 25.0), ("bottle", 0.0), ("drum", 0.0), ("tin", 0.0)],
        second: Some("Palm kernel"), second_measures: &[("bag", 1.0), ("half bag", 2.0), ("paint bucket", 0.0), ("custard bucket", 0.0), ("cup", 0.0), ("basin", 0.0), ("kg", 0.0)] },
    Product { key: "garri", label: "Garri", kind: "process", raw: "cassava", raw_units: CASSAVA_UNITS, raw_stock_names: CASSAVA_NAMES,
        main: "Garri", main_measures: GARRI_MEASURES, second: None, second_measures: &[] },
    Product { key: "fufu", label: "Fufu", kind: "process", raw: "cassava", raw_units: CASSAVA_UNITS, raw_stock_names: CASSAVA_NAMES,
        main: "Fufu", main_measures: &[("bag", 1.0), ("basin", 0.0), ("wrap", 0.0), ("ball", 0.0), ("kg", 0.0)], second: None, second_measures: &[] },
    Product { key: "flour", label: "Cassava flour (lafun)", kind: "process", raw: "cassava", raw_units: CASSAVA_UNITS, raw_stock_names: CASSAVA_NAMES,
        main: "Cassava flour (lafun)", main_measures: GARRI_MEASURES, second: None, second_measures: &[] },
    // Harvest and sell (no processing).
    Product { key: "cucumber", label: "Cucumber", kind: "harvest", raw: "cucumber", raw_units: &[], raw_stock_names: &[],
        main: "Cucumber", main_measures: &[("bag", 1.0), ("basket", 0.0), ("basin", 0.0), ("heap", 0.0), ("piece", 0.0)], second: None, second_measures: &[] },
    Product { key: "pepper", label: "Pepper", kind: "harvest", raw: "pepper", raw_units: &[], raw_stock_names: &[],
        main: "Pepper", main_measures: &[("bag", 1.0), ("basket", 0.0), ("paint bucket", 0.0), ("custard bucket", 0.0), ("cup", 0.0), ("heap", 0.0)], second: None, second_measures: &[] },
    Product { key: "vegetables", label: "Vegetables (ugu, waterleaf…)", kind: "harvest", raw: "vegetables", raw_units: &[], raw_stock_names: &[],
        main: "Vegetables", main_measures: &[("bag", 1.0), ("basket", 0.0), ("bundle", 0.0), ("heap", 0.0)], second: None, second_measures: &[] },
    Product { key: "melon", label: "Melon (egusi)", kind: "harvest", raw: "melon", raw_units: &[], raw_stock_names: &[],
        main: "Melon (egusi)", main_measures: &[("bag", 1.0), ("half bag", 2.0), ("paint bucket", 0.0), ("custard bucket", 0.0), ("derica", 0.0), ("cup", 0.0), ("kg", 0.0)], second: None, second_measures: &[] },
    Product { key: "water_yam", label: "Water yam (ete)", kind: "harvest", raw: "water yam", raw_units: &[], raw_stock_names: &[],
        main: "Water yam (ete)", main_measures: &[("tuber", 1.0), ("heap", 0.0), ("bag", 0.0), ("basket", 0.0)], second: None, second_measures: &[] },
    Product { key: "yam", label: "Yam", kind: "harvest", raw: "yam", raw_units: &[], raw_stock_names: &[],
        main: "Yam", main_measures: &[("tuber", 1.0), ("heap", 0.0), ("hundred tubers", 0.01)], second: None, second_measures: &[] },
];

pub fn product(key: &str) -> Option<&'static Product> { PRODUCTS.iter().find(|p| p.key == key) }
fn part_key(p: &Product, part: &str) -> String { if part == "second" { format!("{}:second", p.key) } else { p.key.to_string() } }

// ------------------------------------------------------------------ selling measures

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Measure {
    pub unit: String,
    /// How many of this measure make one stock unit (1 for the stock unit itself).
    pub per_stock_unit: f64,
    pub price_kobo: i64,
    #[serde(default)] pub is_stock_unit: bool,
}

pub fn measures(conn: &Connection, key: &str) -> Result<Vec<Measure>, String> {
    rows(conn, "SELECT unit,per_stock_unit,price_kobo,is_stock_unit FROM sell_measures WHERE product=?1 ORDER BY is_stock_unit DESC, sort_order, unit", [key], |r| Ok(Measure {
        unit: r.get(0)?, per_stock_unit: r.get(1)?, price_kobo: r.get(2)?, is_stock_unit: r.get::<_, i64>(3)? == 1,
    }))
}

/// Saves how the farmer sells a product. Exactly one measure is the stock unit.
pub fn save_measures(conn: &Connection, key: &str, list: &[Measure]) -> Result<String, String> {
    let list: Vec<&Measure> = list.iter().filter(|m| !m.unit.trim().is_empty()).collect();
    if list.is_empty() { return Err("Choose at least one way you sell it".into()); }
    let stock: Vec<&&Measure> = list.iter().filter(|m| m.is_stock_unit).collect();
    if stock.len() != 1 { return Err("Choose the one measure you count your stock in".into()); }
    for m in &list {
        check_len(m.unit.trim(), 30, "Measure")?;
        check_money(m.price_kobo, "Price")?;
        if !m.is_stock_unit && !(m.per_stock_unit.is_finite() && m.per_stock_unit > 0.0 && m.per_stock_unit <= 1_000_000.0) {
            return Err(format!("How many {} are in one {}?", plural(m.unit.trim()), stock[0].unit.trim()));
        }
    }
    // A product's stock unit cannot change once stock exists in it.
    let old: Option<String> = conn.query_row("SELECT unit FROM sell_measures WHERE product=?1 AND is_stock_unit=1", [key], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    let stock_unit = stock[0].unit.trim().to_string();
    if let Some(old) = old {
        if !old.eq_ignore_ascii_case(&stock_unit) {
            let used: i64 = conn.query_row("SELECT COUNT(*) FROM palm_runs WHERE (product=?1 AND stock_unit=?2 COLLATE NOCASE) OR (product||':second'=?1 AND second_stock_unit=?2 COLLATE NOCASE)", params![key, old], |r| r.get(0)).map_err(|e| e.to_string())?;
            if used > 0 { return Err(format!("Your stock is already counted in {}. Keep {} as the stock measure.", plural(&old), old)); }
        }
    }
    conn.execute("DELETE FROM sell_measures WHERE product=?1", [key]).map_err(|e| e.to_string())?;
    for (i, m) in list.iter().enumerate() {
        conn.execute("INSERT INTO sell_measures(product,unit,per_stock_unit,price_kobo,is_stock_unit,sort_order) VALUES(?1,?2,?3,?4,?5,?6)",
            params![key, m.unit.trim(), if m.is_stock_unit { 1.0 } else { m.per_stock_unit }, m.price_kobo, m.is_stock_unit as i64, i as i64]).map_err(|e| e.to_string())?;
    }
    Ok(stock_unit)
}

pub fn plural(u: &str) -> String { if u.ends_with('s') || u.ends_with("(ete)") { u.to_string() } else if u.ends_with("ch") || u.ends_with("sh") { format!("{u}es") } else { format!("{u}s") } }
fn amount(q: f64, u: &str) -> String { format!("{} {}", crate::db::format_qty(q), if (q - 1.0).abs() < 1e-9 { u.to_string() } else { plural(u) }) }

// ------------------------------------------------------------------ the run form

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(default)]
pub struct NamedAmount { pub name: String, pub amount_kobo: i64 }

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
#[serde(default)]
pub struct RunForm {
    pub id: Option<i64>,
    pub product: String,
    pub start_date: String,
    pub farm_id: Option<i64>,
    // Processing: where the raw material came from.
    /// bought | own | both
    pub source: String,
    pub raw_unit: String,
    pub raw_price_kobo: i64,
    pub bought_qty: f64,
    pub own_qty: f64,
    pub transport_kobo: i64,
    pub mill_name: String,
    pub mill_fee_kobo: i64,
    pub workers: Vec<NamedAmount>,
    pub expenses: Vec<NamedAmount>,
    /// Where it was harvested (harvest runs, optional).
    pub plot: String,
    /// What came out (processing) or was harvested, in one of the selling measures.
    pub made_qty: f64,
    pub made_unit: String,
    pub second_qty: f64,
    pub second_unit: String,
    /// How the farmer sells it, with sizes and prices now.
    pub measures: Vec<Measure>,
    pub second_measures: Vec<Measure>,
    pub notes: String,
}

fn to_stock(measures: &[Measure], unit: &str, qty: f64) -> Result<f64, String> {
    let m = measures.iter().find(|m| m.unit.eq_ignore_ascii_case(unit.trim())).ok_or_else(|| format!("'{unit}' is not one of the ways you sell it. Add it under 'How do you sell it?'"))?;
    Ok(qty / if m.is_stock_unit { 1.0 } else { m.per_stock_unit })
}

fn clean_list(list: &[NamedAmount], what: &str) -> Result<Vec<NamedAmount>, String> {
    let mut out = Vec::new();
    for x in list {
        let name = x.name.trim();
        if name.is_empty() && x.amount_kobo == 0 { continue; }
        check_len(name, 80, "Name")?;
        check_money(x.amount_kobo, "Amount")?;
        if x.amount_kobo <= 0 { return Err(format!("Enter how much was paid for {}", if name.is_empty() { what } else { name })); }
        out.push(NamedAmount { name: name.to_string(), amount_kobo: x.amount_kobo });
    }
    if out.len() > 100 { return Err("Too many lines".into()); }
    Ok(out)
}

fn next_run_no(conn: &Connection, prefix: &str) -> Result<String, String> {
    let mut k: i64 = conn.query_row("SELECT COUNT(*) FROM palm_runs WHERE run_no LIKE ?1", [format!("{prefix}-%")], |r| r.get::<_, i64>(0)).map_err(|e| e.to_string())? + 1;
    loop {
        let no = format!("{prefix}-{k:03}");
        if conn.query_row("SELECT 1 FROM palm_runs WHERE run_no=?1", [&no], |_| Ok(())).optional().map_err(|e| e.to_string())?.is_none() { return Ok(no); }
        k += 1;
    }
}
fn prefix(p: &Product) -> &'static str {
    match p.key { "palm_oil" => "PO", "garri" => "GR", "fufu" => "FF", "flour" => "CF", "cucumber" => "CU", "pepper" => "PP", "vegetables" => "VG", "melon" => "ML", "water_yam" => "WY", _ => "YM" }
}

/// Saves a run from its form: measures, lines and the barn (only the difference moves).
pub fn save_form_tx(conn: &Connection, f: &RunForm, who: &str) -> Result<i64, String> {
    let p = product(&f.product).ok_or("Choose what this run is for")?;
    if !valid_date(&f.start_date) { return Err("Choose the date".into()); }
    let workers = clean_list(&f.workers, "a worker")?;
    let expenses = clean_list(&f.expenses, "an expense")?;
    for (k, name) in [(f.raw_price_kobo, "Price"), (f.transport_kobo, "Transport"), (f.mill_fee_kobo, "Processing fee")] { check_money(k, name)?; }
    for (q, name) in [(f.bought_qty, "Quantity bought"), (f.own_qty, "Quantity from the farm"), (f.made_qty, "Quantity"), (f.second_qty, "Quantity")] { check_qty(q, name)?; }
    for (s, n, name) in [(&f.raw_unit, 30, "Measure"), (&f.mill_name, 120, "Mill"), (&f.plot, 120, "Where"), (&f.notes, 1000, "Notes")] { check_len(s, n, name)?; }
    let process = p.kind == "process";
    if process {
        if !["bought", "own", "both"].contains(&f.source.as_str()) { return Err(format!("Did you buy the {}, or get it from your farm?", p.raw)); }
        if f.raw_unit.trim().is_empty() { return Err(format!("How do you buy the {}? Choose bag, kg, basin…", p.raw)); }
        if f.source != "own" && f.bought_qty <= 0.0 { return Err(format!("How many {} did you buy?", plural(f.raw_unit.trim()))); }
        if f.source != "own" && f.raw_price_kobo <= 0 { return Err(format!("How much is one {} of {}?", f.raw_unit.trim(), p.raw)); }
        if f.source != "bought" && f.own_qty <= 0.0 { return Err(format!("How many {} came from your farm?", plural(f.raw_unit.trim()))); }
    }
    // How the farmer sells it.
    let stock_unit = save_measures(conn, &part_key(p, "main"), &f.measures)?;
    let ms = measures(conn, &part_key(p, "main"))?;
    let made = if f.made_qty > 0.0 { to_stock(&ms, &f.made_unit, f.made_qty)? } else { 0.0 };
    let (second_unit, second_made) = match p.second {
        Some(_) if !f.second_measures.is_empty() => {
            let su = save_measures(conn, &part_key(p, "second"), &f.second_measures)?;
            let sm = measures(conn, &part_key(p, "second"))?;
            (su, if f.second_qty > 0.0 { to_stock(&sm, &f.second_unit, f.second_qty)? } else { 0.0 })
        }
        _ => (String::new(), 0.0),
    };
    let title = if process {
        format!("{} from {}", p.label, amount(f.bought_qty + f.own_qty, f.raw_unit.trim()))
    } else { format!("{} harvest{}", p.label, if f.plot.trim().is_empty() { String::new() } else { format!(" · {}", f.plot.trim()) }) };
    let answers = serde_json::to_string(f).map_err(|e| e.to_string())?;
    let price_now = ms.iter().find(|m| m.is_stock_unit).map(|m| m.price_kobo).unwrap_or(0);
    let id = match f.id {
        Some(id) => {
            // Sales already made cannot be more than what came out.
            let (sold_main, sold_second) = sold(conn, id)?;
            if made + 1e-9 < sold_main { return Err(format!("You have already sold {} from this run. What came out cannot be less than that.", amount(sold_main, &stock_unit))); }
            if second_made + 1e-9 < sold_second { return Err(format!("You have already sold {} of {} from this run.", amount(sold_second, &second_unit), p.second.unwrap_or("").to_lowercase())); }
            let n = conn.execute("UPDATE palm_runs SET product=?1,kind=?2,title=?3,farm_id=?4,start_date=?5,stock_unit=?6,second_stock_unit=?7,oil_unit=?6,kernel_unit=?7,expected_oil_price_kobo=?8,notes=?9,answers=?10 WHERE id=?11",
                params![p.key, p.kind, title, f.farm_id, f.start_date, stock_unit, second_unit, price_now, f.notes.trim(), answers, id]).map_err(|e| e.to_string())?;
            if n == 0 { return Err("Run not found".into()); }
            id
        }
        None => {
            let no = next_run_no(conn, prefix(p))?;
            conn.execute("INSERT INTO palm_runs(run_no,product,kind,title,farm_id,start_date,stock_unit,second_stock_unit,oil_unit,kernel_unit,expected_oil_price_kobo,notes,answers,status)
                VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?7,?8,?9,?10,?11,'open')",
                params![no, p.key, p.kind, title, f.farm_id, f.start_date, stock_unit, second_unit, price_now, f.notes.trim(), answers]).map_err(|e| e.to_string())?;
            conn.last_insert_rowid()
        }
    };
    conn.execute("DELETE FROM palm_run_lines WHERE run_id=?1", [id]).map_err(|e| e.to_string())?;
    let d = &f.start_date;
    let add = |t: &str, c: &str, q: f64, u: &str, price: i64, amount: i64, party: &str| -> Result<(), String> {
        conn.execute("INSERT INTO palm_run_lines(run_id,line_type,category,entry_date,quantity,unit,unit_price_kobo,amount_kobo,party,workers,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![id, t, c, d, q, u, price, amount, party, (c == "labour") as i64, who]).map_err(|e| e.to_string())?;
        Ok(())
    };
    if process {
        if f.source != "own" { add("fruit", "bought", f.bought_qty, f.raw_unit.trim(), f.raw_price_kobo, (f.bought_qty * f.raw_price_kobo as f64).round() as i64, "")?; }
        if f.source != "bought" { add("fruit", "own", f.own_qty, f.raw_unit.trim(), 0, 0, "")?; }
    }
    if f.transport_kobo > 0 { add("cost", "transport", 0.0, "", 0, f.transport_kobo, "")?; }
    if f.mill_fee_kobo > 0 { add("cost", "processing_fee", 0.0, "", 0, f.mill_fee_kobo, f.mill_name.trim())?; }
    for w in &workers { add("cost", "labour", 0.0, "", 0, w.amount_kobo, &w.name)?; }
    for x in &expenses { add("cost", "other", 0.0, "", 0, x.amount_kobo, &x.name)?; }
    if made > 0.0 { add("output", "oil", made, &stock_unit, 0, 0, "")?; }
    if second_made > 0.0 { add("output", "kernel", second_made, &second_unit, 0, 0, "")?; }
    sync_stock(conn, id, p, f, &stock_unit, &second_unit, made, second_made, who)?;
    close_if_sold_out(conn, id)?;
    Ok(id)
}

fn sold(conn: &Connection, run: i64) -> Result<(f64, f64), String> {
    conn.query_row("SELECT COALESCE(SUM(CASE WHEN s.part='main' THEN s.stock_qty END),0), COALESCE(SUM(CASE WHEN s.part='second' THEN s.stock_qty END),0)
        FROM run_sales s LEFT JOIN receipts r ON r.id=s.receipt_id WHERE s.run_id=?1 AND (r.id IS NULL OR r.payment_status<>'void')", [run], |r| Ok((r.get(0)?, r.get(1)?))).map_err(|e| e.to_string())
}

/// A voided receipt from a run gives the run its stock back, so the run opens again.
pub fn receipt_voided(conn: &Connection, receipt_id: i64) -> Result<(), String> {
    let runs: Vec<i64> = rows(conn, "SELECT DISTINCT run_id FROM run_sales WHERE receipt_id=?1", [receipt_id], |r| r.get(0))?;
    for id in runs { close_if_sold_out(conn, id)?; }
    Ok(())
}

/// A run with something made and nothing left is finished: its result is final.
pub fn close_if_sold_out(conn: &Connection, id: i64) -> Result<(), String> {
    let r = run_detail(conn, id)?.run;
    let done = r.oil_made > 0.0 && r.oil_left < 1e-9 && (r.kernel_made == 0.0 || r.kernel_left < 1e-9);
    conn.execute("UPDATE palm_runs SET status=CASE WHEN ?1 THEN 'closed' ELSE 'open' END, end_date=CASE WHEN ?1 THEN COALESCE(NULLIF(end_date,''),date('now','localtime')) ELSE '' END WHERE id=?2",
        params![done, id]).map_err(|e| e.to_string())?;
    Ok(())
}

// ------------------------------------------------------------------ the barn (Farm stock)

#[derive(Serialize, Deserialize, Default, Clone, Copy, Debug)]
pub struct Synced { pub qty: f64, pub unit_cost_kobo: i64 }

fn find_or_create_item(conn: &Connection, name: &str, unit: &str, price: i64) -> Result<i64, String> {
    let found: Option<(i64, i64)> = conn.query_row("SELECT id,selling_price_kobo FROM products WHERE name=?1 COLLATE NOCASE AND unit=?2 COLLATE NOCASE AND active=1 ORDER BY id LIMIT 1", params![name, unit], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional().map_err(|e| e.to_string())?;
    if let Some((id, sp)) = found {
        if price > 0 && sp != price { conn.execute("UPDATE products SET selling_price_kobo=?1 WHERE id=?2", params![price, id]).map_err(|e| e.to_string())?; }
        return Ok(id);
    }
    let base: String = format!("{name}-{unit}").chars().filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-').collect::<String>().to_uppercase().replace(' ', "-");
    let mut sku = format!("RUN-{}", &base[..base.len().min(24)]);
    let mut k = 1;
    while conn.query_row("SELECT 1 FROM products WHERE sku=?1", [&sku], |_| Ok(())).optional().map_err(|e| e.to_string())?.is_some() { k += 1; sku = format!("RUN-{}-{k}", &base[..base.len().min(20)]); }
    conn.execute("INSERT INTO products(sku,name,kind,unit,selling_price_kobo,cost_price_kobo,track_stock,stock_qty,reorder_level,description) VALUES(?1,?2,'product',?3,?4,0,1,0,0,'From your runs')",
        params![sku, name, unit, price.max(1)]).map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn item_for(conn: &Connection, p: &Product, part: &str, unit: &str) -> Result<i64, String> {
    let name = if part == "second" { p.second.unwrap_or(p.main) } else { p.main };
    let price = measures(conn, &part_key(p, part))?.into_iter().find(|m| m.is_stock_unit).map(|m| m.price_kobo).unwrap_or(0);
    find_or_create_item(conn, name, unit, price)
}

fn move_stock(conn: &Connection, pid: i64, delta: f64, unit_cost: i64, kind: &str, note: &str, who: &str) -> Result<(), String> {
    if delta.abs() < 1e-9 { return Ok(()); }
    if delta > 0.0 && kind == "processing_output" { crate::inventory::receive_stock(conn, pid, delta, unit_cost, kind, note, who, None)?; return Ok(()); }
    let cost: i64 = conn.query_row("SELECT cost_price_kobo FROM products WHERE id=?1", [pid], |r| r.get(0)).map_err(|e| e.to_string())?;
    conn.execute("UPDATE products SET stock_qty=stock_qty+?1,updated_at=CURRENT_TIMESTAMP WHERE id=?2", params![delta, pid]).map_err(|e| e.to_string())?;
    conn.execute("INSERT INTO stock_movements(product_id,movement_type,quantity_delta,unit_cost_kobo,note,user_name) VALUES(?1,?2,?3,?4,?5,?6)", params![pid, kind, delta, cost, note, who]).map_err(|e| e.to_string())?;
    if delta < 0.0 { crate::inventory::consume_batches(conn, pid, -delta)?; }
    Ok(())
}

/// Puts everything a run made into the barn (sales take it out again) and takes
/// the farmer's own stored raw material out, moving only the change since the last save.
#[allow(clippy::too_many_arguments)]
fn sync_stock(conn: &Connection, id: i64, p: &Product, f: &RunForm, stock_unit: &str, second_unit: &str, made: f64, second_made: f64, who: &str) -> Result<(), String> {
    let (run_no, prev): (String, String) = conn.query_row("SELECT run_no,stock_sync FROM palm_runs WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).map_err(|e| e.to_string())?;
    let mut prev: BTreeMap<i64, Synced> = serde_json::from_str(&prev).unwrap_or_default();
    let mut next: BTreeMap<i64, Synced> = BTreeMap::new();
    let r = run_detail(conn, id)?.run;
    // The run's costs (before selling costs) are carried by the main product.
    let made_cost = r.total_cost_kobo - r.selling_costs_kobo;
    let note = format!("Run {run_no}");
    if made > 0.0 {
        let pid = item_for(conn, p, "main", stock_unit)?;
        next.insert(pid, Synced { qty: made, unit_cost_kobo: (made_cost as f64 / made).round() as i64 });
    }
    if second_made > 0.0 {
        let pid = item_for(conn, p, "second", second_unit)?;
        next.insert(pid, Synced { qty: second_made, unit_cost_kobo: 0 });
    }
    if p.kind == "process" && f.own_qty > 0.0 {
        for n in p.raw_stock_names {
            let raw: Option<i64> = conn.query_row("SELECT id FROM products WHERE name=?1 COLLATE NOCASE AND unit=?2 COLLATE NOCASE AND active=1 AND track_stock=1 LIMIT 1", params![n, f.raw_unit.trim()], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
            if let Some(pid) = raw { next.insert(pid, Synced { qty: -f.own_qty, unit_cost_kobo: 0 }); break; }
        }
    }
    let ids: std::collections::BTreeSet<i64> = prev.keys().chain(next.keys()).copied().collect();
    for pid in ids {
        let was = prev.remove(&pid).unwrap_or_default();
        let Some(now) = next.get(&pid).copied().or(if was.qty != 0.0 { Some(Synced::default()) } else { None }) else { continue };
        let delta = now.qty - was.qty;
        let kind = if was.qty < 0.0 || now.qty < 0.0 { "processing_issue" } else if delta > 0.0 { "processing_output" } else { "adjustment" };
        move_stock(conn, pid, delta, now.unit_cost_kobo, kind, &note, who)?;
        if now.qty > 0.0 && now.unit_cost_kobo > 0 && delta.abs() < 1e-9 {
            conn.execute("UPDATE products SET cost_price_kobo=?1 WHERE id=?2 AND stock_qty>0", params![now.unit_cost_kobo, pid]).map_err(|e| e.to_string())?;
        }
    }
    conn.execute("UPDATE palm_runs SET stock_sync=?1 WHERE id=?2", params![serde_json::to_string(&next).unwrap_or_else(|_| "{}".into()), id]).map_err(|e| e.to_string())?;
    Ok(())
}

/// Takes back what a run put into (or took from) stock. Returns the run number.
pub fn undo_stock(conn: &Connection, id: i64, who: &str) -> Result<String, String> {
    let (no, sync): (String, String) = conn.query_row("SELECT run_no,stock_sync FROM palm_runs WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(|e| e.to_string())?.ok_or("Run not found")?;
    let sync: BTreeMap<i64, Synced> = serde_json::from_str(&sync).unwrap_or_default();
    for (pid, s) in sync {
        if conn.query_row("SELECT 1 FROM products WHERE id=?1", [pid], |_| Ok(())).optional().map_err(|e| e.to_string())?.is_some() {
            move_stock(conn, pid, -s.qty, 0, "adjustment", &format!("Run {no} deleted"), who)?;
        }
    }
    conn.execute("UPDATE palm_runs SET stock_sync='{}' WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    Ok(no)
}

/// 1.0.4 kept "made − sold" in the barn; from 1.0.5 the barn holds what was made
/// and sales take it out. Old sales already took their stock, so only the record changes.
pub fn upgrade_sync(conn: &Connection) -> Result<(), String> {
    let runs: Vec<(i64, String)> = rows(conn, "SELECT id,stock_sync FROM palm_runs", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
    for (id, s) in runs {
        let mut sync: BTreeMap<i64, Synced> = serde_json::from_str(&s).unwrap_or_default();
        let (sold_main, sold_second) = sold(conn, id)?;
        let names: Vec<(i64, String)> = sync.keys().map(|pid| conn.query_row("SELECT id,name FROM products WHERE id=?1", [pid], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(|e| e.to_string())).collect::<Result<Vec<_>, _>>()?.into_iter().flatten().collect();
        for (pid, name) in names {
            if let Some(x) = sync.get_mut(&pid) {
                if x.qty > 0.0 { x.qty += if name.eq_ignore_ascii_case("Palm kernel") { sold_second } else { sold_main }; }
            }
        }
        conn.execute("UPDATE palm_runs SET stock_sync=?1 WHERE id=?2", params![serde_json::to_string(&sync).unwrap_or_else(|_| "{}".into()), id]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ------------------------------------------------------------------ sales

#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct SaleForm {
    pub run_id: i64,
    /// main | second (palm kernel)
    pub part: String,
    pub sale_date: String,
    pub unit: String,
    pub quantity: f64,
    pub price_kobo: i64,
    pub buyer: String,
    pub buyer_phone: String,
    /// Cash, Transfer, POS… (a payment method name)
    pub method: String,
    /// What the buyer paid now (the rest is owed).
    pub paid_kobo: i64,
    pub transport_kobo: i64,
    pub other_kobo: i64,
}

#[derive(Serialize, Debug)]
pub struct SaleResult { pub sale_id: i64, pub receipt_id: i64, pub receipt_no: String, pub run_finished: bool }

pub fn record_sale_tx(tx: &Transaction<'_>, s: &SaleForm, me: &Session) -> Result<SaleResult, String> {
    let (run_no, key, status, stock_unit, second_unit): (String, String, String, String, String) = tx.query_row("SELECT run_no,product,status,stock_unit,second_stock_unit FROM palm_runs WHERE id=?1", [s.run_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
        .optional().map_err(|e| e.to_string())?.ok_or("Choose which run this is from")?;
    let p = product(&key).ok_or("Unknown product")?;
    let part = if s.part == "second" && p.second.is_some() { "second" } else { "main" };
    if status != "open" { return Err(format!("Run {run_no} is finished: everything from it is sold.")); }
    if !valid_date(&s.sale_date) { return Err("Choose the date of the sale".into()); }
    check_qty(s.quantity, "Quantity")?;
    if s.quantity <= 0.0 { return Err("How many did you sell?".into()); }
    for (k, n) in [(s.price_kobo, "Price"), (s.paid_kobo, "Amount paid"), (s.transport_kobo, "Transport"), (s.other_kobo, "Other costs")] { check_money(k, n)?; }
    if s.price_kobo <= 0 { return Err(format!("How much did you sell one {} for?", s.unit.trim())); }
    check_len(&s.buyer, 120, "Buyer")?;
    check_len(&s.buyer_phone, 40, "Phone")?;
    let ms = measures(tx, &part_key(p, part))?;
    let stock_qty = to_stock(&ms, &s.unit, s.quantity)?;
    let r = run_detail(tx, s.run_id)?.run;
    let left = if part == "second" { r.kernel_left } else { r.oil_left };
    let unit_name = if part == "second" { &second_unit } else { &stock_unit };
    if stock_qty > left + 1e-6 {
        return Err(format!("Run {run_no} has only {} left. You are selling {} ({}).", amount(left, unit_name), amount(s.quantity, s.unit.trim()), amount(stock_qty, unit_name)));
    }
    let total = ((s.quantity * s.price_kobo as f64).round()) as i64;
    let paid = s.paid_kobo.min(total);
    if paid < total && s.buyer.trim().is_empty() { return Err("The buyer has not paid everything. Add the buyer's name so you can follow up.".into()); }
    let method = if s.method.trim().is_empty() { "Cash".to_string() } else { s.method.trim().to_string() };
    if paid > 0 {
        let ok: Option<String> = tx.query_row("SELECT name FROM payment_methods WHERE name=?1 COLLATE NOCASE AND active=1", [&method], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
        if ok.is_none() { return Err(format!("Payment method '{method}' is not available")); }
    }
    let pid = item_for(tx, p, part, unit_name)?;
    let unit_cost: i64 = tx.query_row("SELECT cost_price_kobo FROM products WHERE id=?1", [pid], |r| r.get(0)).map_err(|e| e.to_string())?;
    let name = if part == "second" { p.second.unwrap_or(p.main) } else { p.main };

    // ---- the receipt
    let prefix: String = crate::db::setting(tx, "receipt_prefix")?.chars().filter(|c| c.is_ascii_alphanumeric()).take(8).collect::<String>().to_uppercase();
    let prefix = if prefix.is_empty() { "RCP".to_string() } else { prefix };
    let next_id: i64 = tx.query_row("SELECT COALESCE(MAX(id),0)+1 FROM receipts", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    let receipt_no = format!("{prefix}-{}-{:05}", chrono::Local::now().format("%y%m"), next_id);
    let who = me.display_name();
    let customer_id: Option<i64> = if s.buyer.trim().is_empty() { None } else {
        tx.query_row("SELECT id FROM customers WHERE name=?1 COLLATE NOCASE LIMIT 1", [s.buyer.trim()], |r| r.get(0)).optional().map_err(|e| e.to_string())?
    };
    tx.execute("INSERT INTO receipts(receipt_no,customer_id,customer_name,customer_phone,customer_email,cashier_id,cashier_name,subtotal_kobo,discount_kobo,tax_kobo,total_kobo,amount_paid_kobo,change_kobo,payment_status,fulfilment_status,collection_date,note,created_at)
        VALUES(?1,?2,?3,?4,'',?5,?6,?7,0,0,?7,?8,0,?9,'none','',?10,?11)",
        params![receipt_no, customer_id, s.buyer.trim(), s.buyer_phone.trim(), me.user_id, who, total, paid, crate::sales::status_for(paid, total), format!("From run {run_no}"),
            format!("{} {}", s.sale_date, chrono::Local::now().format("%H:%M:%S"))]).map_err(|e| e.to_string())?;
    let receipt_id = tx.last_insert_rowid();
    let line_cost = if s.quantity > 0.0 { ((unit_cost as f64) * stock_qty / s.quantity).round() as i64 } else { 0 };
    tx.execute("INSERT INTO receipt_items(receipt_id,product_id,product_name,product_kind,unit,quantity,unit_price_kobo,unit_cost_kobo,line_total_kobo,stock_deducted,base_quantity,sold_as) VALUES(?1,?2,?3,'product',?4,?5,?6,?7,?8,1,?9,'')",
        params![receipt_id, pid, name, s.unit.trim(), s.quantity, s.price_kobo, line_cost, total, stock_qty]).map_err(|e| e.to_string())?;
    tx.execute("UPDATE products SET stock_qty=stock_qty-?1,updated_at=CURRENT_TIMESTAMP WHERE id=?2", params![stock_qty, pid]).map_err(|e| e.to_string())?;
    tx.execute("INSERT INTO stock_movements(product_id,movement_type,quantity_delta,unit_cost_kobo,note,receipt_id,user_name) VALUES(?1,'sale',?2,?3,?4,?5,?6)",
        params![pid, -stock_qty, unit_cost, format!("Sale {receipt_no} (run {run_no})"), receipt_id, who]).map_err(|e| e.to_string())?;
    crate::inventory::consume_batches(tx, pid, stock_qty)?;
    if paid > 0 {
        let shift = crate::shifts::open_shift_id(tx, me.user_id)?;
        tx.execute("INSERT INTO receipt_payments(receipt_id,method,amount_kobo,reference,received_by,shift_id) VALUES(?1,?2,?3,'',?4,?5)", params![receipt_id, method, paid, who, shift]).map_err(|e| e.to_string())?;
    }
    tx.execute("INSERT INTO run_sales(run_id,part,sale_date,unit,quantity,stock_qty,price_kobo,amount_kobo,transport_kobo,other_kobo,buyer,receipt_id,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![s.run_id, part, s.sale_date, s.unit.trim(), s.quantity, stock_qty, s.price_kobo, total, s.transport_kobo, s.other_kobo, s.buyer.trim(), receipt_id, who]).map_err(|e| e.to_string())?;
    let sale_id = tx.last_insert_rowid();
    close_if_sold_out(tx, s.run_id)?;
    let finished: String = tx.query_row("SELECT status FROM palm_runs WHERE id=?1", [s.run_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    Ok(SaleResult { sale_id, receipt_id, receipt_no, run_finished: finished == "closed" })
}

/// A sale made in "Sell produce" of an item that came from runs (garri, palm oil,
/// kernel, pepper…) is counted against the open runs of that item, oldest first,
/// so the run's sales, what is left and its profit stay right. Stock was already
/// taken out by the receipt. Anything beyond what the runs have left (stock added
/// by hand) is simply not tied to a run.
pub fn link_pos_sale(conn: &Connection, receipt_id: i64, who: &str) -> Result<(), String> {
    let items: Vec<(String, String, f64, f64, i64, i64)> = rows(conn, "SELECT p.name, p.unit, COALESCE(NULLIF(i.base_quantity,0), i.quantity), i.quantity, i.unit_price_kobo, i.line_total_kobo
        FROM receipt_items i JOIN products p ON p.id=i.product_id WHERE i.receipt_id=?1", [receipt_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?;
    let day: String = conn.query_row("SELECT substr(created_at,1,10) FROM receipts WHERE id=?1", [receipt_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    let buyer: String = conn.query_row("SELECT customer_name FROM receipts WHERE id=?1", [receipt_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    for (name, unit, base_qty, qty, price, total) in items {
        if base_qty <= 0.0 { continue; }
        for p in PRODUCTS {
            let part = if p.main.eq_ignore_ascii_case(&name) { "main" } else if p.second.map(|x| x.eq_ignore_ascii_case(&name)).unwrap_or(false) { "second" } else { continue };
            let col = if part == "main" { "stock_unit" } else { "second_stock_unit" };
            let runs: Vec<i64> = rows(conn, &format!("SELECT id FROM palm_runs WHERE product=?1 AND status='open' AND {col}=?2 COLLATE NOCASE ORDER BY start_date, id"), params![p.key, unit], |r| r.get(0))?;
            let mut rest = base_qty;
            for run in runs {
                if rest <= 1e-9 { break; }
                let r = run_detail(conn, run)?.run;
                let left = if part == "main" { r.oil_left } else { r.kernel_left };
                let take = rest.min(left);
                if take <= 1e-9 { continue; }
                let share = take / base_qty;
                conn.execute("INSERT INTO run_sales(run_id,part,sale_date,unit,quantity,stock_qty,price_kobo,amount_kobo,buyer,receipt_id,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                    params![run, part, day, unit, qty * share, take, price, (total as f64 * share).round() as i64, buyer, receipt_id, who]).map_err(|e| e.to_string())?;
                close_if_sold_out(conn, run)?;
                rest -= take;
            }
            break;
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ the sums

#[derive(Serialize, Debug, Clone, Default)]
pub struct Qty { pub unit: String, pub quantity: f64 }
#[derive(Serialize, Debug, Clone, Default)]
pub struct CostLine { pub category: String, pub label: String, pub detail: String, pub amount_kobo: i64 }
#[derive(Serialize, Debug, Clone)]
pub struct SaleLine { pub id: i64, pub part: String, pub sale_date: String, pub unit: String, pub quantity: f64, pub stock_qty: f64, pub price_kobo: i64, pub amount_kobo: i64, pub transport_kobo: i64, pub other_kobo: i64, pub buyer: String, pub receipt_id: Option<i64>, pub receipt_no: String }

#[derive(Serialize, Debug, Default)]
pub struct RunSummary {
    pub id: i64, pub run_no: String, pub product: String, pub kind: String, pub product_label: String, pub raw_label: String, pub main_label: String, pub second_label: String,
    pub title: String, pub farm_id: Option<i64>, pub farm_name: String, pub start_date: String, pub end_date: String, pub status: String,
    /// Stock units (kept as oil_unit / kernel_unit for older screens and rules).
    pub oil_unit: String, pub kernel_unit: String, pub expected_oil_price_kobo: i64, pub expected_kernel_price_kobo: i64, pub notes: String, pub is_demo: bool,
    pub fruit: Vec<Qty>, pub fruit_own: Vec<Qty>, pub fruit_cost_kobo: i64, pub avg_fruit_price_kobo: Option<i64>,
    pub costs: Vec<CostLine>, pub labour_workers: i64, pub selling_costs_kobo: i64, pub total_cost_kobo: i64,
    pub oil_made: f64, pub kernel_made: f64, pub oil_sold: f64, pub kernel_sold: f64, pub oil_left: f64, pub kernel_left: f64,
    pub oil_sales_kobo: i64, pub kernel_sales_kobo: i64, pub total_sales_kobo: i64, pub profit_kobo: i64,
    pub oil_unsold: f64, pub kernel_unsold: f64, pub unsold_value_kobo: i64, pub projected_profit_kobo: i64,
    pub avg_oil_price_kobo: Option<i64>, pub avg_kernel_price_kobo: Option<i64>, pub cost_per_oil_unit_kobo: Option<i64>, pub break_even_oil_price_kobo: Option<i64>,
    pub oil_per_fruit_unit: Option<f64>, pub fruit_unit: String,
    pub sales: Vec<SaleLine>,
}

#[derive(Serialize, Debug)]
pub struct RunDetail { pub run: RunSummary, pub form: serde_json::Value }

fn per(amount: i64, qty: f64) -> Option<i64> { if qty > 1e-9 { Some((amount as f64 / qty).round() as i64) } else { None } }
fn add_qty(list: &mut Vec<Qty>, unit: &str, q: f64) { match list.iter_mut().find(|x| x.unit.eq_ignore_ascii_case(unit)) { Some(x) => x.quantity += q, None => list.push(Qty { unit: unit.to_string(), quantity: q }) } }

pub fn run_detail(conn: &Connection, id: i64) -> Result<RunDetail, String> {
    let mut answers = String::new();
    let mut s = conn.query_row("SELECT r.id,r.run_no,r.product,r.kind,r.title,r.farm_id,COALESCE(f.name,''),r.start_date,r.end_date,r.status,r.stock_unit,r.second_stock_unit,r.notes,r.is_demo,r.answers
        FROM palm_runs r LEFT JOIN farms f ON f.id=r.farm_id WHERE r.id=?1", [id], |r| {
        answers = r.get(14)?;
        Ok(RunSummary { id: r.get(0)?, run_no: r.get(1)?, product: r.get(2)?, kind: r.get(3)?, title: r.get(4)?, farm_id: r.get(5)?, farm_name: r.get(6)?, start_date: r.get(7)?, end_date: r.get(8)?,
            status: r.get(9)?, oil_unit: r.get(10)?, kernel_unit: r.get(11)?, notes: r.get(12)?, is_demo: r.get::<_, i64>(13)? == 1, ..Default::default() })
    }).optional().map_err(|e| e.to_string())?.ok_or("Run not found")?;
    let p = product(&s.product).unwrap_or(&PRODUCTS[0]);
    s.product_label = p.label.into(); s.raw_label = p.raw.into(); s.main_label = p.main.into(); s.second_label = p.second.unwrap_or("").into();
    let price_of = |key: String| -> Result<i64, String> { Ok(measures(conn, &key)?.into_iter().find(|m| m.is_stock_unit).map(|m| m.price_kobo).unwrap_or(0)) };
    s.expected_oil_price_kobo = price_of(part_key(p, "main"))?;
    s.expected_kernel_price_kobo = if p.second.is_some() { price_of(part_key(p, "second"))? } else { 0 };

    let lines: Vec<(String, String, f64, String, i64, String)> = rows(conn, "SELECT line_type,category,quantity,unit,amount_kobo,party FROM palm_run_lines WHERE run_id=?1 ORDER BY id", [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?;
    let mut costs: Vec<CostLine> = vec![];
    let mut push = |category: &str, label: &str, detail: String, amount: i64| {
        if amount <= 0 { return; }
        match costs.iter_mut().find(|c| c.category == category) {
            Some(c) => { c.amount_kobo += amount; if !detail.is_empty() { c.detail = if c.detail.is_empty() { detail } else { format!("{}, {detail}", c.detail) }; } }
            None => costs.push(CostLine { category: category.into(), label: label.into(), detail, amount_kobo: amount }),
        }
    };
    for (t, c, q, u, amount_k, party) in &lines {
        match t.as_str() {
            "fruit" => { if c == "own" { add_qty(&mut s.fruit_own, u, *q) } else { add_qty(&mut s.fruit, u, *q); s.fruit_cost_kobo += amount_k; } }
            "cost" => {
                let (label, cat) = match c.as_str() {
                    "transport" | "transport_fruit" => ("Transport", "transport"),
                    "processing_fee" => ("Processing (mill) fee", "processing_fee"),
                    "labour" => ("Workers", "labour"),
                    "transport_market" => ("Transport to market", "transport_market"),
                    _ => ("Other expenses", "other"),
                };
                if c == "labour" { s.labour_workers += 1; }
                push(cat, label, if party.is_empty() { String::new() } else if c == "labour" || c == "other" { format!("{party} {}", crate::db::format_money("₦", *amount_k).replace(".00", "").replace("₦ ", "₦")) } else { party.clone() }, *amount_k);
            }
            "output" => { if c == "oil" { s.oil_made += q } else { s.kernel_made += q } }
            _ => {}
        }
    }
    s.sales = rows(conn, "SELECT s.id,s.part,s.sale_date,s.unit,s.quantity,s.stock_qty,s.price_kobo,s.amount_kobo,s.transport_kobo,s.other_kobo,s.buyer,s.receipt_id,COALESCE(r.receipt_no,'')
        FROM run_sales s LEFT JOIN receipts r ON r.id=s.receipt_id WHERE s.run_id=?1 AND (r.id IS NULL OR r.payment_status<>'void') ORDER BY s.sale_date, s.id", [id], |r| Ok(SaleLine {
        id: r.get(0)?, part: r.get(1)?, sale_date: r.get(2)?, unit: r.get(3)?, quantity: r.get(4)?, stock_qty: r.get(5)?, price_kobo: r.get(6)?, amount_kobo: r.get(7)?, transport_kobo: r.get(8)?, other_kobo: r.get(9)?,
        buyer: r.get(10)?, receipt_id: r.get(11)?, receipt_no: r.get(12)?,
    }))?;
    for x in &s.sales {
        if x.part == "second" { s.kernel_sold += x.stock_qty; s.kernel_sales_kobo += x.amount_kobo; } else { s.oil_sold += x.stock_qty; s.oil_sales_kobo += x.amount_kobo; }
    }
    let sale_transport: i64 = s.sales.iter().map(|x| x.transport_kobo).sum();
    let sale_other: i64 = s.sales.iter().map(|x| x.other_kobo).sum();
    push("sale_transport", "Transport for selling", String::new(), sale_transport);
    push("sale_other", "Other selling costs", String::new(), sale_other);
    s.selling_costs_kobo = sale_transport + sale_other;
    let purchase = CostLine { category: "fruit".into(), label: format!("Buying the {}", p.raw), detail: String::new(), amount_kobo: s.fruit_cost_kobo };
    let order = ["transport", "processing_fee", "labour", "other", "transport_market", "sale_transport", "sale_other"];
    let mut ordered = if p.kind == "process" { vec![purchase] } else { vec![] };
    for k in order { if let Some(c) = costs.iter().find(|c| c.category == k) { ordered.push(c.clone()); } }
    s.total_cost_kobo = ordered.iter().map(|c| c.amount_kobo).sum();
    s.costs = ordered;
    if s.fruit.len() == 1 { s.avg_fruit_price_kobo = per(s.fruit_cost_kobo, s.fruit[0].quantity); }
    let all: Vec<&Qty> = s.fruit.iter().chain(s.fruit_own.iter()).collect();
    if let Some(first) = all.first() {
        if all.iter().all(|q| q.unit.eq_ignore_ascii_case(&first.unit)) {
            s.fruit_unit = first.unit.clone();
            let total: f64 = all.iter().map(|q| q.quantity).sum();
            if total > 0.0 && s.oil_made > 0.0 { s.oil_per_fruit_unit = Some(s.oil_made / total); }
        }
    }
    s.total_sales_kobo = s.oil_sales_kobo + s.kernel_sales_kobo;
    s.profit_kobo = s.total_sales_kobo - s.total_cost_kobo;
    s.oil_left = (s.oil_made - s.oil_sold).max(0.0);
    s.kernel_left = (s.kernel_made - s.kernel_sold).max(0.0);
    s.oil_unsold = s.oil_left; s.kernel_unsold = s.kernel_left;
    s.unsold_value_kobo = ((s.oil_left * s.expected_oil_price_kobo as f64) + (s.kernel_left * s.expected_kernel_price_kobo as f64)).round() as i64;
    s.projected_profit_kobo = s.profit_kobo + s.unsold_value_kobo;
    s.avg_oil_price_kobo = per(s.oil_sales_kobo, s.oil_sold);
    s.avg_kernel_price_kobo = per(s.kernel_sales_kobo, s.kernel_sold);
    s.cost_per_oil_unit_kobo = per(s.total_cost_kobo - s.selling_costs_kobo, s.oil_made);
    let other = s.kernel_sales_kobo + (s.kernel_left * s.expected_kernel_price_kobo as f64).round() as i64;
    s.break_even_oil_price_kobo = per((s.total_cost_kobo - other).max(0), s.oil_made);
    Ok(RunDetail { run: s, form: serde_json::from_str(&answers).unwrap_or(serde_json::json!({})) })
}

// ------------------------------------------------------------------ commands

#[derive(Serialize)]
pub struct ProductOption {
    key: &'static str, label: &'static str, kind: &'static str, raw: &'static str, raw_units: &'static [&'static str], main: &'static str, second: Option<&'static str>,
    /// The farmer's saved measures, or the usual ones to start from (sizes blank unless fixed by definition).
    measures: Vec<Measure>, second_measures: Vec<Measure>, saved: bool,
}

fn suggested(list: &[(&str, f64)]) -> Vec<Measure> {
    list.iter().enumerate().map(|(i, (u, per))| Measure { unit: u.to_string(), per_stock_unit: *per, price_kobo: 0, is_stock_unit: i == 0 }).collect()
}

#[tauri::command]
pub fn process_products(state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<ProductOption>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    product_options(&*state.db()?)
}

pub fn product_options(conn: &Connection) -> Result<Vec<ProductOption>, String> {
    PRODUCTS.iter().map(|p| {
        let saved = measures(conn, &part_key(p, "main"))?;
        let saved2 = if p.second.is_some() { measures(conn, &part_key(p, "second"))? } else { vec![] };
        Ok(ProductOption { key: p.key, label: p.label, kind: p.kind, raw: p.raw, raw_units: p.raw_units, main: p.main, second: p.second,
            saved: !saved.is_empty(), measures: if saved.is_empty() { suggested(p.main_measures) } else { saved },
            second_measures: if p.second.is_none() { vec![] } else if saved2.is_empty() { suggested(p.second_measures) } else { saved2 } })
    }).collect()
}

/// Money fields are blanked for roles below Manager.
#[tauri::command]
pub fn list_palm_runs(state: tauri::State<'_, AppState>, session_token: String) -> Result<serde_json::Value, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let conn = state.db()?;
    let allowed = crate::farm_access::allowed(&conn, &me)?;
    let ids: Vec<i64> = rows(&conn, "SELECT id FROM palm_runs ORDER BY status DESC, start_date DESC, id DESC LIMIT 300", [], |r| r.get(0))?;
    let out: Vec<RunSummary> = ids.into_iter().map(|id| run_detail(&conn, id).map(|d| d.run)).collect::<Result<Vec<_>, _>>()?
        .into_iter().filter(|r| crate::farm_access::visible(&allowed, r.farm_id)).collect();
    auth::money_view(&me, &out)
}

#[tauri::command]
pub fn get_palm_run(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<serde_json::Value, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let d = run_detail(&*state.db()?, id)?;
    if me.role < Role::Manager { return auth::money_view(&me, &d.run); }
    serde_json::to_value(&d).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_process_run(state: tauri::State<'_, AppState>, session_token: String, form: RunForm) -> Result<i64, String> {
    let me = auth::require(&state, &session_token, Role::Manager)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let id = save_form_tx(&tx, &form, &me.display_name())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(id)
}

/// Cashiers can sell; the receipt is made in the same step.
#[tauri::command]
pub fn record_run_sale(state: tauri::State<'_, AppState>, session_token: String, sale: SaleForm) -> Result<SaleResult, String> {
    let me = auth::require(&state, &session_token, Role::Cashier)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let r = record_sale_tx(&tx, &sale, &me)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(r)
}

#[tauri::command]
pub fn delete_palm_run(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<(), String> {
    let me = auth::require(&state, &session_token, Role::Owner)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let (main, second) = sold(&tx, id)?;
    if main + second > 0.0 { return Err("This run has sales. Void its receipts first (Receipts → open → Void), then delete the run.".into()); }
    let no = undo_stock(&tx, id, &me.display_name())?;
    tx.execute("DELETE FROM run_sales WHERE run_id=?1", [id]).map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM palm_runs WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    crate::audit::log(&tx, &me.display_name(), "run_deleted", "palm_run", Some(id), &format!("Deleted run {no}"));
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// Value put into the barn by runs started in the period (for the profit and loss:
/// it returns as cost of goods sold when receipts sell it).
pub fn carried_to_stock(conn: &Connection, from: &str, to: &str, include_demo: bool) -> Result<i64, String> {
    let syncs: Vec<String> = rows(conn, "SELECT stock_sync FROM palm_runs WHERE start_date BETWEEN ?1 AND ?2 AND (?3=1 OR is_demo=0)", params![from, to, include_demo as i64], |r| r.get(0))?;
    Ok(syncs.iter().map(|s| serde_json::from_str::<BTreeMap<i64, Synced>>(s).unwrap_or_default().values().filter(|x| x.qty > 0.0).map(|x| (x.qty * x.unit_cost_kobo as f64).round() as i64).sum::<i64>()).sum())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn me() -> Session { Session { user_id: 1, username: "o".into(), full_name: "Owner".into(), role: Role::Owner, last_seen: std::time::Instant::now() } }
    fn garri_measures() -> Vec<Measure> {
        vec![
            Measure { unit: "bag".into(), per_stock_unit: 1.0, price_kobo: 6_000_000, is_stock_unit: true },
            Measure { unit: "half bag".into(), per_stock_unit: 2.0, price_kobo: 3_100_000, is_stock_unit: false },
            Measure { unit: "custard bucket".into(), per_stock_unit: 24.0, price_kobo: 280_000, is_stock_unit: false },
            Measure { unit: "cup".into(), per_stock_unit: 120.0, price_kobo: 60_000, is_stock_unit: false },
        ]
    }
    fn garri() -> RunForm {
        RunForm { product: "garri".into(), start_date: "2026-11-01".into(), source: "bought".into(), raw_unit: "bag".into(), raw_price_kobo: 1_500_000, bought_qty: 10.0,
            transport_kobo: 1_000_000, mill_name: "Ikot Ekpene mill".into(), mill_fee_kobo: 2_000_000,
            workers: vec![NamedAmount { name: "Arit".into(), amount_kobo: 500_000 }, NamedAmount { name: "Emem".into(), amount_kobo: 500_000 }],
            expenses: vec![NamedAmount { name: "Firewood".into(), amount_kobo: 300_000 }, NamedAmount { name: "Palm oil".into(), amount_kobo: 200_000 }],
            made_qty: 6.0, made_unit: "bag".into(), measures: garri_measures(), ..Default::default() }
    }
    fn conn() -> Connection {
        let c = crate::db::test_conn();
        c.execute("INSERT OR IGNORE INTO users(id,username,full_name,password_hash,role) VALUES(1,'o','Owner','x','owner')", []).unwrap();
        c
    }
    fn stock(conn: &Connection, name: &str) -> f64 { conn.query_row("SELECT COALESCE(SUM(stock_qty),0) FROM products WHERE name=?1", [name], |r| r.get(0)).unwrap() }
    fn sale(run: i64, unit: &str, q: f64, price: i64) -> SaleForm {
        SaleForm { run_id: run, part: "main".into(), sale_date: "2026-11-05".into(), unit: unit.into(), quantity: q, price_kobo: price, method: "Cash".into(), paid_kobo: (q * price as f64) as i64, ..Default::default() }
    }

    /// The whole cycle: the run fills the barn, sales by bag, half bag and cup empty it, the result comes out.
    #[test]
    fn garri_run_then_sales_balance_stock_and_close_the_run() {
        let mut conn = conn();
        let tx = conn.transaction().unwrap();
        let id = save_form_tx(&tx, &garri(), "M").unwrap();
        let r = run_detail(&tx, id).unwrap().run;
        assert_eq!(r.run_no, "GR-001");
        assert_eq!(r.total_cost_kobo, 15_000_000 + 1_000_000 + 2_000_000 + 1_000_000 + 500_000, "₦195,000");
        assert_eq!(r.labour_workers, 2);
        assert!(r.costs.iter().any(|c| c.category == "labour" && c.detail.contains("Arit")));
        assert_eq!(stock(&tx, "Garri"), 6.0, "6 bags in the barn");
        assert_eq!(r.cost_per_oil_unit_kobo, Some(3_250_000), "₦195,000 ÷ 6 bags");

        // 4 bags, 1 half bag, 60 cups = 4 + 0.5 + 0.5 = 5 bags.
        record_sale_tx(&tx, &sale(id, "bag", 4.0, 6_000_000), &me()).unwrap();
        record_sale_tx(&tx, &sale(id, "half bag", 1.0, 3_100_000), &me()).unwrap();
        let mut s = sale(id, "cup", 60.0, 60_000); s.transport_kobo = 200_000; s.buyer = "Mma Ekaette".into(); s.paid_kobo = 2_000_000;
        let res = record_sale_tx(&tx, &s, &me()).unwrap();
        assert!(!res.run_finished);
        assert!((stock(&tx, "Garri") - 1.0).abs() < 1e-9, "1 bag left");
        let owed: i64 = tx.query_row("SELECT total_kobo-amount_paid_kobo FROM receipts WHERE id=?1", [res.receipt_id], |r| r.get(0)).unwrap();
        assert_eq!(owed, 1_600_000, "60 cups × ₦600 = ₦36,000, paid ₦20,000");
        // Selling more than is left is refused in plain words.
        let e = record_sale_tx(&tx, &sale(id, "custard bucket", 30.0, 280_000), &me()).unwrap_err();
        assert!(e.contains("only 1 bag left"), "{e}");
        // The last bag, as 24 custard buckets: the run closes and the result is final.
        let last = record_sale_tx(&tx, &sale(id, "custard bucket", 24.0, 280_000), &me()).unwrap();
        assert!(last.run_finished);
        let r = run_detail(&tx, id).unwrap().run;
        assert_eq!(r.status, "closed");
        assert!(stock(&tx, "Garri").abs() < 1e-9);
        assert_eq!(r.total_sales_kobo, 24_000_000 + 3_100_000 + 3_600_000 + 6_720_000);
        assert_eq!(r.total_cost_kobo, 19_500_000 + 200_000, "selling transport is a cost of the run");
        assert_eq!(r.profit_kobo, 37_420_000 - 19_700_000);
        assert_eq!(r.sales.len(), 4);
        let receipts: i64 = tx.query_row("SELECT COUNT(*) FROM receipts WHERE note='From run GR-001'", [], |r| r.get(0)).unwrap();
        assert_eq!(receipts, 4);
    }

    #[test]
    fn plain_mistakes_are_caught() {
        let conn = crate::db::test_conn();
        let mut f = garri(); f.measures[3].per_stock_unit = 0.0;
        assert!(save_form_tx(&conn, &f, "M").unwrap_err().contains("How many cups are in one bag"));
        let mut f = garri(); f.raw_price_kobo = 0;
        assert!(save_form_tx(&conn, &f, "M").unwrap_err().contains("How much is one bag of cassava"));
        let mut f = garri(); f.workers.push(NamedAmount { name: "Udeme".into(), amount_kobo: 0 });
        assert!(save_form_tx(&conn, &f, "M").unwrap_err().contains("Udeme"));
        let mut f = garri(); f.made_unit = "basket".into();
        assert!(save_form_tx(&conn, &f, "M").unwrap_err().contains("not one of the ways you sell it"));
    }

    /// Two runs at once, a harvest run, and editing after a sale.
    #[test]
    fn several_runs_harvests_and_edits() {
        let mut conn = conn();
        let tx = conn.transaction().unwrap();
        let a = save_form_tx(&tx, &garri(), "M").unwrap();
        let b = save_form_tx(&tx, &RunForm { made_qty: 3.0, ..garri() }, "M").unwrap();
        assert_eq!(stock(&tx, "Garri"), 9.0);
        // A bag sold in "Sell produce" counts against the oldest open run (a).
        let pid: i64 = tx.query_row("SELECT id FROM products WHERE name='Garri'", [], |r| r.get(0)).unwrap();
        crate::poultry::tests::pos_sale(&tx, pid, 1.0, 6_000_000);
        assert_eq!(run_detail(&tx, a).unwrap().run.oil_sold, 1.0);
        assert_eq!(stock(&tx, "Garri"), 8.0);
        record_sale_tx(&tx, &sale(b, "bag", 2.0, 6_000_000), &me()).unwrap();
        assert_eq!(run_detail(&tx, a).unwrap().run.oil_sold, 1.0, "a sale counts only against its own run");
        assert_eq!(run_detail(&tx, b).unwrap().run.oil_left, 1.0);
        // Editing what came out cannot go below what was sold; raising it moves only the difference.
        let e = save_form_tx(&tx, &RunForm { id: Some(b), made_qty: 1.0, ..garri() }, "M").unwrap_err();
        assert!(e.contains("already sold 2 bags"), "{e}");
        save_form_tx(&tx, &RunForm { id: Some(b), made_qty: 4.0, ..garri() }, "M").unwrap();
        assert_eq!(stock(&tx, "Garri"), 7.0, "6 + 4 − 2 − 1 sold");
        // A pepper harvest: no buying or mill, sold by the basket.
        let pepper = RunForm { product: "pepper".into(), start_date: "2026-11-02".into(), plot: "Behind the house".into(),
            workers: vec![NamedAmount { name: "Harvest help".into(), amount_kobo: 300_000 }], transport_kobo: 200_000, made_qty: 5.0, made_unit: "bag".into(),
            measures: vec![Measure { unit: "bag".into(), per_stock_unit: 1.0, price_kobo: 4_000_000, is_stock_unit: true }, Measure { unit: "basket".into(), per_stock_unit: 4.0, price_kobo: 1_100_000, is_stock_unit: false }], ..Default::default() };
        let h = save_form_tx(&tx, &pepper, "M").unwrap();
        let r = run_detail(&tx, h).unwrap().run;
        assert_eq!(r.run_no, "PP-001");
        assert_eq!(r.kind, "harvest");
        assert_eq!(r.total_cost_kobo, 500_000);
        let ps = record_sale_tx(&tx, &sale(h, "basket", 6.0, 1_100_000), &me()).unwrap();
        assert!((stock(&tx, "Pepper") - 3.5).abs() < 1e-9, "5 bags − 6 baskets (1.5 bags)");
        // Voiding the receipt gives the stock back to the barn and to the run.
        crate::sales::void_receipt_tx(&tx, &me(), ps.receipt_id, "Wrong entry").unwrap();
        assert!((stock(&tx, "Pepper") - 5.0).abs() < 1e-9);
        let r = run_detail(&tx, h).unwrap().run;
        assert_eq!((r.oil_sold, r.status.as_str()), (0.0, "open"));
        // Selling all of run b closes it; voiding that sale opens it again.
        let last = record_sale_tx(&tx, &sale(b, "bag", 2.0, 6_000_000), &me()).unwrap();
        assert!(last.run_finished);
        crate::sales::void_receipt_tx(&tx, &me(), last.receipt_id, "Buyer cancelled").unwrap();
        assert_eq!(run_detail(&tx, b).unwrap().run.status, "open");
    }
}
