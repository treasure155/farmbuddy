//! The 14 starter farm templates from the specification (section G). A
//! template adds categories, units, starter input and product names (without
//! prices or stock: the farmer enters their own), switches on the modules the
//! business needs, and sets the dashboard cards, check-ups and reports shown
//! first. Templates only ever add; nothing the farmer entered is changed.

use crate::{audit, auth::{self, Role}, db, AppState};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

/// (name, category, kind: material = farm input | product = for sale, unit)
type Item = (&'static str, &'static str, &'static str, &'static str);

pub struct FarmTemplate {
    pub key: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
    /// livestock | crops | processing | sales
    pub modules: &'static [&'static str],
    pub categories: &'static [&'static str],
    pub units: &'static [&'static str],
    pub items: &'static [Item],
    /// Guided check-ups offered first (advisory flow keys).
    pub questions: &'static [&'static str],
    /// Dashboard cards in order.
    pub cards: &'static [&'static str],
    pub reports: &'static [&'static str],
}

const POULTRY_CATS: &[&str] = &["Feed", "Vaccines and medicines", "Litter and bedding", "Equipment", "Birds for sale"];
const POULTRY_UNITS: &[&str] = &["bag", "kg", "bird", "crate", "piece", "vial", "litre"];

pub const FARM_TEMPLATES: &[FarmTemplate] = &[
    FarmTemplate { key: "broiler", name: "Broiler farm", summary: "Meat birds in batches: daily feed, deaths and weights, vaccination, FCR and cost per kg.",
        modules: &["livestock", "sales"], categories: POULTRY_CATS, units: POULTRY_UNITS,
        items: &[("Broiler starter feed", "Feed", "material", "bag"), ("Broiler finisher feed", "Feed", "material", "bag"), ("Wood shavings", "Litter and bedding", "material", "bag"), ("Disinfectant", "Vaccines and medicines", "material", "litre"), ("Live broiler", "Birds for sale", "product", "bird"), ("Dressed broiler", "Birds for sale", "product", "kg")],
        questions: &["poultry"], cards: &["animals", "vaccinations", "low_inputs", "money", "suggestions"], reports: &["livestock", "pl", "activity", "sales"] },
    FarmTemplate { key: "layer", name: "Layer farm", summary: "Egg production: daily collection, cracked and sold eggs, laying percentage and vaccination.",
        modules: &["livestock", "sales"], categories: &["Feed", "Vaccines and medicines", "Litter and bedding", "Equipment", "Eggs", "Birds for sale"], units: &["bag", "kg", "bird", "crate", "piece", "vial", "litre"],
        items: &[("Layer mash", "Feed", "material", "bag"), ("Grower mash", "Feed", "material", "bag"), ("Egg trays", "Equipment", "material", "piece"), ("Eggs (crate of 30)", "Eggs", "product", "crate"), ("Spent layer", "Birds for sale", "product", "bird")],
        questions: &["poultry"], cards: &["eggs", "animals", "vaccinations", "low_inputs", "money"], reports: &["livestock", "pl", "sales"] },
    FarmTemplate { key: "cockerel", name: "Cockerel farm", summary: "Cockerels raised for festive and local markets, with feed, losses and vaccination records.",
        modules: &["livestock", "sales"], categories: POULTRY_CATS, units: POULTRY_UNITS,
        items: &[("Chick starter feed", "Feed", "material", "bag"), ("Grower feed", "Feed", "material", "bag"), ("Wood shavings", "Litter and bedding", "material", "bag"), ("Live cockerel", "Birds for sale", "product", "bird")],
        questions: &["poultry"], cards: &["animals", "vaccinations", "low_inputs", "money"], reports: &["livestock", "pl", "sales"] },
    FarmTemplate { key: "catfish", name: "Catfish farm", summary: "Ponds and fish batches: stocking, feeding, sampling weights, water records and harvest.",
        modules: &["livestock", "sales"], categories: &["Fish feed", "Fingerlings and juveniles", "Pond supplies", "Fish for sale"], units: &["bag", "kg", "piece", "fish"],
        items: &[("Fish feed 2mm", "Fish feed", "material", "bag"), ("Fish feed 4mm", "Fish feed", "material", "bag"), ("Fish feed 6mm", "Fish feed", "material", "bag"), ("Catfish juveniles", "Fingerlings and juveniles", "material", "fish"), ("Live catfish", "Fish for sale", "product", "kg"), ("Smoked catfish", "Fish for sale", "product", "kg")],
        questions: &["fish"], cards: &["animals", "low_inputs", "money", "suggestions"], reports: &["livestock", "pl", "sales"] },
    FarmTemplate { key: "cassava", name: "Cassava farm", summary: "Plots, planting dates, cassava stems, weeding, fertiliser, harvest and yield.",
        modules: &["crops", "sales"], categories: &["Planting material", "Fertiliser", "Crop protection", "Harvest"], units: &["bundle", "bag", "kg", "tonne", "litre"],
        items: &[("Cassava stems", "Planting material", "material", "bundle"), ("NPK fertiliser", "Fertiliser", "material", "bag"), ("Herbicide", "Crop protection", "material", "litre"), ("Fresh cassava roots", "Harvest", "product", "tonne")],
        questions: &["crop"], cards: &["crops", "harvest", "weather", "money"], reports: &["crops", "pl", "activity"] },
    FarmTemplate { key: "yam", name: "Yam farm", summary: "Seed yam, staking, weeding, harvest and storage of tubers.",
        modules: &["crops", "sales"], categories: &["Planting material", "Fertiliser", "Stakes and materials", "Harvest"], units: &["piece", "bag", "kg", "tuber", "bundle"],
        items: &[("Seed yam", "Planting material", "material", "piece"), ("Yam stakes", "Stakes and materials", "material", "bundle"), ("NPK fertiliser", "Fertiliser", "material", "bag"), ("Yam tubers", "Harvest", "product", "tuber")],
        questions: &["crop"], cards: &["crops", "harvest", "weather", "money"], reports: &["crops", "pl"] },
    FarmTemplate { key: "maize", name: "Maize farm", summary: "Seed, planting, fertiliser, weeding, harvest, drying and storage of grain.",
        modules: &["crops", "sales"], categories: &["Seed", "Fertiliser", "Crop protection", "Harvest"], units: &["kg", "bag", "tonne", "litre"],
        items: &[("Maize seed", "Seed", "material", "kg"), ("NPK fertiliser", "Fertiliser", "material", "bag"), ("Urea", "Fertiliser", "material", "bag"), ("Dry maize grain", "Harvest", "product", "bag"), ("Fresh maize cobs", "Harvest", "product", "piece")],
        questions: &["crop"], cards: &["crops", "harvest", "weather", "market", "money"], reports: &["crops", "pl"] },
    FarmTemplate { key: "oil_palm", name: "Oil palm farm", summary: "Plantation blocks, fresh fruit bunch harvests, weeding and fertiliser.",
        modules: &["crops", "sales"], categories: &["Seedlings", "Fertiliser", "Harvest"], units: &["piece", "bag", "bunch", "tonne", "kg"],
        items: &[("Oil palm seedlings", "Seedlings", "material", "piece"), ("NPK fertiliser", "Fertiliser", "material", "bag"), ("Fresh fruit bunches", "Harvest", "product", "bunch")],
        questions: &["crop"], cards: &["crops", "harvest", "money"], reports: &["crops", "pl"] },
    FarmTemplate { key: "mixed_crop", name: "Mixed crop farm", summary: "Several crops on several plots, with a calendar, varieties and plot history.",
        modules: &["crops", "sales"], categories: &["Seed and planting material", "Fertiliser", "Crop protection", "Harvest"], units: &["kg", "bag", "bundle", "tonne", "basket", "litre"],
        items: &[("Seed (various)", "Seed and planting material", "material", "kg"), ("NPK fertiliser", "Fertiliser", "material", "bag"), ("Vegetables", "Harvest", "product", "basket")],
        questions: &["crop"], cards: &["crops", "harvest", "weather", "market", "money"], reports: &["crops", "activity", "pl"] },
    FarmTemplate { key: "palm_oil_processing", name: "Palm oil processing business", summary: "Fresh fruit bunches in, palm oil, kernel and fibre out, with full batch costing.",
        modules: &["processing", "sales"], categories: &["Raw material", "Processing supplies", "Palm oil", "By-products"], units: &["bunch", "kg", "litre", "jerrycan", "drum", "bag"],
        items: &[("Fresh fruit bunches", "Raw material", "material", "bunch"), ("Firewood", "Processing supplies", "material", "bundle"), ("Jerrycans", "Processing supplies", "material", "piece"), ("Palm oil", "Palm oil", "product", "litre"), ("Palm kernel", "By-products", "product", "bag")],
        questions: &["processing"], cards: &["processing", "low_inputs", "money"], reports: &["processing", "pl", "activity"] },
    FarmTemplate { key: "cassava_processing", name: "Cassava processing business", summary: "Cassava roots into garri, fufu or flour, with peeling, pressing and frying costs.",
        modules: &["processing", "sales"], categories: &["Raw material", "Processing supplies", "Garri and flour", "By-products"], units: &["kg", "tonne", "bag", "paint", "basin"],
        items: &[("Fresh cassava roots", "Raw material", "material", "tonne"), ("Packaging bags", "Processing supplies", "material", "piece"), ("Garri (white)", "Garri and flour", "product", "bag"), ("Cassava flour", "Garri and flour", "product", "bag"), ("Cassava peels", "By-products", "product", "bag")],
        questions: &["processing"], cards: &["processing", "low_inputs", "money"], reports: &["processing", "pl"] },
    FarmTemplate { key: "produce_store", name: "Agricultural produce store", summary: "Buying, storing and selling grain and produce, with storage locations, moisture checks and spoilage.",
        modules: &["sales"], categories: &["Grains", "Tubers", "Oils", "Other produce"], units: &["bag", "kg", "tonne", "paint", "litre"],
        items: &[("Maize grain", "Grains", "product", "bag"), ("Rice (paddy)", "Grains", "product", "bag"), ("Beans", "Grains", "product", "bag"), ("Garri", "Tubers", "product", "bag"), ("Palm oil", "Oils", "product", "litre")],
        questions: &[], cards: &["storage", "market", "money", "low_inputs"], reports: &["sales", "stock", "pl"] },
    FarmTemplate { key: "input_shop", name: "Agricultural input shop", summary: "Selling feed, seed, fertiliser, chemicals and equipment to farmers, with expiry dates.",
        modules: &["sales"], categories: &["Feed", "Seed", "Fertiliser", "Crop protection", "Animal health", "Equipment"], units: &["bag", "kg", "litre", "piece", "vial", "pack"],
        items: &[("Broiler starter feed", "Feed", "product", "bag"), ("Layer mash", "Feed", "product", "bag"), ("Maize seed", "Seed", "product", "kg"), ("NPK fertiliser", "Fertiliser", "product", "bag"), ("Knapsack sprayer", "Equipment", "product", "piece"), ("Drinkers", "Equipment", "product", "piece")],
        questions: &[], cards: &["money", "low_inputs", "storage"], reports: &["sales", "stock", "pl"] },
    FarmTemplate { key: "mixed", name: "Mixed agricultural business", summary: "Livestock, crops, processing and trading together, with profit by activity.",
        modules: &["livestock", "crops", "processing", "sales"], categories: &["Feed", "Vaccines and medicines", "Seed and planting material", "Fertiliser", "Raw material", "Farm produce"], units: &["bag", "kg", "bird", "crate", "litre", "tonne", "bundle", "piece"],
        items: &[("Poultry feed", "Feed", "material", "bag"), ("Seed (various)", "Seed and planting material", "material", "kg"), ("NPK fertiliser", "Fertiliser", "material", "bag"), ("Eggs (crate of 30)", "Farm produce", "product", "crate"), ("Fresh cassava roots", "Farm produce", "product", "tonne")],
        questions: &["poultry", "crop", "fish", "processing"], cards: &["animals", "crops", "processing", "money", "suggestions"], reports: &["pl", "activity", "livestock", "crops", "processing"] },
];

