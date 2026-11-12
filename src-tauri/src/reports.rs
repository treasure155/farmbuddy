//! Dashboard figures, sales reports and file exports.

use crate::{auth::{self, Role}, AppState};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::PathBuf;
use tauri::Manager;

#[derive(Serialize)]
pub struct DayPoint { date: String, total_kobo: i64, receipts: i64 }

#[derive(Serialize)]
pub struct LowStock { id: i64, name: String, unit: String, stock_qty: f64, reorder_level: f64 }

#[derive(Serialize)]
pub struct Dashboard {
    today_sales_kobo: i64,
    today_receipts: i64,
    today_collected_kobo: i64,
    yesterday_sales_kobo: i64,
    outstanding_kobo: i64,
    owing_receipts: i64,
    pending_collections: i64,
    ready_collections: i64,
    product_count: i64,
    low_stock_count: i64,
    low_stock: Vec<LowStock>,
    expiring_count: i64,
    expired_count: i64,
    today_expenses_kobo: i64,
    today_returns_kobo: i64,
    last_7_days: Vec<DayPoint>,
    /// Scope: "all" for managers, "mine" for cashiers.
    scope: String,
}

/// Reads one number. SQLite returns a decimal (REAL) for ROUND() and for sums of
/// fractional quantities, so both whole and decimal results are accepted and
/// rounded to whole kobo.
fn scalar(conn: &Connection, sql: &str, p: impl rusqlite::Params) -> Result<i64, String> {
    use rusqlite::types::Value;
    let v: Value = conn.query_row(sql, p, |r| r.get(0)).map_err(|e| e.to_string())?;
    Ok(match v { Value::Integer(i) => i, Value::Real(f) => f.round() as i64, _ => 0 })
}

