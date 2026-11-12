//! "Start afresh": lets the Owner clear the items, stock and test data that a
//! business used while trying the app, without touching the licence, this
//! device, staff accounts, settings or payment methods.
//!
//! A safety backup is always made first, so a reset can be undone from
//! Settings → Backup.

use crate::auth::{self, Role};
use crate::{backup, recovery, templates, AppState};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// Items, categories and everything about stock.
const STOCK_TABLES: &[&str] = &[
    "recipe_items", "stock_batches", "stock_movements", "held_sales",
    "supplier_payments", "purchase_items", "purchases", "suppliers",
    "products", "categories",
];

/// Sales and money records, removed only when the Owner asks for it.
const SALES_TABLES: &[&str] = &[
    "return_items", "returns", "receipt_payments", "receipt_items", "receipts",
    "customers", "expenses", "shifts",
    "flock_sales", "flock_costs", "flock_logs", "flocks", "email_logs",
];

#[derive(Deserialize)]
pub struct ResetInput {
    pub password: String,
    pub confirm: String,
    /// Also clear receipts, customers, expenses and shifts.
    pub include_sales: bool,
    /// Load the ready-made items for the business type again.
    pub reload_items: bool,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct ResetResult {
    pub removed_items: i64,
    pub removed_receipts: i64,
    pub loaded_items: usize,
    pub backup_path: String,
}

fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap_or(0)
}

fn table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row("SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1", [table], |_| Ok(())).is_ok()
}

/// Clears the chosen data inside one transaction. Users, settings, payment
/// methods, licence and security records are never touched.
pub fn reset_tx(conn: &Connection, include_sales: bool, reload_items: bool, user_name: &str) -> Result<ResetResult, String> {
    let removed_items = count(conn, "products");
    let removed_receipts = if include_sales { count(conn, "receipts") } else { 0 };
    let tables: Vec<&str> = if include_sales { SALES_TABLES.iter().chain(STOCK_TABLES).copied().collect() } else { STOCK_TABLES.to_vec() };
    for t in tables {
        if table_exists(conn, t) {
            conn.execute(&format!("DELETE FROM {t}"), []).map_err(|e| format!("Could not clear {t}: {e}"))?;
        }
    }
    // Numbering for items starts again from 1.
    if table_exists(conn, "sqlite_sequence") {
        for t in STOCK_TABLES.iter().chain(if include_sales { SALES_TABLES } else { &[] }) {
            let _ = conn.execute("DELETE FROM sqlite_sequence WHERE name=?1", [t]);
        }
    }
    let mut loaded_items = 0;
    if reload_items {
        loaded_items = templates::apply_template(conn, "farm", true, user_name)?;
    }
    Ok(ResetResult { removed_items, removed_receipts, loaded_items, backup_path: String::new() })
}

