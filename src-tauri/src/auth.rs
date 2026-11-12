//! Sessions, roles, sign-in and staff accounts.

use crate::{db, recovery, templates, AppState};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use password_hash::SaltString;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use uuid::Uuid;

const SESSION_IDLE_TIMEOUT: Duration = Duration::from_secs(12 * 60 * 60);
const MAX_FAILED_LOGINS: u32 = 5;
const LOCKOUT: Duration = Duration::from_secs(60);
pub const MIN_PASSWORD_LEN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Farm worker: daily farm records only; no sales, no money.
    Worker,
    /// Veterinary or agricultural adviser: reads farm records, adds professional
    /// observations, plans vaccinations and reviews guides; no sales, no money.
    Adviser,
    Cashier,
    Manager,
    Owner,
}

impl Role {
    /// Access level used by `require`. Worker, adviser and cashier share the
    /// first level; what separates them is checked with `require_seller` and
    /// `require_adviser_or_manager`.
    pub fn rank(&self) -> u8 { match self { Role::Worker | Role::Adviser | Role::Cashier => 1, Role::Manager => 2, Role::Owner => 3 } }
    pub fn parse(s: &str) -> Result<Role, String> {
        match s {
            "owner" => Ok(Role::Owner),
            "manager" => Ok(Role::Manager),
            "cashier" => Ok(Role::Cashier),
            "worker" => Ok(Role::Worker),
            "adviser" => Ok(Role::Adviser),
            _ => Err("Role must be owner, manager, cashier, farm worker or adviser".into()),
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Owner => "owner",
            Role::Manager => "manager",
            Role::Cashier => "cashier",
            Role::Worker => "worker",
            Role::Adviser => "adviser",
        }
    }
    /// Can sell, take payments and see sales figures.
    pub fn can_sell(&self) -> bool { !matches!(self, Role::Worker | Role::Adviser) }
}

impl PartialOrd for Role {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.rank().cmp(&other.rank())) }
}

#[derive(Debug, Clone)]
pub struct Session {
    pub user_id: i64,
    pub username: String,
    pub full_name: String,
    pub role: Role,
    pub last_seen: Instant,
}

impl Session {
    pub fn display_name(&self) -> String {
        if self.full_name.trim().is_empty() { self.username.clone() } else { self.full_name.clone() }
    }
}

/// Validates the token, refreshes its idle timer and enforces the minimum role.
pub fn require(state: &tauri::State<'_, AppState>, token: &str, min_role: Role) -> Result<Session, String> {
    crate::license::require_active(state)?;
    let mut sessions = state.sessions.lock().map_err(|_| "Session lock was poisoned".to_string())?;
    let expired = match sessions.get(token) {
        Some(s) => s.last_seen.elapsed() > SESSION_IDLE_TIMEOUT,
        None => return Err("Your session has expired. Sign in again.".into()),
    };
    if expired {
        sessions.remove(token);
        return Err("Your session has expired. Sign in again.".into());
    }
    let session = sessions.get_mut(token).expect("checked above");
    session.last_seen = Instant::now();
    if session.role < min_role {
        return Err(format!("This action needs {} access.", min_role.as_str()));
    }
    Ok(session.clone())
}

/// For sales, receipts, customers and shifts: farm workers and advisers cannot sell or see takings.
pub fn require_seller(state: &tauri::State<'_, AppState>, token: &str, min_role: Role) -> Result<Session, String> {
    let s = require(state, token, min_role)?;
    if !s.role.can_sell() { return Err("Your account is for farm records only. Ask a manager to record sales.".into()); }
    Ok(s)
}

/// Managers and owners, or a veterinary / agricultural adviser.
pub fn require_adviser_or_manager(state: &tauri::State<'_, AppState>, token: &str) -> Result<Session, String> {
    let s = require(state, token, Role::Worker)?;
    if s.role == Role::Adviser || s.role >= Role::Manager { Ok(s) } else { Err("This action needs a manager or an adviser.".into()) }
}

