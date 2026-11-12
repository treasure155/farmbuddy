//! Suppliers, purchases (goods received) and what is still owed to suppliers.
//!
//! Receiving a purchase adds stock (in base units, so cartons become pieces),
//! updates the weighted average cost and records expiry batches.

use crate::{auth::{self, Role, Session}, db, inventory, AppState};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};

fn too_long(s: &str, max: usize) -> bool { s.chars().count() > max }

// ------------------------------------------------------------------ suppliers

#[derive(Serialize)]
pub struct Supplier { id: i64, name: String, phone: String, email: String, address: String, notes: String, purchase_count: i64, total_bought_kobo: i64, owed_kobo: i64 }

#[tauri::command]
pub fn list_suppliers(state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<Supplier>, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let conn = state.db()?;
    let mut stmt = conn.prepare(
        "SELECT s.id,s.name,s.phone,s.email,s.address,s.notes,COUNT(p.id),COALESCE(SUM(p.total_kobo),0),COALESCE(SUM(MAX(0,p.total_kobo-p.amount_paid_kobo)),0)
         FROM suppliers s LEFT JOIN purchases p ON p.supplier_id=s.id GROUP BY s.id ORDER BY s.name COLLATE NOCASE").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |r| Ok(Supplier { id: r.get(0)?, name: r.get(1)?, phone: r.get(2)?, email: r.get(3)?, address: r.get(4)?, notes: r.get(5)?, purchase_count: r.get(6)?, total_bought_kobo: r.get(7)?, owed_kobo: r.get(8)? })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct SupplierInput { id: Option<i64>, name: String, phone: String, email: String, address: String, notes: String }

#[tauri::command]
pub fn save_supplier(state: tauri::State<'_, AppState>, session_token: String, input: SupplierInput) -> Result<i64, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let name = input.name.trim();
    if name.is_empty() || too_long(name, 120) { return Err("Enter the supplier's name (up to 120 characters)".into()); }
    if too_long(&input.phone, 40) || too_long(&input.email, 254) || too_long(&input.address, 300) || too_long(&input.notes, 500) { return Err("One of the fields is too long".into()); }
    let email = input.email.trim();
    if !email.is_empty() && !crate::sales::valid_email(email) { return Err("Email address is not valid".into()); }
    let conn = state.db()?;
    match input.id {
        Some(id) => {
            conn.execute("UPDATE suppliers SET name=?1,phone=?2,email=?3,address=?4,notes=?5 WHERE id=?6", params![name, input.phone.trim(), email, input.address.trim(), input.notes.trim(), id]).map_err(|e| e.to_string())?;
            conn.execute("UPDATE purchases SET supplier_name=?1 WHERE supplier_id=?2", params![name, id]).map_err(|e| e.to_string())?;
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO suppliers(name,phone,email,address,notes) VALUES(?1,?2,?3,?4,?5)", params![name, input.phone.trim(), email, input.address.trim(), input.notes.trim()]).map_err(|e| e.to_string())?;
            Ok(conn.last_insert_rowid())
        }
    }
}

#[tauri::command]
pub fn delete_supplier(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let conn = state.db()?;
    let owed: i64 = conn.query_row("SELECT COALESCE(SUM(MAX(0,total_kobo-amount_paid_kobo)),0) FROM purchases WHERE supplier_id=?1", [id], |r| r.get(0)).map_err(|e| e.to_string())?;
    if owed > 0 { return Err("You still owe this supplier. Record the payment before deleting.".into()); }
    // Purchases keep the supplier's name as written.
    conn.execute("DELETE FROM suppliers WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

// ------------------------------------------------------------------ purchases

#[derive(Deserialize)]
pub struct PurchaseLineInput {
    pub product_id: i64,
    pub quantity: f64,
    /// Cost of one unit as bought (one carton when `as_pack`).
    pub unit_cost_kobo: i64,
    #[serde(default)]
    pub as_pack: bool,
    #[serde(default)]
    pub batch_no: String,
    #[serde(default)]
    pub expiry_date: String,
}

#[derive(Deserialize)]
pub struct PurchaseInput {
    pub supplier_id: Option<i64>,
    pub supplier_name: Option<String>,
    pub invoice_ref: String,
    pub note: String,
    pub lines: Vec<PurchaseLineInput>,
    pub amount_paid_kobo: i64,
    pub payment_method: String,
}

#[derive(Serialize)]
pub struct PurchaseLine { product_name: String, unit: String, quantity: f64, base_quantity: f64, unit_cost_kobo: i64, line_total_kobo: i64, batch_no: String, expiry_date: String }

#[derive(Serialize)]
pub struct SupplierPayment { amount_kobo: i64, method: String, reference: String, user_name: String, created_at: String }

impl PurchaseDetail { pub fn id(&self) -> i64 { self.id } }

#[derive(Serialize)]
pub struct PurchaseDetail {
    id: i64, purchase_no: String, supplier_id: Option<i64>, supplier_name: String, invoice_ref: String, total_kobo: i64, amount_paid_kobo: i64,
    balance_kobo: i64, note: String, user_name: String, created_at: String, lines: Vec<PurchaseLine>, payments: Vec<SupplierPayment>,
}

pub fn create_purchase_tx(tx: &Transaction<'_>, me: &Session, input: PurchaseInput) -> Result<PurchaseDetail, String> {
    if input.lines.is_empty() { return Err("Add at least one item to the purchase".into()); }
    if input.lines.len() > 500 { return Err("A purchase can hold at most 500 lines".into()); }
    if too_long(&input.invoice_ref, 80) || too_long(&input.note, 500) { return Err("Invoice number or note is too long".into()); }
    let mut supplier_name = input.supplier_name.as_deref().unwrap_or("").trim().to_string();
    if let Some(sid) = input.supplier_id {
        let n: Option<String> = tx.query_row("SELECT name FROM suppliers WHERE id=?1", [sid], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
        supplier_name = n.ok_or("Selected supplier no longer exists")?;
    }
    if too_long(&supplier_name, 120) { return Err("Supplier name is too long".into()); }

    struct L { pid: i64, name: String, unit: String, qty: f64, base: f64, cost: i64, per_base: i64, total: i64, batch: String, expiry: String }
    let mut lines = Vec::new();
    let mut total: i64 = 0;
    for l in &input.lines {
        if !l.quantity.is_finite() || l.quantity <= 0.0 || l.quantity > 10_000_000.0 { return Err("Every quantity must be greater than zero".into()); }
        if l.unit_cost_kobo < 0 { return Err("Cost cannot be negative".into()); }
        let row: Option<(String, String, String, f64, i64)> = tx.query_row("SELECT name,unit,pack_unit,pack_size,track_stock FROM products WHERE id=?1", [l.product_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).optional().map_err(|e| e.to_string())?;
        let (name, unit, pack_unit, pack_size, track) = row.ok_or("One of the items no longer exists")?;
        if track != 1 { return Err(format!("{name} does not track stock, so it cannot be received on a purchase")); }
        if l.as_pack && (pack_unit.is_empty() || pack_size <= 1.0) { return Err(format!("{name} has no pack size set")); }
        let factor = if l.as_pack { pack_size } else { 1.0 };
        if !l.expiry_date.is_empty() && chrono::NaiveDate::parse_from_str(&l.expiry_date, "%Y-%m-%d").is_err() { return Err(format!("Expiry date for {name} is not valid")); }
        if too_long(&l.batch_no, 40) { return Err("Batch number is too long".into()); }
        let line_total = ((l.unit_cost_kobo as f64) * l.quantity).round() as i64;
        total = total.checked_add(line_total).ok_or("Purchase total is too large")?;
        lines.push(L { pid: l.product_id, name, unit: if l.as_pack { pack_unit } else { unit }, qty: l.quantity, base: l.quantity * factor, cost: l.unit_cost_kobo, per_base: ((l.unit_cost_kobo as f64) / factor).round() as i64, total: line_total, batch: l.batch_no.trim().to_string(), expiry: l.expiry_date.clone() });
    }
    if input.amount_paid_kobo < 0 || input.amount_paid_kobo > total { return Err("Amount paid must be between zero and the purchase total".into()); }
    if input.amount_paid_kobo < total && supplier_name.is_empty() { return Err("Name the supplier so the amount owed can be tracked".into()); }

    let next: i64 = tx.query_row("SELECT COALESCE(MAX(id),0)+1 FROM purchases", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    let purchase_no = format!("PUR-{}-{:05}", chrono::Local::now().format("%y%m"), next);
    let who = me.display_name();
    tx.execute("INSERT INTO purchases(purchase_no,supplier_id,supplier_name,invoice_ref,total_kobo,amount_paid_kobo,note,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![purchase_no, input.supplier_id, supplier_name, input.invoice_ref.trim(), total, input.amount_paid_kobo, input.note.trim(), who]).map_err(|e| e.to_string())?;
    let id = tx.last_insert_rowid();
    for l in &lines {
        tx.execute("INSERT INTO purchase_items(purchase_id,product_id,product_name,unit,quantity,base_quantity,unit_cost_kobo,line_total_kobo,batch_no,expiry_date) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![id, l.pid, l.name, l.unit, l.qty, l.base, l.cost, l.total, l.batch, l.expiry]).map_err(|e| e.to_string())?;
        let note = if supplier_name.is_empty() { purchase_no.clone() } else { format!("{purchase_no} from {supplier_name}") };
        inventory::receive_stock(tx, l.pid, l.base, l.per_base, "purchase", &note, &who, if l.expiry.is_empty() { None } else { Some((&l.batch, &l.expiry)) })?;
    }
    if input.amount_paid_kobo > 0 {
        tx.execute("INSERT INTO supplier_payments(purchase_id,amount_kobo,method,reference,user_name) VALUES(?1,?2,?3,'',?4)", params![id, input.amount_paid_kobo, input.payment_method.trim(), who]).map_err(|e| e.to_string())?;
    }
    load_purchase(tx, id)
}

pub fn load_purchase(conn: &Connection, id: i64) -> Result<PurchaseDetail, String> {
    let mut p = conn.query_row("SELECT id,purchase_no,supplier_id,supplier_name,invoice_ref,total_kobo,amount_paid_kobo,note,user_name,datetime(created_at,'localtime') FROM purchases WHERE id=?1", [id], |r| {
        let total: i64 = r.get(5)?;
        let paid: i64 = r.get(6)?;
        Ok(PurchaseDetail { id: r.get(0)?, purchase_no: r.get(1)?, supplier_id: r.get(2)?, supplier_name: r.get(3)?, invoice_ref: r.get(4)?, total_kobo: total, amount_paid_kobo: paid, balance_kobo: (total - paid).max(0), note: r.get(7)?, user_name: r.get(8)?, created_at: r.get(9)?, lines: vec![], payments: vec![] })
    }).optional().map_err(|e| e.to_string())?.ok_or("Purchase not found")?;
    let mut stmt = conn.prepare("SELECT product_name,unit,quantity,base_quantity,unit_cost_kobo,line_total_kobo,batch_no,expiry_date FROM purchase_items WHERE purchase_id=?1 ORDER BY id").map_err(|e| e.to_string())?;
    p.lines = stmt.query_map([id], |r| Ok(PurchaseLine { product_name: r.get(0)?, unit: r.get(1)?, quantity: r.get(2)?, base_quantity: r.get(3)?, unit_cost_kobo: r.get(4)?, line_total_kobo: r.get(5)?, batch_no: r.get(6)?, expiry_date: r.get(7)? }))
        .map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT amount_kobo,method,reference,user_name,datetime(created_at,'localtime') FROM supplier_payments WHERE purchase_id=?1 ORDER BY id").map_err(|e| e.to_string())?;
    p.payments = stmt.query_map([id], |r| Ok(SupplierPayment { amount_kobo: r.get(0)?, method: r.get(1)?, reference: r.get(2)?, user_name: r.get(3)?, created_at: r.get(4)? }))
        .map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(p)
}

#[tauri::command]
pub fn create_purchase(state: tauri::State<'_, AppState>, session_token: String, input: PurchaseInput) -> Result<PurchaseDetail, String> {
    let me = auth::require(&state, &session_token, Role::Manager)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let p = create_purchase_tx(&tx, &me, input)?;
    tx.commit().map_err(|e| format!("Could not save the purchase: {e}"))?;
    Ok(p)
}

#[derive(Serialize)]
pub struct PurchaseSummary { id: i64, purchase_no: String, supplier_name: String, invoice_ref: String, total_kobo: i64, amount_paid_kobo: i64, balance_kobo: i64, item_count: i64, user_name: String, created_at: String }

#[tauri::command]
pub fn list_purchases(state: tauri::State<'_, AppState>, session_token: String, from: String, to: String, supplier_id: Option<i64>, owing_only: Option<bool>) -> Result<Vec<PurchaseSummary>, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let conn = state.db()?;
    let mut stmt = conn.prepare(
        "SELECT p.id,p.purchase_no,p.supplier_name,p.invoice_ref,p.total_kobo,p.amount_paid_kobo,(SELECT COUNT(*) FROM purchase_items i WHERE i.purchase_id=p.id),p.user_name,datetime(p.created_at,'localtime')
         FROM purchases p WHERE date(p.created_at,'localtime') BETWEEN ?1 AND ?2 AND (?3 IS NULL OR p.supplier_id=?3) AND (?4=0 OR p.amount_paid_kobo<p.total_kobo)
         ORDER BY p.id DESC LIMIT 1000").map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![from, to, supplier_id, owing_only.unwrap_or(false) as i64], |r| {
        let total: i64 = r.get(4)?;
        let paid: i64 = r.get(5)?;
        Ok(PurchaseSummary { id: r.get(0)?, purchase_no: r.get(1)?, supplier_name: r.get(2)?, invoice_ref: r.get(3)?, total_kobo: total, amount_paid_kobo: paid, balance_kobo: (total - paid).max(0), item_count: r.get(6)?, user_name: r.get(7)?, created_at: r.get(8)? })
    }).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_purchase(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<PurchaseDetail, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let conn = state.db()?;
    load_purchase(&conn, id)
}

#[tauri::command]
pub fn record_supplier_payment(state: tauri::State<'_, AppState>, session_token: String, purchase_id: i64, amount_kobo: i64, method: String, reference: String) -> Result<PurchaseDetail, String> {
    let me = auth::require(&state, &session_token, Role::Manager)?;
    if amount_kobo <= 0 { return Err("Enter an amount greater than zero".into()); }
    if too_long(&method, 40) || too_long(&reference, 80) { return Err("Method or reference is too long".into()); }
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let (total, paid): (i64, i64) = tx.query_row("SELECT total_kobo,amount_paid_kobo FROM purchases WHERE id=?1", [purchase_id], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(|e| e.to_string())?.ok_or("Purchase not found")?;
    if amount_kobo > total - paid {
        return Err(format!("Amount is more than the balance of {}", db::format_money(&db::setting(&tx, "currency_symbol")?, total - paid)));
    }
    tx.execute("INSERT INTO supplier_payments(purchase_id,amount_kobo,method,reference,user_name) VALUES(?1,?2,?3,?4,?5)", params![purchase_id, amount_kobo, method.trim(), reference.trim(), me.display_name()]).map_err(|e| e.to_string())?;
    tx.execute("UPDATE purchases SET amount_paid_kobo=amount_paid_kobo+?1 WHERE id=?2", params![amount_kobo, purchase_id]).map_err(|e| e.to_string())?;
    let p = load_purchase(&tx, purchase_id)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn mgr() -> Session { Session { user_id: 1, username: "m".into(), full_name: "Manager".into(), role: Role::Manager, last_seen: Instant::now() } }

    #[test]
    fn purchase_in_cartons_adds_pieces_and_averages_cost() {
        let mut conn = db::test_conn();
        conn.execute("INSERT INTO products(sku,name,kind,unit,selling_price_kobo,cost_price_kobo,track_stock,stock_qty,pack_unit,pack_size,track_expiry) VALUES('N','Noodles','product','piece',30000,20000,1,40,'carton',40,1)", []).unwrap();
        let tx = conn.transaction().unwrap();
        let input = PurchaseInput {
            supplier_id: None, supplier_name: Some("Dangote Depot".into()), invoice_ref: "INV-9".into(), note: String::new(),
            lines: vec![PurchaseLineInput { product_id: 1, quantity: 2.0, unit_cost_kobo: 1_120_000, as_pack: true, batch_no: "B1".into(), expiry_date: "2027-03-01".into() }],
            amount_paid_kobo: 1_000_000, payment_method: "Bank transfer".into(),
        };
        let p = create_purchase_tx(&tx, &mgr(), input).unwrap();
        assert_eq!(p.total_kobo, 2_240_000);
        assert_eq!(p.balance_kobo, 1_240_000);
        let (stock, cost): (f64, i64) = tx.query_row("SELECT stock_qty,cost_price_kobo FROM products WHERE id=1", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(stock, 120.0);
        // (40×200 + 80×280) / 120 = 253.33 naira
        assert_eq!(cost, 25333);
        let batches: f64 = tx.query_row("SELECT quantity_left FROM stock_batches WHERE product_id=1", [], |r| r.get(0)).unwrap();
        assert_eq!(batches, 80.0);
    }

    #[test]
    fn unpaid_purchase_needs_supplier() {
        let mut conn = db::test_conn();
        conn.execute("INSERT INTO products(sku,name,kind,unit,track_stock) VALUES('R','Rice','product','bag',1)", []).unwrap();
        let tx = conn.transaction().unwrap();
        let input = PurchaseInput { supplier_id: None, supplier_name: None, invoice_ref: String::new(), note: String::new(), lines: vec![PurchaseLineInput { product_id: 1, quantity: 1.0, unit_cost_kobo: 5000, as_pack: false, batch_no: String::new(), expiry_date: String::new() }], amount_paid_kobo: 0, payment_method: String::new() };
        assert!(create_purchase_tx(&tx, &mgr(), input).is_err());
    }
}
