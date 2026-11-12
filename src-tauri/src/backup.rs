//! Automatic and manual database backups.
//!
//! A background thread makes one backup a day (checked every 30 minutes) into
//! the backup folder (default: Documents/FarmBuddy Backups), keeping the newest
//! N files. The owner can change the folder (e.g. a flash drive or a synced
//! Google Drive folder), the number kept, or switch automatic backups off.

use crate::{auth::{self, Role}, db, AppState};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}, time::Duration};
use tauri::Manager;

const PREFIX: &str = "farmbuddy-backup-";
const AUTO_INTERVAL_HOURS: i64 = 20;

pub fn default_dir(app: &tauri::AppHandle) -> PathBuf {
    app.path().document_dir().or_else(|_| app.path().home_dir()).unwrap_or_else(|_| std::env::temp_dir()).join("FarmBuddy Backups")
}

pub(crate) fn effective_dir(app: &tauri::AppHandle, conn: &Connection) -> PathBuf {
    let custom = db::setting(conn, "backup_dir").unwrap_or_default();
    if custom.trim().is_empty() { default_dir(app) } else { PathBuf::from(custom.trim()) }
}

/// Copies the live database into `dir` using SQLite's online backup API, then
/// removes the oldest backups beyond `keep`.
pub fn backup_to(conn: &Connection, dir: &Path, keep: usize) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| format!("Could not open the backup folder {}: {e}", dir.display()))?;
    let path = dir.join(format!("{PREFIX}{}.sqlite3", chrono::Local::now().format("%Y%m%d-%H%M%S")));
    let mut dest = Connection::open(&path).map_err(|e| format!("Could not create the backup file: {e}"))?;
    let backup = rusqlite::backup::Backup::new(conn, &mut dest).map_err(|e| e.to_string())?;
    backup.run_to_completion(512, Duration::from_millis(0), None).map_err(|e| format!("Backup failed: {e}"))?;
    drop(backup);
    drop(dest);
    prune(dir, keep);
    Ok(path)
}

fn list_files(dir: &Path) -> Vec<(String, u64, String)> {
    let mut files: Vec<(String, u64, String)> = fs::read_dir(dir)
        .map(|rd| rd.flatten().filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.starts_with(PREFIX) || !name.ends_with(".sqlite3") { return None; }
            let meta = e.metadata().ok()?;
            let modified = meta.modified().ok().map(|t| chrono::DateTime::<chrono::Local>::from(t).format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_default();
            Some((name, meta.len(), modified))
        }).collect())
        .unwrap_or_default();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files
}

fn prune(dir: &Path, keep: usize) {
    for (name, _, _) in list_files(dir).into_iter().skip(keep.max(1)) {
        let _ = fs::remove_file(dir.join(name));
    }
}

pub(crate) fn keep_count(conn: &Connection) -> usize {
    db::setting(conn, "backup_keep").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(14).clamp(1, 365)
}

/// Starts the once-a-day automatic backup worker.
pub fn start_auto(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(20));
        loop {
            let _ = run_auto_if_due(&app);
            std::thread::sleep(Duration::from_secs(30 * 60));
        }
    });
}

fn run_auto_if_due(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let conn = state.db()?;
    if db::setting(&conn, "auto_backup")? != "1" { return Ok(()); }
    let last = db::setting(&conn, "last_auto_backup")?;
    let due = chrono::NaiveDateTime::parse_from_str(&last, "%Y-%m-%d %H:%M:%S")
        .map(|t| chrono::Local::now().naive_local() - t >= chrono::Duration::hours(AUTO_INTERVAL_HOURS))
        .unwrap_or(true);
    if !due { return Ok(()); }
    let dir = effective_dir(app, &conn);
    match backup_to(&conn, &dir, keep_count(&conn)) {
        Ok(_) => { db::set_setting(&conn, "last_auto_backup", &db::now_local())?; db::set_setting(&conn, "last_backup_error", "")?; }
        Err(e) => { db::set_setting(&conn, "last_backup_error", &e)?; }
    }
    Ok(())
}

