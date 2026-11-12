//! Android updates. The desktop apps use Tauri's signed updater; Android has
//! no updater, so the app looks up the newest release itself and hands the APK
//! address to the phone's own download manager (see tools/android-native.mjs).
//! Nobody is sent to a web page. The phone checks the APK is signed with the
//! same key as the installed app before it will install it.

use serde::{Deserialize, Serialize};
use std::time::Duration;

const LATEST: &str = "https://api.github.com/repos/treasure155/farmbuddy/releases/latest";

#[derive(Serialize, Debug, PartialEq)]
pub struct ApkUpdate {
    pub version: String,
    pub notes: String,
    pub url: String,
    pub file_name: String,
    pub size: u64,
}

#[derive(Deserialize)]
struct Release { tag_name: String, #[serde(default)] body: Option<String>, #[serde(default)] draft: bool, #[serde(default)] prerelease: bool, #[serde(default)] assets: Vec<Asset> }
#[derive(Deserialize)]
struct Asset { name: String, browser_download_url: String, #[serde(default)] size: u64 }

/// "v3.10.2" → [3, 10, 2]
fn parts(v: &str) -> Vec<u64> {
    v.trim().trim_start_matches(['v', 'V']).split(['.', '-', '+']).take(3).map(|p| p.parse().unwrap_or(0)).collect()
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    let (mut a, mut b) = (parts(candidate), parts(current));
    a.resize(3, 0);
    b.resize(3, 0);
    a > b
}

fn pick(json: &str, current: &str) -> Result<Option<ApkUpdate>, String> {
    let r: Release = serde_json::from_str(json).map_err(|e| format!("Unexpected reply from the update server: {e}"))?;
    if r.draft || r.prerelease || !is_newer(&r.tag_name, current) { return Ok(None); }
    // The APK is added a few minutes after the desktop files; until then there is nothing to offer.
    let Some(a) = r.assets.into_iter().find(|a| a.name.to_lowercase().ends_with(".apk")) else { return Ok(None) };
    if !a.browser_download_url.starts_with("https://github.com/treasure155/farmbuddy/releases/download/") {
        return Err("The update address was not recognised".into());
    }
    Ok(Some(ApkUpdate {
        version: r.tag_name.trim_start_matches(['v', 'V']).to_string(),
        notes: r.body.unwrap_or_default().chars().take(1500).collect(),
        url: a.browser_download_url,
        file_name: a.name,
        size: a.size,
    }))
}

/// Returns the newer Android version, or nothing when this one is the newest.
#[tauri::command]
pub async fn check_apk_update(app: tauri::AppHandle) -> Result<Option<ApkUpdate>, String> {
    let current = app.package_info().version.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(20)).build();
        let body = agent
            .get(LATEST)
            .set("User-Agent", "FarmBuddy")
            .set("Accept", "application/vnd.github+json")
            .call()
            .map_err(|e| format!("Could not reach the update server ({e})"))?
            .into_string()
            .map_err(|e| e.to_string())?;
        pick(&body, &current)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number() {
        assert!(is_newer("v3.4.0", "3.3.0"));
        assert!(is_newer("3.10.0", "3.9.9"));
        assert!(!is_newer("v3.3.0", "3.3.0"));
        assert!(!is_newer("3.2.9", "3.3.0"));
    }

    #[test]
    fn picks_the_apk_only_when_newer() {
        let json = r#"{"tag_name":"v3.4.0","body":"Fixes","draft":false,"prerelease":false,"assets":[
            {"name":"FarmBuddy_3.4.0_x64-setup.exe","browser_download_url":"https://github.com/treasure155/farmbuddy/releases/download/v3.4.0/a.exe","size":1},
            {"name":"FarmBuddy_3.4.0_android.apk","browser_download_url":"https://github.com/treasure155/farmbuddy/releases/download/v3.4.0/FarmBuddy_3.4.0_android.apk","size":31000000}]}"#;
        let u = pick(json, "3.3.0").unwrap().unwrap();
        assert_eq!(u.version, "3.4.0");
        assert_eq!(u.file_name, "FarmBuddy_3.4.0_android.apk");
        assert_eq!(pick(json, "3.4.0").unwrap(), None);
        let no_apk = r#"{"tag_name":"v3.4.0","assets":[]}"#;
        assert_eq!(pick(no_apk, "3.3.0").unwrap(), None);
        let other_host = r#"{"tag_name":"v3.4.0","assets":[{"name":"x.apk","browser_download_url":"https://evil.example/x.apk"}]}"#;
        assert!(pick(other_host, "3.3.0").is_err());
    }
}