#[tauri::command]
pub fn dashboard(state: tauri::State<'_, AppState>, session_token: String) -> Result<Dashboard, String> {
    let me = auth::require_seller(&state, &session_token, Role::Cashier)?;
    let conn = state.db()?;
    let mine: Option<i64> = if me.role < Role::Manager { Some(me.user_id) } else { None };
    let live = "payment_status<>'void' AND (?1 IS NULL OR cashier_id=?1)";
    let today_sales_kobo = scalar(&conn, &format!("SELECT COALESCE(SUM(total_kobo),0) FROM receipts WHERE {live} AND date(created_at,'localtime')=date('now','localtime')"), [mine])?;
    let today_returns_kobo = scalar(&conn, "SELECT COALESCE(SUM(t.value_kobo),0) FROM returns t JOIN receipts r ON r.id=t.receipt_id WHERE (?1 IS NULL OR r.cashier_id=?1) AND date(t.created_at,'localtime')=date('now','localtime')", [mine])?;
    let today_sales_kobo = today_sales_kobo - today_returns_kobo;
    let today_expenses_kobo = if mine.is_none() { scalar(&conn, "SELECT COALESCE(SUM(amount_kobo),0) FROM expenses WHERE expense_date=date('now','localtime')", [])? } else { 0 };
    let warn_days: i64 = crate::db::setting(&conn, "expiry_warn_days")?.parse().unwrap_or(30);
    let expiring_count = scalar(&conn, "SELECT COUNT(*) FROM stock_batches b JOIN products p ON p.id=b.product_id WHERE p.active=1 AND b.quantity_left>0 AND b.expiry_date>=date('now','localtime') AND b.expiry_date<=date('now','localtime','+'||?1||' days')", [warn_days])?;
    let expired_count = scalar(&conn, "SELECT COUNT(*) FROM stock_batches b JOIN products p ON p.id=b.product_id WHERE p.active=1 AND b.quantity_left>0 AND b.expiry_date<date('now','localtime')", [])?;
    let today_receipts = scalar(&conn, &format!("SELECT COUNT(*) FROM receipts WHERE {live} AND date(created_at,'localtime')=date('now','localtime')"), [mine])?;
    let yesterday_sales_kobo = scalar(&conn, &format!("SELECT COALESCE(SUM(total_kobo),0) FROM receipts WHERE {live} AND date(created_at,'localtime')=date('now','localtime','-1 day')"), [mine])?;
    let today_collected_kobo = scalar(&conn,
        "SELECT COALESCE(SUM(p.amount_kobo),0) FROM receipt_payments p JOIN receipts r ON r.id=p.receipt_id
         WHERE r.payment_status<>'void' AND (?1 IS NULL OR r.cashier_id=?1) AND date(p.created_at,'localtime')=date('now','localtime')", [mine])?
        - scalar(&conn, "SELECT COALESCE(SUM(t.refund_paid_kobo),0) FROM returns t JOIN receipts r ON r.id=t.receipt_id WHERE (?1 IS NULL OR r.cashier_id=?1) AND date(t.created_at,'localtime')=date('now','localtime')", [mine])?;
    let outstanding_kobo = scalar(&conn, &format!("SELECT COALESCE(SUM({}),0) FROM receipts r WHERE r.payment_status IN ('partial','unpaid') AND (?1 IS NULL OR r.cashier_id=?1)", crate::sales::OWED_SQL), [mine])?;
    let owing_receipts = scalar(&conn, "SELECT COUNT(*) FROM receipts WHERE payment_status IN ('partial','unpaid') AND (?1 IS NULL OR cashier_id=?1)", [mine])?;
    let pending_collections = scalar(&conn, "SELECT COUNT(*) FROM receipts WHERE fulfilment_status='pending' AND payment_status<>'void' AND (?1 IS NULL OR cashier_id=?1)", [mine])?;
    let ready_collections = scalar(&conn, "SELECT COUNT(*) FROM receipts WHERE fulfilment_status='ready' AND payment_status<>'void' AND (?1 IS NULL OR cashier_id=?1)", [mine])?;
    let product_count = scalar(&conn, "SELECT COUNT(*) FROM products WHERE active=1 AND kind<>'material'", [])?;
    let low_where = "active=1 AND track_stock=1 AND stock_qty<=reorder_level";
    let low_stock_count = scalar(&conn, &format!("SELECT COUNT(*) FROM products WHERE {low_where}"), [])?;
    let mut stmt = conn.prepare(&format!("SELECT id,name,unit,stock_qty,reorder_level FROM products WHERE {low_where} ORDER BY (stock_qty - reorder_level), name LIMIT 8")).map_err(|e| e.to_string())?;
    let low_stock = stmt.query_map([], |r| Ok(LowStock { id: r.get(0)?, name: r.get(1)?, unit: r.get(2)?, stock_qty: r.get(3)?, reorder_level: r.get(4)? })).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "WITH RECURSIVE days(d) AS (SELECT date('now','localtime','-6 days') UNION ALL SELECT date(d,'+1 day') FROM days WHERE d < date('now','localtime'))
             SELECT d, COALESCE((SELECT SUM(total_kobo-returned_kobo) FROM receipts WHERE payment_status<>'void' AND (?1 IS NULL OR cashier_id=?1) AND date(created_at,'localtime')=d),0),
                       (SELECT COUNT(*) FROM receipts WHERE payment_status<>'void' AND (?1 IS NULL OR cashier_id=?1) AND date(created_at,'localtime')=d)
             FROM days ORDER BY d",
        )
        .map_err(|e| e.to_string())?;
    let last_7_days = stmt.query_map([mine], |r| Ok(DayPoint { date: r.get(0)?, total_kobo: r.get(1)?, receipts: r.get(2)? })).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(Dashboard { today_sales_kobo, today_receipts, today_collected_kobo, yesterday_sales_kobo, outstanding_kobo, owing_receipts, pending_collections, ready_collections, product_count, low_stock_count, low_stock, expiring_count, expired_count, today_expenses_kobo, today_returns_kobo, last_7_days, scope: if mine.is_some() { "mine".into() } else { "all".into() } })
}

#[derive(Serialize)]
pub struct NamedTotal { name: String, count: i64, total_kobo: i64 }

#[derive(Serialize)]
pub struct ItemPerformance { name: String, unit: String, quantity: f64, revenue_kobo: i64, cost_kobo: i64, profit_kobo: i64 }

