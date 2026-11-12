//! Business expenses (diesel, salaries, rent...) so reports can show net profit.

use crate::{auth::{self, Role}, shifts, AppState};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

pub const SUGGESTED_CATEGORIES: &[&str] = &[
    "Diesel / fuel", "Salaries & wages", "Rent", "Electricity", "Transport & logistics", "Repairs & maintenance",
    "Water", "Internet & airtime", "Taxes & levies", "Vet & medication", "Marketing", "Bank charges", "Cleaning & sanitation", "Miscellaneous",
];

#[derive(Serialize)]
pub struct Expense {
    id: i64, expense_date: String, category: String, description: String, amount_kobo: i64,
    method: String, paid_to: String, user_name: String, created_at: String,
}

#[derive(Serialize)]
pub struct ExpenseList { items: Vec<Expense>, total_kobo: i64, by_category: Vec<(String, i64)>, categories: Vec<String> }

fn valid_date(d: &str) -> bool { chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok() }

#[tauri::command]
pub fn list_expenses(state: tauri::State<'_, AppState>, session_token: String, from: String, to: String) -> Result<ExpenseList, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    if !valid_date(&from) || !valid_date(&to) { return Err("Choose a valid date range".into()); }
    let conn = state.db()?;
    let mut stmt = conn.prepare("SELECT id,expense_date,category,description,amount_kobo,method,paid_to,user_name,datetime(created_at,'localtime') FROM expenses WHERE expense_date BETWEEN ?1 AND ?2 ORDER BY expense_date DESC, id DESC LIMIT 2000").map_err(|e| e.to_string())?;
    let items = stmt.query_map(params![from, to], |r| Ok(Expense { id: r.get(0)?, expense_date: r.get(1)?, category: r.get(2)?, description: r.get(3)?, amount_kobo: r.get(4)?, method: r.get(5)?, paid_to: r.get(6)?, user_name: r.get(7)?, created_at: r.get(8)? }))
        .map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    let total_kobo = items.iter().map(|e| e.amount_kobo).sum();
    let mut stmt = conn.prepare("SELECT category,SUM(amount_kobo) FROM expenses WHERE expense_date BETWEEN ?1 AND ?2 GROUP BY category ORDER BY 2 DESC").map_err(|e| e.to_string())?;
    let by_category = stmt.query_map(params![from, to], |r| Ok((r.get(0)?, r.get(1)?))).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT DISTINCT category FROM expenses ORDER BY category").map_err(|e| e.to_string())?;
    let used: Vec<String> = stmt.query_map([], |r| r.get(0)).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    let mut categories: Vec<String> = SUGGESTED_CATEGORIES.iter().map(|s| s.to_string()).collect();
    for c in used { if !categories.contains(&c) { categories.push(c); } }
    Ok(ExpenseList { items, total_kobo, by_category, categories })
}

#[derive(Deserialize)]
pub struct ExpenseInput { id: Option<i64>, expense_date: String, category: String, description: String, amount_kobo: i64, method: String, paid_to: String }

#[tauri::command]
pub fn save_expense(state: tauri::State<'_, AppState>, session_token: String, input: ExpenseInput) -> Result<i64, String> {
    let me = auth::require(&state, &session_token, Role::Manager)?;
    if !valid_date(&input.expense_date) { return Err("Choose the date of the expense".into()); }
    let category = input.category.trim();
    if category.is_empty() || category.chars().count() > 60 { return Err("Choose or type a category (up to 60 characters)".into()); }
    if input.amount_kobo <= 0 || input.amount_kobo > 100_000_000_000 { return Err("Enter an amount greater than zero".into()); }
    if input.description.chars().count() > 300 || input.paid_to.chars().count() > 120 || input.method.chars().count() > 40 { return Err("One of the fields is too long".into()); }
    let conn = state.db()?;
    let shift_id = shifts::open_shift_id(&conn, me.user_id)?;
    match input.id {
        Some(id) => {
            let n = conn.execute("UPDATE expenses SET expense_date=?1,category=?2,description=?3,amount_kobo=?4,method=?5,paid_to=?6 WHERE id=?7",
                params![input.expense_date, category, input.description.trim(), input.amount_kobo, input.method.trim(), input.paid_to.trim(), id]).map_err(|e| e.to_string())?;
            if n == 0 { return Err("Expense not found".into()); }
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO expenses(expense_date,category,description,amount_kobo,method,paid_to,shift_id,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![input.expense_date, category, input.description.trim(), input.amount_kobo, input.method.trim(), input.paid_to.trim(), shift_id, me.display_name()]).map_err(|e| e.to_string())?;
            Ok(conn.last_insert_rowid())
        }
    }
}

#[tauri::command]
pub fn delete_expense(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let conn = state.db()?;
    let exists: Option<i64> = conn.query_row("SELECT id FROM expenses WHERE id=?1", [id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    exists.ok_or("Expense not found")?;
    conn.execute("DELETE FROM expenses WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}
