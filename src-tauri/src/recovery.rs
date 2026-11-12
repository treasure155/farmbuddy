//! Offline owner recovery.
//!
//! The owner proves who they are with EITHER the recovery key shown at setup
//! OR the answers to three security questions. A successful check can reset
//! any account's password ("Forgot password") or create a new Owner account
//! ("Create account" on the sign-in screen). Using the key replaces it with a
//! fresh one, so a key that has been seen by someone else stops working.

use crate::{auth::{self, Role, SessionInfo}, db, AppState};
use password_hash::rand_core::{OsRng, RngCore};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

const MAX_FAILED_RECOVERY: u32 = 5;
const RECOVERY_LOCKOUT: Duration = Duration::from_secs(10 * 60);
/// No 0/O or 1/I/L, so the key is easy to read back from paper.
const KEY_ALPHABET: &[u8; 31] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";

pub fn generate_key() -> String {
    // Rejection sampling keeps every character equally likely.
    let mut raw = String::with_capacity(16);
    let mut buf = [0u8; 32];
    while raw.len() < 16 {
        OsRng.fill_bytes(&mut buf);
        for b in buf {
            if raw.len() == 16 { break; }
            if (b as usize) < 31 * 8 { raw.push(KEY_ALPHABET[(b as usize) % 31] as char); }
        }
    }
    format!("{}-{}-{}-{}", &raw[0..4], &raw[4..8], &raw[8..12], &raw[12..16])
}

fn normalize_key(k: &str) -> String {
    k.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_uppercase()
}