#[tauri::command]
pub fn reset_business_data(app: tauri::AppHandle, state: tauri::State<'_, AppState>, session_token: String, input: ResetInput) -> Result<ResetResult, String> {
    let me = auth::require(&state, &session_token, Role::Owner)?;
    if input.confirm.trim().to_uppercase() != "RESET" {
        return Err("Type RESET in capital letters to confirm.".into());
    }
    let mut conn = state.db()?;
    let hash: String = conn.query_row("SELECT password_hash FROM users WHERE id=?1", [me.user_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    if !auth::verify_password(&input.password, &hash) {
        recovery::log_event(&conn, "reset_denied", &me.username, "Wrong password on Start afresh");
        return Err("That password is not correct.".into());
    }
    // Safety copy first. If it cannot be made, nothing is deleted.
    let dir = backup::effective_dir(&app, &conn);
    let path = backup::backup_to(&conn, &dir, backup::keep_count(&conn).max(5))
        .map_err(|e| format!("Nothing was deleted, because a safety backup could not be made first. {e}"))?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut result = reset_tx(&tx, input.include_sales, input.reload_items, &me.display_name())?;
    let detail = format!("Start afresh: {} items removed{}{}", result.removed_items,
        if input.include_sales { format!(", {} receipts removed", result.removed_receipts) } else { String::new() },
        if input.reload_items { format!(", {} ready-made items loaded", result.loaded_items) } else { String::new() });
    tx.execute("INSERT INTO security_events(event,username,detail) VALUES('data_reset',?1,?2)", params![me.username, detail]).map_err(|e| e.to_string())?;
    crate::audit::log(&tx, &me.display_name(), "data_reset", "business", None, &detail);
    tx.commit().map_err(|e| e.to_string())?;
    let _ = conn.execute_batch("VACUUM;");
    result.backup_path = path.display().to_string();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn seeded() -> Connection {
        let conn = db::test_conn();
        db::set_setting(&conn, "business_name", "Test Farm").unwrap();
        db::set_setting(&conn, "farm_template", "broiler").unwrap();
        templates::apply_template(&conn, "farm", true, "Test").unwrap();
        conn.execute("INSERT INTO users(username,full_name,password_hash,role) VALUES('boss','Boss','x','owner')", []).unwrap();
        let pid: i64 = conn.query_row("SELECT id FROM products LIMIT 1", [], |r| r.get(0)).unwrap();
        conn.execute("INSERT INTO receipts(receipt_no,subtotal_kobo,discount_kobo,tax_kobo,total_kobo,amount_paid_kobo,change_kobo,payment_status) VALUES('T-1',100,0,0,100,100,0,'paid')", []).unwrap();
        let rid = conn.last_insert_rowid();
        conn.execute("INSERT INTO receipt_items(receipt_id,product_id,product_name,product_kind,unit,quantity,unit_price_kobo,line_total_kobo) VALUES(?1,?2,'X','product','pack',1,100,100)", params![rid, pid]).unwrap();
        conn.execute("INSERT INTO receipt_payments(receipt_id,method,amount_kobo) VALUES(?1,'Cash',100)", [rid]).unwrap();
        conn
    }

    #[test]
    fn stock_reset_keeps_sales_and_settings() {
        let conn = seeded();
        let methods = count(&conn, "payment_methods");
        let r = reset_tx(&conn, false, false, "Test").unwrap();
        assert!(r.removed_items >= 5);
        assert_eq!(count(&conn, "products"), 0);
        assert_eq!(count(&conn, "categories"), 0);
        assert_eq!(count(&conn, "stock_movements"), 0);
        assert_eq!(count(&conn, "receipts"), 1);
        assert_eq!(count(&conn, "receipt_items"), 1);
        assert_eq!(count(&conn, "users"), 1);
        assert_eq!(count(&conn, "payment_methods"), methods);
        assert_eq!(db::setting(&conn, "business_name").unwrap(), "Test Farm");
        assert_eq!(db::setting(&conn, "business_type").unwrap(), "farm");
    }

    #[test]
    fn full_reset_clears_sales_and_can_reload_items() {
        let conn = seeded();
        let r = reset_tx(&conn, true, true, "Test").unwrap();
        assert_eq!(r.removed_receipts, 1);
        assert_eq!(count(&conn, "receipts"), 0);
        assert_eq!(count(&conn, "receipt_payments"), 0);
        assert_eq!(count(&conn, "users"), 1);
        assert!(r.loaded_items >= 5);
        assert_eq!(count(&conn, "products") as usize, r.loaded_items);
    }

    #[test]
    fn full_reset_works_with_every_farm_record() {
        let mut base = db::test_conn();
        base.execute("INSERT INTO users(username,password_hash,role) VALUES('boss','x','owner')", []).unwrap();
        let tx = base.transaction().unwrap();
        crate::demo::load_tx(&tx).unwrap();
        reset_tx(&tx, true, false, "Test").unwrap();
        assert_eq!(count(&tx, "products"), 0);
        assert_eq!(count(&tx, "receipts"), 0);
        let fk: i64 = tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0)).unwrap();
        assert_eq!(fk, 0);
    }

}
