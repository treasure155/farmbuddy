//! Offline licensing.
//!
//! Each computer has an Install ID derived from its operating-system machine
//! ID. A licence key is that Install ID plus issue/expiry dates, signed with
//! the vendor's Ed25519 private key (kept only by the admin, see
//! tools/license-admin.mjs). The app holds just the public key, so it can
//! check keys but never create them. A key for one computer does not work on
//! another. New installs get a 14-day trial.

use crate::{db, AppState};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};
use tauri::Manager;

pub const TRIAL_DAYS: i64 = 14;
const PUBLIC_KEY_B64: &str = include_str!("../license_public_key.txt");
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// Day numbers in keys count from 1 January 2024.
const EPOCH_2024: i64 = 1_704_067_200;
const DAY: i64 = 86_400;

// ------------------------------------------------------------------ base32 (Crockford)

pub fn b32_encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    let (mut buf, mut bits) = (0u32, 0u32);
    for &b in bytes {
        buf = (buf << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buf >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 { out.push(ALPHABET[((buf << (5 - bits)) & 31) as usize] as char); }
    out
}

pub fn b32_decode(s: &str) -> Option<Vec<u8>> {
    let (mut buf, mut bits) = (0u32, 0u32);
    let mut out = Vec::new();
    for c in s.chars() {
        let c = match c.to_ascii_uppercase() { 'O' => '0', 'I' | 'L' => '1', 'U' => return None, c => c };
        let v = ALPHABET.iter().position(|&a| a as char == c)? as u32;
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

fn group(s: &str, n: usize) -> String {
    s.as_bytes().chunks(n).map(|c| std::str::from_utf8(c).unwrap_or("")).collect::<Vec<_>>().join("-")
}

// ------------------------------------------------------------------ install id

pub fn install_hash(machine: &str) -> [u8; 10] {
    let digest = Sha256::digest(format!("farmbuddy-install-v1:{machine}").as_bytes());
    let mut out = [0u8; 10];
    out.copy_from_slice(&digest[..10]);
    out
}

pub fn format_install_id(hash: &[u8; 10]) -> String {
    group(&b32_encode(hash), 4)
}

/// The machine identity. Falls back to an ID stored in the app config folder
/// when the operating system does not provide one.
/// On Android there is no machine ID available to apps, so a random ID is
/// created on first run and kept in the app's private config folder
/// (it changes only if the app is uninstalled).
fn machine_identity(app: &tauri::AppHandle) -> String {
    #[cfg(desktop)]
    if let Ok(id) = machine_uid::get() {
        if !id.trim().is_empty() { return id.trim().to_string(); }
    }
    let path = config_dir(app).join(".farmbuddy-machine");
    if let Ok(id) = fs::read_to_string(&path) { if !id.trim().is_empty() { return id.trim().to_string(); } }
    let id = uuid::Uuid::new_v4().to_string();
    let _ = fs::write(&path, &id);
    id
}

fn config_dir(app: &tauri::AppHandle) -> PathBuf {
    let dir = app.path().app_config_dir().unwrap_or_else(|_| std::env::temp_dir());
    let _ = fs::create_dir_all(&dir);
    dir
}

// ------------------------------------------------------------------ keys

#[derive(Debug, Clone, PartialEq)]
pub struct KeyInfo { pub install: [u8; 10], pub issued_day: u16, pub expiry_day: u16, pub edition: u8 }

fn public_key() -> Option<VerifyingKey> {
    use base64::Engine;
    let raw = PUBLIC_KEY_B64.trim();
    if raw.is_empty() || raw == "UNSET" { return None; }
    let bytes = base64::engine::general_purpose::STANDARD.decode(raw).ok()
        .or_else(|| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(raw).ok())?;
    VerifyingKey::from_bytes(&bytes.try_into().ok()?).ok()
}

pub fn parse_key(key: &str, pk: &VerifyingKey) -> Result<KeyInfo, String> {
    let cleaned: String = key.trim().trim_start_matches("FMBD").trim_start_matches("fmbd").chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let bytes = b32_decode(&cleaned).ok_or("That licence key contains characters that are not allowed")?;
    if bytes.len() < 80 { return Err("That licence key is incomplete. Copy the whole key and try again.".into()); }
    let (payload, sig) = (&bytes[..16], &bytes[16..80]);
    let sig = Signature::from_bytes(sig.try_into().map_err(|_| "Invalid key")?);
    pk.verify(payload, &sig).map_err(|_| "That licence key is not valid. Check that it was copied exactly.".to_string())?;
    if payload[0] != 1 { return Err("This licence key is for a different version of FarmBuddy".into()); }
    let mut install = [0u8; 10];
    install.copy_from_slice(&payload[1..11]);
    Ok(KeyInfo { install, issued_day: u16::from_be_bytes([payload[11], payload[12]]), expiry_day: u16::from_be_bytes([payload[13], payload[14]]), edition: payload[15] })
}

fn now_unix() -> i64 { SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0) }
fn day_to_date(day: u16) -> String {
    chrono::DateTime::from_timestamp(EPOCH_2024 + day as i64 * DAY, 0).map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default()
}

// ------------------------------------------------------------------ state

#[derive(Debug, Clone, Serialize)]
pub struct LicenseStatus {
    /// "licensed" | "trial" | "expired"
    pub state: String,
    pub install_id: String,
    pub trial_days_left: i64,
    pub trial_days: i64,
    pub expires_on: String,
    pub licensed_since: String,
    pub can_activate: bool,
    pub message: String,
}

pub struct LicenseState {
    pub install: [u8; 10],
    pub install_id: String,
    /// Unix seconds when use stops being allowed. i64::MAX = no limit.
    pub valid_until: i64,
    pub licensed: bool,
    pub expires_on: String,
    pub licensed_since: String,
    pub trial_end: i64,
}

impl LicenseState {
    pub fn status(&self) -> LicenseStatus {
        let now = now_unix();
        let active = now < self.valid_until;
        let (state, message) = if self.licensed && active {
            ("licensed", if self.expires_on.is_empty() { "Licensed for this computer".to_string() } else { format!("Licensed until {}", self.expires_on) })
        } else if self.licensed {
            ("expired", format!("The licence for this computer ended on {}. Enter a new key to continue.", self.expires_on))
        } else if active {
            ("trial", "Every feature is open during the free trial. Activate before it ends to keep selling without a break.".to_string())
        } else {
            ("expired", "The free trial has ended. Enter a licence key to continue using FarmBuddy.".to_string())
        };
        LicenseStatus {
            state: state.into(), install_id: self.install_id.clone(),
            trial_days_left: if self.licensed { 0 } else { ((self.trial_end - now) as f64 / DAY as f64).ceil().max(0.0) as i64 },
            trial_days: TRIAL_DAYS, expires_on: self.expires_on.clone(), licensed_since: self.licensed_since.clone(),
            can_activate: public_key().is_some(), message,
        }
    }
    pub fn active(&self) -> bool { now_unix() < self.valid_until }
}

/// Earliest recorded first-run time, kept both in the database and in a
/// marker file, so deleting one of them does not restart the trial.
fn trial_start(app: &tauri::AppHandle, conn: &rusqlite::Connection) -> i64 {
    let marker = config_dir(app).join(".farmbuddy-install");
    let from_file = fs::read_to_string(&marker).ok().and_then(|s| s.trim().parse::<i64>().ok());
    let from_db = db::setting(conn, "trial_started").ok().and_then(|s| s.parse::<i64>().ok());
    let start = [from_file, from_db].into_iter().flatten().min().unwrap_or_else(now_unix);
    let _ = fs::write(&marker, start.to_string());
    let _ = db::set_setting(conn, "trial_started", &start.to_string());
    start
}

/// Guards against setting the computer clock back to stretch a trial.
fn effective_now(conn: &rusqlite::Connection) -> i64 {
    let now = now_unix();
    let last = db::setting(conn, "clock_last_seen").ok().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    let eff = now.max(last);
    let _ = db::set_setting(conn, "clock_last_seen", &eff.to_string());
    eff
}

pub fn compute(app: &tauri::AppHandle, conn: &rusqlite::Connection) -> LicenseState {
    let install = install_hash(&machine_identity(app));
    let install_id = format_install_id(&install);
    let start = trial_start(app, conn);
    let now = effective_now(conn);
    let skew = (now - now_unix()).max(0);
    let trial_end = start + TRIAL_DAYS * DAY;
    let mut st = LicenseState { install, install_id, valid_until: trial_end - skew, licensed: false, expires_on: String::new(), licensed_since: String::new(), trial_end: trial_end - skew };
    if let (Ok(key), Some(pk)) = (db::setting(conn, "license_key"), public_key()) {
        if let Ok(info) = parse_key(&key, &pk) {
            if info.install == install {
                st.licensed = true;
                st.licensed_since = day_to_date(info.issued_day);
                if info.expiry_day == 0 { st.valid_until = i64::MAX; } else {
                    st.expires_on = day_to_date(info.expiry_day);
                    st.valid_until = EPOCH_2024 + (info.expiry_day as i64 + 1) * DAY - skew;
                }
            }
        }
    }
    st
}

pub fn require_active(state: &tauri::State<'_, AppState>) -> Result<(), String> {
    let lic = state.license.lock().map_err(|_| "Licence lock poisoned".to_string())?;
    match lic.as_ref() {
        Some(l) if l.active() => Ok(()),
        Some(_) => Err("LICENCE_REQUIRED: Activate FarmBuddy to continue.".into()),
        None => Ok(()),
    }
}

// ------------------------------------------------------------------ commands (public)

#[tauri::command]
pub fn license_status(state: tauri::State<'_, AppState>) -> Result<LicenseStatus, String> {
    let lic = state.license.lock().map_err(|_| "Licence lock poisoned".to_string())?;
    lic.as_ref().map(|l| l.status()).ok_or_else(|| "Licence not loaded".into())
}

#[tauri::command]
pub fn activate_license(app: tauri::AppHandle, state: tauri::State<'_, AppState>, key: String) -> Result<LicenseStatus, String> {
    let pk = public_key().ok_or("This copy of FarmBuddy was built without the vendor's public key, so it cannot be activated. Ask your provider for an official build.")?;
    if key.len() > 400 { return Err("That licence key is too long".into()); }
    let info = parse_key(&key, &pk)?;
    let install = state.license.lock().map_err(|_| "Licence lock poisoned".to_string())?.as_ref().map(|l| l.install).ok_or("Licence not loaded")?;
    if info.install != install {
        return Err("This licence key was made for a different computer. Every computer needs its own key: send this computer's Install ID to your provider.".into());
    }
    if info.expiry_day != 0 && EPOCH_2024 + (info.expiry_day as i64 + 1) * DAY < now_unix() {
        return Err(format!("This licence key expired on {}. Ask your provider for a new one.", day_to_date(info.expiry_day)));
    }
    let conn = state.db()?;
    db::set_setting(&conn, "license_key", key.trim())?;
    let fresh = compute(&app, &conn);
    let status = fresh.status();
    *state.license.lock().map_err(|_| "Licence lock poisoned".to_string())? = Some(fresh);
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn make_key(sk: &SigningKey, install: [u8; 10], issued: u16, expiry: u16) -> String {
        let mut payload = vec![1u8];
        payload.extend_from_slice(&install);
        payload.extend_from_slice(&issued.to_be_bytes());
        payload.extend_from_slice(&expiry.to_be_bytes());
        payload.push(0);
        let sig = sk.sign(&payload);
        payload.extend_from_slice(&sig.to_bytes());
        format!("FMBD-{}", group(&b32_encode(&payload), 8))
    }

    #[test]
    fn base32_roundtrip() {
        let data: Vec<u8> = (0..80u8).collect();
        assert_eq!(b32_decode(&b32_encode(&data)).unwrap()[..80], data[..]);
        let id = format_install_id(&install_hash("abc"));
        assert_eq!(id.len(), 19);
        assert_eq!(b32_decode(&id.replace('-', "")).unwrap()[..10], install_hash("abc"));
    }

    #[test]
    fn keys_verify_only_for_their_computer_and_signer() {
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let pk = sk.verifying_key();
        let mine = install_hash("machine-a");
        let key = make_key(&sk, mine, 1000, 0);
        let info = parse_key(&key.to_lowercase(), &pk).unwrap();
        assert_eq!(info.install, mine);
        assert_ne!(info.install, install_hash("machine-b"));
        // Tampered key fails.
        let mut chars: Vec<char> = key.chars().collect();
        let n = chars.len() - 3;
        chars[n] = if chars[n] == 'A' { 'B' } else { 'A' };
        assert!(parse_key(&chars.into_iter().collect::<String>(), &pk).is_err());
        // Key from another signer fails.
        let other = SigningKey::from_bytes(&[9u8; 32]);
        assert!(parse_key(&make_key(&other, mine, 1000, 0), &pk).is_err());
    }
}