// ------------------------------------------------------------------ commands (owner)

#[derive(Serialize)]
pub struct BackupFile { name: String, size: u64, modified: String }

#[derive(Serialize)]
pub struct BackupStatus { auto: bool, dir: String, default_dir: String, keep: usize, last_auto: String, last_error: String, files: Vec<BackupFile> }

#[tauri::command]
pub fn backup_status(app: tauri::AppHandle, state: tauri::State<'_, AppState>, session_token: String) -> Result<BackupStatus, String> {
    auth::require(&state, &session_token, Role::Owner)?;
    let conn = state.db()?;
    let dir = effective_dir(&app, &conn);
    Ok(BackupStatus {
        auto: db::setting(&conn, "auto_backup")? == "1", dir: dir.display().to_string(), default_dir: default_dir(&app).display().to_string(),
        keep: keep_count(&conn), last_auto: db::setting(&conn, "last_auto_backup")?, last_error: db::setting(&conn, "last_backup_error")?,
        files: list_files(&dir).into_iter().take(60).map(|(name, size, modified)| BackupFile { name, size, modified }).collect(),
    })
}

#[derive(Deserialize)]
pub struct BackupSettingsInput { auto: bool, dir: String, keep: usize }

#[tauri::command]
pub fn save_backup_settings(state: tauri::State<'_, AppState>, session_token: String, input: BackupSettingsInput) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Owner)?;
    if !(1..=365).contains(&input.keep) { return Err("Keep between 1 and 365 backups".into()); }
    let dir = input.dir.trim();
    if dir.len() > 1000 { return Err("Folder path is too long".into()); }
    if !dir.is_empty() {
        let p = Path::new(dir);
        if !p.is_absolute() { return Err("Choose a full folder path".into()); }
        fs::create_dir_all(p).map_err(|e| format!("Cannot use that folder: {e}"))?;
        let probe = p.join(".estock-write-test");
        fs::write(&probe, b"ok").map_err(|e| format!("FarmBuddy cannot write to that folder: {e}"))?;
        let _ = fs::remove_file(probe);
    }
    let conn = state.db()?;
    db::set_setting(&conn, "auto_backup", if input.auto { "1" } else { "0" })?;
    db::set_setting(&conn, "backup_dir", dir)?;
    db::set_setting(&conn, "backup_keep", &input.keep.to_string())
}

#[tauri::command]
pub fn backup_now(app: tauri::AppHandle, state: tauri::State<'_, AppState>, session_token: String) -> Result<String, String> {
    auth::require(&state, &session_token, Role::Owner)?;
    let conn = state.db()?;
    let dir = effective_dir(&app, &conn);
    let path = backup_to(&conn, &dir, keep_count(&conn))?;
    db::set_setting(&conn, "last_auto_backup", &db::now_local())?;
    db::set_setting(&conn, "last_backup_error", "")?;
    Ok(path.display().to_string())
}

// ------------------------------------------------------------------ restore

#[derive(Serialize, Debug)]
pub struct RestoreCheck {
    pub path: String,
    pub schema_version: i64,
    pub needs_upgrade: bool,
    pub business_name: String,
    pub users: i64,
    pub products: i64,
    pub receipts: i64,
    pub batches: i64,
    pub plots: i64,
    pub size_bytes: u64,
}

fn count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap_or(0)
}

