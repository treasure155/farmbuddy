//! Guided advisory check-ups: the farmer picks an activity, answers questions
//! that follow from earlier answers, and gets guidance that says why it was
//! given, where it comes from and what kind of advice it is. Results can be
//! saved and turned into reminders.
//!
//! The guidance is general management, record keeping and "speak to a
//! professional" advice. It never diagnoses disease, never names a medicine or
//! dose, and never gives chemical rates.

use crate::{advisory::Recommendation, auth::{self, Role}, farms::{parse_date, rows, today}, onboarding::{holds, Cond, Q}, AppState};
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::{Map, Value};

const YNU: &[(&str, &str)] = &[("yes", "Yes"), ("no", "No"), ("not_sure", "Not sure")];
const YN: &[(&str, &str)] = &[("yes", "Yes"), ("no", "No")];

pub const FLOWS: &[(&str, &str, &str)] = &[
    ("poultry", "Poultry check-up", "Batch, vaccination, feed, water, litter and visitors"),
    ("crop", "Crop check-up", "Planting material, field work, pests and the season"),
    ("fish", "Fish pond check-up", "Water checks, feeding and losses"),
    ("processing", "Processing check-up", "Costs, outputs and storage of a batch"),
];

const POULTRY: &[Q] = &[
    Q { id: "poultry_type", step: 1, kind: "single", prompt: "What type of poultry do you raise?", help: "", options: &[("broiler", "Broilers"), ("layer", "Layers"), ("cockerel", "Cockerels"), ("noiler", "Noilers"), ("turkey", "Turkeys")], required: true, show_if: Cond::Always },
    Q { id: "age_days", step: 1, kind: "number", prompt: "How old are the birds, in days?", help: "", options: &[], required: true, show_if: Cond::Always },
    Q { id: "hatchery_vaccinated", step: 1, kind: "single", prompt: "Were they vaccinated at the hatchery?", help: "", options: YNU, required: true, show_if: Cond::Always },
    Q { id: "vacc_record", step: 1, kind: "single", prompt: "Do you have a written vaccination record for this batch?", help: "", options: YN, required: true, show_if: Cond::Always },
    Q { id: "daily_feed_kg", step: 1, kind: "number", prompt: "How many kilograms of feed do you give per day?", help: "Leave empty if you do not know.", options: &[], required: false, show_if: Cond::Always },
    Q { id: "bird_count", step: 1, kind: "number", prompt: "How many birds are alive now?", help: "", options: &[], required: false, show_if: Cond::Always },
    Q { id: "deaths", step: 1, kind: "single", prompt: "Have you recorded deaths in the last three days?", help: "", options: &[("none", "No deaths"), ("some", "On one or two days"), ("every_day", "On each of the three days")], required: true, show_if: Cond::Always },
    Q { id: "signs", step: 1, kind: "multi", prompt: "Have you noticed any of these?", help: "Tick all that apply.", options: &[("none", "Nothing unusual"), ("not_eating", "Not eating or drinking well"), ("breathing", "Noisy or difficult breathing"), ("droppings", "Unusual droppings"), ("weak", "Weak or not moving"), ("eggs_drop", "Fewer or poor eggs")], required: true, show_if: Cond::Always },
    Q { id: "water", step: 1, kind: "single", prompt: "Do the birds have clean water all the time?", help: "", options: YN, required: true, show_if: Cond::Always },
    Q { id: "litter", step: 1, kind: "single", prompt: "Is the litter (floor bedding) dry?", help: "", options: &[("dry", "Dry"), ("wet", "Wet or caked")], required: true, show_if: Cond::Always },
    Q { id: "visitors", step: 1, kind: "single", prompt: "Do you control who enters the poultry house?", help: "", options: &[("yes", "Yes, few visitors and clean footwear"), ("no", "No, people come and go")], required: true, show_if: Cond::Always },
];

