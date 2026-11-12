//! Farm advisory: explainable suggestions worked out from the farmer's own
//! records, and farm reminders.
//!
//! Every suggestion says why it appeared (the record that triggered it), what
//! kind of advice it is, and where to read more. The rules only cover record
//! keeping, planning and "speak to a professional" prompts; they never
//! diagnose disease, prescribe medicine or give chemical rates.

use crate::{auth::{self, Role}, farms::{check_len, parse_date, rows, today, today_str, valid_date}, AppState};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug, Clone)]
pub struct Recommendation {
    /// Stable key, e.g. `no-log:12`, so the UI can link a reminder to it.
    pub key: String,
    pub category: String,
    pub title: String,
    pub text: String,
    /// Why this suggestion appeared, in terms of the farmer's own records.
    pub why: String,
    pub safety_level: String,
    pub source: String,
    /// Slug of a library article to read more, if any.
    pub article: String,
    pub priority: u8,
    pub related_view: String,
    pub related_id: Option<i64>,
}

fn rec(key: String, category: &str, title: String, text: &str, why: String, safety: &str, article: &str, priority: u8, view: &str, id: Option<i64>) -> Recommendation {
    Recommendation { key, category: category.into(), title, text: text.into(), why, safety_level: safety.into(), source: "Your FarmBuddy records".into(), article: article.into(), priority, related_view: view.into(), related_id: id }
}

fn days_since(date: &str) -> Option<i64> { parse_date(date).map(|d| (today() - d).num_days()) }

// ------------------------------------------------------------------ configurable rules

/// (key, title, description, default priority, default days)
pub const RULES: &[(&str, &str, &str, i64, Option<i64>)] = &[
    ("no-log", "Missing daily livestock record", "A batch has had no daily record for this many days.", 2, Some(2)),
    ("deaths", "Deaths on consecutive days", "Deaths were recorded on each of this many days in a row.", 1, Some(3)),
    ("weigh", "Weighing needed for FCR", "A meat batch has not been weighed for this many days.", 3, Some(7)),
    ("no-vacc", "Poultry batch without vaccination record", "A poultry batch has no vaccination record at all.", 2, None),
    ("vacc-overdue", "Vaccination overdue", "A planned vaccination is past its date.", 1, None),
    ("vacc-due", "Vaccination due", "A planned vaccination is due within three days.", 2, None),
    ("vacc-review", "Vaccination needs vet review", "A plan came from a general reference.", 2, None),
    ("births-due", "Births expected soon", "A pregnancy is due within this many days.", 2, Some(7)),
    ("water", "Fish pond water check", "A stocked pond has no water-quality record for this many days.", 3, Some(7)),
    ("feed-days", "Feed running out", "Feed in stock covers fewer than this many days at the current rate.", 2, Some(7)),
    ("crop-idle", "No field work recorded", "A growing crop has had no field work recorded for this many days.", 3, Some(14)),
    ("crop-health", "Crop problem not confirmed", "A pest or disease observation has not been confirmed by an extension officer or agronomist.", 2, None),
    ("harvest-late", "Harvest window passed", "The expected harvest window ended with no harvest recorded.", 2, None),
    ("harvest-soon", "Harvest expected soon", "The expected harvest starts within this many days.", 3, Some(14)),
    ("proc-out", "Processing outputs missing", "A processing batch has run this many days with no outputs.", 2, Some(3)),
    ("low", "Inputs running low", "A farm input is at or below its reorder level.", 3, None),
    ("owed", "Buyers owe money", "Credit sales are still outstanding.", 3, None),
    ("palm-loss", "Processing run losing money", "A processing run's costs are higher than its sales plus the unsold oil and kernel at your expected prices.", 1, None),
    ("palm-unsold", "Processed products not sold yet", "A processing run still has products unsold this many days after it started.", 3, Some(21)),
    ("palm-output", "Oil palm run without outputs", "Raw material was recorded this many days ago but nothing has come out yet.", 2, Some(5)),
];