fn normalize_answer(a: &str) -> String {
    a.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

pub(crate) fn log_event(conn: &Connection, event: &str, username: &str, detail: &str) {
    let _ = conn.execute("INSERT INTO security_events(event,username,detail) VALUES(?1,?2,?3)", params![event, username, detail]);
}

/// Stores a new recovery key and returns it in readable form.
pub fn rotate_key(conn: &Connection) -> Result<String, String> {
    let key = generate_key();
    let hash = auth::hash_password(&normalize_key(&key))?;
    conn.execute("UPDATE owner_recovery SET key_hash=?1, updated_at=CURRENT_TIMESTAMP WHERE id=1", [hash]).map_err(|e| e.to_string())?;
    Ok(key)
}

pub fn validate_questions(questions: &[String], answers: &[String]) -> Result<(), String> {
    if questions.len() != 3 || answers.len() != 3 {
        return Err("Set exactly three security questions with answers".into());
    }
    for q in questions {
        let q = q.trim();
        if q.chars().count() < 5 || q.chars().count() > 140 { return Err("Each security question must be 5 to 140 characters".into()); }
    }
    let norm: Vec<String> = questions.iter().map(|q| normalize_answer(q)).collect();
    if norm[0] == norm[1] || norm[0] == norm[2] || norm[1] == norm[2] {
        return Err("Choose three different security questions".into());
    }
    for a in answers {
        let n = normalize_answer(a);
        if n.chars().count() < 2 || n.chars().count() > 120 { return Err("Each answer must be 2 to 120 characters".into()); }
    }
    Ok(())
}

pub fn save_questions(conn: &Connection, questions: &[String], answers: &[String]) -> Result<(), String> {
    validate_questions(questions, answers)?;
    let h: Vec<String> = answers.iter().map(|a| auth::hash_password(&normalize_answer(a))).collect::<Result<_, _>>()?;
    conn.execute(
        "UPDATE owner_recovery SET q1=?1,q2=?2,q3=?3,a1_hash=?4,a2_hash=?5,a3_hash=?6,updated_at=CURRENT_TIMESTAMP WHERE id=1",
        params![questions[0].trim(), questions[1].trim(), questions[2].trim(), h[0], h[1], h[2]],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

struct Stored { key_hash: String, questions: [String; 3], answer_hashes: [String; 3] }

fn load(conn: &Connection) -> Result<Stored, String> {
    conn.query_row("SELECT key_hash,q1,q2,q3,a1_hash,a2_hash,a3_hash FROM owner_recovery WHERE id=1", [], |r| {
        Ok(Stored { key_hash: r.get(0)?, questions: [r.get(1)?, r.get(2)?, r.get(3)?], answer_hashes: [r.get(4)?, r.get(5)?, r.get(6)?] })
    })
    .optional()
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "Account recovery has not been set up".to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method { Key, Questions }

/// Returns true when the proof matches. Never says which part was wrong.
fn verify(stored: &Stored, method: &Method, key: Option<&str>, answers: Option<&[String]>) -> Result<bool, String> {
    match method {
        Method::Key => {
            if stored.key_hash.is_empty() { return Err("No recovery key has been set up. Use the security questions instead.".into()); }
            Ok(key.map(|k| auth::verify_password(&normalize_key(k), &stored.key_hash)).unwrap_or(false))
        }
        Method::Questions => {
            if stored.answer_hashes.iter().any(|h| h.is_empty()) { return Err("Security questions have not been set up. Use the recovery key instead.".into()); }
            let answers = answers.unwrap_or(&[]);
            if answers.len() != 3 { return Ok(false); }
            Ok(answers.iter().zip(stored.answer_hashes.iter()).all(|(a, h)| auth::verify_password(&normalize_answer(a), h)))
        }
    }
}

// ------------------------------------------------------------------ public commands (sign-in screen)

#[derive(Serialize)]
pub struct RecoveryOptions { key_set: bool, questions: Vec<String> }

#[tauri::command]
pub fn recovery_options(state: tauri::State<'_, AppState>) -> Result<RecoveryOptions, String> {
    let conn = state.db()?;
    let s = load(&conn)?;
    let questions_set = s.answer_hashes.iter().all(|h| !h.is_empty());
    Ok(RecoveryOptions { key_set: !s.key_hash.is_empty(), questions: if questions_set { s.questions.to_vec() } else { vec![] } })
}

#[derive(Deserialize)]
pub struct RecoverInput {
    method: Method,
    recovery_key: Option<String>,
    answers: Option<Vec<String>>,
    /// "reset" an existing account's password, or "create_owner".
    action: String,
    username: String,
    full_name: Option<String>,
    new_password: String,
}

#[derive(Serialize)]
pub struct RecoverResult { session: SessionInfo, new_recovery_key: Option<String> }

fn check_lockout(state: &tauri::State<'_, AppState>) -> Result<(), String> {
    let f = state.recovery_failures.lock().map_err(|_| "Lock poisoned".to_string())?;
    if f.0 >= MAX_FAILED_RECOVERY && f.1.elapsed() < RECOVERY_LOCKOUT {
        let mins = ((RECOVERY_LOCKOUT - f.1.elapsed()).as_secs() / 60).max(1);
        return Err(format!("Too many failed recovery attempts. Try again in {mins} minute{}.", if mins == 1 { "" } else { "s" }));
    }
    Ok(())
}

fn record_failure(state: &tauri::State<'_, AppState>) {
    if let Ok(mut f) = state.recovery_failures.lock() {
        if f.1.elapsed() >= RECOVERY_LOCKOUT { *f = (0, Instant::now()); }
        f.0 += 1;
        f.1 = Instant::now();
    }
}

#[tauri::command]
pub fn recover_account(state: tauri::State<'_, AppState>, input: RecoverInput) -> Result<RecoverResult, String> {
    check_lockout(&state)?;
    auth::validate_password(&input.new_password)?;
    let creating = match input.action.as_str() { "reset" => false, "create_owner" => true, _ => return Err("Unknown recovery action".into()) };
    let username = if creating { auth::validate_username(&input.username)? } else { input.username.trim().to_string() };
    let full_name = if creating { auth::validate_name(input.full_name.as_deref().unwrap_or(""))? } else { String::new() };

    let stored = { let conn = state.db()?; load(&conn)? };
    // Argon2 checks are slow on purpose; they run without holding the database lock.
    let ok = verify(&stored, &input.method, input.recovery_key.as_deref(), input.answers.as_deref())?;
    if !ok {
        record_failure(&state);
        let conn = state.db()?;
        log_event(&conn, "recovery_failed", &username, match input.method { Method::Key => "wrong recovery key", Method::Questions => "wrong security answers" });
        return Err(match input.method {
            Method::Key => "That recovery key is not correct. Check each character and try again.".into(),
            Method::Questions => "One or more answers are not correct. Answers are not case-sensitive.".into(),
        });
    }
    if let Ok(mut f) = state.recovery_failures.lock() { *f = (0, Instant::now()); }

    let new_hash = auth::hash_password(&input.new_password)?;
    let (user_id, uname, fname, role, new_key) = {
        let mut conn = state.db()?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let (id, uname, fname, role) = if creating {
            tx.execute("INSERT INTO users(username,full_name,password_hash,role,active,last_login_at) VALUES(?1,?2,?3,'owner',1,?4)", params![username, full_name, new_hash, db::now_local()])
                .map_err(|e| if e.to_string().contains("UNIQUE") { "That username is already taken. Choose another, or use Forgot password for that account.".to_string() } else { e.to_string() })?;
            (tx.last_insert_rowid(), username.clone(), full_name.clone(), Role::Owner)
        } else {
            let row: Option<(i64, String, String, String)> = tx
                .query_row("SELECT id,username,full_name,role FROM users WHERE username=?1 COLLATE NOCASE", [username.as_str()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                .optional()
                .map_err(|e| e.to_string())?;
            let (id, uname, fname, role) = row.ok_or("No account has that username. Check the spelling, or create a new owner account instead.")?;
            // Recovery proves owner authority, so a deactivated account is switched back on.
            tx.execute("UPDATE users SET password_hash=?1,active=1,last_login_at=?2,updated_at=CURRENT_TIMESTAMP WHERE id=?3", params![new_hash, db::now_local(), id]).map_err(|e| e.to_string())?;
            (id, uname, fname, Role::parse(&role)?)
        };
        let new_key = match input.method { Method::Key => Some(rotate_key(&tx)?), Method::Questions => None };
        let via = match input.method { Method::Key => "recovery key", Method::Questions => "security questions" };
        log_event(&tx, if creating { "owner_created_by_recovery" } else { "password_reset_by_recovery" }, &uname, via);
        tx.commit().map_err(|e| e.to_string())?;
        (id, uname, fname, role, new_key)
    };
    auth::revoke_user_sessions(&state, user_id, None);
    state.login_failures.lock().map_err(|_| "Lock poisoned".to_string())?.remove(&uname.to_lowercase());
    let session = auth::start_session(&state, user_id, uname, fname, role)?;
    Ok(RecoverResult { session, new_recovery_key: new_key })
}

// ------------------------------------------------------------------ owner commands (Settings -> Security)

#[derive(Serialize)]
pub struct SecurityEvent { event: String, username: String, detail: String, created_at: String }

#[derive(Serialize)]
pub struct SecurityStatus { key_set: bool, questions: Vec<String>, updated_at: String, events: Vec<SecurityEvent> }

#[tauri::command]
pub fn security_status(state: tauri::State<'_, AppState>, session_token: String) -> Result<SecurityStatus, String> {
    auth::require(&state, &session_token, Role::Owner)?;
    let conn = state.db()?;
    let s = load(&conn)?;
    let updated_at: String = conn.query_row("SELECT datetime(updated_at,'localtime') FROM owner_recovery WHERE id=1", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT event,username,detail,datetime(created_at,'localtime') FROM security_events ORDER BY id DESC LIMIT 20").map_err(|e| e.to_string())?;
    let events = stmt.query_map([], |r| Ok(SecurityEvent { event: r.get(0)?, username: r.get(1)?, detail: r.get(2)?, created_at: r.get(3)? })).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    let questions_set = s.answer_hashes.iter().all(|h| !h.is_empty());
    Ok(SecurityStatus { key_set: !s.key_hash.is_empty(), questions: if questions_set { s.questions.to_vec() } else { vec![] }, updated_at, events })
}

fn confirm_password(state: &tauri::State<'_, AppState>, user_id: i64, password: &str) -> Result<(), String> {
    let hash: String = state.db()?.query_row("SELECT password_hash FROM users WHERE id=?1", [user_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    if auth::verify_password(password, &hash) { Ok(()) } else { Err("Your current password is incorrect".into()) }
}

#[tauri::command]
pub fn regenerate_recovery_key(state: tauri::State<'_, AppState>, session_token: String, current_password: String) -> Result<String, String> {
    let me = auth::require(&state, &session_token, Role::Owner)?;
    confirm_password(&state, me.user_id, &current_password)?;
    let conn = state.db()?;
    let key = rotate_key(&conn)?;
    log_event(&conn, "recovery_key_changed", &me.username, "new key generated");
    Ok(key)
}

#[tauri::command]
pub fn save_security_questions(state: tauri::State<'_, AppState>, session_token: String, current_password: String, questions: Vec<String>, answers: Vec<String>) -> Result<(), String> {
    let me = auth::require(&state, &session_token, Role::Owner)?;
    validate_questions(&questions, &answers)?;
    confirm_password(&state, me.user_id, &current_password)?;
    let conn = state.db()?;
    save_questions(&conn, &questions, &answers)?;
    log_event(&conn, "security_questions_changed", &me.username, "");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_format_and_normalisation() {
        let k = generate_key();
        assert_eq!(k.len(), 19);
        assert_eq!(k.matches('-').count(), 3);
        assert_eq!(normalize_key(&k.to_lowercase().replace('-', " ")), k.replace('-', ""));
        assert_ne!(generate_key(), generate_key());
    }

    #[test]
    fn key_and_questions_verify() {
        let conn = db::test_conn();
        let key = rotate_key(&conn).unwrap();
        let qs = vec!["Name of my first school?".to_string(), "Town where I was born?".to_string(), "My first phone brand?".to_string()];
        let ans = vec!["St. Mary's ".to_string(), "Ikot  Ekpene".to_string(), "Nokia".to_string()];
        save_questions(&conn, &qs, &ans).unwrap();
        let s = load(&conn).unwrap();
        assert!(verify(&s, &Method::Key, Some(&key.to_lowercase()), None).unwrap());
        assert!(!verify(&s, &Method::Key, Some("AAAA-BBBB-CCCC-DDDD"), None).unwrap());
        let good = vec!["st. mary's".to_string(), "ikot ekpene".to_string(), "NOKIA".to_string()];
        assert!(verify(&s, &Method::Questions, None, Some(&good)).unwrap());
        let bad = vec!["st. mary's".to_string(), "uyo".to_string(), "nokia".to_string()];
        assert!(!verify(&s, &Method::Questions, None, Some(&bad)).unwrap());
    }

    #[test]
    fn question_rules() {
        let a = vec!["xx".to_string(), "yy".to_string(), "zz".to_string()];
        assert!(validate_questions(&["Same question?".into(), "Same question?".into(), "Other one?".into()], &a).is_err());
        assert!(validate_questions(&["Question one?".into(), "Question two?".into()], &a[..2]).is_err());
        assert!(validate_questions(&["Question one?".into(), "Question two?".into(), "Question three?".into()], &["x".into(), "yy".into(), "zz".into()]).is_err());
    }
}