const CROP: &[Q] = &[
    Q { id: "crop", step: 1, kind: "single", prompt: "Which crop are you growing?", help: "", options: &[("Cassava", "Cassava"), ("Yam", "Yam"), ("Maize", "Maize"), ("Rice", "Rice"), ("Plantain", "Plantain"), ("Oil palm", "Oil palm"), ("Vegetables", "Vegetables")], required: true, show_if: Cond::Always },
    Q { id: "state", step: 1, kind: "state", prompt: "Which state is the farm in?", help: "", options: &[], required: true, show_if: Cond::Always },
    Q { id: "planted", step: 1, kind: "date", prompt: "When did you plant?", help: "Leave empty if you have not planted yet.", options: &[], required: false, show_if: Cond::Always },
    Q { id: "material", step: 1, kind: "single", prompt: "Where did the seed or planting material come from?", help: "", options: &[("certified", "Certified seller, with label or receipt"), ("own", "My own from last season"), ("unknown", "Market or not sure")], required: true, show_if: Cond::Always },
    Q { id: "done", step: 1, kind: "multi", prompt: "What have you done so far?", help: "", options: &[("land_preparation", "Land preparation"), ("planting", "Planting"), ("weeding", "Weeding"), ("fertiliser", "Fertiliser"), ("spraying", "Spraying")], required: false, show_if: Cond::Always },
    Q { id: "pests", step: 1, kind: "single", prompt: "Have you noticed pests or disease signs?", help: "", options: YNU, required: true, show_if: Cond::Always },
    Q { id: "pest_notes", step: 1, kind: "text", prompt: "Describe what you see", help: "For example: holes in leaves, yellow patches, wilting.", options: &[], required: false, show_if: Cond::AnyOf(&[Cond::Has("pests", "yes"), Cond::Has("pests", "not_sure")]) },
];

const FISH: &[Q] = &[
    Q { id: "species", step: 1, kind: "single", prompt: "Which fish do you raise?", help: "", options: &[("catfish", "Catfish"), ("tilapia", "Tilapia"), ("other_fish", "Other")], required: true, show_if: Cond::Always },
    Q { id: "water_measured", step: 1, kind: "single", prompt: "Have you measured the water (temperature, pH or oxygen) this week?", help: "", options: YN, required: true, show_if: Cond::Always },
    Q { id: "water_change", step: 1, kind: "single", prompt: "Has the water changed colour or smell?", help: "", options: YNU, required: true, show_if: Cond::Always },
    Q { id: "gasping", step: 1, kind: "single", prompt: "Are fish gathering at the surface or gasping?", help: "", options: YN, required: true, show_if: Cond::Always },
    Q { id: "deaths", step: 1, kind: "single", prompt: "Have fish died in the last three days?", help: "", options: YN, required: true, show_if: Cond::Always },
    Q { id: "sampled", step: 1, kind: "single", prompt: "Have you weighed a sample of fish in the last two weeks?", help: "", options: YN, required: true, show_if: Cond::Always },
];

const PROCESSING: &[Q] = &[
    Q { id: "product", step: 1, kind: "single", prompt: "What are you processing?", help: "", options: &[("palm_oil", "Palm oil"), ("cassava_garri", "Garri"), ("cassava_flour", "Cassava flour"), ("grain_milling", "Grain milling"), ("feed_milling", "Animal feed"), ("other", "Other")], required: true, show_if: Cond::Always },
    Q { id: "raw_recorded", step: 1, kind: "single", prompt: "Have you recorded the raw material and what you paid for it?", help: "", options: YN, required: true, show_if: Cond::Always },
    Q { id: "costs_recorded", step: 1, kind: "single", prompt: "Have you recorded transport, labour, milling and other costs?", help: "", options: YN, required: true, show_if: Cond::Always },
    Q { id: "outputs_recorded", step: 1, kind: "single", prompt: "Have you recorded what came out (products, by-products, waste)?", help: "", options: YN, required: true, show_if: Cond::Always },
    Q { id: "storage", step: 1, kind: "single", prompt: "Where is the finished product kept?", help: "", options: &[("store", "In a named store in FarmBuddy"), ("not_recorded", "Not recorded")], required: true, show_if: Cond::Always },
];

fn questions(flow: &str) -> Result<&'static [Q], String> {
    match flow { "poultry" => Ok(POULTRY), "crop" => Ok(CROP), "fish" => Ok(FISH), "processing" => Ok(PROCESSING), _ => Err("Unknown check-up".into()) }
}

