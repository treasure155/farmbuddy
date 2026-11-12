//! Business settings, branding and theme. Every key is validated on save.

use crate::{auth::{self, Role}, db, sales::valid_email, AppState};
use serde::Serialize;
use std::collections::HashMap;

const SECRET_KEYS: [&str; 1] = ["smtp_password"];
const BOOL_KEYS: [&str; 13] = ["content_require_signature", "tax_enabled", "allow_negative_stock", "allow_price_override", "track_collection", "email_bcc_self", "require_shift", "feature_flocks", "feature_recipes",
    "module_livestock", "module_crops", "module_processing", "module_sales"];
const MAX_LOGO_BYTES: usize = 700 * 1024;

pub fn is_hex_color(v: &str) -> bool {
    v.len() == 7 && v.starts_with('#') && v[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn max_len(v: &str, n: usize, label: &str) -> Result<(), String> {
    if v.chars().count() > n { Err(format!("{label} must be {n} characters or fewer")) } else { Ok(()) }
}

/// Returns the normalised value to store, or an error explaining what is wrong.
fn validate(key: &str, value: &str) -> Result<String, String> {
    let v = value.trim();
    if BOOL_KEYS.contains(&key) {
        return if v == "1" || v == "0" { Ok(v.into()) } else { Err(format!("{key} must be on or off")) };
    }
    match key {
        "business_name" => { if v.is_empty() { return Err("Business name is required".into()); } max_len(v, 120, "Business name")?; }
        "business_address" => max_len(v, 300, "Address")?,
        "business_phone" => max_len(v, 60, "Phone")?,
        "business_email" | "smtp_from_email" | "smtp_reply_to" => { if !v.is_empty() && !valid_email(v) { return Err(format!("{v} is not a valid email address")); } }
        "business_logo" => {
            if !v.is_empty() {
                let ok_prefix = ["data:image/png;base64,", "data:image/jpeg;base64,", "data:image/webp;base64,"].iter().any(|p| v.starts_with(p));
                if !ok_prefix { return Err("Logo must be a PNG, JPG or WebP image".into()); }
                if v.len() > MAX_LOGO_BYTES { return Err("Logo is too large. Use an image under 500 KB.".into()); }
            }
        }
        "currency_symbol" => { if v.is_empty() { return Err("Currency symbol is required".into()); } max_len(v, 4, "Currency symbol")?; }
        "receipt_width_mm" => { if !["58", "80", "a4"].contains(&v) { return Err("Receipt size must be 58 mm, 80 mm or A4".into()); } }
        "receipt_title" => max_len(v, 60, "Receipt title")?,
        "receipt_footer" => max_len(v, 300, "Receipt footer")?,
        "receipt_prefix" => {
            if v.is_empty() || v.len() > 8 || !v.chars().all(|c| c.is_ascii_alphanumeric()) { return Err("Receipt prefix must be 1 to 8 letters or numbers".into()); }
            return Ok(v.to_uppercase());
        }
        "tax_label" => max_len(v, 20, "Tax label")?,
        "tax_rate" => {
            let rate: f64 = v.parse().map_err(|_| "Tax rate must be a number".to_string())?;
            if !(0.0..=100.0).contains(&rate) { return Err("Tax rate must be between 0 and 100".into()); }
        }
        "label_item" | "label_items" | "label_customer" => { if v.is_empty() { return Err("Labels cannot be empty".into()); } max_len(v, 30, "Label")?; }
        "theme_primary" | "theme_secondary" => { if !is_hex_color(v) { return Err("Colours must be hex values like #E8731A".into()); } return Ok(v.to_uppercase()); }
        "language" => { if !["en", "ib"].contains(&v) { return Err("Language must be English or Ibibio".into()); } }
        "theme_mode" => { if !["light", "dark"].contains(&v) { return Err("Theme must be light or dark".into()); } }
        "smtp_provider" => { if !["gmail", "outlook", "yahoo", "zoho", "custom"].contains(&v) { return Err("Unknown email provider".into()); } }
        "smtp_host" => {
            max_len(v, 253, "SMTP server")?;
            if !v.is_empty() && !v.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') { return Err("SMTP server must be a host name like smtp.gmail.com".into()); }
        }
        "smtp_port" => { let p: u32 = v.parse().map_err(|_| "SMTP port must be a number".to_string())?; if p == 0 || p > 65535 { return Err("SMTP port must be between 1 and 65535".into()); } }
        "smtp_security" => { if !["starttls", "ssl", "none"].contains(&v) { return Err("Connection security must be STARTTLS, SSL/TLS or none".into()); } }
        "smtp_username" => max_len(v, 254, "SMTP username")?,
        "smtp_password" => { if value.len() > 256 { return Err("SMTP password is too long".into()); } return Ok(value.to_string()); }
        "smtp_from_name" => max_len(v, 80, "Sender name")?,
        "expiry_warn_days" => { let d: u32 = v.parse().map_err(|_| "Expiry warning must be a number of days".to_string())?; if !(1..=365).contains(&d) { return Err("Expiry warning must be between 1 and 365 days".into()); } }
        "feed_low_days" => { let d: u32 = v.parse().map_err(|_| "Feed warning must be a number of days".to_string())?; if !(1..=90).contains(&d) { return Err("Feed warning must be between 1 and 90 days".into()); } }
        "whatsapp_country_code" => { if v.is_empty() || v.len() > 4 || !v.chars().all(|c| c.is_ascii_digit()) { return Err("Country code must be 1 to 4 digits, e.g. 234".into()); } }
        _ => return Err(format!("Unknown setting: {key}")),
    }
    Ok(v.to_string())
}

#[tauri::command]
pub fn get_settings(state: tauri::State<'_, AppState>, session_token: String) -> Result<HashMap<String, String>, String> {
    let me = auth::require(&state, &session_token, Role::Cashier)?;
    let conn = state.db()?;
    let mut all = db::all_settings(&conn)?;
    let has_password = !all.get("smtp_password").map(|s| s.is_empty()).unwrap_or(true);
    for k in SECRET_KEYS { all.remove(k); }
    for k in ["license_key", "clock_last_seen", "trial_started"] { all.remove(k); }
    if me.role < Role::Owner {
        all.retain(|k, _| !k.starts_with("smtp_"));
    } else {
        all.insert("smtp_password_set".into(), if has_password { "1".into() } else { "0".into() });
    }
    Ok(all)
}

#[tauri::command]
pub fn save_settings(state: tauri::State<'_, AppState>, session_token: String, values: HashMap<String, String>) -> Result<(), String> {
    let me = auth::require(&state, &session_token, Role::Owner)?;
    let mut clean = Vec::with_capacity(values.len());
    for (k, v) in &values {
        // An empty password field means "keep the saved one".
        if k == "smtp_password" && v.is_empty() { continue; }
        if k == "smtp_password_set" || k == "business_type" { continue; }
        if ["license_key", "trial_started", "clock_last_seen", "auto_backup", "backup_dir", "backup_keep", "last_auto_backup", "last_backup_error"].contains(&k.as_str()) { continue; }
        clean.push((k.clone(), validate(k, v)?));
    }
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut keys: Vec<&str> = clean.iter().map(|(k, _)| k.as_str()).filter(|k| *k != "smtp_password").collect();
    keys.sort();
    crate::audit::log(&tx, &me.display_name(), "settings_changed", "settings", None, &keys.join(", "));
    for (k, v) in clean { db::set_setting(&tx, &k, &v)?; }
    tx.commit().map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct Branding {
    business_name: String,
    business_logo: String,
    theme_primary: String,
    theme_secondary: String,
    theme_mode: String,
}

/// Public: the sign-in screen uses the business colours and logo.
#[tauri::command]
pub fn branding(state: tauri::State<'_, AppState>) -> Result<Branding, String> {
    let conn = state.db()?;
    Ok(Branding {
        business_name: db::setting(&conn, "business_name")?,
        business_logo: db::setting(&conn, "business_logo")?,
        theme_primary: db::setting(&conn, "theme_primary")?,
        theme_secondary: db::setting(&conn, "theme_secondary")?,
        theme_mode: db::setting(&conn, "theme_mode")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validation_rules() {
        assert_eq!(validate("theme_primary", "#e8731a").unwrap(), "#E8731A");
        assert!(validate("theme_primary", "orange").is_err());
        assert!(validate("receipt_width_mm", "a4").is_ok());
        assert!(validate("smtp_host", "smtp.gmail.com; rm").is_err());
        assert!(validate("business_logo", "data:image/svg+xml;base64,AAA").is_err());
        assert!(validate("tax_rate", "120").is_err());
        assert_eq!(validate("receipt_prefix", "spm").unwrap(), "SPM");
        assert!(validate("is_admin", "1").is_err());
    }
}