/// Copies the chosen file to a temporary place, checks it is a healthy
/// FarmBuddy (or eStock) database from this or an older version, and upgrades
/// the copy. The live database is not touched. Returns the checked copy.
pub fn prepare_restore(path: &Path) -> Result<(Connection, RestoreCheck, PathBuf), String> {
    let meta = fs::metadata(path).map_err(|e| format!("Could not open the backup file: {e}"))?;
    if meta.len() < 1024 { return Err("This file is too small to be a FarmBuddy backup".into()); }
    let mut head = [0u8; 16];
    use std::io::Read;
    fs::File::open(path).and_then(|mut f| f.read_exact(&mut head)).map_err(|e| format!("Could not read the backup file: {e}"))?;
    if &head != b"SQLite format 3\0" { return Err("This file is not a FarmBuddy backup (it is not a database file)".into()); }
    let tmp = std::env::temp_dir().join(format!("farmbuddy-restore-{}.sqlite3", uuid::Uuid::new_v4()));
    fs::copy(path, &tmp).map_err(|e| format!("Could not copy the backup file: {e}"))?;
    let conn = Connection::open(&tmp).map_err(|e| format!("Could not open the backup: {e}"))?;
    let ok: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0)).map_err(|e| format!("The backup is damaged: {e}"))?;
    if ok != "ok" { return Err(format!("The backup is damaged and cannot be restored ({ok})")); }
    for t in ["users", "settings", "products", "receipts"] {
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1", [t], |r| r.get(0)).map_err(|e| e.to_string())?;
        if n == 0 { return Err(format!("This database is not a FarmBuddy backup (no {t} table)")); }
    }
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    if version > db::SCHEMA_VERSION { return Err(format!("This backup comes from a newer FarmBuddy (data version {version}). Update the app first, then restore.")); }
    let owners: i64 = conn.query_row("SELECT COUNT(*) FROM users WHERE role='owner' AND active=1", [], |r| r.get(0)).unwrap_or(0);
    if owners == 0 { return Err("This backup has no active owner account, so nobody could sign in after restoring it".into()); }
    db::configure(&conn)?;
    db::migrate(&conn).map_err(|e| format!("The backup could not be upgraded to this version: {e}"))?;
    let has = |t: &str| conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1", [t], |r| r.get::<_, i64>(0)).unwrap_or(0) > 0;
    let check = RestoreCheck {
        path: path.display().to_string(),
        schema_version: version,
        needs_upgrade: version < db::SCHEMA_VERSION,
        business_name: db::setting(&conn, "business_name").unwrap_or_default(),
        users: count(&conn, "users"),
        products: count(&conn, "products"),
        receipts: count(&conn, "receipts"),
        batches: if has("flocks") { count(&conn, "flocks") } else { 0 },
        plots: if has("farm_plots") { count(&conn, "farm_plots") } else { 0 },
        size_bytes: meta.len(),
    };
    Ok((conn, check, tmp))
}

/// Replaces the live database with the checked copy, in place.
pub fn restore_into(src: &Connection, live: &mut Connection) -> Result<(), String> {
    let backup = rusqlite::backup::Backup::new(src, live).map_err(|e| e.to_string())?;
    backup.run_to_completion(512, Duration::from_millis(0), None).map_err(|e| format!("Restore failed: {e}"))?;
    drop(backup);
    db::configure(live)?;
    db::migrate(live)
}

#[derive(Serialize)]
pub struct RestorableFile { name: String, path: String, size_bytes: u64, modified: String }