pub fn find(key: &str) -> Option<&'static FarmTemplate> { FARM_TEMPLATES.iter().find(|t| t.key == key) }

fn sku_prefix(key: &str) -> String { format!("FT-{}-", key.to_uppercase().replace('_', "")) }

#[derive(Serialize, Debug, Default, PartialEq)]
pub struct ApplyResult { pub categories: usize, pub items: usize, pub modules: Vec<String> }

pub fn apply_tx(conn: &Connection, key: &str, user: &str) -> Result<ApplyResult, String> {
    let t = find(key).ok_or("Unknown farm template")?;
    let mut r = ApplyResult::default();
    for (i, c) in t.categories.iter().enumerate() {
        r.categories += conn.execute("INSERT OR IGNORE INTO categories(name,sort_order) VALUES(?1,?2)", params![c, 50 + i as i64]).map_err(|e| e.to_string())?;
    }
    for (i, (name, cat, kind, unit)) in t.items.iter().enumerate() {
        let sku = format!("{}{:02}", sku_prefix(t.key), i + 1);
        let exists: Option<i64> = conn.query_row("SELECT id FROM products WHERE sku=?1 OR (lower(name)=lower(?2) AND active=1)", params![sku, name], |x| x.get(0)).optional().map_err(|e| e.to_string())?;
        if exists.is_some() { continue; }
        let cat_id: Option<i64> = conn.query_row("SELECT id FROM categories WHERE name=?1", [cat], |x| x.get(0)).optional().map_err(|e| e.to_string())?;
        // No prices and no stock: those are the farmer's own figures.
        conn.execute("INSERT INTO products(sku,name,category_id,kind,unit,selling_price_kobo,cost_price_kobo,track_stock,stock_qty,reorder_level) VALUES(?1,?2,?3,?4,?5,0,0,1,0,0)",
            params![sku, name, cat_id, kind, unit]).map_err(|e| e.to_string())?;
        r.items += 1;
    }
    for m in t.modules {
        db::set_setting(conn, &format!("module_{m}"), "1")?;
        r.modules.push(m.to_string());
    }
    if t.modules.contains(&"livestock") { db::set_setting(conn, "feature_flocks", "1")?; }
    db::set_setting(conn, "farm_template", t.key)?;
    db::set_setting(conn, "dashboard_cards", &t.cards.join(","))?;
    db::set_setting(conn, "template_questions", &t.questions.join(","))?;
    db::set_setting(conn, "template_reports", &t.reports.join(","))?;
    audit::log(conn, user, "farm_template_applied", "settings", None, &format!("{}: {} categories, {} items added", t.name, r.categories, r.items));
    Ok(r)
}