#[derive(Serialize, Debug)]
pub struct FlowQuestion { id: &'static str, kind: &'static str, prompt: &'static str, help: &'static str, options: Vec<(&'static str, &'static str)>, required: bool }

pub fn visible(flow: &str, answers: &Map<String, Value>) -> Result<Vec<FlowQuestion>, String> {
    Ok(questions(flow)?.iter().filter(|q| holds(&q.show_if, answers)).map(|q| FlowQuestion { id: q.id, kind: q.kind, prompt: q.prompt, help: q.help, options: q.options.to_vec(), required: q.required }).collect())
}

fn s<'a>(a: &'a Map<String, Value>, k: &str) -> &'a str { a.get(k).and_then(|v| v.as_str()).unwrap_or("") }
fn n(a: &Map<String, Value>, k: &str) -> Option<f64> { match a.get(k) { Some(Value::Number(x)) => x.as_f64(), Some(Value::String(t)) => t.trim().replace(',', "").parse().ok(), _ => None } }
fn has(a: &Map<String, Value>, k: &str, v: &str) -> bool { match a.get(k) { Some(Value::Array(x)) => x.iter().any(|y| y.as_str() == Some(v)), Some(Value::String(x)) => x == v, _ => false } }

fn r(key: &str, cat: &str, title: &str, text: &str, why: String, safety: &str, article: &str, priority: u8, view: &str) -> Recommendation {
    Recommendation { key: key.into(), category: cat.into(), title: title.into(), text: text.into(), why, safety_level: safety.into(), source: "FarmBuddy guidance (pending expert review) and your answers".into(), article: article.into(), priority, related_view: view.into(), related_id: None }
}