#[tauri::command]
pub fn list_backup_files(app: tauri::AppHandle, state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<RestorableFile>, String> {
    auth::require(&state, &session_token, Role::Owner)?;
    let dir = effective_dir(&app, &*state.db()?);
    Ok(list_files(&dir).into_iter().map(|(name, size, modified)| RestorableFile { path: dir.join(&name).display().to_string(), name, size_bytes: size, modified }).collect())
}

#[tauri::command]
pub fn check_backup_file(state: tauri::State<'_, AppState>, session_token: String, path: String) -> Result<RestoreCheck, String> {
    auth::require(&state, &session_token, Role::Owner)?;
    let (conn, check, tmp) = prepare_restore(Path::new(&path))?;
    drop(conn);
    let _ = fs::remove_file(&tmp);
    Ok(check)
}

#[derive(Deserialize)]
pub struct RestoreInput { pub path: String, pub password: String, pub confirm: String }

#[derive(Serialize)]
pub struct RestoreResult { pub safety_backup: String, pub check: RestoreCheck }

#[tauri::command]
pub fn restore_backup(app: tauri::AppHandle, state: tauri::State<'_, AppState>, session_token: String, input: RestoreInput) -> Result<RestoreResult, String> {
    let me = auth::require(&state, &session_token, Role::Owner)?;
    if input.confirm.trim().to_uppercase() != "RESTORE" { return Err("Type RESTORE in capital letters to confirm.".into()); }
    let mut live = state.db()?;
    let hash: String = live.query_row("SELECT password_hash FROM users WHERE id=?1", [me.user_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    if !auth::verify_password(&input.password, &hash) { return Err("That password is not correct.".into()); }
    let (src, check, tmp) = prepare_restore(Path::new(&input.path))?;
    // Safety copy of what is on the device now. If it fails, nothing changes.
    let dir = effective_dir(&app, &live);
    let safety = backup_to(&live, &dir, keep_count(&live).max(5)).map_err(|e| format!("Nothing was restored, because a safety backup of the current data could not be made first. {e}"))?;
    let result = restore_into(&src, &mut live);
    drop(src);
    let _ = fs::remove_file(&tmp);
    result?;
    crate::audit::log(&live, &me.display_name(), "backup_restored", "database", None, &format!("from {} (safety copy {})", input.path, safety.display()));
    drop(live);
    // Everyone signs in again against the restored staff list.
    if let Ok(mut s) = state.sessions.lock() { s.clear(); }
    Ok(RestoreResult { safety_backup: safety.display().to_string(), check })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_validates_and_replaces_data() {
        let dir = std::env::temp_dir().join(format!("fb-restore-{}", uuid::Uuid::new_v4()));
        let a = db::test_conn();
        a.execute("INSERT INTO users(username,password_hash,role) VALUES('boss','x','owner')", []).unwrap();
        db::set_setting(&a, "business_name", "Backup Farm").unwrap();
        a.execute("INSERT INTO farms(name) VALUES('From backup')", []).unwrap();
        let file = backup_to(&a, &dir, 5).unwrap();
        let (src, check, tmp) = prepare_restore(&file).unwrap();
        assert_eq!(check.business_name, "Backup Farm");
        let mut live = db::test_conn();
        live.execute("INSERT INTO farms(name) VALUES('Live only')", []).unwrap();
        restore_into(&src, &mut live).unwrap();
        let names: Vec<String> = crate::farms::rows(&live, "SELECT name FROM farms", [], |r| r.get(0)).unwrap();
        assert_eq!(names, vec!["From backup".to_string()]);
        let _ = fs::remove_file(tmp);
        // Rubbish and owner-less databases are refused.
        let junk = dir.join("junk.sqlite3");
        fs::write(&junk, vec![b'x'; 4096]).unwrap();
        assert!(prepare_restore(&junk).is_err());
        let empty = dir.join("empty.sqlite3");
        let e = Connection::open(&empty).unwrap();
        e.execute_batch("CREATE TABLE users(id INTEGER, role TEXT, active INTEGER); CREATE TABLE settings(key TEXT); CREATE TABLE products(id INTEGER); CREATE TABLE receipts(id INTEGER); CREATE TABLE pad(x BLOB); INSERT INTO pad VALUES(zeroblob(4096));").unwrap();
        drop(e);
        assert!(prepare_restore(&empty).unwrap_err().contains("owner"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn backups_are_made_and_pruned() {
        let conn = db::test_conn();
        let dir = std::env::temp_dir().join(format!("estock-test-{}", uuid::Uuid::new_v4()));
        for i in 0..4 {
            let p = backup_to(&conn, &dir, 2).unwrap();
            assert!(p.exists());
            // File names carry the time to the second; rename so each run is distinct.
            let renamed = dir.join(format!("{PREFIX}2000010{i}-000000.sqlite3"));
            fs::rename(&p, &renamed).unwrap();
        }
        prune(&dir, 2);
        assert_eq!(list_files(&dir).len(), 2);
        let _ = fs::remove_dir_all(dir);
    }
}