pub fn seed_rules(conn: &Connection) -> Result<(), String> {
    let has: i64 = conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='recommendation_rules'", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    if has == 0 { return Ok(()); }
    for (k, t, d, p, days) in RULES {
        conn.execute("INSERT INTO recommendation_rules(rule_key,title,description,priority,param_days) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(rule_key) DO UPDATE SET title=excluded.title,description=excluded.description",
            params![k, t, d, p, days]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Rule { pub rule_key: String, pub title: String, pub description: String, pub enabled: bool, pub priority: i64, pub param_days: Option<i64> }

pub fn rules_tx(conn: &Connection) -> Result<Vec<Rule>, String> {
    rows(conn, "SELECT rule_key,title,description,enabled,priority,param_days FROM recommendation_rules ORDER BY priority, title", [], |r| Ok(Rule {
        rule_key: r.get(0)?, title: r.get(1)?, description: r.get(2)?, enabled: r.get::<_, i64>(3)? == 1, priority: r.get(4)?, param_days: r.get(5)?,
    }))
}

struct Rules(std::collections::HashMap<String, Rule>);
impl Rules {
    fn load(conn: &Connection) -> Result<Rules, String> { Ok(Rules(rules_tx(conn)?.into_iter().map(|r| (r.rule_key.clone(), r)).collect())) }
    fn on(&self, k: &str) -> bool { self.0.get(k).map(|r| r.enabled).unwrap_or(true) }
    fn days(&self, k: &str, default: i64) -> i64 { self.0.get(k).and_then(|r| r.param_days).unwrap_or(default).max(1) }
    fn prio(&self, k: &str, default: u8) -> u8 { self.0.get(k).map(|r| r.priority.clamp(1, 3) as u8).unwrap_or(default) }
}

#[tauri::command]
pub fn list_rules(state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<Rule>, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    rules_tx(&*state.db()?)
}

#[tauri::command]
pub fn save_rule(state: tauri::State<'_, AppState>, session_token: String, rule_key: String, enabled: bool, priority: i64, param_days: Option<i64>) -> Result<(), String> {
    let me = auth::require(&state, &session_token, Role::Owner)?;
    if !(1..=3).contains(&priority) { return Err("Priority must be 1, 2 or 3".into()); }
    if let Some(d) = param_days { if !(1..=365).contains(&d) { return Err("Days must be between 1 and 365".into()); } }
    let conn = state.db()?;
    let n = conn.execute("UPDATE recommendation_rules SET enabled=?1,priority=?2,param_days=CASE WHEN param_days IS NULL THEN NULL ELSE ?3 END WHERE rule_key=?4", params![enabled as i64, priority, param_days, rule_key]).map_err(|e| e.to_string())?;
    if n == 0 { return Err("Rule not found".into()); }
    crate::audit::log(&conn, &me.display_name(), "rule_changed", "recommendation_rule", None, &format!("{rule_key}: enabled={enabled}, priority={priority}, days={param_days:?}"));
    Ok(())
}

pub fn recommendations(conn: &Connection) -> Result<Vec<Recommendation>, String> {
    let mut out = Vec::new();
    let t = today();
    let rules = Rules::load(conn)?;

    // ---- livestock batches
    let batches: Vec<i64> = rows(conn, "SELECT id FROM flocks WHERE status='active'", [], |r| r.get(0))?;
    for id in batches {
        let f = crate::flocks::summary(conn, id)?;
        let since_log = if f.last_log_date.is_empty() { days_since(&f.placed_date) } else { days_since(&f.last_log_date) };
        if let Some(d) = since_log.filter(|d| *d >= rules.days("no-log", 2)) {
            let why = if f.last_log_date.is_empty() { format!("{} was placed {d} days ago and has no daily record yet.", f.code) } else { format!("The last daily record for {} was {d} days ago ({}).", f.code, f.last_log_date) };
            out.push(rec(format!("no-log:{id}"), "Record keeping", format!("Record today's figures for {}", f.code),
                "Enter deaths, feed and any other figures for each day. Daily records keep the animal count, mortality and costs accurate.", why, "record_keeping", "daily-livestock-records", 2, "livestock", Some(id)));
        }
        // Deaths on each of the last three days: prompt to record observations and involve a vet.
        let recent: Vec<(String, i64)> = rows(conn, "SELECT log_date,SUM(mortality) FROM flock_logs WHERE flock_id=?1 AND log_date>=?2 GROUP BY log_date HAVING SUM(mortality)>0",
            params![id, (t - chrono::Duration::days(rules.days("deaths", 3) - 1)).format("%Y-%m-%d").to_string()], |r| Ok((r.get(0)?, r.get(1)?)))?;
        if recent.len() as i64 >= rules.days("deaths", 3) {
            let total: i64 = recent.iter().map(|(_, n)| n).sum();
            out.push(rec(format!("deaths:{id}"), "Health", format!("Deaths on {} days running in {}", recent.len(), f.code),
                "Write down what you have observed in Health records and contact a veterinary professional. FarmBuddy does not diagnose disease.",
                format!("{total} deaths were recorded in {} over the last {} days, with deaths on each day.", f.code, recent.len()), "professional_confirmation", "biosecurity-basics", 1, "health", Some(id)));
        }
        if f.group != "livestock" && f.alive > 0 && ["broiler", "cockerel", "noiler", "turkey", "catfish", "tilapia"].contains(&f.species.as_str()) {
            let last_weight: Option<String> = conn.query_row("SELECT MAX(log_date) FROM flock_logs WHERE flock_id=?1 AND avg_weight_kg>0", [id], |r| r.get(0)).map_err(|e| e.to_string())?;
            let age = days_since(&f.placed_date).unwrap_or(0);
            let stale = match &last_weight { Some(d) => days_since(d).unwrap_or(0) >= rules.days("weigh", 7), None => age >= rules.days("weigh", 7) };
            if stale {
                let why = match last_weight { Some(d) => format!("The last weighing for {} was on {d}.", f.code), None => format!("{} is {age} days old and has not been weighed yet.", f.code) };
                out.push(rec(format!("weigh:{id}"), "Feed", format!("Weigh a sample from {}", f.code),
                    "Weigh a few animals, work out the average and enter it in the daily record. FarmBuddy needs it to calculate the feed conversion ratio and cost per kg.", why, "record_keeping", "feed-conversion-ratio", 3, "livestock", Some(id)));
            }
        }
        let vacc: i64 = conn.query_row("SELECT COUNT(*) FROM vaccination_events WHERE flock_id=?1", [id], |r| r.get(0)).map_err(|e| e.to_string())?;
        if vacc == 0 && f.group == "poultry" {
            out.push(rec(format!("no-vacc:{id}"), "Health", format!("Add the vaccination record for {}", f.code),
                "Ask the hatchery what was given before delivery, and plan the rest of the schedule with a veterinary professional. Record each vaccination in FarmBuddy.",
                format!("{} has no vaccination record.", f.code), "professional_confirmation", "vaccination-records", 2, "health", Some(id)));
        }
    }

    // ---- vaccinations
    for v in crate::health::list_vacc_tx(conn, None, true)? {
        let (id, name, code, due) = (Some(v.id), v.vaccine_name.as_str(), v.flock_code.as_str(), v.due_date.as_str());
        match v.display_status.as_str() {
            "overdue" => out.push(rec(format!("vacc-overdue:{}", v.id), "Health", format!("{name} for {code} is overdue"),
                "Speak to your veterinary professional about what to do now, then record it as completed or skipped with the reason.",
                format!("It was planned for {due} and has not been marked as given."), "professional_confirmation", "vaccination-records", 1, "health", id)),
            "due" => out.push(rec(format!("vacc-due:{}", v.id), "Health", format!("{name} for {code} is due"),
                "Prepare the vaccine as instructed by your vet or the product label, and record it when it is given.",
                format!("It is planned for {due}."), "professional_confirmation", "vaccination-records", 2, "health", id)),
            "needs_review" => out.push(rec(format!("vacc-review:{}", v.id), "Health", format!("Confirm {name} for {code} with a vet"),
                "This plan came from a general reference. Ask a veterinary professional to confirm it for your animals, then record who confirmed it.",
                format!("The schedule source is marked 'pending review' (planned for {due})."), "vet_only", "vaccination-records", 2, "health", id)),
            _ => {}
        }
    }

    // ---- crops
    let cycles: Vec<i64> = rows(conn, "SELECT id FROM crop_cycles WHERE status='growing'", [], |r| r.get(0))?;
    for id in cycles {
        let c = crate::crops::cycle_summary(conn, id)?;
        let label = format!("{} on {}", c.crop, c.plot_code);
        let since = if c.last_activity_date.is_empty() { days_since(&c.planting_date) } else { days_since(&c.last_activity_date) };
        if let Some(d) = since.filter(|d| *d >= rules.days("crop-idle", 14)) {
            out.push(rec(format!("crop-idle:{id}"), "Record keeping", format!("Record field work on {label}"),
                "Record weeding, fertiliser, spraying and other work with its labour and input costs, so the cost of production stays complete.",
                format!("Nothing has been recorded on {label} for {d} days."), "record_keeping", "crop-records", 3, "crops", Some(id)));
        }
        if let Some(to) = parse_date(&c.expected_harvest_to) {
            if to < t && c.harvest_count == 0 {
                out.push(rec(format!("harvest-late:{id}"), "Planning", format!("Harvest window passed for {label}"),
                    "Record the harvest, or update the expected harvest dates if the crop needs more time.",
                    format!("The expected harvest window ended on {}, and no harvest is recorded.", c.expected_harvest_to), "planning", "crop-records", 2, "crops", Some(id)));
            }
        } else if let Some(from) = parse_date(&c.expected_harvest_from) {
            let d = (from - t).num_days();
            if (0..=rules.days("harvest-soon", 14)).contains(&d) {
                out.push(rec(format!("harvest-soon:{id}"), "Planning", format!("Harvest of {label} expected soon"),
                    "Plan labour, bags or crates, transport and storage, and line up buyers.", format!("You expected harvest to start on {}.", c.expected_harvest_from), "planning", "crop-records", 3, "crops", Some(id)));
            }
        }
    }

    // ---- processing
    let batches: Vec<(i64, String, String)> = rows(conn, "SELECT id,batch_no,start_date FROM processing_batches WHERE status='open'", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    for (id, no, start) in batches {
        let outputs: i64 = conn.query_row("SELECT COUNT(*) FROM processing_outputs WHERE batch_id=?1", [id], |r| r.get(0)).map_err(|e| e.to_string())?;
        if let Some(d) = days_since(&start).filter(|d| *d >= rules.days("proc-out", 3)) {
            if outputs == 0 {
                out.push(rec(format!("proc-out:{id}"), "Processing", format!("Record what batch {no} produced"),
                    "Enter the main products, by-products and waste, then close the batch to get the cost per unit and add the products to stock.",
                    format!("Batch {no} started {d} days ago and has no outputs recorded."), "record_keeping", "processing-batch-costing", 2, "processing", Some(id)));
            }
        }
    }

    // ---- inputs running low (feed, fertiliser, vaccines and other materials)
    let low: Vec<(i64, String, f64, String)> = rows(conn, "SELECT id,name,stock_qty,unit FROM products WHERE active=1 AND track_stock=1 AND kind='material' AND reorder_level>0 AND stock_qty<=reorder_level ORDER BY name LIMIT 10", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    for (id, name, qty, unit) in low {
        out.push(rec(format!("low:{id}"), "Inputs", format!("{name} is running low"), "Reorder before it runs out, or record a delivery in Purchases.",
            format!("Only {} {unit} left, at or below the reorder level you set.", crate::db::format_qty(qty)), "planning", "", 3, "inventory", Some(id)));
    }

    // ---- money owed by buyers
    let (owing, total): (i64, i64) = conn.query_row(&format!("SELECT COUNT(DISTINCT r.customer_id),COALESCE(SUM({}),0) FROM receipts r WHERE r.payment_status IN ('partial','unpaid') AND r.customer_id IS NOT NULL", crate::sales::OWED_SQL), [], |r| Ok((r.get(0)?, r.get(1)?))).map_err(|e| e.to_string())?;
    if owing > 0 && total > 0 {
        let sym = crate::db::setting(conn, "currency_symbol")?;
        out.push(rec("owed".into(), "Money", format!("{owing} buyer{} owe you money", if owing == 1 { "" } else { "s" }),
            "Follow up and record payments as they come in. Customer statements show what each buyer owes.",
            format!("{} is outstanding on credit sales.", crate::db::format_money(&sym, total)), "planning", "", 3, "customers", None));
    }

    // ---- births due
    let soon = (t + chrono::Duration::days(rules.days("births-due", 7))).format("%Y-%m-%d").to_string();
    let due: Vec<(i64, i64, String, String, String)> = rows(conn, "SELECT b.id,b.flock_id,f.code,b.expected_due,COALESCE(a.tag,'') FROM breeding_records b JOIN flocks f ON f.id=b.flock_id LEFT JOIN livestock_animals a ON a.id=b.dam_id
        WHERE b.status IN ('mated','pregnant') AND b.expected_due<>'' AND b.expected_due<=?1 AND f.status='active'", [soon], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
    for (id, flock, code, date, tag) in due {
        out.push(rec(format!("births-due:{id}"), "Breeding", format!("Birth expected in {code}{}", if tag.is_empty() { String::new() } else { format!(" ({tag})") }),
            "Prepare a clean, dry place for the birth, check on the animal often and have your vet's number ready. Record the birth when it happens.",
            format!("The expected due date is {date}."), "planning", "daily-livestock-records", 2, "livestock", Some(flock)));
    }
    // ---- fish ponds without a recent water check
    let ponds: Vec<(i64, String, String)> = rows(conn, "SELECT p.id,p.code,COALESCE((SELECT MAX(record_date) FROM water_quality_records w WHERE w.pond_id=p.id),'') FROM fish_ponds p
        WHERE p.status='active' AND EXISTS(SELECT 1 FROM flocks f WHERE f.pond_id=p.id AND f.status='active')", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    for (id, code, last) in ponds {
        let stale = if last.is_empty() { true } else { days_since(&last).unwrap_or(0) >= rules.days("water", 7) };
        if stale {
            out.push(rec(format!("water:{id}"), "Fish", format!("Check the water in pond {code}"),
                "Measure what you can (temperature, pH, oxygen) and record the readings. FarmBuddy never treats a missing reading as safe.",
                if last.is_empty() { format!("Pond {code} has fish but no water-quality record.") } else { format!("The last water record for pond {code} was on {last}.") }, "general", "", 3, "livestock", None));
        }
    }
    // ---- feed stock against recent use
    let feed_used_7d: f64 = conn.query_row("SELECT COALESCE(SUM(l.feed_kg),0) FROM flock_logs l JOIN flocks f ON f.id=l.flock_id WHERE f.status='active' AND l.log_date>=?1",
        [(t - chrono::Duration::days(7)).format("%Y-%m-%d").to_string()], |r| r.get(0)).map_err(|e| e.to_string())?;
    if feed_used_7d > 0.0 {
        let feed_kg: f64 = conn.query_row("SELECT COALESCE(SUM(p.stock_qty),0) FROM products p LEFT JOIN categories c ON c.id=p.category_id WHERE p.active=1 AND p.track_stock=1 AND lower(p.unit)='kg' AND (lower(COALESCE(c.name,'')) LIKE '%feed%' OR lower(p.name) LIKE '%feed%' OR lower(p.name) LIKE '%mash%' OR lower(p.name) LIKE '%finisher%' OR lower(p.name) LIKE '%starter%')", [], |r| r.get(0)).map_err(|e| e.to_string())?;
        let per_day = feed_used_7d / 7.0;
        let days_left = feed_kg / per_day;
        if days_left < rules.days("feed-days", 7) as f64 {
            out.push(rec("feed-days".into(), "Feed", format!("Feed in stock lasts about {} day{}", days_left.floor() as i64, if days_left.floor() as i64 == 1 { "" } else { "s" }),
                "Order feed now so the animals' ration does not change suddenly. Record the delivery in Purchases.",
                format!("{} kg of feed in stock (items in kg named or categorised as feed); about {} kg used per day over the last 7 days.", crate::db::format_qty(feed_kg.round()), crate::db::format_qty(per_day.round())),
                "planning", "feed-conversion-ratio", 2, "inventory", None));
        }
    }
    // ---- crop health observations awaiting confirmation
    let ch: Vec<(i64, i64, String, String, String)> = rows(conn, "SELECT h.id,h.cycle_id,c.crop,p.code,h.observation FROM crop_health_records h JOIN crop_cycles c ON c.id=h.cycle_id JOIN farm_plots p ON p.id=c.plot_id WHERE h.review_status='unconfirmed' AND c.status='growing' ORDER BY h.observed_date DESC LIMIT 10", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
    for (id, cycle, crop, plot, obs) in ch {
        out.push(rec(format!("crop-health:{id}"), "Crop health", format!("Confirm the problem on {crop} ({plot})"),
            "Show the affected plants to an extension officer or agronomist before treating. If a product is used, follow its registered label.",
            format!("You recorded: \"{obs}\". It has not been confirmed yet."), "professional_confirmation", "pesticide-labels", 2, "crops", Some(cycle)));
    }

    // ---- oil palm runs
    let runs: Vec<i64> = rows(conn, "SELECT id FROM palm_runs WHERE status='open'", [], |r| r.get(0))?;
    for id in runs {
        let r = crate::palm_oil::run_detail(conn, id)?.run;
        let age = days_since(&r.start_date).unwrap_or(0);
        let money = |k: i64| format!("₦{}", crate::db::format_qty(((k as f64) / 100.0).round()));
        if r.total_cost_kobo > 0 && r.oil_made > 0.0 && r.projected_profit_kobo < 0 {
            let be = r.break_even_oil_price_kobo.map(|b| format!(" To cover every cost, each {} must sell for at least {}.", r.oil_unit, money(b))).unwrap_or_default();
            out.push(rec(format!("palm-loss:{id}"), "Money", format!("{} run {} is heading for a loss", r.product_label, r.run_no),
                "Check the price you expect to sell at, and the costs you can reduce next time (buying price, transport, mill fee, workers).",
                format!("Costs so far {}; sales {} plus what is not sold yet, valued at {} (your prices now).{be}", money(r.total_cost_kobo), money(r.total_sales_kobo), money(r.unsold_value_kobo)),
                "planning", "", 1, "palm-oil", Some(id)));
        }
        if r.oil_unsold > 0.0 && age >= rules.days("palm-unsold", 21) {
            out.push(rec(format!("palm-unsold:{id}"), "Selling", format!("{} {} of {} from {} not sold yet", crate::db::format_qty(r.oil_unsold), r.oil_unit, r.main_label.to_lowercase(), r.run_no),
                "Update the run with any sales you have made. It is also in Farm stock, so you can sell it from Sell produce.",
                format!("The run started {age} days ago; {} {} made, {} sold.", crate::db::format_qty(r.oil_made), r.oil_unit, crate::db::format_qty(r.oil_sold)),
                "planning", "", 3, "palm-oil", Some(id)));
        }
        let fruit_logged = !r.fruit.is_empty() || !r.fruit_own.is_empty();
        if fruit_logged && r.oil_made == 0.0 && age >= rules.days("palm-output", 5) {
            out.push(rec(format!("palm-output:{id}"), "Record keeping", format!("Record what came out of {}", r.run_no),
                "Open the run and answer how much you got, so FarmBuddy can work out your cost per unit and profit.",
                format!("The {} was recorded {age} days ago and no {} has been recorded yet.", r.raw_label, r.main_label.to_lowercase()), "record_keeping", "", 2, "palm-oil", Some(id)));
        }
    }

    // Apply the owner's rule settings: switched-off rules disappear and priorities follow the settings.
    out.retain(|r| rules.on(r.key.split(':').next().unwrap_or("")));
    for r in out.iter_mut() { let k = r.key.split(':').next().unwrap_or("").to_string(); r.priority = rules.prio(&k, r.priority); }
    out.sort_by(|a, b| a.priority.cmp(&b.priority).then(a.title.cmp(&b.title)));
    Ok(out)
}

#[tauri::command]
pub fn get_recommendations(state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<Recommendation>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    recommendations(&*state.db()?)
}

// ------------------------------------------------------------------ reminders

#[derive(Serialize, Debug, Clone)]
pub struct Reminder { pub id: i64, pub title: String, pub due_date: String, pub related_kind: String, pub related_id: Option<i64>, pub source: String, pub status: String, pub notes: String, pub is_demo: bool, pub overdue: bool, pub user_name: String }

fn reminder_rows(conn: &Connection, sql: &str, p: impl rusqlite::Params) -> Result<Vec<Reminder>, String> {
    let t = today_str();
    rows(conn, sql, p, |r| {
        let due: String = r.get(2)?;
        let status: String = r.get(6)?;
        Ok(Reminder { id: r.get(0)?, title: r.get(1)?, overdue: status == "open" && due < t, due_date: due, related_kind: r.get(3)?, related_id: r.get(4)?, source: r.get(5)?, status, notes: r.get(7)?, is_demo: r.get::<_, i64>(8)? == 1, user_name: r.get(9)? })
    })
}

const REM_COLS: &str = "SELECT id,title,due_date,related_kind,related_id,source,status,notes,is_demo,user_name FROM farm_reminders";

/// Open reminders due within `days` days, overdue ones first.
pub fn open_reminders(conn: &Connection, days: i64) -> Result<Vec<Reminder>, String> {
    let until = (today() + chrono::Duration::days(days)).format("%Y-%m-%d").to_string();
    reminder_rows(conn, &format!("{REM_COLS} WHERE status='open' AND due_date<=?1 ORDER BY due_date, id LIMIT 50"), [until])
}

#[tauri::command]
pub fn list_reminders(state: tauri::State<'_, AppState>, session_token: String, include_done: Option<bool>) -> Result<Vec<Reminder>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    let conn = state.db()?;
    reminder_rows(&conn, &format!("{REM_COLS} WHERE ?1=1 OR status='open' ORDER BY CASE status WHEN 'open' THEN 0 ELSE 1 END, due_date, id LIMIT 500"), [include_done.unwrap_or(false) as i64])
}

#[derive(Deserialize, Default)]
pub struct ReminderInput { pub id: Option<i64>, pub title: String, pub due_date: String, #[serde(default)] pub related_kind: String, #[serde(default)] pub related_id: Option<i64>, #[serde(default)] pub source: String, #[serde(default)] pub notes: String }

pub fn save_reminder_tx(conn: &Connection, i: &ReminderInput, who: &str) -> Result<i64, String> {
    if i.title.trim().is_empty() { return Err("Say what to do".into()); }
    check_len(&i.title, 200, "Reminder")?;
    check_len(&i.notes, 500, "Notes")?;
    if !valid_date(&i.due_date) { return Err("Choose the date".into()); }
    if !["", "livestock", "health", "crops", "processing", "inventory", "customers"].contains(&i.related_kind.as_str()) { return Err("Unknown link".into()); }
    let source = if i.source.is_empty() { "manual" } else { i.source.as_str() };
    if !["manual", "advisory", "vaccination"].contains(&source) { return Err("Unknown reminder source".into()); }
    match i.id {
        Some(id) => {
            let n = conn.execute("UPDATE farm_reminders SET title=?1,due_date=?2,notes=?3 WHERE id=?4", params![i.title.trim(), i.due_date, i.notes.trim(), id]).map_err(|e| e.to_string())?;
            if n == 0 { return Err("Reminder not found".into()); }
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO farm_reminders(title,due_date,related_kind,related_id,source,notes,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![i.title.trim(), i.due_date, i.related_kind, i.related_id, source, i.notes.trim(), who]).map_err(|e| e.to_string())?;
            Ok(conn.last_insert_rowid())
        }
    }
}

#[tauri::command]
pub fn save_reminder(state: tauri::State<'_, AppState>, session_token: String, input: ReminderInput) -> Result<i64, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    save_reminder_tx(&*state.db()?, &input, &me.display_name())
}

#[tauri::command]
pub fn set_reminder_status(state: tauri::State<'_, AppState>, session_token: String, id: i64, status: String) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Worker)?;
    if !["open", "done", "dismissed"].contains(&status.as_str()) { return Err("Unknown status".into()); }
    let done_at = if status == "open" { String::new() } else { crate::db::now_local() };
    let n = state.db()?.execute("UPDATE farm_reminders SET status=?1,done_at=?2 WHERE id=?3", params![status, done_at, id]).map_err(|e| e.to_string())?;
    if n == 0 { return Err("Reminder not found".into()); }
    Ok(())
}

#[tauri::command]
pub fn delete_reminder(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let n = state.db()?.execute("DELETE FROM farm_reminders WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    if n == 0 { return Err("Reminder not found".into()); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ago(d: i64) -> String { (today() - chrono::Duration::days(d)).format("%Y-%m-%d").to_string() }

    #[test]
    fn suggestions_explain_themselves() {
        let conn = crate::db::test_conn();
        conn.execute("INSERT INTO flocks(code,placed_date,chicks_placed,species) VALUES('BR-1',?1,500,'broiler')", [ago(10)]).unwrap();
        for d in 0..3 { conn.execute("INSERT INTO flock_logs(flock_id,log_date,mortality,feed_kg) VALUES(1,?1,2,40)", [ago(d)]).unwrap(); }
        let recs = recommendations(&conn).unwrap();
        let keys: Vec<&str> = recs.iter().map(|r| r.key.as_str()).collect();
        assert!(keys.contains(&"deaths:1"), "{keys:?}");
        assert!(keys.contains(&"weigh:1"));
        assert!(keys.contains(&"no-vacc:1"));
        assert!(!keys.contains(&"no-log:1"), "logged today");
        for r in &recs { assert!(!r.why.is_empty() && !r.text.is_empty()); }
        // Highest priority first.
        assert_eq!(recs[0].key, "deaths:1");
        let deaths = recs.iter().find(|r| r.key == "deaths:1").unwrap();
        assert_eq!(deaths.safety_level, "professional_confirmation");
        assert!(deaths.why.contains("6 deaths"));
    }

    #[test]
    fn idle_crop_and_unfinished_processing() {
        let conn = crate::db::test_conn();
        conn.execute("INSERT INTO farm_plots(code,area) VALUES('P1',1)", []).unwrap();
        conn.execute("INSERT INTO crop_cycles(plot_id,crop,planting_date,expected_harvest_to) VALUES(1,'Maize',?1,?2)", [ago(40), ago(1)]).unwrap();
        conn.execute("INSERT INTO processing_batches(batch_no,process_type,start_date) VALUES('PB-1','palm_oil',?1)", [ago(5)]).unwrap();
        let keys: Vec<String> = recommendations(&conn).unwrap().into_iter().map(|r| r.key).collect();
        assert!(keys.contains(&"crop-idle:1".to_string()));
        assert!(keys.contains(&"harvest-late:1".to_string()));
        assert!(keys.contains(&"proc-out:1".to_string()));
    }

    #[test]
    fn reminders() {
        let conn = crate::db::test_conn();
        save_reminder_tx(&conn, &ReminderInput { title: "Buy feed".into(), due_date: ago(1), ..Default::default() }, "W").unwrap();
        save_reminder_tx(&conn, &ReminderInput { title: "Later".into(), due_date: (today() + chrono::Duration::days(30)).format("%Y-%m-%d").to_string(), ..Default::default() }, "W").unwrap();
        assert!(save_reminder_tx(&conn, &ReminderInput { title: "x".into(), due_date: "soon".into(), ..Default::default() }, "W").is_err());
        let due = open_reminders(&conn, 7).unwrap();
        assert_eq!(due.len(), 1);
        assert!(due[0].overdue);
    }
}
