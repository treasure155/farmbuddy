//! Per-farm access. Owners and managers see every farm. A worker, adviser or
//! cashier with farm assignments sees only those farms' batches, plots and
//! processing batches; one with no assignments sees all farms, as before.
//! Records not linked to any farm stay visible to everyone allowed to see the
//! module.

use crate::{audit, auth::{self, Role, Session}, farms::rows, AppState};
use rusqlite::{params, Connection};

/// `None` means "every farm".
pub fn allowed(conn: &Connection, me: &Session) -> Result<Option<Vec<i64>>, String> {
    if me.role >= Role::Manager { return Ok(None); }
    let ids: Vec<i64> = rows(conn, "SELECT farm_id FROM user_farms WHERE user_id=?1", [me.user_id], |r| r.get(0))?;
    Ok(if ids.is_empty() { None } else { Some(ids) })
}

pub fn visible(allowed: &Option<Vec<i64>>, farm_id: Option<i64>) -> bool {
    match (allowed, farm_id) { (None, _) | (_, None) => true, (Some(ids), Some(f)) => ids.contains(&f) }
}

pub fn user_farms_tx(conn: &Connection, user_id: i64) -> Result<Vec<i64>, String> {
    rows(conn, "SELECT farm_id FROM user_farms WHERE user_id=?1 ORDER BY farm_id", [user_id], |r| r.get(0))
}

pub fn set_user_farms_tx(conn: &Connection, user_id: i64, farm_ids: &[i64]) -> Result<(), String> {
    let exists: i64 = conn.query_row("SELECT COUNT(*) FROM users WHERE id=?1", [user_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    if exists == 0 { return Err("Staff account not found".into()); }
    conn.execute("DELETE FROM user_farms WHERE user_id=?1", [user_id]).map_err(|e| e.to_string())?;
    for f in farm_ids {
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM farms WHERE id=?1", [f], |r| r.get(0)).map_err(|e| e.to_string())?;
        if n == 0 { return Err("One of the selected farms no longer exists".into()); }
        conn.execute("INSERT OR IGNORE INTO user_farms(user_id,farm_id) VALUES(?1,?2)", params![user_id, f]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_user_farms(state: tauri::State<'_, AppState>, session_token: String, user_id: i64) -> Result<Vec<i64>, String> {
    auth::require(&state, &session_token, Role::Owner)?;
    user_farms_tx(&*state.db()?, user_id)
}

#[tauri::command]
pub fn set_user_farms(state: tauri::State<'_, AppState>, session_token: String, user_id: i64, farm_ids: Vec<i64>) -> Result<(), String> {
    let me = auth::require(&state, &session_token, Role::Owner)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    set_user_farms_tx(&tx, user_id, &farm_ids)?;
    audit::log(&tx, &me.display_name(), "farm_access_changed", "user", Some(user_id), &format!("farms: {:?}", farm_ids));
    tx.commit().map_err(|e| e.to_string())
}

/// The farms the signed-in user may pick in the farm switcher.
#[tauri::command]
pub fn my_farms(state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<crate::farms::Farm>, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let conn = state.db()?;
    let a = allowed(&conn, &me)?;
    Ok(crate::farms::list_farms_tx(&conn, false)?.into_iter().filter(|f| visible(&a, Some(f.id))).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn assignment_limits_visibility() {
        let conn = crate::db::test_conn();
        conn.execute("INSERT INTO farms(name) VALUES('A'),('B')", []).unwrap();
        conn.execute("INSERT INTO users(username,password_hash,role) VALUES('w','x','worker')", []).unwrap();
        let uid = conn.last_insert_rowid();
        let me = Session { user_id: uid, username: "w".into(), full_name: String::new(), role: Role::Worker, last_seen: std::time::Instant::now() };
        assert!(allowed(&conn, &me).unwrap().is_none());
        set_user_farms_tx(&conn, uid, &[1]).unwrap();
        let a = allowed(&conn, &me).unwrap();
        assert!(visible(&a, Some(1)) && !visible(&a, Some(2)) && visible(&a, None));
        let boss = Session { role: Role::Manager, ..me };
        assert!(allowed(&conn, &boss).unwrap().is_none());
        assert!(set_user_farms_tx(&conn, uid, &[99]).is_err());
    }
}
