//! Guided onboarding: a conditional question engine.
//!
//! Questions are defined once here, each with a condition on earlier answers.
//! The UI asks for the visible questions of a step, sends the answers back and
//! receives the next set; nothing is a fixed questionnaire. Completing the
//! flow saves the farm profile, switches modules on or off and creates the
//! first livestock batches and crop plots from the answers.

use crate::{auth::{self, Role}, db, farms::{today_str, valid_date}, AppState};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{Map, Value};

/// A condition on earlier answers.
pub enum Cond {
    Always,
    /// The answer to `q` is `v`, or (for multi-select) includes `v`.
    Has(&'static str, &'static str),
    AnyOf(&'static [Cond]),
}

pub struct Q {
    pub id: &'static str,
    pub step: u8,
    /// single | multi | text | number | date | state | lga
    pub kind: &'static str,
    pub prompt: &'static str,
    pub help: &'static str,
    pub options: &'static [(&'static str, &'static str)],
    pub required: bool,
    pub show_if: Cond,
}

const YES_NO_UNSURE: &[(&str, &str)] = &[("yes", "Yes"), ("no", "No"), ("not_sure", "Not sure")];
const HAS_CROPS: Cond = Cond::AnyOf(&[Cond::Has("manage", "crops"), Cond::Has("manage", "cassava"), Cond::Has("manage", "oil_palm"), Cond::Has("manage", "other_crops")]);

pub const STEPS: [(u8, &str); 5] = [(1, "Language"), (2, "Your business"), (3, "What you manage"), (4, "Size of your business"), (5, "How you want to work")];

pub const QUESTIONS: &[Q] = &[
    Q { id: "language", step: 1, kind: "single", prompt: "Which language do you want to use?", help: "You can change this later in Settings.", options: &[("en", "English"), ("ib", "Ibibio")], required: true, show_if: Cond::Always },

    Q { id: "farm_name", step: 2, kind: "text", prompt: "What is the name of your farm or agricultural business?", help: "", options: &[], required: true, show_if: Cond::Always },
    Q { id: "state", step: 2, kind: "state", prompt: "Which state do you operate in?", help: "", options: &[], required: true, show_if: Cond::Always },
    Q { id: "lga", step: 2, kind: "lga", prompt: "Which Local Government Area?", help: "Type it if it is not in the list.", options: &[], required: false, show_if: Cond::Always },
    Q { id: "community", step: 2, kind: "text", prompt: "Which community or town?", help: "", options: &[], required: false, show_if: Cond::Always },
    Q { id: "phone", step: 2, kind: "text", prompt: "What is your phone number?", help: "Printed on receipts and reports.", options: &[], required: false, show_if: Cond::Always },
    Q { id: "address", step: 2, kind: "text", prompt: "What is your business address?", help: "", options: &[], required: false, show_if: Cond::Always },
    Q { id: "business_type", step: 2, kind: "single", prompt: "What type of agricultural business do you operate?", help: "", required: true, show_if: Cond::Always, options: &[
        ("poultry", "Poultry farming"), ("livestock", "Livestock farming"), ("crops", "Crop farming"), ("fish", "Fish farming"), ("oil_palm", "Oil palm production"),
        ("cassava", "Cassava production"), ("processing", "Agricultural processing"), ("trading", "Produce trading"), ("input_retail", "Farm input retail"), ("mixed", "Mixed farming"), ("other", "Other")] },

    Q { id: "manage", step: 3, kind: "multi", prompt: "What do you currently manage?", help: "Select all that apply. Only the questions and modules you need will follow.", required: true, show_if: Cond::Always, options: &[
        ("poultry", "Poultry"), ("goats", "Goats"), ("sheep", "Sheep"), ("pigs", "Pigs"), ("cattle", "Cattle"), ("fish", "Fish"), ("rabbits", "Rabbits"), ("snails", "Snails"),
        ("crops", "Crops"), ("oil_palm", "Oil palm (my own palms)"), ("palm_trade", "Palm oil: I buy fruit and process it"), ("cassava", "Cassava"), ("other_livestock", "Other livestock"), ("other_crops", "Other crops"), ("processing", "Processing"), ("resale", "Produce for resale")] },

    Q { id: "poultry_type", step: 4, kind: "single", prompt: "What type of poultry do you raise?", help: "", required: true, show_if: Cond::Has("manage", "poultry"), options: &[
        ("broiler", "Broilers"), ("layer", "Layers"), ("cockerel", "Cockerels"), ("noiler", "Noilers"), ("turkey", "Turkeys"), ("duck", "Ducks"), ("other_poultry", "Other poultry")] },
    Q { id: "poultry_count", step: 4, kind: "number", prompt: "How many birds do you currently have?", help: "", options: &[], required: true, show_if: Cond::Has("manage", "poultry") },
    Q { id: "poultry_placed", step: 4, kind: "date", prompt: "When did you receive them?", help: "Leave empty if you are not sure.", options: &[], required: false, show_if: Cond::Has("manage", "poultry") },
    Q { id: "poultry_source", step: 4, kind: "text", prompt: "Where did the chicks come from?", help: "The hatchery or seller.", options: &[], required: false, show_if: Cond::Has("manage", "poultry") },
    Q { id: "poultry_hatchery_vaccinated", step: 4, kind: "single", prompt: "Were they vaccinated at the hatchery?", help: "Ask the hatchery for the record if you are not sure.", options: YES_NO_UNSURE, required: false, show_if: Cond::Has("manage", "poultry") },
    Q { id: "goats_count", step: 4, kind: "number", prompt: "How many goats do you keep?", help: "", options: &[], required: true, show_if: Cond::Has("manage", "goats") },
    Q { id: "sheep_count", step: 4, kind: "number", prompt: "How many sheep do you keep?", help: "", options: &[], required: true, show_if: Cond::Has("manage", "sheep") },
    Q { id: "pigs_count", step: 4, kind: "number", prompt: "How many pigs do you keep?", help: "", options: &[], required: true, show_if: Cond::Has("manage", "pigs") },
    Q { id: "cattle_count", step: 4, kind: "number", prompt: "How many cattle do you keep?", help: "", options: &[], required: true, show_if: Cond::Has("manage", "cattle") },
    Q { id: "rabbits_count", step: 4, kind: "number", prompt: "How many rabbits do you keep?", help: "", options: &[], required: true, show_if: Cond::Has("manage", "rabbits") },
    Q { id: "snails_count", step: 4, kind: "number", prompt: "How many snails do you keep?", help: "", options: &[], required: true, show_if: Cond::Has("manage", "snails") },
    Q { id: "fish_species", step: 4, kind: "single", prompt: "Which fish do you raise?", help: "", required: true, show_if: Cond::Has("manage", "fish"), options: &[("catfish", "Catfish"), ("tilapia", "Tilapia"), ("other_fish", "Other fish")] },
    Q { id: "fish_ponds", step: 4, kind: "number", prompt: "How many fish ponds do you operate?", help: "", options: &[], required: false, show_if: Cond::Has("manage", "fish") },
    Q { id: "fish_count", step: 4, kind: "number", prompt: "How many fish (fingerlings or juveniles) are stocked now?", help: "", options: &[], required: true, show_if: Cond::Has("manage", "fish") },
    Q { id: "crop_main", step: 4, kind: "single", prompt: "Which crop do you mainly grow?", help: "You can add more crops later.", required: true, show_if: HAS_CROPS, options: &[
        ("Cassava", "Cassava"), ("Yam", "Yam"), ("Maize", "Maize"), ("Rice", "Rice"), ("Plantain", "Plantain"), ("Oil palm", "Oil palm"), ("Vegetables", "Vegetables"), ("Other", "Other")] },
    Q { id: "land_size", step: 4, kind: "number", prompt: "How much land do you cultivate?", help: "", options: &[], required: false, show_if: HAS_CROPS },
    Q { id: "land_unit", step: 4, kind: "single", prompt: "In what unit?", help: "A local plot size differs from place to place.", options: &[("hectare", "Hectares"), ("acre", "Acres"), ("plot", "Plots"), ("square_metre", "Square metres")], required: false, show_if: HAS_CROPS },
    Q { id: "planting_date", step: 4, kind: "date", prompt: "When did you plant the current crop?", help: "Leave empty if you have not planted yet.", options: &[], required: false, show_if: HAS_CROPS },
    Q { id: "processing_types", step: 4, kind: "multi", prompt: "What do you process?", help: "", required: true, show_if: Cond::Has("manage", "processing"), options: &[
        ("palm_oil", "Palm oil"), ("cassava_garri", "Garri"), ("cassava_flour", "Cassava flour"), ("cassava_fufu", "Fufu"), ("grain_milling", "Grain milling"), ("feed_milling", "Animal feed"), ("other", "Other")] },
    Q { id: "palm_bunches", step: 4, kind: "number", prompt: "How many palm fruit bunches do you normally process in one batch?", help: "", options: &[], required: false, show_if: Cond::Has("processing_types", "palm_oil") },
    Q { id: "sell_to", step: 4, kind: "single", prompt: "Who do you mainly sell to?", help: "", options: &[("consumers", "Directly to consumers"), ("wholesalers", "Wholesalers and traders"), ("both", "Both")], required: false, show_if: Cond::Always },

    Q { id: "features", step: 5, kind: "multi", prompt: "What do you want FarmBuddy to help you with?", help: "You can change this later in Settings.", required: false, show_if: Cond::Always, options: &[
        ("purchases", "Record purchases"), ("sales", "Record sales and print receipts"), ("stock", "Manage stock"), ("costs", "Track production costs"),
        ("workers", "Manage workers"), ("reminders", "Reminders for farm activities"), ("contacts", "Track suppliers and buyers"), ("reports", "Print reports")] },
];

fn answer_has(answers: &Map<String, Value>, q: &str, v: &str) -> bool {
    match answers.get(q) {
        Some(Value::String(s)) => s == v,
        Some(Value::Array(a)) => a.iter().any(|x| x.as_str() == Some(v)),
        _ => false,
    }
}

pub fn holds(c: &Cond, answers: &Map<String, Value>) -> bool {
    match c {
        Cond::Always => true,
        Cond::Has(q, v) => answer_has(answers, q, v),
        Cond::AnyOf(list) => list.iter().any(|c| holds(c, answers)),
    }
}

#[derive(Serialize, Debug)]
pub struct QuestionOut { id: &'static str, step: u8, kind: &'static str, prompt: &'static str, help: &'static str, options: Vec<(&'static str, &'static str)>, required: bool }

#[derive(Serialize, Debug)]
pub struct StepOut { step: u8, title: &'static str, questions: Vec<QuestionOut> }

/// The steps and the questions visible for the current answers.
pub fn flow(answers: &Map<String, Value>) -> Vec<StepOut> {
    STEPS.iter().map(|(n, title)| StepOut {
        step: *n, title,
        questions: QUESTIONS.iter().filter(|q| q.step == *n && holds(&q.show_if, answers)).map(|q| QuestionOut {
            id: q.id, step: q.step, kind: q.kind, prompt: q.prompt, help: q.help, options: q.options.to_vec(), required: q.required,
        }).collect(),
    }).collect()
}

fn text(answers: &Map<String, Value>, k: &str) -> String { answers.get(k).and_then(|v| v.as_str()).unwrap_or("").trim().to_string() }
fn number(answers: &Map<String, Value>, k: &str) -> Option<f64> {
    match answers.get(k) { Some(Value::Number(n)) => n.as_f64(), Some(Value::String(s)) if !s.trim().is_empty() => s.trim().replace(',', "").parse().ok(), _ => None }
}

/// Validates the visible questions of `step` (or of every step when None).
pub fn validate(answers: &Map<String, Value>, step: Option<u8>) -> Result<(), String> {
    for q in QUESTIONS.iter().filter(|q| step.is_none_or(|s| q.step == s) && holds(&q.show_if, answers)) {
        let v = answers.get(q.id);
        let empty = match v { None | Some(Value::Null) => true, Some(Value::String(s)) => s.trim().is_empty(), Some(Value::Array(a)) => a.is_empty(), _ => false };
        if empty {
            if q.required { return Err(format!("Please answer: {}", q.prompt)); }
            continue;
        }
        let v = v.expect("checked");
        match q.kind {
            "single" => { let s = v.as_str().unwrap_or(""); if !q.options.iter().any(|(k, _)| *k == s) { return Err(format!("Choose one of the options for: {}", q.prompt)); } }
            "multi" => {
                let arr = v.as_array().ok_or_else(|| format!("Choose from the options for: {}", q.prompt))?;
                if arr.iter().any(|x| !q.options.iter().any(|(k, _)| Some(*k) == x.as_str())) { return Err(format!("Choose from the options for: {}", q.prompt)); }
            }
            "number" => match number(answers, q.id) { Some(n) if n.is_finite() && (0.0..=100_000_000.0).contains(&n) => {}, _ => return Err(format!("Enter a number for: {}", q.prompt)) },
            "date" => { if !valid_date(v.as_str().unwrap_or("")) { return Err(format!("Choose a valid date for: {}", q.prompt)); } }
            _ => { let s = v.as_str().ok_or_else(|| format!("Check your answer to: {}", q.prompt))?; if s.chars().count() > 200 { return Err(format!("The answer to \"{}\" is too long", q.prompt)); } }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn onboarding_flow(state: tauri::State<'_, AppState>, session_token: String, answers: Map<String, Value>) -> Result<Vec<StepOut>, String> {
    auth::require(&state, &session_token, Role::Owner)?;
    Ok(flow(&answers))
}

#[tauri::command]
pub fn onboarding_check_step(state: tauri::State<'_, AppState>, session_token: String, answers: Map<String, Value>, step: u8) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Owner)?;
    validate(&answers, Some(step))
}

fn installation_is_new(state: &AppState) -> Result<(), String> {
    let conn = state.db()?;
    let users: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    if users > 0 { return Err("This installation has already been set up".into()); }
    Ok(())
}

/// The farm questions before any account exists (first run only).
#[tauri::command]
pub fn setup_flow(state: tauri::State<'_, AppState>, answers: Map<String, Value>) -> Result<Vec<StepOut>, String> {
    installation_is_new(&state)?;
    Ok(flow(&answers))
}

#[tauri::command]
pub fn setup_check_step(state: tauri::State<'_, AppState>, answers: Map<String, Value>, step: u8) -> Result<(), String> {
    installation_is_new(&state)?;
    validate(&answers, Some(step))
}

/// The farm template that fits the answers: starter categories and items.
pub fn template_for(answers: &Map<String, Value>) -> &'static str {
    if answer_has(answers, "manage", "palm_trade") && !["poultry", "fish", "crops", "livestock"].contains(&text(answers, "business_type").as_str()) { return "palm_oil_processing"; }
    match text(answers, "business_type").as_str() {
        "poultry" => match text(answers, "poultry_type").as_str() { "layer" => "layer", "cockerel" => "cockerel", _ => "broiler" },
        "fish" => "catfish",
        "cassava" => "cassava",
        "oil_palm" => "oil_palm",
        "crops" => match text(answers, "crop_main").as_str() { "Cassava" => "cassava", "Yam" => "yam", "Maize" => "maize", "Oil palm" => "oil_palm", _ => "mixed_crop" },
        "processing" => if answer_has(answers, "processing_types", "palm_oil") || answer_has(answers, "manage", "palm_trade") { "palm_oil_processing" } else { "cassava_processing" },
        "trading" => "produce_store",
        "input_retail" => "input_shop",
        _ => "mixed",
    }
}

fn unique_code(conn: &Connection, table: &str, column: &str, prefix: &str) -> Result<String, String> {
    for n in 1..1000 {
        let code = format!("{prefix}-{n}");
        let taken: Option<i64> = conn.query_row(&format!("SELECT id FROM {table} WHERE {column}=?1"), [&code], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
        if taken.is_none() { return Ok(code); }
    }
    Err("Could not create a unique code".into())
}

#[derive(Serialize, Debug, Default)]
pub struct OnboardingResult { pub farm_id: i64, pub created: Vec<String>, pub modules: Vec<String> }

pub fn complete_tx(conn: &Connection, answers: &Map<String, Value>) -> Result<OnboardingResult, String> {
    validate(answers, None)?;
    let first_time = !db::setting_bool(conn, "onboarding_done")?;
    let mut res = OnboardingResult::default();
    let has = |v: &str| answer_has(answers, "manage", v);
    let livestock = ["poultry", "goats", "sheep", "pigs", "cattle", "fish", "rabbits", "snails", "other_livestock"].iter().any(|v| has(v));
    let crops = ["crops", "cassava", "oil_palm", "other_crops"].iter().any(|v| has(v));
    let processing = has("processing");
    let palm = has("palm_trade") || has("oil_palm") || answer_has(answers, "processing_types", "palm_oil") || ["oil_palm"].contains(&text(answers, "business_type").as_str());
    let features: Vec<String> = answers.get("features").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
    // Selling stays on when nothing was chosen; a farm can always record a sale.
    let sales = features.is_empty() || features.iter().any(|f| f == "sales" || f == "stock") || has("resale");
    for (k, on) in [("module_livestock", livestock), ("module_crops", crops), ("module_processing", processing), ("module_palm", palm), ("module_sales", sales)] {
        db::set_setting(conn, k, if on { "1" } else { "0" })?;
        if on { res.modules.push(k.trim_start_matches("module_").to_string()); }
    }
    db::set_setting(conn, "language", &text(answers, "language"))?;
    let name = text(answers, "farm_name");
    let current = db::setting(conn, "business_name")?;
    if ["", "My Business", "My Eatery", "My Farm"].contains(&current.trim()) { db::set_setting(conn, "business_name", &name)?; }
    let address = [text(answers, "address"), text(answers, "community"), text(answers, "lga"), text(answers, "state")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
    if db::setting(conn, "business_address")?.trim().is_empty() && !address.is_empty() { db::set_setting(conn, "business_address", &address)?; }
    if db::setting(conn, "business_phone")?.trim().is_empty() && !text(answers, "phone").is_empty() { db::set_setting(conn, "business_phone", &text(answers, "phone"))?; }

    let activities = answers.get("manage").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(",")).unwrap_or_default();
    let existing: Option<i64> = conn.query_row("SELECT id FROM farms WHERE lower(name)=lower(?1) AND status='active'", [&name], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    res.farm_id = match existing {
        Some(id) => {
            conn.execute("UPDATE farms SET state=?1,lga=?2,community=?3,address=?4,phone=?5,activities=?6 WHERE id=?7",
                params![text(answers, "state"), text(answers, "lga"), text(answers, "community"), text(answers, "address"), text(answers, "phone"), activities, id]).map_err(|e| e.to_string())?;
            id
        }
        None => {
            conn.execute("INSERT INTO farms(name,state,lga,community,address,phone,activities) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![name, text(answers, "state"), text(answers, "lga"), text(answers, "community"), text(answers, "address"), text(answers, "phone"), activities]).map_err(|e| e.to_string())?;
            res.created.push(format!("Farm: {name}"));
            conn.last_insert_rowid()
        }
    };

    // Starting records are created once; running setup again only updates the profile and modules.
    if first_time {
        let today = today_str();
        let mut batch = |species: &str, count_key: &str, prefix: &str, label: &str, placed: String, source: String, purpose: &str| -> Result<(), String> {
            let n = number(answers, count_key).unwrap_or(0.0).round() as i64;
            if n <= 0 { return Ok(()); }
            let code = unique_code(conn, "flocks", "code", prefix)?;
            conn.execute("INSERT INTO flocks(code,placed_date,chicks_placed,species,purpose,source,farm_id,notes) VALUES(?1,?2,?3,?4,?5,?6,?7,'Created during setup')",
                params![code, placed, n, species, purpose, source, res.farm_id]).map_err(|e| e.to_string())?;
            res.created.push(format!("{label} batch {code}: {n}"));
            Ok(())
        };
        if has("poultry") {
            let kind = text(answers, "poultry_type");
            let placed = Some(text(answers, "poultry_placed")).filter(|d| valid_date(d)).unwrap_or_else(|| today.clone());
            let prefix = match kind.as_str() { "broiler" => "BR", "layer" => "LY", "cockerel" => "CK", "noiler" => "NO", "turkey" => "TK", "duck" => "DK", _ => "PL" };
            let purpose = if kind == "layer" { "Eggs" } else { "Meat" };
            batch(&kind, "poultry_count", prefix, "Poultry", placed, text(answers, "poultry_source"), purpose)?;
        }
        for (key, species, prefix, label) in [("goats", "goat", "GT", "Goat"), ("sheep", "sheep", "SH", "Sheep"), ("pigs", "pig", "PG", "Pig"), ("cattle", "cattle", "CT", "Cattle"), ("rabbits", "rabbit", "RB", "Rabbit"), ("snails", "snail", "SN", "Snail")] {
            if has(key) { batch(species, &format!("{key}_count"), prefix, label, today.clone(), String::new(), "")?; }
        }
        if has("fish") {
            let sp = text(answers, "fish_species");
            batch(&sp, "fish_count", "FS", "Fish", today.clone(), String::new(), "Table fish")?;
        }
        if crops {
            let code = unique_code(conn, "farm_plots", "code", "PLOT")?;
            let area = number(answers, "land_size").unwrap_or(0.0);
            let unit = Some(text(answers, "land_unit")).filter(|u| !u.is_empty()).unwrap_or_else(|| "hectare".into());
            conn.execute("INSERT INTO farm_plots(farm_id,code,name,area,area_unit) VALUES(?1,?2,'Main plot',?3,?4)", params![res.farm_id, code, area, unit]).map_err(|e| e.to_string())?;
            let plot = conn.last_insert_rowid();
            res.created.push(format!("Plot {code}"));
            let planted = text(answers, "planting_date");
            if valid_date(&planted) {
                let crop = text(answers, "crop_main");
                conn.execute("INSERT INTO crop_cycles(plot_id,crop,planting_date,notes) VALUES(?1,?2,?3,'Created during setup')", params![plot, crop, planted]).map_err(|e| e.to_string())?;
                res.created.push(format!("{crop} planted on {planted}"));
            }
        }
    }
    db::set_setting(conn, "onboarding_answers", &Value::Object(answers.clone()).to_string())?;
    db::set_setting(conn, "onboarding_done", "1")?;
    db::set_setting(conn, "feature_flocks", if livestock { "1" } else { "0" })?;
    Ok(res)
}

#[tauri::command]
pub fn complete_onboarding(state: tauri::State<'_, AppState>, session_token: String, answers: Map<String, Value>) -> Result<OnboardingResult, String> {
    auth::require(&state, &session_token, Role::Owner)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let r = complete_tx(&tx, &answers)?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(v: Value) -> Map<String, Value> { v.as_object().unwrap().clone() }
    fn ids(a: &Map<String, Value>, step: u8) -> Vec<&'static str> { flow(a).into_iter().find(|s| s.step == step).unwrap().questions.into_iter().map(|q| q.id).collect() }

    #[test]
    fn questions_follow_the_answers() {
        let poultry = obj(json!({ "manage": ["poultry"] }));
        let q = ids(&poultry, 4);
        assert!(q.contains(&"poultry_type") && q.contains(&"poultry_count"));
        assert!(!q.contains(&"crop_main") && !q.contains(&"goats_count"));
        let mixed = obj(json!({ "manage": ["cassava", "processing"], "processing_types": ["palm_oil"] }));
        let q = ids(&mixed, 4);
        assert!(q.contains(&"crop_main") && q.contains(&"palm_bunches"));
        assert!(!q.contains(&"poultry_type"));
    }

    #[test]
    fn step_validation() {
        let a = obj(json!({ "language": "en", "farm_name": "", "state": "Akwa Ibom", "business_type": "poultry" }));
        assert!(validate(&a, Some(2)).is_err(), "farm name is required");
        let a = obj(json!({ "manage": ["poultry"], "poultry_type": "dragon", "poultry_count": "500" }));
        assert!(validate(&a, Some(4)).is_err(), "unknown option");
        let a = obj(json!({ "manage": ["poultry"], "poultry_type": "broiler", "poultry_count": "abc" }));
        assert!(validate(&a, Some(4)).is_err(), "not a number");
    }

    #[test]
    fn completing_creates_the_workspace_once() {
        let conn = crate::db::test_conn();
        let a = obj(json!({
            "language": "en", "farm_name": "Ikot Ekpene Farms", "state": "Akwa Ibom", "lga": "Ikot Ekpene", "business_type": "mixed",
            "manage": ["poultry", "cassava", "goats"], "poultry_type": "broiler", "poultry_count": "500", "goats_count": 12,
            "crop_main": "Cassava", "land_size": "2", "land_unit": "hectare", "planting_date": "2026-04-01", "features": ["sales", "reminders"]
        }));
        let r = complete_tx(&conn, &a).unwrap();
        assert_eq!(r.modules, vec!["livestock", "crops", "sales"]);
        let birds: i64 = conn.query_row("SELECT chicks_placed FROM flocks WHERE species='broiler'", [], |r| r.get(0)).unwrap();
        assert_eq!(birds, 500);
        let goats: String = conn.query_row("SELECT code FROM flocks WHERE species='goat'", [], |r| r.get(0)).unwrap();
        assert_eq!(goats, "GT-1");
        let cycles: i64 = conn.query_row("SELECT COUNT(*) FROM crop_cycles", [], |r| r.get(0)).unwrap();
        assert_eq!(cycles, 1);
        assert_eq!(db::setting(&conn, "module_processing").unwrap(), "0");
        assert_eq!(db::setting(&conn, "business_name").unwrap(), "Ikot Ekpene Farms");
        // Running it again does not duplicate records.
        complete_tx(&conn, &a).unwrap();
        let batches: i64 = conn.query_row("SELECT COUNT(*) FROM flocks", [], |r| r.get(0)).unwrap();
        assert_eq!(batches, 2);
        let farms: i64 = conn.query_row("SELECT COUNT(*) FROM farms", [], |r| r.get(0)).unwrap();
        assert_eq!(farms, 1);
    }

    #[test]
    fn first_run_farm_setup_creates_a_farm_not_a_shop() {
        let conn = db::test_conn();
        let a = obj(json!({ "language": "en", "farm_name": "Dearborn Farms", "state": "Akwa Ibom", "lga": "Etinan", "business_type": "poultry",
            "manage": ["poultry"], "poultry_type": "broiler", "poultry_count": "500", "features": ["sales", "stock", "reminders"] }));
        assert_eq!(template_for(&a), "broiler");
        crate::templates::apply_template(&conn, "farm", false, "Owner").unwrap();
        crate::farm_templates::apply_tx(&conn, template_for(&a), "Owner").unwrap();
        complete_tx(&conn, &a).unwrap();
        assert_eq!(db::setting(&conn, "business_name").unwrap(), "Dearborn Farms");
        assert_eq!(db::setting(&conn, "business_type").unwrap(), "farm");
        assert_eq!(db::setting(&conn, "farm_template").unwrap(), "broiler");
        let flocks: i64 = conn.query_row("SELECT COUNT(*) FROM flocks WHERE chicks_placed=500", [], |r| r.get(0)).unwrap();
        assert_eq!(flocks, 1);
        let shop: i64 = conn.query_row("SELECT COUNT(*) FROM products WHERE name LIKE '%Peak%' OR name LIKE '%Indomie%' OR name LIKE '%Jollof%'", [], |r| r.get(0)).unwrap();
        assert_eq!(shop, 0);
        let priced: i64 = conn.query_row("SELECT COUNT(*) FROM products WHERE selling_price_kobo<>0", [], |r| r.get(0)).unwrap();
        assert_eq!(priced, 0);
        for (bt, extra, want) in [("fish", json!({}), "catfish"), ("crops", json!({"crop_main": "Yam"}), "yam"), ("trading", json!({}), "produce_store"), ("input_retail", json!({}), "input_shop"), ("mixed", json!({}), "mixed")] {
            let mut m = obj(json!({ "business_type": bt })); m.extend(obj(extra));
            assert_eq!(template_for(&m), want);
        }
    }
}