/// Owners and managers see money; everyone else does not.
pub fn require_money(state: &tauri::State<'_, AppState>, token: &str) -> Result<Session, String> { require(state, token, Role::Manager) }

pub(crate) fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut password_hash::rand_core::OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}

pub(crate) fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
        .unwrap_or(false)
}

pub(crate) fn validate_username(username: &str) -> Result<String, String> {
    let u = username.trim();
    if u.len() < 3 || u.len() > 64 {
        return Err("Username must be between 3 and 64 characters".into());
    }
    if !u.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')) {
        return Err("Username can only contain letters, numbers, dots, dashes and underscores".into());
    }
    Ok(u.to_string())
}

pub(crate) fn validate_password(password: &str) -> Result<(), String> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(format!("Password must contain at least {MIN_PASSWORD_LEN} characters"));
    }
    if password.len() > 256 {
        return Err("Password is too long".into());
    }
    Ok(())
}

pub(crate) fn validate_name(name: &str) -> Result<String, String> {
    let n = name.trim();
    if n.is_empty() || n.chars().count() > 80 {
        return Err("Enter a full name of up to 80 characters".into());
    }
    Ok(n.to_string())
}

#[derive(Serialize)]
pub struct SessionInfo {
    token: String,
    user_id: i64,
    username: String,
    full_name: String,
    role: Role,
}

pub(crate) fn start_session(state: &tauri::State<'_, AppState>, user_id: i64, username: String, full_name: String, role: Role) -> Result<SessionInfo, String> {
    let token = Uuid::new_v4().to_string();
    state
        .sessions
        .lock()
        .map_err(|_| "Session lock was poisoned".to_string())?
        .insert(token.clone(), Session { user_id, username: username.clone(), full_name: full_name.clone(), role, last_seen: Instant::now() });
    Ok(SessionInfo { token, user_id, username, full_name, role })
}

pub(crate) fn revoke_user_sessions(state: &tauri::State<'_, AppState>, user_id: i64, keep_token: Option<&str>) {
    if let Ok(mut sessions) = state.sessions.lock() {
        sessions.retain(|t, s| s.user_id != user_id || Some(t.as_str()) == keep_token);
    }
}

// ------------------------------------------------------------------ public commands

#[derive(Serialize)]
pub struct SetupStatus {
    needs_setup: bool,
}

#[tauri::command]
pub fn setup_status(state: tauri::State<'_, AppState>) -> Result<SetupStatus, String> {
    let conn = state.db()?;
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    Ok(SetupStatus { needs_setup: count == 0 })
}

#[derive(Deserialize)]
pub struct SetupInput {
    business_name: String,
    business_type: String,
    load_samples: bool,
    full_name: String,
    username: String,
    password: String,
    security_questions: Vec<String>,
    security_answers: Vec<String>,
}

#[derive(Serialize)]
pub struct SetupResult {
    session: SessionInfo,
    /// Shown once so the owner can write it down; only its hash is stored.
    recovery_key: String,
}

/// First run: creates the owner account, the recovery key and security
/// questions, and configures the business template.
#[tauri::command]
pub fn setup_owner(state: tauri::State<'_, AppState>, input: SetupInput) -> Result<SetupResult, String> {
    let username = validate_username(&input.username)?;
    let full_name = validate_name(&input.full_name)?;
    validate_password(&input.password)?;
    recovery::validate_questions(&input.security_questions, &input.security_answers)?;
    let business_name = input.business_name.trim();
    if business_name.is_empty() || business_name.chars().count() > 120 {
        return Err("Enter your business name (up to 120 characters)".into());
    }
    let hash = hash_password(&input.password)?;
    let user_id = {
        let mut conn = state.db()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let count: i64 = tx.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        if count > 0 {
            return Err("This installation has already been set up".into());
        }
        tx.execute(
            "INSERT INTO users(username,full_name,password_hash,role,last_login_at) VALUES(?1,?2,?3,'owner',?4)",
            params![username, full_name, hash, db::now_local()],
        )
        .map_err(|e| format!("Could not create owner account: {e}"))?;
        let id = tx.last_insert_rowid();
        db::set_setting(&tx, "business_name", business_name)?;
        templates::apply_template(&tx, &input.business_type, input.load_samples, &full_name)?;
        recovery::save_questions(&tx, &input.security_questions, &input.security_answers)?;
        let key = recovery::rotate_key(&tx)?;
        tx.commit().map_err(|e| e.to_string())?;
        (id, key)
    };
    let (user_id, recovery_key) = user_id;
    Ok(SetupResult { session: start_session(&state, user_id, username, full_name, Role::Owner)?, recovery_key })
}

