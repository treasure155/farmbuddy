//! Livestock batches with plain questions, from intake to sale.
//!
//! The farm manager answers simple questions; FarmBuddy works out everything else:
//! live birds and mortality, the feed plan (the farm's bags per 100 birds, 17 × 25 kg
//! by default, always changeable), the feed store with days left and restocking
//! alerts, the vaccination timetable, medicine withdrawal (sale is blocked until it
//! ends), weighing, FCR, expenses (approved by the owner), sales with receipts and
//! customer debts, and the actual and projected profit or loss of every batch.
//!
//! Nothing is assumed as fact: the feed plan is a budget compared with what was
//! really used, the timetable needs a vet's confirmation, and withdrawal periods
//! come only from the product label or the vet.

use crate::{auth::{self, Role, Session}, farms::{check_len, check_money, check_qty, rows, valid_date}, AppState};
use chrono::{Duration, Local, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

fn d(s: &str) -> Option<NaiveDate> { NaiveDate::parse_from_str(s, "%Y-%m-%d").ok() }
fn today() -> String { Local::now().format("%Y-%m-%d").to_string() }
fn plus(s: &str, days: i64) -> String { d(s).map(|x| (x + Duration::days(days)).format("%Y-%m-%d").to_string()).unwrap_or_default() }
fn e(x: rusqlite::Error) -> String { x.to_string() }

// ------------------------------------------------------------------ settings (owner can change)

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Limits { pub warn_pct: f64, pub low_pct: f64, pub critical_days: f64, pub mortality_pct: f64, pub weigh_days: i64 }

pub fn limits(conn: &Connection) -> Limits {
    let g = |k: &str, def: f64| crate::db::setting(conn, k).ok().and_then(|v| v.trim().parse::<f64>().ok()).unwrap_or(def);
    Limits { warn_pct: g("pb_warn_pct", 50.0), low_pct: g("pb_low_pct", 25.0), critical_days: g("pb_critical_days", 3.0), mortality_pct: g("pb_mortality_pct", 0.0), weigh_days: g("pb_weigh_days", 7.0) as i64 }
}

// ------------------------------------------------------------------ 1–2. the batch: setup and intake

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
#[serde(default)]
pub struct BatchForm {
    pub id: Option<i64>, pub farm_id: Option<i64>, pub name: String, pub bird_type: String, pub breed: String, pub house: String, pub manager: String,
    pub planned_birds: i64, pub arrival_date: String, pub age_at_arrival: i64, pub weeks: i64, pub sell_age_weeks: i64, pub expected_sell: i64,
    pub expected_weight_kg: f64, pub price_per: String, pub expected_price_kobo: i64, pub budget_kobo: i64,
    pub bags_per_100: f64, pub bag_kg: f64, pub plan_starter: f64, pub plan_grower: f64, pub plan_finisher: f64, pub feed_change: String,
    /// Share of the feed plan that is starter (the farm's practical split is 30% starter, 70% finisher) and how many weeks the starter lasts.
    pub starter_pct: f64, pub starter_weeks: i64,
    pub chicks_ordered: i64, pub chicks_delivered: i64, pub dead_on_arrival: i64, pub chicks_paid_for: i64, pub chick_price_kobo: i64, pub chick_transport_kobo: i64,
    pub chick_supplier: String, pub chick_amount_paid_kobo: i64, pub supplier_receipt: bool, pub hatchery_vaccines: String, pub notes: String,
    /// Add the broiler day 1–56 planning timetable (vet confirmation required).
    pub add_timetable: bool,
}

/// The farm's broiler planning calendar (bird age from–to, activity, purpose).
pub const BROILER_TIMETABLE: &[(i64, i64, &str, &str)] = &[
    (1, 1, "Confirm Marek's vaccine at hatchery", "Marek's disease protection"),
    (1, 1, "Confirm hatchery Newcastle/IB vaccination, if given", "Early protection"),
    (1, 3, "Water and appropriate electrolytes if needed", "Transport recovery"),
    (4, 6, "Vitamins if required", "Nutritional support"),
    (7, 7, "Newcastle vaccine, such as Lasota or approved alternative", "Newcastle disease"),
    (10, 14, "First Gumboro vaccine", "Infectious Bursal Disease"),
    (17, 21, "Gumboro booster if required", "IBD protection"),
    (18, 21, "Newcastle booster according to vet's programme", "Newcastle disease"),
    (22, 28, "Monitor growth, health and droppings", "Health assessment"),
    (28, 35, "Review vaccination and medication records", "Check programme compliance"),
    (35, 42, "Additional vaccination only if indicated", "Local disease-risk management"),
    (43, 49, "Weigh birds and prepare sales plan", "Marketing preparation"),
    (50, 56, "Final weighing and sale preparation", "Harvest"),
];

pub fn save_batch(conn: &Connection, f: &BatchForm, who: &str) -> Result<i64, String> {
    let name = f.name.trim();
    if name.is_empty() { return Err("What is the name or number of this batch?".into()); }
    check_len(name, 80, "Batch name")?;
    for (s, n) in [(&f.bird_type, "Type"), (&f.breed, "Breed"), (&f.house, "House"), (&f.manager, "Manager"), (&f.chick_supplier, "Supplier")] { check_len(s, 120, n)?; }
    check_len(&f.notes, 1000, "Notes")?; check_len(&f.hatchery_vaccines, 500, "Hatchery vaccines")?; check_len(&f.feed_change, 300, "Feed change")?;
    if !valid_date(&f.arrival_date) { return Err("When do (did) the chicks arrive? Choose the date.".into()); }
    if f.planned_birds <= 0 { return Err("How many birds do you intend to rear?".into()); }
    for (v, n) in [(f.planned_birds, "Birds"), (f.chicks_ordered, "Chicks ordered"), (f.chicks_delivered, "Chicks delivered"), (f.dead_on_arrival, "Dead on arrival"), (f.chicks_paid_for, "Chicks bought"), (f.expected_sell, "Birds to sell")] {
        if !(0..=10_000_000).contains(&v) { return Err(format!("{n}: enter a number from 0")); }
    }
    for (k, n) in [(f.chick_price_kobo, "Price per chick"), (f.chick_transport_kobo, "Transport"), (f.chick_amount_paid_kobo, "Amount paid"), (f.expected_price_kobo, "Selling price"), (f.budget_kobo, "Budget")] { check_money(k, n)?; }
    for (q, n) in [(f.bags_per_100, "Bags per 100 birds"), (f.plan_starter, "Starter bags"), (f.plan_grower, "Grower bags"), (f.plan_finisher, "Finisher bags"), (f.expected_weight_kg, "Selling weight")] { check_qty(q, n)?; }
    if !(f.bag_kg > 0.0 && f.bag_kg <= 1000.0) { return Err("What is the weight of one bag of feed (kg)?".into()); }
    if f.dead_on_arrival > f.chicks_delivered { return Err("Dead on arrival cannot be more than the chicks delivered.".into()); }
    if !(0.0..=100.0).contains(&f.starter_pct) { return Err("Starter share must be from 0 to 100%".into()); }
    let weeks_total = f.weeks.max(1);
    if f.starter_weeks < 0 || f.starter_weeks > weeks_total { return Err(format!("Starter weeks must be from 0 to {weeks_total}")); }
    // The plan in whole bags: starter = share of the total (rounded), finisher = the rest.
    let total_bags = (f.planned_birds as f64 / 100.0 * f.bags_per_100).round();
    let (starter_bags, finisher_bags) = if f.plan_starter + f.plan_finisher > 0.0 { (f.plan_starter, f.plan_finisher) } else { let st = (total_bags * f.starter_pct / 100.0).round(); (st, (total_bags - st).max(0.0)) };
    let f = &BatchForm { plan_starter: starter_bags, plan_finisher: finisher_bags, plan_grower: 0.0, ..f.clone() };
    let price_per = if f.price_per == "kg" { "kg" } else { "bird" };
    let bird_type = if f.bird_type.trim().is_empty() { "Broilers" } else { f.bird_type.trim() };
    let vals = params![f.farm_id, name, bird_type, f.breed.trim(), f.house.trim(), f.manager.trim(), f.planned_birds, f.arrival_date, f.age_at_arrival.max(0), f.weeks.max(0), f.sell_age_weeks.max(0),
        f.expected_sell, f.expected_weight_kg, price_per, f.expected_price_kobo, f.budget_kobo, f.bags_per_100, f.bag_kg, f.plan_starter, f.plan_grower, f.plan_finisher, f.feed_change.trim(),
        f.chicks_ordered, f.chicks_delivered, f.dead_on_arrival, f.chicks_paid_for, f.chick_price_kobo, f.chick_transport_kobo, f.chick_supplier.trim(), f.chick_amount_paid_kobo, f.supplier_receipt as i64,
        f.hatchery_vaccines.trim(), f.notes.trim(), f.starter_pct, f.starter_weeks];
    const COLS: &str = "farm_id,name,bird_type,breed,house,manager,planned_birds,arrival_date,age_at_arrival,weeks,sell_age_weeks,expected_sell,expected_weight_kg,price_per,expected_price_kobo,budget_kobo,bags_per_100,bag_kg,plan_starter,plan_grower,plan_finisher,feed_change,chicks_ordered,chicks_delivered,dead_on_arrival,chicks_paid_for,chick_price_kobo,chick_transport_kobo,chick_supplier,chick_amount_paid_kobo,supplier_receipt,hatchery_vaccines,notes,starter_pct,starter_weeks";
    let id = match f.id {
        Some(id) => {
            let set: String = COLS.split(',').enumerate().map(|(i, c)| format!("{c}=?{}", i + 1)).collect::<Vec<_>>().join(",");
            let n = conn.execute(&format!("UPDATE pb_batches SET {set} WHERE id={id}"), vals).map_err(e)?;
            if n == 0 { return Err("Batch not found".into()); }
            crate::audit::log(conn, who, "batch_edited", "pb_batch", Some(id), &format!("Changed batch {name}"));
            id
        }
        None => {
            let marks: String = (1..=COLS.split(',').count()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(",");
            conn.execute(&format!("INSERT INTO pb_batches({COLS}) VALUES({marks})"), vals).map_err(e)?;
            let id = conn.last_insert_rowid();
            conn.execute("UPDATE pb_batches SET created_by=?1 WHERE id=?2", params![who, id]).map_err(e)?;
            if f.add_timetable {
                for (a, b, what, why) in BROILER_TIMETABLE {
                    conn.execute("INSERT INTO pb_health(batch_id,kind,name,purpose,due_from,due_to,status,user_name) VALUES(?1,'plan',?2,?3,?4,?5,'planned',?6)",
                        params![id, what, why, plus(&f.arrival_date, a - f.age_at_arrival.max(0)), plus(&f.arrival_date, b - f.age_at_arrival.max(0)), who]).map_err(e)?;
                }
            }
            crate::audit::log(conn, who, "batch_created", "pb_batch", Some(id), &format!("New batch {name}: {} birds planned", f.planned_birds));
            // So the birds can also be sold from "Sell produce".
            let item = bird_item(conn, "bird")?;
            if price_per == "bird" && f.expected_price_kobo > 0 { conn.execute("UPDATE products SET selling_price_kobo=?1 WHERE id=?2", params![f.expected_price_kobo, item]).map_err(e)?; }
            id
        }
    };
    Ok(id)
}

// ------------------------------------------------------------------ 3. the feed store

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct FeedForm { pub id: Option<i64>, pub farm_id: Option<i64>, pub name: String, pub brand: String, pub bag_kg: f64, pub min_bags: f64, pub full_bags: f64, pub opening_bags: f64, pub opening_price_kobo: i64 }

pub fn save_feed(conn: &Connection, f: &FeedForm, who: &str) -> Result<i64, String> {
    let name = f.name.trim();
    if name.is_empty() { return Err("Which feed is this? (e.g. Broiler starter)".into()); }
    check_len(name, 80, "Feed")?; check_len(&f.brand, 80, "Brand")?;
    if !(f.bag_kg > 0.0 && f.bag_kg <= 1000.0) { return Err("What is the weight of one bag (kg)?".into()); }
    for (q, n) in [(f.min_bags, "Restock level"), (f.full_bags, "Full store"), (f.opening_bags, "Bags in store now")] { check_qty(q, n)?; }
    check_money(f.opening_price_kobo, "Price per bag")?;
    let id = match f.id {
        Some(id) => { conn.execute("UPDATE pb_feeds SET name=?1,brand=?2,bag_kg=?3,min_bags=?4,full_bags=?5 WHERE id=?6", params![name, f.brand.trim(), f.bag_kg, f.min_bags, f.full_bags, id]).map_err(e)?; id }
        None => {
            conn.execute("INSERT INTO pb_feeds(farm_id,name,brand,bag_kg,min_bags,full_bags) VALUES(?1,?2,?3,?4,?5,?6)", params![f.farm_id, name, f.brand.trim(), f.bag_kg, f.min_bags, f.full_bags]).map_err(e)?;
            let id = conn.last_insert_rowid();
            if f.opening_bags > 0.0 {
                conn.execute("INSERT INTO pb_feed_moves(feed_id,move_date,kind,bags,price_kobo,amount_kobo,paid_kobo,user_name) VALUES(?1,?2,'opening',?3,?4,?5,?5,?6)",
                    params![id, today(), f.opening_bags, f.opening_price_kobo, (f.opening_bags * f.opening_price_kobo as f64).round() as i64, who]).map_err(e)?;
            }
            id
        }
    };
    Ok(id)
}

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct FeedMoveForm {
    pub feed_id: i64, pub move_date: String, /** purchase | damaged | returned */ pub kind: String, pub bags: f64, pub price_kobo: i64, pub paid_kobo: i64,
    pub supplier: String, pub good_condition: bool, pub has_receipt: bool, pub batch_id: Option<i64>, pub note: String,
}

pub fn feed_move(conn: &Connection, f: &FeedMoveForm, who: &str) -> Result<i64, String> {
    if !["purchase", "damaged", "returned"].contains(&f.kind.as_str()) { return Err("Choose what happened to the feed".into()); }
    if !valid_date(&f.move_date) { return Err("Choose the date".into()); }
    check_qty(f.bags, "Bags")?;
    if f.bags <= 0.0 { return Err("How many bags?".into()); }
    check_money(f.price_kobo, "Price")?; check_money(f.paid_kobo, "Paid")?; check_len(&f.supplier, 120, "Supplier")?; check_len(&f.note, 300, "Note")?;
    if f.kind == "purchase" && f.price_kobo <= 0 { return Err("How much did each bag cost?".into()); }
    if f.kind == "damaged" { let s = feed_stock(conn, f.feed_id)?; if f.bags > s.available + 1e-9 { return Err(format!("Only {} bags are in the store.", crate::db::format_qty(s.available))); } }
    let amount = if f.kind == "purchase" { (f.bags * f.price_kobo as f64).round() as i64 } else { 0 };
    conn.execute("INSERT INTO pb_feed_moves(feed_id,move_date,kind,bags,price_kobo,amount_kobo,paid_kobo,supplier,good_condition,has_receipt,batch_id,note,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![f.feed_id, f.move_date, f.kind, f.bags, f.price_kobo, amount, f.paid_kobo.min(amount), f.supplier.trim(), f.good_condition as i64, f.has_receipt as i64, f.batch_id, f.note.trim(), who]).map_err(e)?;
    Ok(conn.last_insert_rowid())
}

#[derive(Serialize, Debug, Default, Clone)]
pub struct FeedStock {
    pub id: i64, pub name: String, pub brand: String, pub bag_kg: f64, pub min_bags: f64, pub full_bags: f64,
    pub opening: f64, pub purchased: f64, pub issued: f64, pub damaged: f64, pub returned: f64, pub available: f64,
    pub avg_price_kobo: i64, pub value_kobo: i64, pub owed_kobo: i64, pub last_supplier: String, pub last_price_kobo: i64,
    pub daily_bags: f64, pub days_left: Option<f64>, pub finish_date: String, pub buy_bags: f64,
    /// The same in kg: what is in the store and what is used a day.
    pub available_kg: f64, pub daily_kg: f64,
    /// normal | reducing | restock | urgent | finished
    pub level: String, pub message: String,
}

pub fn feed_stock(conn: &Connection, id: i64) -> Result<FeedStock, String> {
    let lim = limits(conn);
    let mut s = conn.query_row("SELECT id,name,brand,bag_kg,min_bags,full_bags FROM pb_feeds WHERE id=?1", [id], |r| Ok(FeedStock {
        id: r.get(0)?, name: r.get(1)?, brand: r.get(2)?, bag_kg: r.get(3)?, min_bags: r.get(4)?, full_bags: r.get(5)?, ..Default::default() })).optional().map_err(e)?.ok_or("Feed not found")?;
    let (o, p, i, dm, rt, amt, bought, owed): (f64, f64, f64, f64, f64, i64, f64, i64) = conn.query_row("SELECT
        COALESCE(SUM(CASE WHEN kind='opening' THEN bags END),0), COALESCE(SUM(CASE WHEN kind='purchase' THEN bags END),0), COALESCE(SUM(CASE WHEN kind='issue' THEN bags END),0),
        COALESCE(SUM(CASE WHEN kind='damaged' THEN bags END),0), COALESCE(SUM(CASE WHEN kind='returned' THEN bags END),0),
        COALESCE(SUM(CASE WHEN kind IN ('opening','purchase') THEN amount_kobo END),0), COALESCE(SUM(CASE WHEN kind IN ('opening','purchase') AND price_kobo>0 THEN bags END),0),
        COALESCE(SUM(CASE WHEN kind='purchase' THEN amount_kobo-paid_kobo END),0) FROM pb_feed_moves WHERE feed_id=?1", [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?))).map_err(e)?;
    s.opening = o; s.purchased = p; s.issued = i; s.damaged = dm; s.returned = rt; s.owed_kobo = owed;
    s.available = ((o + p + rt - i - dm) * 1000.0).round() / 1000.0;
    s.available_kg = (s.available * s.bag_kg * 10.0).round() / 10.0;
    s.avg_price_kobo = if bought > 0.0 { (amt as f64 / bought).round() as i64 } else { 0 };
    s.value_kobo = (s.available.max(0.0) * s.avg_price_kobo as f64).round() as i64;
    if let Some((sup, pr)) = conn.query_row("SELECT supplier,price_kobo FROM pb_feed_moves WHERE feed_id=?1 AND kind='purchase' ORDER BY move_date DESC,id DESC LIMIT 1", [id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))).optional().map_err(e)? {
        s.last_supplier = sup; s.last_price_kobo = pr;
    }
    // Average daily use from the last 7 days that have issues.
    let (bags, days): (f64, i64) = conn.query_row("SELECT COALESCE(SUM(bags),0), COUNT(DISTINCT move_date) FROM pb_feed_moves WHERE feed_id=?1 AND kind='issue' AND move_date >= date(?2,'-6 day')",
        params![id, today()], |r| Ok((r.get(0)?, r.get(1)?))).map_err(e)?;
    if days > 0 && bags > 0.0 {
        s.daily_bags = bags / days as f64;
        s.daily_kg = (s.daily_bags * s.bag_kg * 10.0).round() / 10.0;
        let left = s.available.max(0.0) / s.daily_bags;
        s.days_left = Some((left * 10.0).floor() / 10.0);
        s.finish_date = plus(&today(), left.floor() as i64);
    }
    if s.full_bags > 0.0 { s.buy_bags = (s.full_bags - s.available).max(0.0).ceil(); }
    let pct = if s.full_bags > 0.0 { s.available / s.full_bags * 100.0 } else { 100.0 };
    let (level, msg) = if s.available <= 1e-9 { ("finished", "Feed finished. Buy feed now.".to_string()) }
        else if s.min_bags > 0.0 && s.available <= s.min_bags { ("urgent", format!("Urgent restocking: only {} bags left (your minimum is {}).", crate::db::format_qty(s.available), crate::db::format_qty(s.min_bags))) }
        else if s.full_bags > 0.0 && pct <= lim.low_pct { ("restock", format!("Restocking required: {}% left.", pct.round())) }
        else if s.full_bags > 0.0 && pct <= lim.warn_pct { ("reducing", format!("Feed stock reducing: {}% left.", pct.round())) }
        else { ("normal", "Normal stock".to_string()) };
    s.level = level.into(); s.message = msg;
    if let Some(dl) = s.days_left {
        if s.available > 1e-9 && dl <= lim.critical_days {
            s.level = "urgent".into();
            s.message = format!("Critical: feed finishes in about {} day{}. Buy more now.", crate::db::format_qty(dl), if dl == 1.0 { "" } else { "s" });
        }
    }
    Ok(s)
}

pub fn feeds(conn: &Connection) -> Result<Vec<FeedStock>, String> {
    let ids: Vec<i64> = rows(conn, "SELECT id FROM pb_feeds WHERE active=1 ORDER BY id", [], |r| r.get(0))?;
    ids.into_iter().map(|id| feed_stock(conn, id)).collect()
}

// ------------------------------------------------------------------ 5. daily record (birds and feed)

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct DailyForm {
    pub batch_id: i64, pub day: String, pub deaths: i64, pub cause: String, pub disposed: bool, pub sick: i64, pub removed: i64, pub added: i64,
    pub feed_id: Option<i64>, /** kg taken from the store to the birds */ pub kg_given: f64, pub bags_issued: f64, pub kg_eaten: f64, pub kg_wasted: f64, pub water_l: f64,
    pub feed_unusual: String, pub observations: String, pub problems: String, pub tomorrow: String,
}

pub fn save_daily(conn: &Connection, f: &DailyForm, who: &str) -> Result<i64, String> {
    if !valid_date(&f.day) { return Err("Choose the day".into()); }
    if f.day > today() { return Err("You cannot record a day that has not come yet.".into()); }
    for (v, n) in [(f.deaths, "Deaths"), (f.sick, "Sick birds"), (f.removed, "Birds removed"), (f.added, "Birds added")] { if !(0..=10_000_000).contains(&v) { return Err(format!("{n}: enter 0 or more")); } }
    for (q, n) in [(f.kg_given, "Kg given"), (f.bags_issued, "Bags"), (f.kg_eaten, "Kg eaten"), (f.kg_wasted, "Kg wasted"), (f.water_l, "Water")] { check_qty(q, n)?; }
    // Feed is counted in kg; the store keeps bags of its own size.
    let bag_kg: f64 = match f.feed_id { Some(fid) => conn.query_row("SELECT bag_kg FROM pb_feeds WHERE id=?1", [fid], |r| r.get(0)).optional().map_err(e)?.ok_or("Feed not found")?, None => 25.0 };
    let kg_given = if f.kg_given > 0.0 { f.kg_given } else { f.bags_issued * bag_kg };
    let f = &DailyForm { kg_given, bags_issued: kg_given / bag_kg, ..f.clone() };
    for (s, n) in [(&f.cause, "Cause"), (&f.feed_unusual, "Feed"), (&f.observations, "Observations"), (&f.problems, "Problems"), (&f.tomorrow, "Tomorrow")] { check_len(s, 500, n)?; }
    let (status, arrival): (String, String) = conn.query_row("SELECT status,arrival_date FROM pb_batches WHERE id=?1", [f.batch_id], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(e)?.ok_or("Batch not found")?;
    if status != "open" { return Err("This batch is closed.".into()); }
    if f.day < arrival { return Err("This day is before the birds arrived.".into()); }
    let old: Option<(i64, i64, i64, i64)> = conn.query_row("SELECT id,deaths,removed,added FROM pb_daily WHERE batch_id=?1 AND day=?2", params![f.batch_id, f.day], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional().map_err(e)?;
    // Birds cannot go below zero.
    let s = summary(conn, f.batch_id)?;
    let (od, or, oa) = old.map(|o| (o.1, o.2, o.3)).unwrap_or((0, 0, 0));
    let alive_before = s.alive + od + or - oa;
    if f.deaths + f.removed > alive_before + f.added { return Err(format!("Only {} birds are alive. Deaths and removals cannot be more than that.", alive_before + f.added)); }
    if f.sick > alive_before + f.added - f.deaths - f.removed { return Err("Sick birds cannot be more than the birds alive.".into()); }
    if f.kg_given > 0.0 && f.feed_id.is_none() { return Err("Which feed did you give?".into()); }
    if let Some(fid) = f.feed_id {
        if f.bags_issued > 0.0 {
            let st = feed_stock(conn, fid)?;
            let prev: f64 = match old { Some(o) => conn.query_row("SELECT COALESCE(SUM(bags),0) FROM pb_feed_moves WHERE daily_id=?1 AND feed_id=?2", params![o.0, fid], |r| r.get(0)).map_err(e)?, None => 0.0 };
            if f.bags_issued > st.available + prev + 1e-9 { return Err(format!("Only {} kg of {} is in the store. Record the feed you bought first.", crate::db::format_qty(((st.available + prev) * bag_kg * 10.0).round() / 10.0), st.name)); }
        }
    }
    let kg_eaten = if f.kg_eaten > 0.0 { f.kg_eaten } else { 0.0 };
    let id = match old {
        Some(o) => {
            conn.execute("UPDATE pb_daily SET deaths=?1,cause=?2,disposed=?3,sick=?4,removed=?5,added=?6,feed_id=?7,bags_issued=?8,kg_eaten=?9,kg_wasted=?10,water_l=?11,feed_unusual=?12,observations=?13,problems=?14,tomorrow=?15,user_name=?16,kg_given=?18,updated_at=CURRENT_TIMESTAMP WHERE id=?17",
                params![f.deaths, f.cause.trim(), f.disposed as i64, f.sick, f.removed, f.added, f.feed_id, f.bags_issued, kg_eaten, f.kg_wasted, f.water_l, f.feed_unusual.trim(), f.observations.trim(), f.problems.trim(), f.tomorrow.trim(), who, o.0, f.kg_given]).map_err(e)?;
            crate::audit::log(conn, who, "daily_edited", "pb_daily", Some(o.0), &format!("Changed the {} record (deaths {} → {})", f.day, o.1, f.deaths));
            o.0
        }
        None => {
            conn.execute("INSERT INTO pb_daily(batch_id,day,deaths,cause,disposed,sick,removed,added,feed_id,bags_issued,kg_eaten,kg_wasted,water_l,feed_unusual,observations,problems,tomorrow,user_name,kg_given) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
                params![f.batch_id, f.day, f.deaths, f.cause.trim(), f.disposed as i64, f.sick, f.removed, f.added, f.feed_id, f.bags_issued, kg_eaten, f.kg_wasted, f.water_l, f.feed_unusual.trim(), f.observations.trim(), f.problems.trim(), f.tomorrow.trim(), who, f.kg_given]).map_err(e)?;
            conn.last_insert_rowid()
        }
    };
    conn.execute("DELETE FROM pb_feed_moves WHERE daily_id=?1", [id]).map_err(e)?;
    if let (Some(fid), true) = (f.feed_id, f.kg_given > 0.0) {
        conn.execute("INSERT INTO pb_feed_moves(feed_id,move_date,kind,bags,batch_id,daily_id,user_name) VALUES(?1,?2,'issue',?3,?4,?5,?6)", params![fid, f.day, f.bags_issued, f.batch_id, id, who]).map_err(e)?;
    }
    Ok(id)
}

// ------------------------------------------------------------------ 4. vaccines, medicine, vitamins

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct HealthForm {
    pub batch_id: i64, pub plan_id: Option<i64>, /** vaccine | medicine | vitamin */ pub kind: String, pub name: String, pub purpose: String, pub done_date: String,
    pub brand: String, pub method: String, pub doses: i64, pub birds: i64, pub expiry: String, pub cost_kobo: i64, pub given_by: String,
    pub success: bool, pub reaction: String, pub end_date: String, pub withdrawal_days: i64, pub next_due: String,
}

pub fn save_health(conn: &Connection, f: &HealthForm, who: &str) -> Result<i64, String> {
    if !["vaccine", "medicine", "vitamin"].contains(&f.kind.as_str()) { return Err("Is it a vaccine, a medicine or vitamins?".into()); }
    if f.name.trim().is_empty() { return Err(if f.kind == "vaccine" { "Which vaccine?" } else { "What is the name of the medicine?" }.into()); }
    if !valid_date(&f.done_date) { return Err("When was it given?".into()); }
    for (s, n) in [(&f.name, "Name"), (&f.purpose, "Purpose"), (&f.brand, "Brand"), (&f.method, "Method"), (&f.given_by, "Given by"), (&f.reaction, "Reaction")] { check_len(s, 200, n)?; }
    check_money(f.cost_kobo, "Cost")?;
    if !(0..=365).contains(&f.withdrawal_days) { return Err("Withdrawal days: 0 to 365, from the product label or your vet".into()); }
    for (x, n) in [(&f.expiry, "Expiry"), (&f.end_date, "End date"), (&f.next_due, "Next due")] { if !x.is_empty() && !valid_date(x) { return Err(format!("{n}: choose a date")); } }
    if f.kind == "medicine" && !f.end_date.is_empty() && f.end_date < f.done_date { return Err("Treatment cannot end before it began.".into()); }
    if f.kind == "vaccine" && !f.expiry.is_empty() && f.expiry < f.done_date { return Err("This vaccine had expired on the day it was given. Check the expiry date.".into()); }
    let alive = summary(conn, f.batch_id)?.alive;
    if f.birds > alive { return Err(format!("Only {alive} birds are alive.")); }
    conn.execute("INSERT INTO pb_health(batch_id,kind,name,purpose,done_date,status,brand,method,doses,birds,expiry,cost_kobo,given_by,success,reaction,end_date,withdrawal_days,next_due,plan_id,user_name)
        VALUES(?1,?2,?3,?4,?5,'done',?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
        params![f.batch_id, f.kind, f.name.trim(), f.purpose.trim(), f.done_date, f.brand.trim(), f.method.trim(), f.doses.max(0), f.birds.max(0), f.expiry, f.cost_kobo, f.given_by.trim(), f.success as i64, f.reaction.trim(),
            f.end_date, f.withdrawal_days, f.next_due, f.plan_id, who]).map_err(e)?;
    let id = conn.last_insert_rowid();
    if let Some(p) = f.plan_id { conn.execute("UPDATE pb_health SET status='done',done_date=?1 WHERE id=?2 AND kind='plan'", params![f.done_date, p]).map_err(e)?; }
    Ok(id)
}

pub fn mark_plan(conn: &Connection, plan_id: i64, status: &str, date: &str, who: &str) -> Result<(), String> {
    if !["done", "skipped", "planned"].contains(&status) { return Err("Unknown status".into()); }
    conn.execute("UPDATE pb_health SET status=?1,done_date=?2 WHERE id=?3 AND kind='plan'", params![status, if status == "planned" { "" } else { date }, plan_id]).map_err(e)?;
    crate::audit::log(conn, who, "timetable_updated", "pb_health", Some(plan_id), &format!("Marked {status}"));
    Ok(())
}

// ------------------------------------------------------------------ 6. weighing

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct WeighForm { pub batch_id: i64, pub weigh_date: String, pub sampled: i64, pub total_kg: f64, pub smaller: String, pub notes: String }

pub fn save_weight(conn: &Connection, f: &WeighForm, who: &str) -> Result<i64, String> {
    if !valid_date(&f.weigh_date) { return Err("When did you weigh?".into()); }
    if f.sampled <= 0 { return Err("How many birds did you weigh?".into()); }
    if !(f.total_kg > 0.0 && f.total_kg < 1_000_000.0) { return Err("What was the total weight of the birds you weighed (kg)?".into()); }
    check_len(&f.smaller, 300, "Smaller birds")?; check_len(&f.notes, 500, "Notes")?;
    conn.execute("INSERT INTO pb_weights(batch_id,weigh_date,sampled,total_kg,smaller,notes,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7)", params![f.batch_id, f.weigh_date, f.sampled, f.total_kg, f.smaller.trim(), f.notes.trim(), who]).map_err(e)?;
    Ok(conn.last_insert_rowid())
}

// ------------------------------------------------------------------ 7. expenses

pub const EXPENSE_CATEGORIES: &[&str] = &["Purchase of chicks", "Feed", "Vaccines", "Medication", "Vitamins", "Veterinary services", "Workers' salaries", "Transportation", "Electricity and fuel", "Water", "Bedding materials", "Disinfectants", "Repairs and maintenance", "Equipment", "Other expenses"];

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct ExpenseForm { pub batch_id: i64, pub spent_date: String, pub category: String, pub item: String, pub quantity: f64, pub unit_price_kobo: i64, pub paid_kobo: i64, pub method: String, pub bought_by: String, pub supplier: String, pub has_receipt: bool }

pub fn save_expense(conn: &Connection, f: &ExpenseForm, me: &Session) -> Result<i64, String> {
    if !valid_date(&f.spent_date) { return Err("When was the money spent?".into()); }
    if !EXPENSE_CATEGORIES.contains(&f.category.as_str()) { return Err("Choose what kind of expense it is".into()); }
    if f.item.trim().is_empty() { return Err("What did you buy or pay for?".into()); }
    for (s, n) in [(&f.item, "Item"), (&f.method, "Method"), (&f.bought_by, "Bought by"), (&f.supplier, "Supplier")] { check_len(s, 120, n)?; }
    check_qty(f.quantity, "Quantity")?; check_money(f.unit_price_kobo, "Price")?; check_money(f.paid_kobo, "Paid")?;
    let q = if f.quantity > 0.0 { f.quantity } else { 1.0 };
    let amount = (q * f.unit_price_kobo as f64).round() as i64;
    if amount <= 0 { return Err("How much did it cost?".into()); }
    // Feed bought through the feed store is already counted when it is fed; chicks are counted on the batch.
    let owner = me.role == Role::Owner;
    let who = me.display_name();
    conn.execute("INSERT INTO pb_expenses(batch_id,spent_date,category,item,quantity,unit_price_kobo,amount_kobo,paid_kobo,method,bought_by,supplier,has_receipt,approved,approved_by,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        params![f.batch_id, f.spent_date, f.category, f.item.trim(), q, f.unit_price_kobo, amount, f.paid_kobo.min(amount), f.method.trim(), if f.bought_by.trim().is_empty() { who.as_str() } else { f.bought_by.trim() },
            f.supplier.trim(), f.has_receipt as i64, owner as i64, if owner { who.as_str() } else { "" }, who]).map_err(e)?;
    Ok(conn.last_insert_rowid())
}

// ------------------------------------------------------------------ 8. sales

#[derive(Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct SaleForm {
    pub batch_id: i64, pub reservation_id: Option<i64>, /** sold | reserved */ pub status: String, pub sale_date: String, pub birds: i64, pub price_per: String, pub avg_kg: f64, pub price_kobo: i64,
    pub paid_kobo: i64, pub method: String, pub buyer: String, pub phone: String, pub delivery: String, pub delivery_cost_kobo: i64, pub evidence: String,
}

#[derive(Serialize, Debug)]
pub struct SaleDone { pub sale_id: i64, pub receipt_id: Option<i64>, pub receipt_no: String }

fn bird_item(conn: &Connection, per: &str) -> Result<i64, String> {
    let unit = if per == "kg" { "kg" } else { "bird" };
    let name = "Live birds";
    if let Some(id) = conn.query_row("SELECT id FROM products WHERE name=?1 AND unit=?2 AND active=1 LIMIT 1", params![name, unit], |r| r.get::<_, i64>(0)).optional().map_err(e)? { return Ok(id); }
    let sku = format!("PB-LIVE-{}", unit.to_uppercase());
    conn.execute("INSERT OR IGNORE INTO products(sku,name,kind,unit,selling_price_kobo,cost_price_kobo,track_stock,stock_qty,reorder_level,description) VALUES(?1,?2,'product',?3,1,0,0,0,0,'Sold from livestock batches')", params![sku, name, unit]).map_err(e)?;
    conn.query_row("SELECT id FROM products WHERE sku=?1", [sku], |r| r.get(0)).map_err(e)
}

pub fn save_sale(conn: &Connection, f: &SaleForm, me: &Session) -> Result<SaleDone, String> {
    let status = if f.status == "reserved" { "reserved" } else { "sold" };
    if !valid_date(&f.sale_date) { return Err("Choose the date".into()); }
    if f.birds <= 0 { return Err("How many birds?".into()); }
    let per = if f.price_per == "kg" { "kg" } else { "bird" };
    check_qty(f.avg_kg, "Average weight")?;
    for (k, n) in [(f.price_kobo, "Price"), (f.paid_kobo, "Paid"), (f.delivery_cost_kobo, "Delivery cost")] { check_money(k, n)?; }
    for (s, n) in [(&f.buyer, "Buyer"), (&f.phone, "Phone"), (&f.delivery, "Delivery"), (&f.evidence, "Evidence"), (&f.method, "Method")] { check_len(s, 120, n)?; }
    if per == "kg" && f.avg_kg <= 0.0 { return Err("You sell per kg: what is the average weight of one bird (kg)?".into()); }
    if f.price_kobo <= 0 { return Err(format!("What is the selling price per {per}?")); }
    if status == "reserved" && f.buyer.trim().is_empty() { return Err("Who are the birds reserved for?".into()); }
    let s = summary(conn, f.batch_id)?;
    if s.status != "open" { return Err("This batch is closed.".into()); }
    let held_for_this = match f.reservation_id { Some(r) => conn.query_row("SELECT birds FROM pb_sales WHERE id=?1 AND status='reserved' AND batch_id=?2", params![r, f.batch_id], |x| x.get::<_, i64>(0)).optional().map_err(e)?.ok_or("Reservation not found")?, None => 0 };
    let free = s.alive - s.reserved_birds + held_for_this;
    if f.birds > free { return Err(format!("Only {free} birds are free to sell ({} alive, {} reserved).", s.alive, s.reserved_birds - held_for_this)); }
    if status == "sold" && !s.eligible { return Err(format!("These birds cannot be sold or slaughtered before {} (medicine withdrawal period).", s.withdrawal_until)); }
    let amount = if per == "kg" { (f.birds as f64 * f.avg_kg * f.price_kobo as f64).round() as i64 } else { f.birds * f.price_kobo };
    let who = me.display_name();
    if status == "reserved" {
        conn.execute("INSERT INTO pb_sales(batch_id,sale_date,status,birds,price_per,avg_kg,price_kobo,amount_kobo,delivery,delivery_cost_kobo,buyer,phone,evidence,user_name) VALUES(?1,?2,'reserved',?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![f.batch_id, f.sale_date, f.birds, per, f.avg_kg, f.price_kobo, amount, f.delivery.trim(), f.delivery_cost_kobo, f.buyer.trim(), f.phone.trim(), f.evidence.trim(), who]).map_err(e)?;
        return Ok(SaleDone { sale_id: conn.last_insert_rowid(), receipt_id: None, receipt_no: String::new() });
    }
    let paid = f.paid_kobo.min(amount);
    if paid < amount && f.buyer.trim().is_empty() { return Err("The buyer has not paid everything. Add the buyer's name so you can follow up.".into()); }
    let method = if f.method.trim().is_empty() { "Cash".to_string() } else { f.method.trim().to_string() };
    if paid > 0 && conn.query_row("SELECT 1 FROM payment_methods WHERE name=?1 COLLATE NOCASE AND active=1", [&method], |_| Ok(())).optional().map_err(e)?.is_none() { return Err(format!("Payment method '{method}' is not available")); }
    // The receipt.
    let prefix: String = crate::db::setting(conn, "receipt_prefix")?.chars().filter(|c| c.is_ascii_alphanumeric()).take(8).collect::<String>().to_uppercase();
    let prefix = if prefix.is_empty() { "RCP".to_string() } else { prefix };
    let next: i64 = conn.query_row("SELECT COALESCE(MAX(id),0)+1 FROM receipts", [], |r| r.get(0)).map_err(e)?;
    let receipt_no = format!("{prefix}-{}-{:05}", Local::now().format("%y%m"), next);
    let customer_id: Option<i64> = if f.buyer.trim().is_empty() { None } else { conn.query_row("SELECT id FROM customers WHERE name=?1 COLLATE NOCASE LIMIT 1", [f.buyer.trim()], |r| r.get(0)).optional().map_err(e)? };
    conn.execute("INSERT INTO receipts(receipt_no,customer_id,customer_name,customer_phone,customer_email,cashier_id,cashier_name,subtotal_kobo,discount_kobo,tax_kobo,total_kobo,amount_paid_kobo,change_kobo,payment_status,fulfilment_status,collection_date,note,created_at)
        VALUES(?1,?2,?3,?4,'',?5,?6,?7,0,0,?7,?8,0,?9,'none','',?10,?11)",
        params![receipt_no, customer_id, f.buyer.trim(), f.phone.trim(), me.user_id, who, amount, paid, crate::sales::status_for(paid, amount), format!("Batch {}", s.name), format!("{} {}", f.sale_date, Local::now().format("%H:%M:%S"))]).map_err(e)?;
    let receipt_id = conn.last_insert_rowid();
    let pid = bird_item(conn, per)?;
    let (qty, unit_price) = if per == "kg" { (f.birds as f64 * f.avg_kg, f.price_kobo) } else { (f.birds as f64, f.price_kobo) };
    conn.execute("INSERT INTO receipt_items(receipt_id,product_id,product_name,product_kind,unit,quantity,unit_price_kobo,unit_cost_kobo,line_total_kobo,stock_deducted,base_quantity,sold_as) VALUES(?1,?2,?3,'product',?4,?5,?6,0,?7,0,?5,'')",
        params![receipt_id, pid, format!("Live birds ({} birds)", f.birds), per, qty, unit_price, amount]).map_err(e)?;
    if paid > 0 {
        let shift = crate::shifts::open_shift_id(conn, me.user_id)?;
        conn.execute("INSERT INTO receipt_payments(receipt_id,method,amount_kobo,reference,received_by,shift_id) VALUES(?1,?2,?3,?4,?5,?6)", params![receipt_id, method, paid, f.evidence.trim(), who, shift]).map_err(e)?;
    }
    let sale_id = match f.reservation_id {
        Some(r) => {
            conn.execute("UPDATE pb_sales SET status='sold',sale_date=?1,birds=?2,price_per=?3,avg_kg=?4,price_kobo=?5,amount_kobo=?6,delivery=?7,delivery_cost_kobo=?8,buyer=?9,phone=?10,evidence=?11,receipt_id=?12,user_name=?13 WHERE id=?14",
                params![f.sale_date, f.birds, per, f.avg_kg, f.price_kobo, amount, f.delivery.trim(), f.delivery_cost_kobo, f.buyer.trim(), f.phone.trim(), f.evidence.trim(), receipt_id, who, r]).map_err(e)?;
            r
        }
        None => {
            conn.execute("INSERT INTO pb_sales(batch_id,sale_date,status,birds,price_per,avg_kg,price_kobo,amount_kobo,delivery,delivery_cost_kobo,buyer,phone,evidence,receipt_id,user_name) VALUES(?1,?2,'sold',?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
                params![f.batch_id, f.sale_date, f.birds, per, f.avg_kg, f.price_kobo, amount, f.delivery.trim(), f.delivery_cost_kobo, f.buyer.trim(), f.phone.trim(), f.evidence.trim(), receipt_id, who]).map_err(e)?;
            conn.last_insert_rowid()
        }
    };
    Ok(SaleDone { sale_id, receipt_id: Some(receipt_id), receipt_no })
}

/// Live birds sold in "Sell produce" come off the batches (oldest open batch first,
/// only birds that are free and past any medicine withdrawal).
pub fn link_pos_sale(conn: &Connection, receipt_id: i64, who: &str) -> Result<(), String> {
    let items: Vec<(String, f64, i64, i64)> = rows(conn, "SELECT p.unit, i.quantity, i.unit_price_kobo, i.line_total_kobo FROM receipt_items i JOIN products p ON p.id=i.product_id WHERE i.receipt_id=?1 AND p.name='Live birds'", [receipt_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    if items.is_empty() { return Ok(()); }
    let (day, buyer, phone): (String, String, String) = conn.query_row("SELECT substr(created_at,1,10),customer_name,customer_phone FROM receipts WHERE id=?1", [receipt_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(e)?;
    for (unit, q, price, total) in items {
        if unit != "bird" { return Err("Birds sold per kg need their weight: use Livestock → Sell birds.".into()); }
        if (q - q.round()).abs() > 1e-9 { return Err("Sell whole birds.".into()); }
        let want = q.round() as i64;
        let ids: Vec<i64> = rows(conn, "SELECT id FROM pb_batches WHERE status='open' ORDER BY arrival_date, id", [], |r| r.get(0))?;
        let mut plan: Vec<(i64, i64)> = vec![];
        let (mut rest, mut blocked) = (want, String::new());
        for id in ids {
            if rest == 0 { break; }
            let s = summary(conn, id)?;
            let free = s.alive - s.reserved_birds;
            if free <= 0 { continue; }
            if !s.eligible { blocked = s.withdrawal_until.clone(); continue; }
            let take = rest.min(free);
            plan.push((id, take)); rest -= take;
        }
        if rest > 0 {
            let have = want - rest;
            return Err(if !blocked.is_empty() { format!("Only {have} birds can be sold now; others are under medicine withdrawal until {blocked}.") } else { format!("Only {have} birds are free to sell in your batches.") });
        }
        for (id, take) in plan {
            let share = take as f64 / want as f64;
            conn.execute("INSERT INTO pb_sales(batch_id,sale_date,status,birds,price_per,price_kobo,amount_kobo,buyer,phone,receipt_id,user_name) VALUES(?1,?2,'sold',?3,'bird',?4,?5,?6,?7,?8,?9)",
                params![id, day, take, price, (total as f64 * share).round() as i64, buyer, phone, receipt_id, who]).map_err(e)?;
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ the summary (dashboard of one batch)

#[derive(Serialize, Debug, Default, Clone)]
pub struct Line { pub label: String, pub amount_kobo: i64 }
#[derive(Serialize, Debug, Default, Clone)]
pub struct Alert { /** info | warn | bad */ pub level: String, pub text: String }
#[derive(Serialize, Debug, Default, Clone)]
pub struct PlanItem { pub id: i64, pub name: String, pub purpose: String, pub due_from: String, pub due_to: String, pub status: String, pub done_date: String, pub state: String }

#[derive(Serialize, Debug, Default, Clone)]
pub struct Summary {
    pub id: i64, pub farm_id: Option<i64>, pub name: String, pub bird_type: String, pub breed: String, pub house: String, pub manager: String, pub status: String, pub is_demo: bool,
    pub arrival_date: String, pub age_days: i64, pub weeks: i64, pub sell_age_weeks: i64, pub sell_date: String, pub days_to_sell: i64,
    pub planned_birds: i64, pub chicks_ordered: i64, pub chicks_delivered: i64, pub dead_on_arrival: i64, pub initial_live: i64,
    pub added: i64, pub deaths: i64, pub removed: i64, pub sold_birds: i64, pub reserved_birds: i64, pub alive: i64, pub sick_now: i64,
    pub deaths_today: i64, pub mortality_pct: f64, pub last_record: String, pub recorded_today: bool,
    // feed
    pub bags_per_100: f64, pub bag_kg: f64, pub planned_bags: f64, pub planned_kg: f64, pub buy_bags_plan: i64, pub plan_starter: f64, pub plan_grower: f64, pub plan_finisher: f64,
    pub starter_pct: f64, pub starter_weeks: i64, pub starter_kg: f64, pub finisher_kg: f64, pub stage: String, pub plan_kg_today: f64, pub given_kg_today: f64, pub given_kg: f64,
    pub issued_bags: f64, pub eaten_kg: f64, pub wasted_kg: f64, pub plan_left_bags: f64, pub feed_cost_kobo: i64, pub feed_per_bird_kg: f64,
    // growth
    pub latest_avg_kg: Option<f64>, pub prev_avg_kg: Option<f64>, pub gain_kg: Option<f64>, pub adg_g: Option<f64>, pub fcr: Option<f64>, pub est_market_kg: Option<f64>, pub last_weighed: String,
    // health
    pub withdrawal_until: String, pub eligible: bool, pub plan: Vec<PlanItem>,
    // money
    pub costs: Vec<Line>, pub total_cost_kobo: i64, pub pending_expenses: i64, pub budget_kobo: i64,
    pub revenue_kobo: i64, pub received_kobo: i64, pub customers_owe_kobo: i64, pub suppliers_owed_kobo: i64,
    pub price_per: String, pub expected_price_kobo: i64, pub expected_weight_kg: f64, pub remaining_value_kobo: i64, pub profit_kobo: i64, pub projected_profit_kobo: i64,
    pub alerts: Vec<Alert>,
}

fn sum_i(conn: &Connection, sql: &str, id: i64) -> Result<i64, String> { conn.query_row(sql, [id], |r| r.get::<_, Option<i64>>(0)).map(|v| v.unwrap_or(0)).map_err(e) }
fn sum_f(conn: &Connection, sql: &str, id: i64) -> Result<f64, String> { conn.query_row(sql, [id], |r| r.get::<_, Option<f64>>(0)).map(|v| v.unwrap_or(0.0)).map_err(e) }

pub fn summary(conn: &Connection, id: i64) -> Result<Summary, String> {
    let lim = limits(conn);
    let now = today();
    let mut s = conn.query_row("SELECT id,farm_id,name,bird_type,breed,house,manager,status,is_demo,arrival_date,age_at_arrival,weeks,sell_age_weeks,planned_birds,chicks_ordered,chicks_delivered,dead_on_arrival,
        bags_per_100,bag_kg,plan_starter,plan_grower,plan_finisher,budget_kobo,price_per,expected_price_kobo,expected_weight_kg,chicks_paid_for,chick_price_kobo,chick_transport_kobo,chick_amount_paid_kobo,end_date,starter_pct,starter_weeks FROM pb_batches WHERE id=?1", [id], |r| {
        let age0: i64 = r.get(10)?;
        let arrival: String = r.get(9)?;
        let paid_for: i64 = r.get(26)?; let price: i64 = r.get(27)?; let transport: i64 = r.get(28)?; let paid: i64 = r.get(29)?;
        let mut s = Summary { id: r.get(0)?, farm_id: r.get(1)?, name: r.get(2)?, bird_type: r.get(3)?, breed: r.get(4)?, house: r.get(5)?, manager: r.get(6)?, status: r.get(7)?, is_demo: r.get::<_, i64>(8)? == 1,
            arrival_date: arrival.clone(), weeks: r.get(11)?, sell_age_weeks: r.get(12)?, planned_birds: r.get(13)?, chicks_ordered: r.get(14)?, chicks_delivered: r.get(15)?, dead_on_arrival: r.get(16)?,
            bags_per_100: r.get(17)?, bag_kg: r.get(18)?, plan_starter: r.get(19)?, plan_grower: r.get(20)?, plan_finisher: r.get(21)?, budget_kobo: r.get(22)?, price_per: r.get(23)?, expected_price_kobo: r.get(24)?, expected_weight_kg: r.get(25)?, ..Default::default() };
        let end: String = r.get(30)?;
        s.starter_pct = r.get(31)?; s.starter_weeks = r.get(32)?;
        let upto = if s.status == "closed" && !end.is_empty() { end } else { today() };
        s.age_days = d(&arrival).zip(d(&upto)).map(|(a, b)| (b - a).num_days() + age0).unwrap_or(age0);
        s.sell_date = plus(&arrival, s.sell_age_weeks * 7 - age0);
        s.days_to_sell = d(&s.sell_date).zip(d(&today())).map(|(a, b)| (a - b).num_days()).unwrap_or(0);
        s.costs = vec![Line { label: "Chicks".into(), amount_kobo: paid_for * price }, Line { label: "Transport of chicks".into(), amount_kobo: transport }];
        s.suppliers_owed_kobo = (paid_for * price + transport - paid).max(0);
        Ok(s)
    }).optional().map_err(e)?.ok_or("Batch not found")?;
    // Birds
    s.initial_live = s.chicks_delivered - s.dead_on_arrival;
    s.added = sum_i(conn, "SELECT SUM(added) FROM pb_daily WHERE batch_id=?1", id)?;
    s.deaths = sum_i(conn, "SELECT SUM(deaths) FROM pb_daily WHERE batch_id=?1", id)?;
    s.removed = sum_i(conn, "SELECT SUM(removed) FROM pb_daily WHERE batch_id=?1", id)?;
    s.sold_birds = sum_i(conn, "SELECT SUM(s.birds) FROM pb_sales s LEFT JOIN receipts r ON r.id=s.receipt_id WHERE s.batch_id=?1 AND s.status='sold' AND (r.id IS NULL OR r.payment_status<>'void')", id)?;
    s.reserved_birds = sum_i(conn, "SELECT SUM(birds) FROM pb_sales WHERE batch_id=?1 AND status='reserved'", id)?;
    s.alive = s.initial_live + s.added - s.deaths - s.removed - s.sold_birds;
    let base = s.initial_live + s.added;
    s.mortality_pct = if base > 0 { (s.deaths as f64 / base as f64 * 1000.0).round() / 10.0 } else { 0.0 };
    if let Some((day, sick, dt)) = conn.query_row("SELECT day,sick,deaths FROM pb_daily WHERE batch_id=?1 ORDER BY day DESC LIMIT 1", [id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))).optional().map_err(e)? {
        s.recorded_today = day == now; if s.recorded_today { s.deaths_today = dt; } s.last_record = day; s.sick_now = sick;
    }
    // Feed: the plan is a budget; actual use is compared with it.
    s.planned_bags = (s.planned_birds as f64 / 100.0 * s.bags_per_100 * 1000.0).round() / 1000.0;
    s.planned_kg = (s.planned_bags * s.bag_kg * 1000.0).round() / 1000.0;
    s.buy_bags_plan = s.planned_bags.ceil() as i64;
    s.issued_bags = sum_f(conn, "SELECT SUM(bags) FROM pb_feed_moves WHERE batch_id=?1 AND kind='issue'", id)?;
    // The plan in kg, by stage, and what that means for one day.
    s.starter_kg = s.plan_starter * s.bag_kg;
    s.finisher_kg = (s.plan_finisher + s.plan_grower) * s.bag_kg;
    let starter_days = (s.starter_weeks * 7).max(0);
    let finisher_days = ((s.weeks - s.starter_weeks) * 7).max(0);
    let in_starter = s.age_days <= starter_days;
    s.stage = if in_starter { "Starter".into() } else { "Finisher".into() };
    s.plan_kg_today = ((if in_starter { if starter_days > 0 { s.starter_kg / starter_days as f64 } else { 0.0 } } else if finisher_days > 0 { s.finisher_kg / finisher_days as f64 } else { 0.0 }) * 10.0).round() / 10.0;
    s.given_kg = sum_f(conn, "SELECT SUM(kg_given) FROM pb_daily WHERE batch_id=?1", id)?;
    s.given_kg_today = conn.query_row("SELECT COALESCE(SUM(kg_given),0) FROM pb_daily WHERE batch_id=?1 AND day=?2", params![id, now], |r| r.get(0)).map_err(e)?;
    s.wasted_kg = sum_f(conn, "SELECT SUM(kg_wasted) FROM pb_daily WHERE batch_id=?1", id)?;
    let issued_kg = sum_f(conn, "SELECT SUM(m.bags*f.bag_kg) FROM pb_feed_moves m JOIN pb_feeds f ON f.id=m.feed_id WHERE m.batch_id=?1 AND m.kind='issue'", id)?;
    let told_eaten = sum_f(conn, "SELECT SUM(kg_eaten) FROM pb_daily WHERE batch_id=?1", id)?;
    s.eaten_kg = if told_eaten > 0.0 { told_eaten } else { (issued_kg - s.wasted_kg).max(0.0) };
    s.plan_left_bags = ((s.planned_bags - s.issued_bags) * 1000.0).round() / 1000.0;
    s.feed_cost_kobo = conn.query_row("SELECT SUM(m.bags * COALESCE((SELECT 1.0*SUM(p.amount_kobo)/NULLIF(SUM(CASE WHEN p.price_kobo>0 THEN p.bags END),0) FROM pb_feed_moves p WHERE p.feed_id=m.feed_id AND p.kind IN ('opening','purchase')),0))
        FROM pb_feed_moves m WHERE m.batch_id=?1 AND m.kind='issue'", [id], |r| r.get::<_, Option<f64>>(0)).map_err(e)?.unwrap_or(0.0).round() as i64;
    let fed_birds = (s.alive + s.sold_birds).max(1);
    s.feed_per_bird_kg = (s.eaten_kg / fed_birds as f64 * 100.0).round() / 100.0;
    // Growth
    let w: Vec<(String, f64)> = rows(conn, "SELECT weigh_date, total_kg/sampled FROM pb_weights WHERE batch_id=?1 ORDER BY weigh_date DESC, id DESC LIMIT 2", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
    if let Some((date, avg)) = w.first() {
        s.latest_avg_kg = Some((avg * 1000.0).round() / 1000.0); s.last_weighed = date.clone();
        if let Some((pd, pavg)) = w.get(1) {
            s.prev_avg_kg = Some((pavg * 1000.0).round() / 1000.0);
            s.gain_kg = Some(((avg - pavg) * 1000.0).round() / 1000.0);
            let days = d(pd).zip(d(date)).map(|(a, b)| (b - a).num_days()).unwrap_or(0);
            if days > 0 { s.adg_g = Some(((avg - pavg) / days as f64 * 1000.0).round()); }
        }
        let sold_kg = sum_f(conn, "SELECT SUM(CASE WHEN price_per='kg' THEN birds*avg_kg ELSE 0 END) FROM pb_sales WHERE batch_id=?1 AND status='sold'", id)?;
        let live_kg = s.alive as f64 * avg + sold_kg;
        if live_kg > 0.0 && s.eaten_kg > 0.0 { s.fcr = Some((s.eaten_kg / live_kg * 100.0).round() / 100.0); }
        if let Some(adg) = s.adg_g { if s.days_to_sell > 0 { s.est_market_kg = Some(((avg + adg / 1000.0 * s.days_to_sell as f64) * 100.0).round() / 100.0); } }
    }
    // Health: timetable and withdrawal
    s.plan = rows(conn, "SELECT id,name,purpose,due_from,due_to,status,done_date FROM pb_health WHERE batch_id=?1 AND kind='plan' ORDER BY due_from,id", [id], |r| Ok(PlanItem {
        id: r.get(0)?, name: r.get(1)?, purpose: r.get(2)?, due_from: r.get(3)?, due_to: r.get(4)?, status: r.get(5)?, done_date: r.get(6)?, state: String::new() }))?;
    for p in &mut s.plan {
        p.state = if p.status != "planned" { p.status.clone() } else if p.due_to < now { "overdue".into() } else if p.due_from <= now { "due".into() } else { "later".into() };
    }
    let wd: Vec<(String, String, i64)> = rows(conn, "SELECT done_date,end_date,withdrawal_days FROM pb_health WHERE batch_id=?1 AND kind IN ('medicine','vaccine','vitamin') AND withdrawal_days>0", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    s.withdrawal_until = wd.iter().map(|(a, b, n)| plus(if b.is_empty() { a } else { b }, *n)).max().unwrap_or_default();
    s.eligible = s.withdrawal_until.is_empty() || s.withdrawal_until <= now;
    // Money
    s.costs.push(Line { label: "Feed used".into(), amount_kobo: s.feed_cost_kobo });
    let health = sum_i(conn, "SELECT SUM(cost_kobo) FROM pb_health WHERE batch_id=?1 AND kind<>'plan'", id)?;
    s.costs.push(Line { label: "Vaccines, medicine and vitamins".into(), amount_kobo: health });
    for (cat, amt) in rows(conn, "SELECT category,SUM(amount_kobo) FROM pb_expenses WHERE batch_id=?1 GROUP BY category ORDER BY MIN(id)", [id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
        s.costs.push(Line { label: cat, amount_kobo: amt });
    }
    s.costs.push(Line { label: "Delivery of birds sold".into(), amount_kobo: sum_i(conn, "SELECT SUM(delivery_cost_kobo) FROM pb_sales WHERE batch_id=?1 AND status='sold'", id)? });
    s.costs.retain(|c| c.amount_kobo > 0);
    s.total_cost_kobo = s.costs.iter().map(|c| c.amount_kobo).sum();
    s.pending_expenses = sum_i(conn, "SELECT COUNT(*) FROM pb_expenses WHERE batch_id=?1 AND approved=0", id)?;
    s.suppliers_owed_kobo += sum_i(conn, "SELECT SUM(amount_kobo-paid_kobo) FROM pb_expenses WHERE batch_id=?1", id)?;
    let (rev, rec): (i64, i64) = conn.query_row("SELECT COALESCE(SUM(r.total_kobo),0), COALESCE(SUM(r.amount_paid_kobo),0) FROM pb_sales s JOIN receipts r ON r.id=s.receipt_id WHERE s.batch_id=?1 AND s.status='sold' AND r.payment_status<>'void'", [id], |r| Ok((r.get(0)?, r.get(1)?))).map_err(e)?;
    s.revenue_kobo = rev; s.received_kobo = rec; s.customers_owe_kobo = (rev - rec).max(0);
    let weight = s.latest_avg_kg.unwrap_or(s.expected_weight_kg);
    s.remaining_value_kobo = if s.price_per == "kg" { (s.alive as f64 * weight * s.expected_price_kobo as f64).round() as i64 } else { s.alive * s.expected_price_kobo };
    s.profit_kobo = s.revenue_kobo - s.total_cost_kobo;
    s.projected_profit_kobo = s.profit_kobo + s.remaining_value_kobo;
    // Alerts (plain words, no money: workers see them too)
    let mut a = |level: &str, text: String| s.alerts.push(Alert { level: level.into(), text });
    if s.status == "open" {
        if !s.recorded_today { a("warn", "Today's record is not in yet (birds, deaths and feed).".into()); }
        if lim.mortality_pct > 0.0 && base > 0 && (s.deaths_today as f64 / base as f64 * 100.0) > lim.mortality_pct { a("bad", format!("Unusual mortality: {} birds died today, more than your limit of {}%.", s.deaths_today, lim.mortality_pct)); }
        let (due, over): (Vec<&PlanItem>, Vec<&PlanItem>) = (s.plan.iter().filter(|p| p.state == "due").collect(), s.plan.iter().filter(|p| p.state == "overdue").collect());
        let names = |l: &[&PlanItem]| l.iter().map(|p| p.name.split(',').next().unwrap_or("").trim().to_string()).collect::<Vec<_>>().join("; ");
        if !over.is_empty() { a("bad", format!("Overdue on the timetable ({}): {}. Record it, or mark it not needed.", over.len(), names(&over))); }
        if !due.is_empty() { a("warn", format!("Due now: {}", names(&due))); }
        let next_due: Vec<(String, String)> = rows(conn, "SELECT name,next_due FROM pb_health WHERE batch_id=?1 AND kind<>'plan' AND next_due<>'' AND next_due<=date(?2,'+2 day') AND next_due>=?2", params![id, now], |r| Ok((r.get(0)?, r.get(1)?)))?;
        for (n, dd) in next_due { a("warn", format!("{n} is due again on {dd}.")); }
        if !s.eligible { a("bad", format!("Medicine withdrawal: do not sell or slaughter before {}.", s.withdrawal_until)); }
        if s.plan_kg_today > 0.0 && s.given_kg_today > s.plan_kg_today * 1.15 { a("warn", format!("Feed given today ({} kg) is more than the plan for today ({} kg). Check for waste.", crate::db::format_qty(s.given_kg_today), crate::db::format_qty(s.plan_kg_today))); }
        if s.issued_bags > s.planned_bags + 1e-9 && s.planned_bags > 0.0 { a("bad", format!("Feed used ({} bags) is more than the plan ({} bags). Check for waste or wrong records.", crate::db::format_qty(s.issued_bags), crate::db::format_qty(s.planned_bags))); }
        if s.age_days >= 7 && (s.last_weighed.is_empty() || d(&s.last_weighed).zip(d(&now)).map(|(a, b)| (b - a).num_days() > lim.weigh_days).unwrap_or(true)) { a("warn", "Weekly weighing is due.".into()); }
        if s.days_to_sell >= 0 && s.days_to_sell <= 7 { a("info", format!("Birds reach selling age in {} day{}.", s.days_to_sell, if s.days_to_sell == 1 { "" } else { "s" })); }
        if s.budget_kobo > 0 && s.total_cost_kobo > s.budget_kobo { a("bad", "Spending is above the production budget.".into()); }
        if s.pending_expenses > 0 { a("info", format!("{} expense{} waiting for the owner's approval.", s.pending_expenses, if s.pending_expenses == 1 { "" } else { "s" })); }
        if s.customers_owe_kobo > 0 { a("warn", "Some customers still owe you for birds.".into()); }
        if s.suppliers_owed_kobo > 0 { a("warn", "You still owe some suppliers.".into()); }
    }
    Ok(s)
}

#[derive(Serialize, Debug)]
pub struct Detail {
    pub batch: Summary, pub form: serde_json::Value, pub daily: Vec<serde_json::Value>, pub health: Vec<serde_json::Value>, pub weights: Vec<serde_json::Value>,
    pub expenses: Vec<serde_json::Value>, pub sales: Vec<serde_json::Value>, pub feeds: Vec<FeedStock>,
}

fn json_rows(conn: &Connection, sql: &str, id: i64) -> Result<Vec<serde_json::Value>, String> {
    let mut st = conn.prepare(sql).map_err(e)?;
    let names: Vec<String> = st.column_names().iter().map(|s| s.to_string()).collect();
    let out = st.query_map([id], |r| {
        let mut m = serde_json::Map::new();
        for (i, n) in names.iter().enumerate() {
            let v = match r.get_ref(i)? { rusqlite::types::ValueRef::Null => serde_json::Value::Null, rusqlite::types::ValueRef::Integer(x) => x.into(), rusqlite::types::ValueRef::Real(x) => x.into(),
                rusqlite::types::ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned().into(), rusqlite::types::ValueRef::Blob(_) => serde_json::Value::Null };
            m.insert(n.clone(), v);
        }
        Ok(serde_json::Value::Object(m))
    }).map_err(e)?.collect::<Result<Vec<_>, _>>().map_err(e)?;
    Ok(out)
}

pub fn detail(conn: &Connection, id: i64) -> Result<Detail, String> {
    let batch = summary(conn, id)?;
    let form = json_rows(conn, "SELECT * FROM pb_batches WHERE id=?1", id)?.into_iter().next().unwrap_or_default();
    Ok(Detail {
        batch, form,
        daily: json_rows(conn, "SELECT d.*, COALESCE(f.name,'') AS feed_name FROM pb_daily d LEFT JOIN pb_feeds f ON f.id=d.feed_id WHERE d.batch_id=?1 ORDER BY d.day DESC", id)?,
        health: json_rows(conn, "SELECT * FROM pb_health WHERE batch_id=?1 AND kind<>'plan' ORDER BY done_date DESC, id DESC", id)?,
        weights: json_rows(conn, "SELECT *, ROUND(total_kg/sampled,3) AS avg_kg FROM pb_weights WHERE batch_id=?1 ORDER BY weigh_date DESC, id DESC", id)?,
        expenses: json_rows(conn, "SELECT * FROM pb_expenses WHERE batch_id=?1 ORDER BY spent_date DESC, id DESC", id)?,
        sales: json_rows(conn, "SELECT s.*, COALESCE(r.receipt_no,'') AS receipt_no, COALESCE(r.total_kobo-r.amount_paid_kobo,0) AS owed_kobo, COALESCE(r.payment_status,'') AS payment_status FROM pb_sales s LEFT JOIN receipts r ON r.id=s.receipt_id WHERE s.batch_id=?1 ORDER BY s.sale_date DESC, s.id DESC", id)?,
        feeds: feeds(conn)?,
    })
}

/// Production cost of livestock batches in a period, for the farm profit and loss
/// (revenue comes from the receipts their sales make).
pub fn period_costs(conn: &Connection, from: &str, to: &str, include_demo: bool) -> Result<i64, String> {
    let ids: Vec<i64> = rows(conn, "SELECT id FROM pb_batches WHERE (?1=1 OR is_demo=0)", [include_demo as i64], |r| r.get(0))?;
    let mut total = 0i64;
    for id in ids {
        let p = params![id, from, to];
        let q = |sql: &str| -> Result<i64, String> { conn.query_row(sql, p, |r| r.get::<_, Option<f64>>(0)).map(|v| v.unwrap_or(0.0).round() as i64).map_err(e) };
        total += q("SELECT SUM(1.0*(chicks_paid_for*chick_price_kobo+chick_transport_kobo)) FROM pb_batches WHERE id=?1 AND arrival_date BETWEEN ?2 AND ?3")?;
        total += q("SELECT SUM(m.bags * COALESCE((SELECT 1.0*SUM(x.amount_kobo)/NULLIF(SUM(CASE WHEN x.price_kobo>0 THEN x.bags END),0) FROM pb_feed_moves x WHERE x.feed_id=m.feed_id AND x.kind IN ('opening','purchase')),0)) FROM pb_feed_moves m WHERE m.batch_id=?1 AND m.kind='issue' AND m.move_date BETWEEN ?2 AND ?3")?;
        total += q("SELECT 1.0*SUM(cost_kobo) FROM pb_health WHERE batch_id=?1 AND kind<>'plan' AND done_date BETWEEN ?2 AND ?3")?;
        total += q("SELECT 1.0*SUM(amount_kobo) FROM pb_expenses WHERE batch_id=?1 AND spent_date BETWEEN ?2 AND ?3")?;
        total += q("SELECT 1.0*SUM(delivery_cost_kobo) FROM pb_sales WHERE batch_id=?1 AND status='sold' AND sale_date BETWEEN ?2 AND ?3")?;
    }
    Ok(total)
}

// ------------------------------------------------------------------ commands

fn tx<T>(state: &AppState, f: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
    let mut conn = state.db()?;
    let t = conn.transaction().map_err(e)?;
    let out = f(&t)?;
    t.commit().map_err(e)?;
    Ok(out)
}

#[derive(Serialize)]
pub struct Home { batches: Vec<Summary>, feeds: Vec<FeedStock>, limits: Limits, categories: &'static [&'static str], timetable: Vec<(i64, i64, &'static str, &'static str)> }

pub fn home(conn: &Connection, me: &Session) -> Result<Home, String> {
    let allowed = crate::farm_access::allowed(conn, me)?;
    let ids: Vec<i64> = rows(conn, "SELECT id FROM pb_batches ORDER BY status DESC, arrival_date DESC, id DESC LIMIT 200", [], |r| r.get(0))?;
    let batches = ids.into_iter().map(|id| summary(conn, id)).collect::<Result<Vec<_>, _>>()?.into_iter().filter(|b| crate::farm_access::visible(&allowed, b.farm_id)).collect();
    Ok(Home { batches, feeds: feeds(conn)?, limits: limits(conn), categories: EXPENSE_CATEGORIES, timetable: BROILER_TIMETABLE.to_vec() })
}

#[tauri::command]
pub fn pb_home(state: tauri::State<'_, AppState>, session_token: String) -> Result<serde_json::Value, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    auth::money_view(&me, &home(&*state.db()?, &me)?)
}
#[tauri::command]
pub fn pb_batch(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<serde_json::Value, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    auth::money_view(&me, &detail(&*state.db()?, id)?)
}
#[tauri::command]
pub fn pb_save_batch(state: tauri::State<'_, AppState>, session_token: String, form: BatchForm) -> Result<i64, String> {
    let me = auth::require(&state, &session_token, Role::Manager)?;
    tx(&state, |c| save_batch(c, &form, &me.display_name()))
}
#[tauri::command]
pub fn pb_close_batch(state: tauri::State<'_, AppState>, session_token: String, id: i64, reopen: bool) -> Result<(), String> {
    let me = auth::require(&state, &session_token, Role::Manager)?;
    tx(&state, |c| {
        c.execute("UPDATE pb_batches SET status=?1,end_date=?2 WHERE id=?3", params![if reopen { "open" } else { "closed" }, if reopen { String::new() } else { today() }, id]).map_err(e)?;
        crate::audit::log(c, &me.display_name(), if reopen { "batch_reopened" } else { "batch_closed" }, "pb_batch", Some(id), "");
        Ok(())
    })
}
#[tauri::command]
pub fn pb_save_daily(state: tauri::State<'_, AppState>, session_token: String, form: DailyForm) -> Result<i64, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    tx(&state, |c| save_daily(c, &form, &me.display_name()))
}
#[tauri::command]
pub fn pb_save_feed(state: tauri::State<'_, AppState>, session_token: String, form: FeedForm) -> Result<i64, String> {
    let me = auth::require(&state, &session_token, Role::Manager)?;
    tx(&state, |c| save_feed(c, &form, &me.display_name()))
}
#[tauri::command]
pub fn pb_feed_move(state: tauri::State<'_, AppState>, session_token: String, form: FeedMoveForm) -> Result<i64, String> {
    let me = auth::require(&state, &session_token, Role::Manager)?;
    tx(&state, |c| feed_move(c, &form, &me.display_name()))
}
#[tauri::command]
pub fn pb_save_health(state: tauri::State<'_, AppState>, session_token: String, form: HealthForm) -> Result<i64, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    tx(&state, |c| save_health(c, &form, &me.display_name()))
}
#[tauri::command]
pub fn pb_mark_plan(state: tauri::State<'_, AppState>, session_token: String, id: i64, status: String) -> Result<(), String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    tx(&state, |c| mark_plan(c, id, &status, &today(), &me.display_name()))
}
#[tauri::command]
pub fn pb_save_weight(state: tauri::State<'_, AppState>, session_token: String, form: WeighForm) -> Result<i64, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    tx(&state, |c| save_weight(c, &form, &me.display_name()))
}
#[tauri::command]
pub fn pb_save_expense(state: tauri::State<'_, AppState>, session_token: String, form: ExpenseForm) -> Result<i64, String> {
    let me = auth::require(&state, &session_token, Role::Manager)?;
    tx(&state, |c| save_expense(c, &form, &me))
}
#[tauri::command]
pub fn pb_approve_expense(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<(), String> {
    let me = auth::require(&state, &session_token, Role::Owner)?;
    tx(&state, |c| {
        c.execute("UPDATE pb_expenses SET approved=1,approved_by=?1 WHERE id=?2", params![me.display_name(), id]).map_err(e)?;
        crate::audit::log(c, &me.display_name(), "expense_approved", "pb_expense", Some(id), "");
        Ok(())
    })
}
#[tauri::command]
pub fn pb_save_sale(state: tauri::State<'_, AppState>, session_token: String, form: SaleForm) -> Result<SaleDone, String> {
    let me = auth::require(&state, &session_token, Role::Cashier)?;
    tx(&state, |c| save_sale(c, &form, &me))
}
#[tauri::command]
pub fn pb_save_limits(state: tauri::State<'_, AppState>, session_token: String, limits: Limits) -> Result<(), String> {
    let me = auth::require(&state, &session_token, Role::Owner)?;
    for v in [limits.warn_pct, limits.low_pct, limits.mortality_pct] { if !(0.0..=100.0).contains(&v) { return Err("Percentages are from 0 to 100".into()); } }
    if !(0.0..=60.0).contains(&limits.critical_days) || !(1..=60).contains(&limits.weigh_days) { return Err("Days must be between 1 and 60".into()); }
    tx(&state, |c| {
        for (k, v) in [("pb_warn_pct", limits.warn_pct), ("pb_low_pct", limits.low_pct), ("pb_critical_days", limits.critical_days), ("pb_mortality_pct", limits.mortality_pct), ("pb_weigh_days", limits.weigh_days as f64)] { crate::db::set_setting(c, k, &v.to_string())?; }
        crate::audit::log(c, &me.display_name(), "livestock_limits", "settings", None, "Changed livestock alert limits");
        Ok(())
    })
}
/// Owner only, and always logged: records are never removed silently.
#[tauri::command]
pub fn pb_delete(state: tauri::State<'_, AppState>, session_token: String, kind: String, id: i64, reason: String) -> Result<(), String> {
    let me = auth::require(&state, &session_token, Role::Owner)?;
    if reason.trim().len() < 3 { return Err("Say why this record is being removed".into()); }
    let table = match kind.as_str() { "daily" => "pb_daily", "health" => "pb_health", "weight" => "pb_weights", "expense" => "pb_expenses", "feed_move" => "pb_feed_moves", "reservation" => "pb_sales", _ => return Err("Unknown record".into()) };
    tx(&state, |c| {
        if kind == "feed_move" && c.query_row("SELECT kind FROM pb_feed_moves WHERE id=?1", [id], |r| r.get::<_, String>(0)).optional().map_err(e)?.as_deref() == Some("issue") { return Err("Feed given to birds is changed on that day's record.".into()); }
        let extra = if kind == "reservation" { " AND status='reserved'" } else { "" };
        let n = c.execute(&format!("DELETE FROM {table} WHERE id=?1{extra}"), [id]).map_err(e)?;
        if n == 0 { return Err("Record not found (sold birds are undone by voiding the receipt).".into()); }
        crate::audit::log(c, &me.display_name(), "livestock_record_deleted", table, Some(id), reason.trim());
        Ok(())
    })
}

#[cfg(test)]
pub mod tests {
    use super::*;
    fn me(role: Role) -> Session { Session { user_id: 1, username: "o".into(), full_name: "Owner".into(), role, last_seen: std::time::Instant::now() } }
    fn conn() -> Connection {
        let c = crate::db::test_conn();
        c.execute("INSERT OR IGNORE INTO users(id,username,full_name,password_hash,role) VALUES(1,'o','Owner','x','owner')", []).unwrap();
        c
    }
    /// A plain "Sell produce" receipt for one item, paid in cash.
    pub fn pos_sale(c: &Connection, pid: i64, q: f64, price: i64) -> i64 {
        let input: crate::sales::CreateReceiptInput = serde_json::from_value(serde_json::json!({
            "discount_kobo": 0, "items": [{"product_id": pid, "quantity": q, "unit_price_kobo": price}], "payments": [{"method": "Cash", "amount_kobo": (q * price as f64) as i64}] })).unwrap();
        crate::sales::create_receipt_tx(c, input, &me(Role::Owner)).unwrap().id
    }
    fn batch(arrival: &str) -> BatchForm {
        BatchForm { name: "Batch 1".into(), bird_type: "Broilers".into(), planned_birds: 650, arrival_date: arrival.into(), age_at_arrival: 1, weeks: 8, sell_age_weeks: 8,
            bags_per_100: 17.0, bag_kg: 25.0, chicks_ordered: 650, chicks_delivered: 650, dead_on_arrival: 5, chicks_paid_for: 650, chick_price_kobo: 80_000, chick_transport_kobo: 2_000_000,
            chick_amount_paid_kobo: 50_000_000, price_per: "bird".into(), expected_price_kobo: 800_000, add_timetable: true, starter_pct: 30.0, starter_weeks: 4, ..Default::default() }
    }

    /// The farm's own example: 650 birds × 17 bags per 100 = 110.5 bags = 2,762.5 kg.
    #[test]
    fn feed_plan_matches_the_farm_example_and_the_cycle_balances() {
        let c = conn();
        let start = plus(&today(), -20);
        let id = save_batch(&c, &batch(&start), "M").unwrap();
        let s = summary(&c, id).unwrap();
        assert_eq!((s.planned_bags, s.planned_kg, s.buy_bags_plan), (110.5, 2762.5, 111));
        // 30% starter / 70% finisher of the whole bags (111): 33 starter, 78 finisher.
        assert_eq!((s.plan_starter, s.plan_finisher), (33.0, 78.0));
        assert_eq!(s.initial_live, 645);
        assert_eq!(s.age_days, 21);
        assert_eq!(s.plan.len(), BROILER_TIMETABLE.len());
        assert!(s.plan.iter().any(|p| p.name.starts_with("Newcastle vaccine") && p.state == "overdue"), "day 7 is past, not done");
        // Feed store: 20 bags of starter at ₦15,000, then 2 bags a day.
        let feed = save_feed(&c, &FeedForm { name: "Broiler starter".into(), bag_kg: 25.0, min_bags: 10.0, full_bags: 40.0, opening_bags: 20.0, opening_price_kobo: 1_500_000, ..Default::default() }, "M").unwrap();
        for i in 0..3 {
            save_daily(&c, &DailyForm { batch_id: id, day: plus(&today(), i - 2), deaths: 2, feed_id: Some(feed), kg_given: 50.0, disposed: true, ..Default::default() }, "W").unwrap();
        }
        let f = feed_stock(&c, feed).unwrap();
        assert_eq!(f.available, 14.0);
        assert_eq!(f.daily_bags, 2.0);
        assert_eq!(f.days_left, Some(7.0));
        assert_eq!(f.buy_bags, 26.0, "target 40 − 14 available");
        assert_eq!(f.level, "reducing", "14 of 40 bags = 35%");
    }

    /// The farm's 600-bird example: 102 bags = 31 starter (775 kg) + 71 finisher (1,775 kg);
    /// a day of 27 kg takes 1 bag and 2 kg out of the store exactly.
    #[test]
    fn six_hundred_birds_split_and_daily_kg() {
        let c = conn();
        let id = save_batch(&c, &BatchForm { planned_birds: 600, chicks_delivered: 600, chicks_ordered: 600, dead_on_arrival: 0, starter_pct: 30.0, starter_weeks: 4, ..batch(&plus(&today(), -2)) }, "M").unwrap();
        let s = summary(&c, id).unwrap();
        assert_eq!((s.planned_bags, s.plan_starter, s.plan_finisher, s.starter_kg, s.finisher_kg), (102.0, 31.0, 71.0, 775.0, 1775.0));
        assert_eq!((s.stage.as_str(), s.plan_kg_today), ("Starter", 27.7), "775 kg over 28 days");
        let feed = save_feed(&c, &FeedForm { name: "Starter".into(), bag_kg: 25.0, opening_bags: 30.0, opening_price_kobo: 1_500_000, ..Default::default() }, "M").unwrap();
        save_daily(&c, &DailyForm { batch_id: id, day: today(), feed_id: Some(feed), kg_given: 27.0, ..Default::default() }, "W").unwrap();
        let f = feed_stock(&c, feed).unwrap();
        assert_eq!(f.available_kg, 723.0);
        assert!((f.available - 28.92).abs() < 1e-9);
        // Birds: started 600, 1 died, 50 sold → 549.
        save_daily(&c, &DailyForm { batch_id: id, day: plus(&today(), -1), deaths: 1, ..Default::default() }, "W").unwrap();
        save_sale(&c, &SaleForm { batch_id: id, sale_date: today(), birds: 50, price_kobo: 1_000_000, paid_kobo: 50_000_000, method: "Cash".into(), ..Default::default() }, &me(Role::Owner)).unwrap();
        let s = summary(&c, id).unwrap();
        assert_eq!((s.initial_live, s.deaths, s.sold_birds, s.alive), (600, 1, 50, 549));
    }

    #[test]
    fn levels_withdrawal_sales_and_profit() {
        let c = conn();
        let id = save_batch(&c, &batch(&plus(&today(), -40)), "M").unwrap();
        let feed = save_feed(&c, &FeedForm { name: "Finisher".into(), bag_kg: 25.0, min_bags: 5.0, full_bags: 40.0, opening_bags: 30.0, opening_price_kobo: 1_000_000, ..Default::default() }, "M").unwrap();
        save_daily(&c, &DailyForm { batch_id: id, day: today(), deaths: 5, feed_id: Some(feed), kg_given: 500.0, ..Default::default() }, "W").unwrap();
        let f = feed_stock(&c, feed).unwrap();
        assert_eq!((f.available, f.level.as_str()), (10.0, "urgent"), "10 of 40 = 25%, and 20 bags a day leaves under a day");
        // More than the store has is refused in plain words.
        let err = save_daily(&c, &DailyForm { batch_id: id, day: plus(&today(), -1), feed_id: Some(feed), kg_given: 251.0, ..Default::default() }, "W").unwrap_err();
        assert!(err.contains("Only 250 kg"), "{err}");
        // Medicine with a 5-day withdrawal blocks sale.
        save_health(&c, &HealthForm { batch_id: id, kind: "medicine".into(), name: "Antibiotic X".into(), done_date: plus(&today(), -2), end_date: today(), withdrawal_days: 5, success: true, ..Default::default() }, "V").unwrap();
        let s = summary(&c, id).unwrap();
        assert!(!s.eligible);
        let sale = SaleForm { batch_id: id, sale_date: today(), birds: 100, price_kobo: 800_000, paid_kobo: 50_000_000, method: "Cash".into(), buyer: "Mama Udeme".into(), ..Default::default() };
        assert!(save_sale(&c, &sale, &me(Role::Owner)).unwrap_err().contains("withdrawal"));
        c.execute("UPDATE pb_health SET withdrawal_days=0", []).unwrap();
        // Reserve 50, sell 100 (part paid): receipt, debt, birds and profit update.
        save_sale(&c, &SaleForm { status: "reserved".into(), birds: 50, ..sale.clone() }, &me(Role::Owner)).unwrap();
        let done = save_sale(&c, &sale, &me(Role::Owner)).unwrap();
        assert!(done.receipt_id.is_some());
        let s = summary(&c, id).unwrap();
        assert_eq!((s.alive, s.reserved_birds, s.sold_birds), (540, 50, 100));
        assert_eq!(s.revenue_kobo, 80_000_000);
        assert_eq!(s.customers_owe_kobo, 30_000_000);
        assert_eq!(s.feed_cost_kobo, 20_000_000);
        assert_eq!(s.total_cost_kobo, 52_000_000 + 2_000_000 + 20_000_000);
        assert_eq!(s.profit_kobo, 80_000_000 - 74_000_000);
        assert_eq!(s.remaining_value_kobo, 540 * 800_000);
        assert!(save_sale(&c, &SaleForm { birds: 500, ..sale.clone() }, &me(Role::Owner)).unwrap_err().contains("free to sell"));
        // Expenses by a manager wait for the owner.
        save_expense(&c, &ExpenseForm { batch_id: id, spent_date: today(), category: "Water".into(), item: "Borehole fuel".into(), quantity: 1.0, unit_price_kobo: 500_000, paid_kobo: 500_000, ..Default::default() }, &me(Role::Manager)).unwrap();
        assert_eq!(summary(&c, id).unwrap().pending_expenses, 1);
        // Weighing gives average, gain and FCR.
        save_weight(&c, &WeighForm { batch_id: id, weigh_date: plus(&today(), -7), sampled: 10, total_kg: 18.0, ..Default::default() }, "W").unwrap();
        save_weight(&c, &WeighForm { batch_id: id, weigh_date: today(), sampled: 10, total_kg: 25.0, ..Default::default() }, "W").unwrap();
        let s = summary(&c, id).unwrap();
        assert_eq!((s.latest_avg_kg, s.gain_kg, s.adg_g), (Some(2.5), Some(0.7), Some(100.0)));
        assert!(s.fcr.is_some());
        assert!(period_costs(&c, "2000-01-01", "2100-01-01", true).unwrap() >= 74_500_000);
        assert_eq!(period_costs(&c, "1990-01-01", "1990-01-31", true).unwrap(), 0, "a period with nothing in it");
        // A sale in "Sell produce" of Live birds comes off the batch too.
        let pid: i64 = c.query_row("SELECT id FROM products WHERE name='Live birds' AND unit='bird'", [], |r| r.get(0)).unwrap();
        let rid = pos_sale(&c, pid, 10.0, 800_000);
        let s = summary(&c, id).unwrap();
        assert_eq!((s.sold_birds, s.alive), (110, 530));
        assert!(c.query_row("SELECT 1 FROM pb_sales WHERE receipt_id=?1", [rid], |_| Ok(())).is_ok());
    }
}
