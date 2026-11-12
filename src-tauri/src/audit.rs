//! Audit trail for sensitive changes: deletions, closures, reviews, settings,
//! staff and role changes, demo data, restores and content imports. Entries
//! are append-only; nothing in the app edits or deletes them.

use crate::{auth::{self, Role}, farms::rows, AppState};
use rusqlite::{params, Connection};
use serde::Serialize;

/// Records one action. Failures are ignored so an audit problem never blocks the work itself.
pub fn log(conn: &Connection, user: &str, action: &str, entity: &str, entity_id: Option<i64>, details: &str) {
    let details: String = details.chars().take(500).collect();
    let _ = conn.execute("INSERT INTO audit_logs(at,user_name,action,entity,entity_id,details) VALUES(?1,?2,?3,?4,?5,?6)",
        params![crate::db::now_local(), user, action, entity, entity_id, details]);
}

#[derive(Serialize)]
pub struct AuditEntry { id: i64, at: String, user_name: String, action: String, entity: String, entity_id: Option<i64>, details: String }

pub fn list_tx(conn: &Connection, query: &str, limit: i64) -> Result<Vec<AuditEntry>, String> {
    let q = format!("%{}%", query.trim().to_lowercase());
    rows(conn, "SELECT id,at,user_name,action,entity,entity_id,details FROM audit_logs WHERE ?1='%%' OR lower(action||' '||entity||' '||details||' '||user_name) LIKE ?1 ORDER BY id DESC LIMIT ?2",
        params![q, limit.clamp(1, 2000)], |r| Ok(AuditEntry { id: r.get(0)?, at: r.get(1)?, user_name: r.get(2)?, action: r.get(3)?, entity: r.get(4)?, entity_id: r.get(5)?, details: r.get(6)? }))
}

#[tauri::command]
pub fn list_audit(state: tauri::State<'_, AppState>, session_token: String, query: Option<String>) -> Result<Vec<AuditEntry>, String> {
    auth::require(&state, &session_token, Role::Owner)?;
    list_tx(&*state.db()?, &query.unwrap_or_default(), 500)
}

#[cfg(test)]
mod tests {
    #[test]
    fn audit_is_searchable() {
        let conn = crate::db::test_conn();
        super::log(&conn, "Ada", "delete", "flock", Some(3), "Deleted batch BR-1");
        super::log(&conn, "Ada", "settings", "settings", None, "language");
        assert_eq!(super::list_tx(&conn, "br-1", 10).unwrap().len(), 1);
        assert_eq!(super::list_tx(&conn, "", 10).unwrap().len(), 2);
    }
}