#[derive(Deserialize)]
pub struct FarmSetupInput {
    /// Answers to the guided farm questions (language, profile, activities, size, preferences).
    answers: serde_json::Map<String, serde_json::Value>,
    full_name: String,
    username: String,
    password: String,
    security_questions: Vec<String>,
    security_answers: Vec<String>,
}

/// First run: the farm questions, the owner account and account recovery, in
/// one transaction. Sets farm wording, adds the matching farm template's
/// starter items (no prices or stock) and creates the farm, batches and plots
/// from the answers.
#[tauri::command]
pub fn setup_farm(state: tauri::State<'_, AppState>, input: FarmSetupInput) -> Result<SetupResult, String> {
    let username = validate_username(&input.username)?;
    let full_name = validate_name(&input.full_name)?;
    validate_password(&input.password)?;
    recovery::validate_questions(&input.security_questions, &input.security_answers)?;
    crate::onboarding::validate(&input.answers, None)?;
    let hash = hash_password(&input.password)?;
    let (user_id, recovery_key) = {
        let mut conn = state.db()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let count: i64 = tx.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        if count > 0 {
            return Err("This installation has already been set up".into());
        }
        tx.execute(
            "INSERT INTO users(username,full_name,password_hash,role,last_login_at) VALUES(?1,?2,?3,'owner',?4)",
            params![username, full_name, hash, db::now_local()],
        )
        .map_err(|e| format!("Could not create owner account: {e}"))?;
        let id = tx.last_insert_rowid();
        templates::apply_template(&tx, "farm", false, &full_name)?;
        crate::farm_templates::apply_tx(&tx, crate::onboarding::template_for(&input.answers), &full_name)?;
        crate::onboarding::complete_tx(&tx, &input.answers)?;
        recovery::save_questions(&tx, &input.security_questions, &input.security_answers)?;
        let key = recovery::rotate_key(&tx)?;
        tx.commit().map_err(|e| e.to_string())?;
        (id, key)
    };
    Ok(SetupResult { session: start_session(&state, user_id, username, full_name, Role::Owner)?, recovery_key })
}