#[derive(Serialize)]
pub struct SalesReport {
    from: String,
    to: String,
    receipts: i64,
    voided: i64,
    gross_kobo: i64,
    discount_kobo: i64,
    tax_kobo: i64,
    net_kobo: i64,
    collected_kobo: i64,
    outstanding_kobo: i64,
    returns_kobo: i64,
    refunds_kobo: i64,
    cost_kobo: i64,
    profit_kobo: i64,
    expenses_kobo: i64,
    net_profit_kobo: i64,
    by_expense: Vec<NamedTotal>,
    by_day: Vec<DayPoint>,
    by_method: Vec<NamedTotal>,
    by_cashier: Vec<NamedTotal>,
    by_category: Vec<NamedTotal>,
    items: Vec<ItemPerformance>,
}

#[tauri::command]
pub fn sales_report(state: tauri::State<'_, AppState>, session_token: String, from: String, to: String) -> Result<SalesReport, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let valid = |d: &str| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok();
    if !valid(&from) || !valid(&to) { return Err("Choose a valid date range".into()); }
    if from > to { return Err("The start date must be before the end date".into()); }
    let conn = state.db()?;
    let range = "date(r.created_at,'localtime') BETWEEN ?1 AND ?2";
    let (receipts, gross, discount, tax, net, paid): (i64, i64, i64, i64, i64, i64) = conn
        .query_row(&format!("SELECT COUNT(*),COALESCE(SUM(subtotal_kobo),0),COALESCE(SUM(discount_kobo),0),COALESCE(SUM(tax_kobo),0),COALESCE(SUM(total_kobo),0),COALESCE(SUM(amount_paid_kobo),0) FROM receipts r WHERE r.payment_status<>'void' AND {range}"), params![from, to], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))
        .map_err(|e| e.to_string())?;
    let voided = scalar(&conn, &format!("SELECT COUNT(*) FROM receipts r WHERE r.payment_status='void' AND {range}"), params![from, to])?;
    let cost_kobo = scalar(&conn, &format!("SELECT CAST(COALESCE(SUM(ROUND(i.unit_cost_kobo*i.quantity)),0) AS INTEGER) FROM receipt_items i JOIN receipts r ON r.id=i.receipt_id WHERE r.payment_status<>'void' AND {range}"), params![from, to])?;
    // Returns count on the day goods came back; their cost comes back into stock.
    let t_range = "date(t.created_at,'localtime') BETWEEN ?1 AND ?2";
    let returns_kobo = scalar(&conn, &format!("SELECT COALESCE(SUM(t.value_kobo),0) FROM returns t WHERE {t_range}"), params![from, to])?;
    let refunds_kobo = scalar(&conn, &format!("SELECT COALESCE(SUM(t.refund_paid_kobo),0) FROM returns t WHERE {t_range}"), params![from, to])?;
    let returned_cost = scalar(&conn, &format!("SELECT COALESCE(SUM(x.cost_kobo),0) FROM return_items x JOIN returns t ON t.id=x.return_id WHERE {t_range}"), params![from, to])?;
    let cost_kobo = cost_kobo - returned_cost;
    let expenses_kobo = scalar(&conn, "SELECT COALESCE(SUM(amount_kobo),0) FROM expenses WHERE expense_date BETWEEN ?1 AND ?2", params![from, to])?;

    let mut stmt = conn.prepare("WITH RECURSIVE days(d) AS (SELECT ?1 UNION ALL SELECT date(d,'+1 day') FROM days WHERE d < ?2 LIMIT 400)
         SELECT d, COALESCE((SELECT SUM(total_kobo-returned_kobo) FROM receipts r WHERE r.payment_status<>'void' AND date(r.created_at,'localtime')=d),0),
                   (SELECT COUNT(*) FROM receipts r WHERE r.payment_status<>'void' AND date(r.created_at,'localtime')=d)
         FROM days ORDER BY d").map_err(|e| e.to_string())?;
    let by_day = stmt.query_map(params![from, to], |r| Ok(DayPoint { date: r.get(0)?, total_kobo: r.get(1)?, receipts: r.get(2)? })).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;

    let named = |sql: String| -> Result<Vec<NamedTotal>, String> {
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt.query_map(params![from, to], |r| Ok(NamedTotal { name: r.get(0)?, count: r.get(1)?, total_kobo: r.get(2)? })).map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
    };
    // Payments are counted on the day money was received, which is what a cash-up needs.
    let by_method = named("SELECT p.method,COUNT(*),SUM(p.amount_kobo) FROM receipt_payments p JOIN receipts r ON r.id=p.receipt_id WHERE r.payment_status<>'void' AND date(p.created_at,'localtime') BETWEEN ?1 AND ?2 GROUP BY p.method ORDER BY 3 DESC".into())?;
    let by_cashier = named(format!("SELECT CASE WHEN r.cashier_name='' THEN 'Unknown' ELSE r.cashier_name END,COUNT(*),SUM(r.total_kobo) FROM receipts r WHERE r.payment_status<>'void' AND {range} GROUP BY 1 ORDER BY 3 DESC"))?;
    let by_category = named(format!(
        "SELECT COALESCE(c.name,'Uncategorised'),COUNT(*),SUM(i.line_total_kobo) FROM receipt_items i JOIN receipts r ON r.id=i.receipt_id
         LEFT JOIN products p ON p.id=i.product_id LEFT JOIN categories c ON c.id=p.category_id
         WHERE r.payment_status<>'void' AND {range} GROUP BY 1 ORDER BY 3 DESC"))?;
    let by_expense = named("SELECT category,COUNT(*),SUM(amount_kobo) FROM expenses WHERE expense_date BETWEEN ?1 AND ?2 GROUP BY category ORDER BY 3 DESC".into())?;

    let mut stmt = conn.prepare(&format!(
        "SELECT i.product_name,i.unit,SUM(i.quantity-i.returned_qty),SUM(ROUND(i.line_total_kobo*(i.quantity-i.returned_qty)/i.quantity)),SUM(ROUND(i.unit_cost_kobo*(i.quantity-i.returned_qty)))
         FROM receipt_items i JOIN receipts r ON r.id=i.receipt_id WHERE r.payment_status<>'void' AND {range}
         GROUP BY i.product_name,i.unit ORDER BY 4 DESC LIMIT 200")).map_err(|e| e.to_string())?;
    let items = stmt
        .query_map(params![from, to], |r| {
            let revenue: i64 = r.get::<_, f64>(3)? as i64;
            let cost: i64 = r.get::<_, f64>(4)? as i64;
            Ok(ItemPerformance { name: r.get(0)?, unit: r.get(1)?, quantity: r.get(2)?, revenue_kobo: revenue, cost_kobo: cost, profit_kobo: revenue - cost })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let refunded_on_range: i64 = scalar(&conn, &format!("SELECT COALESCE(SUM(refunded_kobo),0) FROM receipts r WHERE r.payment_status<>'void' AND {range}"), params![from, to])?;
    let returned_on_range: i64 = scalar(&conn, &format!("SELECT COALESCE(SUM(returned_kobo),0) FROM receipts r WHERE r.payment_status<>'void' AND {range}"), params![from, to])?;
    let net_sales = net - returns_kobo;
    // Profit excludes tax (it belongs to the government) on the share of sales kept.
    let tax_kept = if net > 0 { ((tax as f64) * (net_sales as f64) / (net as f64)).round() as i64 } else { 0 };
    let profit_kobo = (net_sales - tax_kept) - cost_kobo;
    Ok(SalesReport {
        from, to, receipts, voided, gross_kobo: gross, discount_kobo: discount, tax_kobo: tax, net_kobo: net_sales,
        collected_kobo: paid - refunded_on_range, outstanding_kobo: ((net - returned_on_range) - (paid - refunded_on_range)).max(0),
        returns_kobo, refunds_kobo, cost_kobo, profit_kobo, expenses_kobo, net_profit_kobo: profit_kobo - expenses_kobo, by_expense,
        by_day, by_method, by_cashier, by_category, items,
    })
}

// ------------------------------------------------------------------ files

fn downloads_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path().download_dir().or_else(|_| app.path().document_dir()).map_err(|e| format!("Could not find your Downloads folder: {e}"))
}

fn safe_file_name(name: &str) -> String {
    let cleaned: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '-' }).collect();
    let trimmed = cleaned.trim_matches(|c| c == '.' || c == '-').chars().take(80).collect::<String>();
    if trimmed.is_empty() { "export".into() } else { trimmed }
}

/// Saves CSV text produced by the UI into the user's Downloads folder.
#[tauri::command]
pub fn save_export(app: tauri::AppHandle, state: tauri::State<'_, AppState>, session_token: String, file_name: String, content: String) -> Result<String, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    if content.len() > 20 * 1024 * 1024 { return Err("Export is too large".into()); }
    let mut name = safe_file_name(&file_name);
    if !name.to_lowercase().ends_with(".csv") { name.push_str(".csv"); }
    let path = downloads_dir(&app)?.join(name);
    // UTF-8 BOM so Excel shows ₦ and other characters correctly.
    std::fs::write(&path, format!("\u{feff}{content}")).map_err(|e| format!("Could not save file: {e}"))?;
    Ok(path.display().to_string())
}

/// Saves a PDF report built by the UI (base64) into the Downloads folder.
#[tauri::command]
pub fn save_pdf_export(app: tauri::AppHandle, state: tauri::State<'_, AppState>, session_token: String, file_name: String, base64_pdf: String) -> Result<String, String> {
    use base64::Engine;
    auth::require(&state, &session_token, Role::Manager)?;
    if base64_pdf.len() > 30 * 1024 * 1024 { return Err("Report is too large".into()); }
    let bytes = base64::engine::general_purpose::STANDARD.decode(base64_pdf.trim()).map_err(|_| "The report could not be prepared".to_string())?;
    if !bytes.starts_with(b"%PDF-") { return Err("The report could not be prepared".into()); }
    let mut name = safe_file_name(&file_name);
    if !name.to_lowercase().ends_with(".pdf") { name.push_str(".pdf"); }
    let path = downloads_dir(&app)?.join(name);
    std::fs::write(&path, bytes).map_err(|e| format!("Could not save file: {e}"))?;
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_names_are_sanitised() {
        assert_eq!(safe_file_name("../../etc/passwd"), "etc-passwd");
        assert_eq!(safe_file_name("sales 2026-10.csv"), "sales-2026-10.csv");
        assert_eq!(safe_file_name("..."), "export");
    }

    // Reports failed with "Invalid column type Real" once a sale existed,
    // because ROUND() returns a decimal in SQLite.
    #[test]
    fn totals_accept_decimal_results() {
        let conn = crate::db::test_conn();
        let v = scalar(&conn, "SELECT COALESCE(SUM(ROUND(x)),0) FROM (SELECT 4800.0*1.5 AS x UNION ALL SELECT 4500*2)", []).unwrap();
        assert_eq!(v, 16200);
        assert_eq!(scalar(&conn, "SELECT COALESCE(SUM(1),0) WHERE 0", []).unwrap(), 0);
        assert_eq!(scalar(&conn, "SELECT 2.6", []).unwrap(), 3);
    }

    #[test]
    fn cost_of_sales_reads_with_real_sales() {
        let conn = crate::db::test_conn();
        crate::templates::apply_template(&conn, "farm", false, "Test").unwrap();
        conn.execute("INSERT INTO receipts(receipt_no,subtotal_kobo,total_kobo,amount_paid_kobo,payment_status) VALUES('T-1',960000,960000,960000,'paid')", []).unwrap();
        let rid = conn.last_insert_rowid();
        conn.execute("INSERT INTO receipt_items(receipt_id,product_name,product_kind,unit,quantity,unit_price_kobo,unit_cost_kobo,line_total_kobo) VALUES(?1,'Rice','product','kg',2.5,384000,300000,960000)", [rid]).unwrap();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let range = "date(r.created_at,'localtime') BETWEEN ?1 AND ?2";
        let cost = scalar(&conn, &format!("SELECT CAST(COALESCE(SUM(ROUND(i.unit_cost_kobo*i.quantity)),0) AS INTEGER) FROM receipt_items i JOIN receipts r ON r.id=i.receipt_id WHERE r.payment_status<>'void' AND {range}"), rusqlite::params![today, today]).unwrap();
        assert_eq!(cost, 750000);
    }
}
