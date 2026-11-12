//! Install usage notice for TechAlpha.
//!
//! FarmBuddy works offline. When the device has internet, it sends
//! TechAlpha one small notice the first time ("install") and then at most one
//! a day ("heartbeat"), so TechAlpha knows how many businesses use the app and
//! which versions are in use. If there is no internet, nothing happens and the
//! notice is tried again later (every 15 minutes while the app is open).
//!
//! What is sent: the Install ID (the same one shown in Settings → Licence), app
//! version, device type, business name and type from Settings, whether the app
//! is licensed, and a few totals (items, receipts, customers, staff accounts).
//! Never sent: sales details, prices, customer names or phone numbers,
//! passwords or email settings. The notice is described in Help → How to.

use crate::{db, AppState};
use rusqlite::Connection;
use std::time::Duration;
use tauri::Manager;

const ENDPOINT: &str = "https://api.techalphahub.online/api/products/telemetry";
const PRODUCT: &str = "farmbuddy";
const FIRST_TRY_AFTER: Duration = Duration::from_secs(45);
const RETRY_EVERY: Duration = Duration::from_secs(15 * 60);

/// Starts the background thread. Called once from `setup`.
pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(FIRST_TRY_AFTER);
        loop {
            let _ = send_if_due(&app);
            std::thread::sleep(RETRY_EVERY);
        }
    });
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap_or(0)
}

fn send_if_due(app: &tauri::AppHandle) -> Result<(), String> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let state = app.state::<AppState>();

    // Read everything while holding the database lock, then release it before
    // going to the network, so the till is never kept waiting.
    let (payload, event) = {
        let conn = state.db()?;
        let install_sent = db::setting(&conn, "telemetry_install_sent")? == "1";
        if install_sent && db::setting(&conn, "telemetry_last_day")? == today {
            return Ok(());
        }
        // Not set up yet (no owner account): wait until the business exists.
        if count(&conn, "SELECT COUNT(*) FROM users") == 0 {
            return Ok(());
        }
        let (install_id, licensed) = state
            .license
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|l| (l.install_id.clone(), l.licensed)))
            .unwrap_or_default();
        if install_id.is_empty() {
            return Ok(());
        }
        let event = if install_sent { "heartbeat" } else { "install" };
        let payload = serde_json::json!({
            "product": PRODUCT,
            "install_id": install_id,
            "event": event,
            "app_version": app.package_info().version.to_string(),
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "business_name": db::setting(&conn, "business_name").unwrap_or_default(),
            "business_type": db::setting(&conn, "business_type").unwrap_or_default(),
            "licensed": licensed,
            "usage": {
                "items": count(&conn, "SELECT COUNT(*) FROM products"),
                "receipts": count(&conn, "SELECT COUNT(*) FROM receipts"),
                "receipts_30_days": count(&conn, "SELECT COUNT(*) FROM receipts WHERE created_at >= date('now', '-30 day')"),
                "customers": count(&conn, "SELECT COUNT(*) FROM customers"),
                "staff": count(&conn, "SELECT COUNT(*) FROM users"),
            },
        });
        (payload, event)
    };

    post(&payload)?;

    let conn = state.db()?;
    if event == "install" {
        db::set_setting(&conn, "telemetry_install_sent", "1")?;
    }
    db::set_setting(&conn, "telemetry_last_day", &today)?;
    Ok(())
}

/// One small HTTPS request with a short timeout. Any failure (offline, slow
/// network, server down) is silently retried later.
fn post(payload: &serde_json::Value) -> Result<(), String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout(Duration::from_secs(20))
        .build();
    agent
        .post(ENDPOINT)
        .set("Content-Type", "application/json")
        .send_string(&payload.to_string())
        .map(|_| ())
        .map_err(|e| e.to_string())
}