#[tauri::command]
pub fn login(state: tauri::State<'_, AppState>, username: String, password: String) -> Result<SessionInfo, String> {
    let key = username.trim().to_lowercase();
    {
        let failures = state.login_failures.lock().map_err(|_| "Lock poisoned".to_string())?;
        if let Some((count, since)) = failures.get(&key) {
            if *count >= MAX_FAILED_LOGINS && since.elapsed() < LOCKOUT {
                let wait = (LOCKOUT - since.elapsed()).as_secs().max(1);
                return Err(format!("Too many failed attempts. Try again in {wait} seconds."));
            }
        }
    }
    let row: Option<(i64, String, String, String, String, i64)> = {
        let conn = state.db()?;
        conn.query_row(
            "SELECT id,username,full_name,password_hash,role,active FROM users WHERE username=?1 COLLATE NOCASE",
            [username.trim()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?
    };
    let ok = matches!(&row, Some((_, _, _, hash, _, _)) if verify_password(&password, hash));
    if !ok {
        let mut failures = state.login_failures.lock().map_err(|_| "Lock poisoned".to_string())?;
        let entry = failures.entry(key).or_insert((0, Instant::now()));
        if entry.1.elapsed() >= LOCKOUT { *entry = (0, Instant::now()); }
        entry.0 += 1;
        entry.1 = Instant::now();
        return Err("Incorrect username or password".into());
    }
    let (id, uname, full_name, _, role, active) = row.expect("verified above");
    if active == 0 {
        return Err("This account has been deactivated. Ask the owner to reactivate it.".into());
    }
    state.login_failures.lock().map_err(|_| "Lock poisoned".to_string())?.remove(&key);
    state.db()?.execute("UPDATE users SET last_login_at=?1 WHERE id=?2", params![db::now_local(), id]).map_err(|e| e.to_string())?;
    start_session(&state, id, uname, full_name, Role::parse(&role)?)
}

#[tauri::command]
pub fn logout(state: tauri::State<'_, AppState>, session_token: String) -> Result<(), String> {
    state.sessions.lock().map_err(|_| "Session lock was poisoned".to_string())?.remove(&session_token);
    Ok(())
}

#[tauri::command]
pub fn change_own_password(state: tauri::State<'_, AppState>, session_token: String, current_password: String, new_password: String) -> Result<(), String> {
    let session = require(&state, &session_token, Role::Cashier)?;
    validate_password(&new_password)?;
    let conn = state.db()?;
    let hash: String = conn.query_row("SELECT password_hash FROM users WHERE id=?1", [session.user_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    if !verify_password(&current_password, &hash) {
        return Err("Your current password is incorrect".into());
    }
    conn.execute("UPDATE users SET password_hash=?1,updated_at=CURRENT_TIMESTAMP WHERE id=?2", params![hash_password(&new_password)?, session.user_id]).map_err(|e| e.to_string())?;
    drop(conn);
    revoke_user_sessions(&state, session.user_id, Some(&session_token));
    Ok(())
}

// ------------------------------------------------------------------ staff management (owner)

#[derive(Serialize)]
pub struct UserRow {
    id: i64,
    username: String,
    full_name: String,
    role: String,
    active: bool,
    last_login_at: Option<String>,
    created_at: String,
}

#[tauri::command]
pub fn list_users(state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<UserRow>, String> {
    require(&state, &session_token, Role::Owner)?;
    let conn = state.db()?;
    let mut stmt = conn
        .prepare("SELECT id,username,full_name,role,active,last_login_at,created_at FROM users ORDER BY active DESC, CASE role WHEN 'owner' THEN 1 WHEN 'manager' THEN 2 ELSE 3 END, full_name COLLATE NOCASE")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok(UserRow { id: r.get(0)?, username: r.get(1)?, full_name: r.get(2)?, role: r.get(3)?, active: r.get::<_, i64>(4)? == 1, last_login_at: r.get(5)?, created_at: r.get(6)? }))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct UserInput {
    id: Option<i64>,
    username: String,
    full_name: String,
    role: String,
    active: bool,
    password: Option<String>,
}

fn active_owner_count(conn: &Connection, excluding: i64) -> Result<i64, String> {
    conn.query_row("SELECT COUNT(*) FROM users WHERE role='owner' AND active=1 AND id<>?1", [excluding], |r| r.get(0)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_user(state: tauri::State<'_, AppState>, session_token: String, input: UserInput) -> Result<i64, String> {
    let me = require(&state, &session_token, Role::Owner)?;
    let username = validate_username(&input.username)?;
    let full_name = validate_name(&input.full_name)?;
    let role = Role::parse(&input.role)?;
    let conn = state.db()?;
    let id = match input.id {
        Some(id) => {
            let current_role: String = conn.query_row("SELECT role FROM users WHERE id=?1", [id], |r| r.get(0)).optional().map_err(|e| e.to_string())?.ok_or("Staff account not found")?;
            if id == me.user_id && !input.active {
                return Err("You cannot deactivate your own account".into());
            }
            if id == me.user_id && role != me.role {
                return Err("You cannot change your own role. Ask another owner to do it.".into());
            }
            if current_role == "owner" && (role != Role::Owner || !input.active) && active_owner_count(&conn, id)? == 0 {
                return Err("Keep at least one active owner account".into());
            }
            conn.execute(
                "UPDATE users SET username=?1,full_name=?2,role=?3,active=?4,updated_at=CURRENT_TIMESTAMP WHERE id=?5",
                params![username, full_name, role.as_str(), input.active as i64, id],
            )
            .map_err(|e| if e.to_string().contains("UNIQUE") { "That username is already taken".to_string() } else { e.to_string() })?;
            if let Some(pw) = input.password.as_deref().filter(|p| !p.is_empty()) {
                validate_password(pw)?;
                conn.execute("UPDATE users SET password_hash=?1 WHERE id=?2", params![hash_password(pw)?, id]).map_err(|e| e.to_string())?;
            }
            id
        }
        None => {
            let pw = input.password.as_deref().unwrap_or("");
            validate_password(pw)?;
            conn.execute(
                "INSERT INTO users(username,full_name,password_hash,role,active) VALUES(?1,?2,?3,?4,?5)",
                params![username, full_name, hash_password(pw)?, role.as_str(), input.active as i64],
            )
            .map_err(|e| if e.to_string().contains("UNIQUE") { "That username is already taken".to_string() } else { e.to_string() })?;
            conn.last_insert_rowid()
        }
    };
    crate::audit::log(&conn, &me.display_name(), if input.id.is_some() { "user_updated" } else { "user_created" }, "user", Some(id), &format!("{} as {}{}", username, role.as_str(), if input.active { "" } else { " (inactive)" }));
    drop(conn);
    // Role or status changes take effect immediately: the user signs in again.
    if input.id.is_some() && id != me.user_id {
        revoke_user_sessions(&state, id, None);
    }
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_ordering() {
        assert!(Role::Owner > Role::Manager && Role::Manager > Role::Cashier);
    }

    #[test]
    fn password_roundtrip() {
        let h = hash_password("correct horse").unwrap();
        assert!(verify_password("correct horse", &h));
        assert!(!verify_password("wrong", &h));
    }

    #[test]
    fn username_rules() {
        assert!(validate_username("ada.o").is_ok());
        assert!(validate_username("a b").is_err());
        assert!(validate_username("ab").is_err());
    }
}

/// Serialises a result for the UI, blanking every money field (`*_kobo`) and
/// marking `money_hidden` for roles below Manager. Workers, advisers and
/// cashiers see quantities, never costs, prices, revenue or profit.
pub fn money_view<T: serde::Serialize>(me: &Session, v: &T) -> Result<serde_json::Value, String> {
    let mut j = serde_json::to_value(v).map_err(|e| e.to_string())?;
    if me.role < Role::Manager { blank_money(&mut j); }
    Ok(j)
}

fn blank_money(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(m) => {
            let mut has_money = false;
            for (k, x) in m.iter_mut() {
                if k.ends_with("_kobo") { *x = serde_json::Value::from(0); has_money = true; } else { blank_money(x); }
            }
            if has_money { m.insert("money_hidden".into(), serde_json::Value::Bool(true)); }
        }
        serde_json::Value::Array(a) => a.iter_mut().for_each(blank_money),
        _ => {}
    }
}

#[cfg(test)]
mod money_tests {
    use super::*;
    #[test]
    fn workers_never_receive_money() {
        let s = |role| Session { user_id: 1, username: "u".into(), full_name: String::new(), role, last_seen: std::time::Instant::now() };
        let v = serde_json::json!({ "alive": 10, "profit_kobo": 5000, "entries": [{ "amount_kobo": 7, "feed_kg": 2.0 }] });
        let w = money_view(&s(Role::Worker), &v).unwrap();
        assert_eq!(w["profit_kobo"], 0);
        assert_eq!(w["entries"][0]["amount_kobo"], 0);
        assert_eq!(w["entries"][0]["feed_kg"], 2.0);
        assert_eq!(w["alive"], 10);
        assert_eq!(money_view(&s(Role::Manager), &v).unwrap()["profit_kobo"], 5000);
    }
}