pub fn evaluate(conn: &Connection, flow: &str, a: &Map<String, Value>) -> Result<Vec<Recommendation>, String> {
    for q in questions(flow)?.iter().filter(|q| q.required && holds(&q.show_if, a)) {
        let empty = match a.get(q.id) { None | Some(Value::Null) => true, Some(Value::String(x)) => x.trim().is_empty(), Some(Value::Array(x)) => x.is_empty(), _ => false };
        if empty { return Err(format!("Please answer: {}", q.prompt)); }
    }
    let mut out = Vec::new();
    match flow {
        "poultry" => {
            let sick_signs: Vec<&str> = ["not_eating", "breathing", "droppings", "weak", "eggs_drop"].into_iter().filter(|x| has(a, "signs", x)).collect();
            if s(a, "deaths") == "every_day" || !sick_signs.is_empty() {
                out.push(r("p-vet", "Health", "Contact a veterinary professional", "Separate birds that look sick, keep records of what you see and how many are affected, and ask a vet to examine the birds. Do not start medicines without a vet's advice.",
                    format!("You reported {}{}.", if s(a, "deaths") == "every_day" { "deaths on each of the last three days" } else { "no deaths every day" }, if sick_signs.is_empty() { String::new() } else { format!(" and these signs: {}", sick_signs.join(", ").replace('_', " ")) }),
                    "vet_only", "biosecurity-basics", 1, "health"));
            }
            if s(a, "hatchery_vaccinated") != "yes" {
                out.push(r("p-hatchery", "Health", "Get the hatchery vaccination record", "Ask the hatchery what vaccines were given before delivery, then plan the rest of the schedule with a vet. Record each vaccination in Health and vaccination.",
                    format!("You answered \"{}\" to hatchery vaccination.", s(a, "hatchery_vaccinated").replace('_', " ")), "professional_confirmation", "vaccination-records", 2, "health"));
            }
            if s(a, "vacc_record") == "no" {
                out.push(r("p-record", "Record keeping", "Start a vaccination record for this batch", "Write down every vaccine with its date, name, lot number and who gave it. FarmBuddy keeps it and reminds you of what is due.", "You have no written vaccination record.".into(), "record_keeping", "vaccination-records", 2, "health"));
            }
            if s(a, "water") == "no" {
                out.push(r("p-water", "Management", "Give clean water at all times", "Clean the drinkers daily and keep water available all day. Record water use if you can measure it.", "You said the birds do not always have clean water.".into(), "general", "biosecurity-basics", 1, "livestock"));
            }
            if s(a, "litter") == "wet" {
                out.push(r("p-litter", "Management", "Keep the litter dry", "Fix leaking drinkers, improve airflow and replace wet or caked litter.", "You said the litter is wet or caked.".into(), "general", "biosecurity-basics", 2, "livestock"));
            }
            if s(a, "visitors") == "no" {
                out.push(r("p-visitors", "Biosecurity", "Control who enters the poultry house", "Limit visitors, keep a visitor book, and ask everyone to clean hands and footwear before entering.", "You said people come and go.".into(), "general", "biosecurity-basics", 2, "livestock"));
            }
            let age = n(a, "age_days").unwrap_or(0.0).round() as i64;
            let species = s(a, "poultry_type");
            if let (Some(feed), Some(birds)) = (n(a, "daily_feed_kg"), n(a, "bird_count")) {
                if birds > 0.0 {
                    let guide = crate::feed::list_guides_tx(conn)?.into_iter().find(|g| g.species == species && age >= g.age_from_day && age <= g.age_to_day);
                    match guide {
                        Some(g) => {
                            let expected = g.grams_per_animal_day * birds / 1000.0;
                            out.push(r("p-feed", "Feed", &format!("Feed: {:.0} kg given, about {:.0} kg expected", feed, expected),
                                if (feed - expected).abs() / expected.max(0.001) > 0.2 { "Your daily feed differs from the guide by more than a fifth. Check the feed weighing, wastage and the guide itself with your feed supplier." } else { "Your daily feed is close to the guide." },
                                format!("{} birds at {} days; guide \"{}\" gives {} g per bird per day ({}).", birds, age, g.source, g.grams_per_animal_day, if g.review_status == "approved" { "reviewed" } else { "not yet reviewed" }), "general", "feed-conversion-ratio", 3, "feed"));
                        }
                        None => out.push(r("p-feed-guide", "Feed", "Add a feed guide to compare feed use", "Enter the daily amount per bird from your feed bag label or feed supplier in Feed → Guides. FarmBuddy does not assume one amount for every breed and age.",
                            format!("No feed guide covers {} at {} days.", species, age), "record_keeping", "feed-conversion-ratio", 3, "feed")),
                    }
                }
            }
            if ["broiler", "cockerel", "noiler", "turkey"].contains(&species) && age >= 7 {
                out.push(r("p-weigh", "Feed", "Weigh a sample every week", "Weigh a few birds, enter the average in the daily record, and FarmBuddy works out the feed conversion ratio and cost per kg.", format!("Meat birds at {age} days."), "record_keeping", "feed-conversion-ratio", 3, "livestock"));
            }
        }
        "crop" => {
            let crop = s(a, "crop");
            if s(a, "material") != "certified" {
                out.push(r("c-seed", "Crops", "Use certified seed or planting material next time", "Buy from registered sellers and keep the label or receipt. Record the variety and seller when you plant.", "Your planting material is not from a certified seller.".into(), "general", "certified-seed", 3, "crops"));
            }
            if let Some(p) = parse_date(s(a, "planted")) {
                let days = (today() - p).num_days();
                if days > 21 && !has(a, "done", "weeding") {
                    out.push(r("c-weed", "Crops", "Plan weeding", "Weeds compete with young crops for water and nutrients. Ask your extension officer when to weed this crop, and record weeding with its labour cost.", format!("{crop} was planted {days} days ago and you have not weeded yet."), "planning", "crop-records", 2, "crops"));
                }
            }
            if has(a, "done", "fertiliser") || has(a, "done", "spraying") {
                out.push(r("c-label", "Crop health", "Record products and follow their labels", "Record the product, quantity and date in field work. Use registered products only and follow the label; FarmBuddy does not give rates.", "You have applied fertiliser or sprayed.".into(), "chemical_label", "pesticide-labels", 2, "crops"));
            }
            if s(a, "pests") != "no" {
                out.push(r("c-pests", "Crop health", "Confirm the problem before treating", "Record what you see as a crop health observation and show the plants to an extension officer or agronomist. Treat only on their advice and the product label.",
                    format!("You answered \"{}\" about pests or disease{}.", s(a, "pests").replace('_', " "), if s(a, "pest_notes").is_empty() { String::new() } else { format!(": {}", s(a, "pest_notes")) }), "professional_confirmation", "pesticide-labels", 1, "crops"));
            }
            let state = s(a, "state");
            let year = today().format("%Y").to_string().parse::<i64>().unwrap_or(2026);
            let outlook: Vec<(String, String, String)> = rows(conn, "SELECT onset_date,cessation_date,source FROM climate_outlooks WHERE year=?1 AND state=?2 ORDER BY lga LIMIT 1", params![year, state], |x| Ok((x.get(0)?, x.get(1)?, x.get(2)?)))?;
            match outlook.first() {
                Some((on, off, src)) => out.push(r("c-season", "Weather", "This season's rainfall outlook", "Plan planting, weeding and harvest around the expected rains for your area.", format!("Saved outlook for {state} {year}: onset {}, cessation {} (source: {src}).", if on.is_empty() { "not given" } else { on }, if off.is_empty() { "not given" } else { off }), "planning", "seasonal-planning", 3, "weather")),
                None => out.push(r("c-season", "Weather", "Add this year's rainfall outlook", "Enter NiMet's Seasonal Climate Prediction for your state in Weather → Outlooks, so the crop calendar shows expected onset and cessation.", format!("No outlook is saved for {state} {year}."), "planning", "seasonal-planning", 3, "weather")),
            }
        }
        "fish" => {
            if s(a, "gasping") == "yes" || s(a, "deaths") == "yes" || s(a, "water_change") == "yes" {
                out.push(r("f-help", "Fish", "Get help from a fisheries officer or vet", "Stop feeding until you know the cause, check and record the water, and ask a fisheries extension officer or vet. Record losses in the batch daily record.",
                    format!("You reported{}{}{}.", if s(a, "gasping") == "yes" { " fish gasping at the surface" } else { "" }, if s(a, "deaths") == "yes" { " deaths in the last three days" } else { "" }, if s(a, "water_change") == "yes" { " a change in water colour or smell" } else { "" }), "professional_confirmation", "", 1, "livestock"));
            }
            if s(a, "water_measured") == "no" {
                out.push(r("f-water", "Fish", "Measure and record the water", "Record what you can measure in the pond's water records. FarmBuddy never treats a missing reading as safe.", "You have not measured the water this week.".into(), "general", "", 2, "livestock"));
            }
            if s(a, "sampled") == "no" {
                out.push(r("f-sample", "Feed", "Weigh a sample of fish", "Net and weigh a few fish, and record the average weight. It shows growth and lets FarmBuddy work out FCR.", "No sample weighing in the last two weeks.".into(), "record_keeping", "feed-conversion-ratio", 3, "livestock"));
            }
        }
        "processing" => {
            if s(a, "raw_recorded") == "no" || s(a, "costs_recorded") == "no" {
                out.push(r("pr-costs", "Processing", "Record every cost of the batch", "Add the raw material and every cost (transport, labour, milling or pressing fee, fuel, packaging) so the cost per litre or kg is complete.", "Some costs are not recorded yet.".into(), "record_keeping", "processing-batch-costing", 1, "processing"));
            }
            if s(a, "outputs_recorded") == "no" {
                out.push(r("pr-out", "Processing", "Record what came out", "Enter main products with their share of the cost, by-products and waste, then close the batch to put the products into stock at their real cost.", "Outputs are not recorded yet.".into(), "record_keeping", "processing-batch-costing", 1, "processing"));
            }
            if s(a, "storage") == "not_recorded" {
                out.push(r("pr-store", "Storage", "Record where the product is stored", "Put the finished product away in a named store, so you can track spoilage, moisture and transfers.", "The storage place is not recorded.".into(), "record_keeping", "", 3, "storage"));
            }
        }
        _ => return Err("Unknown check-up".into()),
    }
    if out.is_empty() {
        out.push(r("ok", "General", "Nothing needs attention from these answers", "Keep recording daily figures so FarmBuddy can spot changes early.", "None of your answers pointed to a problem.".into(), "general", "daily-livestock-records", 3, ""));
    }
    out.sort_by_key(|x| x.priority);
    Ok(out)
}

