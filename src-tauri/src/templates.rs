//! Base wording for a FarmBuddy installation: receipt title, numbering and
//! the units a farm uses. FarmBuddy has no shop, eatery or laundry business
//! types: the starter categories and items for each kind of farm come from the
//! farm templates in `farm_templates.rs` (no prices, no stock: the farmer
//! enters their own). Applying the base only ever adds; nothing is deleted.

use crate::{auth::{self, Role}, db, farm_templates, AppState};
use rusqlite::Connection;
use serde::Serialize;

pub struct Template {
    pub key: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
    pub units: &'static [&'static str],
    /// (setting key, value)
    pub settings: &'static [(&'static str, &'static str)],
}

pub const FARM_UNITS: &[&str] = &["piece", "bag", "kg", "tonne", "litre", "crate", "bird", "fish", "tuber", "bundle", "basin", "tin", "vial", "head"];

pub const TEMPLATES: &[Template] = &[Template {
    key: "farm",
    name: "Farm",
    summary: "Farm wording for receipts and stock. Starter categories and items come from the farm type chosen during setup.",
    units: FARM_UNITS,
    settings: &[
        ("label_item", "Item"), ("label_items", "Items"), ("label_customer", "Buyer"),
        ("receipt_title", "Farm sales receipt"), ("receipt_prefix", "FB"), ("receipt_footer", "Thank you for buying from our farm."),
    ],
}];

pub fn find(key: &str) -> Option<&'static Template> {
    TEMPLATES.iter().find(|t| t.key == key)
}

/// Sets the farm wording. When `load_items` is true, the starter items of the
/// farm template chosen at setup are added again (names only, no prices).
/// Returns the number of items added.
pub fn apply_template(conn: &Connection, key: &str, load_items: bool, user_name: &str) -> Result<usize, String> {
    let t = find(key).or_else(|| find("farm")).ok_or("Unknown business type")?;
    db::set_setting(conn, "business_type", t.key)?;
    for (k, v) in t.settings {
        // Wording the farmer already changed is kept.
        let cur = db::setting(conn, k)?;
        if cur.trim().is_empty() || is_shop_default(k, &cur) { db::set_setting(conn, k, v)?; }
    }
    let mut added = 0;
    if load_items {
        let farm_key = db::setting(conn, "farm_template")?;
        if farm_templates::find(&farm_key).is_some() {
            added = farm_templates::apply_tx(conn, &farm_key, user_name)?.items;
        }
    }
    Ok(added)
}

/// Wording left behind by the older shop setup, replaced by the farm wording.
fn is_shop_default(key: &str, value: &str) -> bool {
    matches!((key, value), ("receipt_title", "Sales receipt") | ("receipt_title", "Receipt") | ("receipt_prefix", "RCP")
        | ("receipt_footer", "Thank you for your patronage.") | ("label_customer", "Customer"))
}

// ------------------------------------------------------------------ commands

#[derive(Serialize)]
pub struct TemplateInfo {
    key: &'static str,
    name: &'static str,
    summary: &'static str,
    categories: Vec<&'static str>,
    units: Vec<&'static str>,
    sample_count: usize,
}

#[tauri::command]
pub fn list_templates() -> Vec<TemplateInfo> {
    TEMPLATES
        .iter()
        .map(|t| TemplateInfo { key: t.key, name: t.name, summary: t.summary, categories: vec![], units: t.units.to_vec(), sample_count: 0 })
        .collect()
}

#[tauri::command]
pub fn apply_business_template(state: tauri::State<'_, AppState>, session_token: String, key: String, load_samples: bool) -> Result<usize, String> {
    let me = auth::require(&state, &session_token, Role::Owner)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let added = apply_template(&tx, &key, load_samples, &me.display_name())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(added)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn farm_wording_applies_and_no_shop_types_exist() {
        let conn = db::test_conn();
        db::set_setting(&conn, "receipt_title", "Sales receipt").unwrap();
        apply_template(&conn, "farm", false, "Test").unwrap();
        assert_eq!(db::setting(&conn, "business_type").unwrap(), "farm");
        assert_eq!(db::setting(&conn, "receipt_title").unwrap(), "Farm sales receipt");
        for shop in ["supermarket", "eatery", "dry_cleaning", "general"] { assert!(find(shop).is_none(), "{shop} must not exist"); }
        // A farm template chosen at setup is reloaded by name only, with no prices.
        db::set_setting(&conn, "farm_template", "broiler").unwrap();
        let n = apply_template(&conn, "farm", true, "Test").unwrap();
        assert!(n > 0);
        let priced: i64 = conn.query_row("SELECT COUNT(*) FROM products WHERE selling_price_kobo<>0 OR cost_price_kobo<>0 OR stock_qty<>0", [], |r| r.get(0)).unwrap();
        assert_eq!(priced, 0);
        assert_eq!(apply_template(&conn, "farm", true, "Test").unwrap(), 0);
    }
}
