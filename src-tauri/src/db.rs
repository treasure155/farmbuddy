//! Database bootstrap, versioned migrations and small shared helpers.

use rusqlite::{params, Connection, OptionalExtension};
use std::{collections::HashMap, fs, path::PathBuf};
use tauri::Manager;

///
/// FarmBuddy uses its own database file, separate from eStock Manager.
pub const DB_FILE: &str = "farmbuddy.sqlite3";
#[cfg_attr(not(test), allow(dead_code))]
pub const SCHEMA_VERSION: i64 = 12;

pub fn db_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create app data directory: {e}"))?;
    Ok(dir.join(DB_FILE))
}

pub fn open(path: &PathBuf) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| format!("Could not open database: {e}"))?;
    configure(&conn)?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn configure(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA busy_timeout = 5000;
         PRAGMA foreign_keys = ON;",
    )
    .map_err(|e| format!("Database configuration failed: {e}"))
}

/// Applies migrations in order. 001 is idempotent (CREATE IF NOT EXISTS) and
/// runs only on a fresh database; later migrations are gated on `PRAGMA user_version`.
pub fn migrate(conn: &Connection) -> Result<(), String> {
    let version = user_version(conn)?;
    if version < 2 {
        conn.execute_batch(include_str!("../migrations/001_initial.sql"))
            .map_err(|e| format!("Database initialization failed: {e}"))?;
        run_migration(conn, include_str!("../migrations/002_universal.sql"), 2, true)?;
    }
    if user_version(conn)? < 3 {
        run_migration(conn, include_str!("../migrations/003_recovery.sql"), 3, false)?;
    }
    if user_version(conn)? < 4 {
        run_migration(conn, include_str!("../migrations/004_v2.sql"), 4, true)?;
    }
    if user_version(conn)? < 5 {
        run_migration(conn, include_str!("../migrations/005_farm.sql"), 5, true)?;
    }
    if user_version(conn)? < 6 {
        run_migration(conn, include_str!("../migrations/006_complete.sql"), 6, true)?;
    }
    if user_version(conn)? < 7 {
        run_migration(conn, include_str!("../migrations/007_identity.sql"), 7, true)?;
    }
    if user_version(conn)? < 8 {
        run_migration(conn, include_str!("../migrations/008_palm.sql"), 8, true)?;
    }
    if user_version(conn)? < 9 {
        run_migration(conn, include_str!("../migrations/009_process_runs.sql"), 9, true)?;
    }
    if user_version(conn)? < 10 {
        run_migration(conn, include_str!("../migrations/010_runs_and_sales.sql"), 10, true)?;
        crate::palm_oil::upgrade_sync(conn)?;
    }
    if user_version(conn)? < 11 {
        run_migration(conn, include_str!("../migrations/011_poultry.sql"), 11, true)?;
    }
    if user_version(conn)? < 12 {
        run_migration(conn, include_str!("../migrations/012_feed_kg.sql"), 12, true)?;
    }
    // Built-in knowledge articles and sources are refreshed on every start;
    // articles the owner has edited are left alone.
    crate::knowledge::seed_builtin(conn)?;
    crate::advisory::seed_rules(conn)?;
    Ok(())
}

fn user_version(conn: &Connection) -> Result<i64, String> {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(|e| e.to_string())
}

/// Runs one migration in a transaction. Table rebuilds need foreign keys off;
/// that PRAGMA is a no-op inside a transaction, so it is toggled around it.
fn run_migration(conn: &Connection, sql: &str, version: i64, rebuilds_tables: bool) -> Result<(), String> {
    if rebuilds_tables { conn.execute_batch("PRAGMA foreign_keys = OFF;").map_err(|e| e.to_string())?; }
    let result = (|| -> Result<(), String> {
        conn.execute_batch("BEGIN IMMEDIATE;").map_err(|e| e.to_string())?;
        if let Err(e) = conn.execute_batch(sql) {
            let _ = conn.execute_batch("ROLLBACK;");
            return Err(format!("Upgrade to schema {version} failed: {e}"));
        }
        let violations: i64 = conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        if violations > 0 {
            let _ = conn.execute_batch("ROLLBACK;");
            return Err(format!("Upgrade aborted: {violations} foreign key problems found"));
        }
        conn.execute_batch(&format!("PRAGMA user_version = {version}; COMMIT;")).map_err(|e| e.to_string())
    })();
    if rebuilds_tables { conn.execute_batch("PRAGMA foreign_keys = ON;").map_err(|e| e.to_string())?; }
    result
}