#[derive(Serialize)]
pub struct FlowInfo { key: &'static str, title: &'static str, summary: &'static str }

#[tauri::command]
pub fn list_advisory_flows() -> Vec<FlowInfo> {
    FLOWS.iter().map(|(key, title, summary)| FlowInfo { key, title, summary }).collect()
}

#[tauri::command]
pub fn advisory_questions(state: tauri::State<'_, AppState>, session_token: String, flow: String, answers: Map<String, Value>) -> Result<Vec<FlowQuestion>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    visible(&flow, &answers)
}

#[derive(Serialize)]
pub struct AdvisoryRun { id: i64, results: Vec<Recommendation> }

#[tauri::command]
pub fn run_advisory(state: tauri::State<'_, AppState>, session_token: String, flow: String, answers: Map<String, Value>) -> Result<AdvisoryRun, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let conn = state.db()?;
    let results = evaluate(&conn, &flow, &answers)?;
    conn.execute("INSERT INTO advisory_history(flow,answers_json,results_json,user_name) VALUES(?1,?2,?3,?4)",
        params![flow, Value::Object(answers).to_string(), serde_json::to_string(&results).map_err(|e| e.to_string())?, me.display_name()]).map_err(|e| e.to_string())?;
    Ok(AdvisoryRun { id: conn.last_insert_rowid(), results })
}

#[derive(Serialize)]
pub struct HistoryRow { id: i64, flow: String, results: Value, user_name: String, created_at: String }

#[tauri::command]
pub fn advisory_history(state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<HistoryRow>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    let conn = state.db()?;
    rows(&conn, "SELECT id,flow,results_json,user_name,created_at FROM advisory_history ORDER BY id DESC LIMIT 100", [], |x| Ok(HistoryRow {
        id: x.get(0)?, flow: x.get(1)?, results: serde_json::from_str(&x.get::<_, String>(2)?).unwrap_or(Value::Null), user_name: x.get(3)?, created_at: x.get(4)?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(v: Value) -> Map<String, Value> { v.as_object().unwrap().clone() }

    #[test]
    fn poultry_answers_drive_explained_guidance() {
        let conn = crate::db::test_conn();
        let a = obj(json!({ "poultry_type": "broiler", "age_days": "21", "hatchery_vaccinated": "not_sure", "vacc_record": "no", "deaths": "every_day", "signs": ["breathing"], "water": "yes", "litter": "dry", "visitors": "yes", "daily_feed_kg": "60", "bird_count": "485" }));
        let r = evaluate(&conn, "poultry", &a).unwrap();
        assert_eq!(r[0].key, "p-vet");
        assert_eq!(r[0].safety_level, "vet_only");
        assert!(r[0].why.contains("breathing"));
        assert!(r.iter().any(|x| x.key == "p-feed-guide"), "no guide entered, so no expected feed is invented");
        for x in &r { assert!(!x.text.to_lowercase().contains("mg/") && !x.text.to_lowercase().contains("ml per")); }
        assert!(evaluate(&conn, "poultry", &obj(json!({}))).is_err());
    }

    #[test]
    fn crop_questions_follow_answers() {
        let conn = crate::db::test_conn();
        assert!(!visible("crop", &obj(json!({ "pests": "no" }))).unwrap().iter().any(|q| q.id == "pest_notes"));
        assert!(visible("crop", &obj(json!({ "pests": "yes" }))).unwrap().iter().any(|q| q.id == "pest_notes"));
        let r = evaluate(&conn, "crop", &obj(json!({ "crop": "Maize", "state": "Akwa Ibom", "material": "certified", "pests": "no", "done": ["planting"] }))).unwrap();
        assert!(r.iter().any(|x| x.key == "c-season" && x.title.starts_with("Add")));
    }
}