#[derive(Serialize)]
pub struct FarmTemplateInfo { key: &'static str, name: &'static str, summary: &'static str, modules: Vec<&'static str>, categories: Vec<&'static str>, units: Vec<&'static str>, items: Vec<(&'static str, &'static str)>, questions: Vec<&'static str>, cards: Vec<&'static str>, reports: Vec<&'static str> }

#[tauri::command]
pub fn list_farm_templates() -> Vec<FarmTemplateInfo> {
    FARM_TEMPLATES.iter().map(|t| FarmTemplateInfo {
        key: t.key, name: t.name, summary: t.summary, modules: t.modules.to_vec(), categories: t.categories.to_vec(), units: t.units.to_vec(),
        items: t.items.iter().map(|i| (i.0, i.3)).collect(), questions: t.questions.to_vec(), cards: t.cards.to_vec(), reports: t.reports.to_vec(),
    }).collect()
}

#[tauri::command]
pub fn apply_farm_template(state: tauri::State<'_, AppState>, session_token: String, key: String) -> Result<ApplyResult, String> {
    let me = auth::require(&state, &session_token, Role::Owner)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let r = apply_tx(&tx, &key, &me.display_name())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fourteen_templates_apply_safely() {
        assert_eq!(FARM_TEMPLATES.len(), 14);
        let keys: std::collections::HashSet<_> = FARM_TEMPLATES.iter().map(|t| t.key).collect();
        assert_eq!(keys.len(), 14);
        let conn = crate::db::test_conn();
        db::set_setting(&conn, "module_crops", "1").unwrap();
        for t in FARM_TEMPLATES {
            assert!(!t.cards.is_empty() && !t.reports.is_empty() && !t.items.is_empty());
            for (_, cat, kind, _) in t.items { assert!(t.categories.contains(cat), "{} item category", t.key); assert!(*kind == "material" || *kind == "product"); }
            apply_tx(&conn, t.key, "Test").unwrap();
            assert_eq!(apply_tx(&conn, t.key, "Test").unwrap().items, 0, "second apply adds nothing");
        }
        let priced: i64 = conn.query_row("SELECT COUNT(*) FROM products WHERE selling_price_kobo<>0 OR stock_qty<>0", [], |r| r.get(0)).unwrap();
        assert_eq!(priced, 0);
        // Never switches a module off.
        assert_eq!(db::setting(&conn, "module_crops").unwrap(), "1");
    }
}
