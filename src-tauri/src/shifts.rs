//! Cash drawer shifts: open with a float, close with a count, see the difference.
//!
//! Expected cash = opening float + cash payments taken in the shift
//!                 − cash refunds given in the shift − cash expenses paid in the shift.

use crate::{auth::{self, Role}, AppState};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

pub const CASH_MATCH: &str = "%cash%";

pub fn open_shift_id(conn: &Connection, user_id: i64) -> Result<Option<i64>, String> {
    conn.query_row("SELECT id FROM shifts WHERE user_id=?1 AND status='open'", [user_id], |r| r.get(0)).optional().map_err(|e| e.to_string())
}

#[derive(Serialize, Clone)]
pub struct MethodTotal { method: String, amount_kobo: i64 }

#[derive(Serialize)]
pub struct ShiftReport {
    id: i64, user_name: String, status: String, opened_at: String, closed_at: Option<String>,
    opening_float_kobo: i64, cash_sales_kobo: i64, cash_refunds_kobo: i64, cash_expenses_kobo: i64,
    expected_cash_kobo: i64, counted_cash_kobo: i64, difference_kobo: i64,
    receipts: i64, sales_kobo: i64, by_method: Vec<MethodTotal>, note: String,
}

#[allow(clippy::type_complexity)]
pub fn report(conn: &Connection, id: i64) -> Result<ShiftReport, String> {
    let (user_id, user_name, status, opened_at, closed_at, float, counted, note, opened_raw, closed_raw): (i64, String, String, String, Option<String>, i64, i64, String, String, Option<String>) = conn
        .query_row("SELECT user_id,user_name,status,datetime(opened_at,'localtime'),datetime(closed_at,'localtime'),opening_float_kobo,counted_cash_kobo,note,opened_at,closed_at FROM shifts WHERE id=?1", [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?)))
        .optional().map_err(|e| e.to_string())?.ok_or("Shift not found")?;
    let sum = |sql: &str| -> Result<i64, String> { conn.query_row(sql, params![id, CASH_MATCH], |r| r.get(0)).map_err(|e| e.to_string()) };
    let cash_sales = sum("SELECT COALESCE(SUM(p.amount_kobo),0) FROM receipt_payments p JOIN receipts r ON r.id=p.receipt_id WHERE p.shift_id=?1 AND p.method LIKE ?2 AND r.payment_status<>'void'")?;
    let cash_refunds = sum("SELECT COALESCE(SUM(refund_paid_kobo),0) FROM returns WHERE shift_id=?1 AND refund_method LIKE ?2")?;
    let cash_expenses = sum("SELECT COALESCE(SUM(amount_kobo),0) FROM expenses WHERE shift_id=?1 AND method LIKE ?2")?;
    let expected = float + cash_sales - cash_refunds - cash_expenses;
    let mut stmt = conn.prepare("SELECT p.method,SUM(p.amount_kobo) FROM receipt_payments p JOIN receipts r ON r.id=p.receipt_id WHERE p.shift_id=?1 AND r.payment_status<>'void' GROUP BY p.method ORDER BY 2 DESC").map_err(|e| e.to_string())?;
    let by_method = stmt.query_map([id], |r| Ok(MethodTotal { method: r.get(0)?, amount_kobo: r.get(1)? })).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    let end = closed_raw.unwrap_or_else(|| "9999-12-31".into());
    let (receipts, sales): (i64, i64) = conn.query_row(
        "SELECT COUNT(*),COALESCE(SUM(total_kobo-returned_kobo),0) FROM receipts WHERE cashier_id=?1 AND payment_status<>'void' AND created_at>=?2 AND created_at<=?3",
        params![user_id, opened_raw, end], |r| Ok((r.get(0)?, r.get(1)?))).map_err(|e| e.to_string())?;
    let closed = status == "closed";
    Ok(ShiftReport {
        id, user_name, status, opened_at, closed_at, opening_float_kobo: float, cash_sales_kobo: cash_sales, cash_refunds_kobo: cash_refunds, cash_expenses_kobo: cash_expenses,
        expected_cash_kobo: expected, counted_cash_kobo: counted, difference_kobo: if closed { counted - expected } else { 0 }, receipts, sales_kobo: sales, by_method, note,
    })
}