pub fn setting(conn: &Connection, key: &str) -> Result<String, String> {
    conn.query_row("SELECT value FROM settings WHERE key=?1", [key], |r| r.get::<_, String>(0))
        .optional()
        .map_err(|e| e.to_string())
        .map(|v| v.unwrap_or_default())
}

pub fn setting_bool(conn: &Connection, key: &str) -> Result<bool, String> {
    Ok(setting(conn, key)? == "1")
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        params![key, value],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

pub fn all_settings(conn: &Connection) -> Result<HashMap<String, String>, String> {
    let mut stmt = conn.prepare("SELECT key,value FROM settings").map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<HashMap<_, _>, _>>().map_err(|e| e.to_string())
}

/// Formats kobo as a currency string, e.g. `₦ 12,500.00`.
pub fn format_money(symbol: &str, kobo: i64) -> String {
    let negative = kobo < 0;
    let abs = kobo.unsigned_abs();
    let whole = (abs / 100).to_string();
    let mut grouped = String::new();
    for (i, ch) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    let sym = if symbol.trim().is_empty() { "₦" } else { symbol.trim() };
    format!("{}{} {}.{:02}", if negative { "-" } else { "" }, sym, grouped, abs % 100)
}

pub fn format_qty(q: f64) -> String {
    if (q - q.round()).abs() < 1e-9 {
        format!("{}", q.round() as i64)
    } else {
        let s = format!("{:.3}", q);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

pub fn now_local() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(test)]
pub fn test_conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    migrate(&conn).unwrap();
    conn
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn money_formatting() {
        assert_eq!(format_money("₦", 1250000), "₦ 12,500.00");
        assert_eq!(format_money("", 5), "₦ 0.05");
        assert_eq!(format_money("$", 123456789), "$ 1,234,567.89");
    }

    #[test]
    fn fresh_database_reaches_latest_schema() {
        let conn = test_conn();
        let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        assert_eq!(setting(&conn, "theme_primary").unwrap(), "#17845A");
        assert_eq!(setting(&conn, "language").unwrap(), "en");
        // Running migrate again is a no-op.
        migrate(&conn).unwrap();
    }

    #[test]
    fn upgrades_eatery_database_preserving_data() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/001_initial.sql")).unwrap();
        conn.execute_batch(
            "INSERT INTO users(username,password_hash) VALUES('admin','x');
             UPDATE settings SET value='Mama Put' WHERE key='eatery_name';
             INSERT INTO products(sku,name,kind,unit,selling_price_kobo,stock_qty) VALUES('JR1','Jollof rice','finished','plate',150000,10);
             INSERT INTO products(sku,name,kind,unit,stock_qty) VALUES('RICE','Rice','ingredient','kg',50);
             INSERT INTO receipts(receipt_no,subtotal_kobo,total_kobo) VALUES('EAT-1',300000,300000);
             INSERT INTO receipt_items(receipt_id,product_id,product_name,product_kind,unit,quantity,unit_price_kobo,line_total_kobo) VALUES(1,1,'Jollof rice','finished','plate',2,150000,300000);
             INSERT INTO receipt_payments(receipt_id,method,amount_kobo) VALUES(1,'transfer',300000);
             INSERT INTO stock_movements(product_id,movement_type,quantity_delta) VALUES(1,'sale',-2);",
        )
        .unwrap();
        migrate(&conn).unwrap();
        let role: String = conn.query_row("SELECT role FROM users", [], |r| r.get(0)).unwrap();
        assert_eq!(role, "owner");
        let kinds: Vec<String> = conn
            .prepare("SELECT kind FROM products ORDER BY id").unwrap()
            .query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
        assert_eq!(kinds, vec!["product", "material"]);
        assert_eq!(setting(&conn, "business_name").unwrap(), "Mama Put");
        assert_eq!(setting(&conn, "business_type").unwrap(), "farm");
        let method: String = conn.query_row("SELECT method FROM receipt_payments", [], |r| r.get(0)).unwrap();
        assert_eq!(method, "Bank transfer");
        let paid: i64 = conn.query_row("SELECT amount_paid_kobo FROM receipts", [], |r| r.get(0)).unwrap();
        assert_eq!(paid, 300000);
        let fk: i64 = conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0)).unwrap();
        assert_eq!(fk, 0);
    }
}