#[tauri::command]
pub fn current_shift(state: tauri::State<'_, AppState>, session_token: String) -> Result<Option<ShiftReport>, String> {
    let me = auth::require_seller(&state, &session_token, Role::Cashier)?;
    let conn = state.db()?;
    match open_shift_id(&conn, me.user_id)? { Some(id) => Ok(Some(report(&conn, id)?)), None => Ok(None) }
}

#[tauri::command]
pub fn open_shift(state: tauri::State<'_, AppState>, session_token: String, opening_float_kobo: i64) -> Result<ShiftReport, String> {
    let me = auth::require_seller(&state, &session_token, Role::Cashier)?;
    if !(0..=100_000_000_000).contains(&opening_float_kobo) { return Err("Enter the cash in the drawer (zero or more)".into()); }
    let conn = state.db()?;
    if open_shift_id(&conn, me.user_id)?.is_some() { return Err("You already have an open shift".into()); }
    conn.execute("INSERT INTO shifts(user_id,user_name,opening_float_kobo) VALUES(?1,?2,?3)", params![me.user_id, me.display_name(), opening_float_kobo]).map_err(|e| e.to_string())?;
    report(&conn, conn.last_insert_rowid())
}

#[derive(Deserialize)]
pub struct CloseShiftInput { counted_cash_kobo: i64, note: String }

#[tauri::command]
pub fn close_shift(state: tauri::State<'_, AppState>, session_token: String, input: CloseShiftInput) -> Result<ShiftReport, String> {
    let me = auth::require_seller(&state, &session_token, Role::Cashier)?;
    if !(0..=100_000_000_000).contains(&input.counted_cash_kobo) { return Err("Enter the cash you counted (zero or more)".into()); }
    if input.note.chars().count() > 300 { return Err("Note is too long".into()); }
    let conn = state.db()?;
    let id = open_shift_id(&conn, me.user_id)?.ok_or("You do not have an open shift")?;
    let expected = report(&conn, id)?.expected_cash_kobo;
    conn.execute("UPDATE shifts SET status='closed',closed_at=CURRENT_TIMESTAMP,expected_cash_kobo=?1,counted_cash_kobo=?2,note=?3 WHERE id=?4", params![expected, input.counted_cash_kobo, input.note.trim(), id]).map_err(|e| e.to_string())?;
    report(&conn, id)
}

#[derive(Serialize)]
pub struct ShiftRow { id: i64, user_name: String, status: String, opened_at: String, closed_at: Option<String>, opening_float_kobo: i64, expected_cash_kobo: i64, counted_cash_kobo: i64, difference_kobo: i64 }

#[tauri::command]
pub fn list_shifts(state: tauri::State<'_, AppState>, session_token: String, from: String, to: String) -> Result<Vec<ShiftRow>, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let conn = state.db()?;
    let mut stmt = conn.prepare("SELECT id,user_name,status,datetime(opened_at,'localtime'),datetime(closed_at,'localtime'),opening_float_kobo,expected_cash_kobo,counted_cash_kobo FROM shifts WHERE date(opened_at,'localtime') BETWEEN ?1 AND ?2 ORDER BY id DESC LIMIT 500").map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![from, to], |r| {
        let status: String = r.get(2)?;
        let expected: i64 = r.get(6)?;
        let counted: i64 = r.get(7)?;
        Ok(ShiftRow { id: r.get(0)?, user_name: r.get(1)?, opened_at: r.get(3)?, closed_at: r.get(4)?, opening_float_kobo: r.get(5)?, expected_cash_kobo: expected, counted_cash_kobo: counted, difference_kobo: if status == "closed" { counted - expected } else { 0 }, status })
    }).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn shift_report(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<ShiftReport, String> {
    let me = auth::require_seller(&state, &session_token, Role::Cashier)?;
    let conn = state.db()?;
    if me.role < Role::Manager {
        let owner: i64 = conn.query_row("SELECT user_id FROM shifts WHERE id=?1", [id], |r| r.get(0)).map_err(|_| "Shift not found".to_string())?;
        if owner != me.user_id { return Err("You can only view your own shifts".into()); }
    }
    report(&conn, id)
}

#[cfg(test)]
impl ShiftReport {
    pub fn expected_cash_kobo_for_test(&self) -> i64 { self.expected_cash_kobo }
}
