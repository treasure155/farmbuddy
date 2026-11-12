//! Weather and market information, all entered or imported by the farmer.
//!
//! FarmBuddy cannot know live weather or prices while offline. It keeps the
//! farm's own weather records, seasonal outlooks typed in or imported from
//! NiMet's published Seasonal Climate Prediction (with source and date), and
//! market prices observed by the farmer, with history and comparison. Demo
//! prices are labelled illustrative and never shown as current.

use crate::{auth::{self, Role}, farms::{check_len, check_opt_date, rows, valid_date}, AppState};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

fn range(v: Option<f64>, lo: f64, hi: f64, label: &str) -> Result<(), String> {
    match v { Some(x) if !(x.is_finite() && x >= lo && x <= hi) => Err(format!("{label} must be between {lo} and {hi}")), _ => Ok(()) }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Weather { #[serde(default)] pub id: i64, #[serde(default)] pub farm_id: Option<i64>, pub record_date: String, pub rainfall_mm: Option<f64>, pub temp_min_c: Option<f64>, pub temp_max_c: Option<f64>, #[serde(default)] pub conditions: String, #[serde(default)] pub notes: String, #[serde(default)] pub user_name: String }

pub fn list_weather_tx(conn: &Connection, from: Option<String>, to: Option<String>) -> Result<Vec<Weather>, String> {
    rows(conn, "SELECT id,farm_id,record_date,rainfall_mm,temp_min_c,temp_max_c,conditions,notes,user_name FROM weather_records WHERE (?1='' OR record_date>=?1) AND (?2='' OR record_date<=?2) ORDER BY record_date DESC, id DESC LIMIT 1000",
        params![from.unwrap_or_default(), to.unwrap_or_default()], |r| Ok(Weather { id: r.get(0)?, farm_id: r.get(1)?, record_date: r.get(2)?, rainfall_mm: r.get(3)?, temp_min_c: r.get(4)?, temp_max_c: r.get(5)?, conditions: r.get(6)?, notes: r.get(7)?, user_name: r.get(8)? }))
}

#[tauri::command]
pub fn list_weather(state: tauri::State<'_, AppState>, session_token: String, from: Option<String>, to: Option<String>) -> Result<Vec<Weather>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    let conn = state.db()?;
    list_weather_tx(&conn, from, to)
}

pub fn add_weather_tx(conn: &Connection, w: &Weather, who: &str) -> Result<i64, String> {
    if !valid_date(&w.record_date) { return Err("Choose the date".into()); }
    range(w.rainfall_mm, 0.0, 1000.0, "Rainfall (mm)")?;
    range(w.temp_min_c, -10.0, 55.0, "Minimum temperature")?;
    range(w.temp_max_c, -10.0, 55.0, "Maximum temperature")?;
    if let (Some(a), Some(b)) = (w.temp_min_c, w.temp_max_c) { if a > b { return Err("The minimum is above the maximum".into()); } }
    if w.rainfall_mm.is_none() && w.temp_min_c.is_none() && w.temp_max_c.is_none() && w.conditions.trim().is_empty() && w.notes.trim().is_empty() { return Err("Enter at least one reading or observation".into()); }
    check_len(&w.conditions, 80, "Conditions")?;
    check_len(&w.notes, 500, "Notes")?;
    conn.execute("INSERT INTO weather_records(farm_id,record_date,rainfall_mm,temp_min_c,temp_max_c,conditions,notes,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![w.farm_id, w.record_date, w.rainfall_mm, w.temp_min_c, w.temp_max_c, w.conditions.trim(), w.notes.trim(), who]).map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn add_weather(state: tauri::State<'_, AppState>, session_token: String, input: Weather) -> Result<i64, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    add_weather_tx(&*state.db()?, &input, &me.display_name())
}

#[tauri::command]
pub fn delete_weather(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    state.db()?.execute("DELETE FROM weather_records WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Serialize, Debug)]
pub struct MonthRain { pub month: String, pub rainfall_mm: f64, pub rain_days: i64, pub records: i64 }

pub fn rainfall_tx(conn: &Connection, year: i64) -> Result<Vec<MonthRain>, String> {
    rows(conn, "SELECT substr(record_date,1,7),COALESCE(SUM(rainfall_mm),0),SUM(CASE WHEN rainfall_mm>0 THEN 1 ELSE 0 END),COUNT(*) FROM weather_records WHERE substr(record_date,1,4)=?1 GROUP BY 1 ORDER BY 1",
        [year.to_string()], |r| Ok(MonthRain { month: r.get(0)?, rainfall_mm: r.get(1)?, rain_days: r.get(2)?, records: r.get(3)? }))
}

#[tauri::command]
pub fn rainfall_by_month(state: tauri::State<'_, AppState>, session_token: String, year: i64) -> Result<Vec<MonthRain>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    let conn = state.db()?;
    rainfall_tx(&conn, year)
}

// ------------------------------------------------------------------ seasonal outlooks

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Outlook { #[serde(default)] pub id: i64, pub year: i64, pub state: String, #[serde(default)] pub lga: String, #[serde(default)] pub onset_date: String, #[serde(default)] pub cessation_date: String, #[serde(default)] pub season_days: Option<i64>, #[serde(default)] pub dry_spell_notes: String, #[serde(default)] pub notes: String, pub source: String, #[serde(default)] pub published_date: String, #[serde(default)] pub imported_at: String }

pub fn list_outlooks_tx(conn: &Connection) -> Result<Vec<Outlook>, String> {
    rows(conn, "SELECT id,year,state,lga,onset_date,cessation_date,season_days,dry_spell_notes,notes,source,published_date,imported_at FROM climate_outlooks ORDER BY year DESC, state, lga", [], |r| Ok(Outlook {
        id: r.get(0)?, year: r.get(1)?, state: r.get(2)?, lga: r.get(3)?, onset_date: r.get(4)?, cessation_date: r.get(5)?, season_days: r.get(6)?, dry_spell_notes: r.get(7)?, notes: r.get(8)?, source: r.get(9)?, published_date: r.get(10)?, imported_at: r.get(11)?,
    }))
}

#[tauri::command]
pub fn list_outlooks(state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<Outlook>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    list_outlooks_tx(&*state.db()?)
}

/// Saves (or replaces, for the same year and place) a seasonal outlook. The source is required.
pub fn save_outlook_tx(conn: &Connection, o: &Outlook) -> Result<(), String> {
    if !(2000..=2100).contains(&o.year) { return Err("Enter the year".into()); }
    if o.state.trim().is_empty() { return Err("Choose the state".into()); }
    if o.source.trim().chars().count() < 3 { return Err("Enter the source, for example NiMet Seasonal Climate Prediction".into()); }
    for (d, l) in [(&o.onset_date, "Onset"), (&o.cessation_date, "Cessation"), (&o.published_date, "Published date")] { check_opt_date(d, l)?; }
    if !o.onset_date.is_empty() && !o.cessation_date.is_empty() && o.cessation_date < o.onset_date { return Err("Cessation is before onset".into()); }
    if let Some(d) = o.season_days { if !(1..=366).contains(&d) { return Err("Season length must be 1–366 days".into()); } }
    for (v, n, l) in [(&o.state, 60, "State"), (&o.lga, 80, "LGA"), (&o.dry_spell_notes, 500, "Dry spells"), (&o.notes, 500, "Notes"), (&o.source, 200, "Source")] { check_len(v, n, l)?; }
    conn.execute("INSERT INTO climate_outlooks(year,state,lga,onset_date,cessation_date,season_days,dry_spell_notes,notes,source,published_date,imported_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
        ON CONFLICT(year,state,lga) DO UPDATE SET onset_date=excluded.onset_date,cessation_date=excluded.cessation_date,season_days=excluded.season_days,dry_spell_notes=excluded.dry_spell_notes,notes=excluded.notes,source=excluded.source,published_date=excluded.published_date,imported_at=excluded.imported_at",
        params![o.year, o.state.trim(), o.lga.trim(), o.onset_date, o.cessation_date, o.season_days, o.dry_spell_notes.trim(), o.notes.trim(), o.source.trim(), o.published_date, crate::db::now_local()]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn save_outlook(state: tauri::State<'_, AppState>, session_token: String, input: Outlook) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    save_outlook_tx(&*state.db()?, &input)
}

#[tauri::command]
pub fn delete_outlook(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    state.db()?.execute("DELETE FROM climate_outlooks WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

// ------------------------------------------------------------------ market prices

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct MarketPrice {
    #[serde(default)] pub id: i64, pub product: String, #[serde(default)] pub market: String, #[serde(default)] pub state: String, #[serde(default)] pub lga: String, pub price_date: String, pub unit: String,
    pub wholesale_kobo: Option<i64>, pub retail_kobo: Option<i64>, pub buyer_offer_kobo: Option<i64>, #[serde(default)] pub quantity_available: String, #[serde(default)] pub source: String, #[serde(default)] pub notes: String,
    #[serde(default)] pub is_demo: bool, #[serde(default)] pub user_name: String,
}

pub fn list_prices_tx(conn: &Connection, product: &str) -> Result<Vec<MarketPrice>, String> {
    rows(conn, "SELECT id,product,market,state,lga,price_date,unit,wholesale_kobo,retail_kobo,buyer_offer_kobo,quantity_available,source,notes,is_demo,user_name FROM market_prices WHERE ?1='' OR lower(product)=lower(?1) ORDER BY price_date DESC, id DESC LIMIT 1000", [product], |r| Ok(MarketPrice {
        id: r.get(0)?, product: r.get(1)?, market: r.get(2)?, state: r.get(3)?, lga: r.get(4)?, price_date: r.get(5)?, unit: r.get(6)?, wholesale_kobo: r.get(7)?, retail_kobo: r.get(8)?, buyer_offer_kobo: r.get(9)?,
        quantity_available: r.get(10)?, source: r.get(11)?, notes: r.get(12)?, is_demo: r.get::<_, i64>(13)? == 1, user_name: r.get(14)?,
    }))
}

#[tauri::command]
pub fn list_market_prices(state: tauri::State<'_, AppState>, session_token: String, product: Option<String>) -> Result<Vec<MarketPrice>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    list_prices_tx(&*state.db()?, &product.unwrap_or_default())
}

pub fn add_price_tx(conn: &Connection, p: &MarketPrice, who: &str) -> Result<i64, String> {
    if p.product.trim().is_empty() { return Err("Enter the product".into()); }
    if !valid_date(&p.price_date) { return Err("Choose the date you saw the price".into()); }
    if p.unit.trim().is_empty() { return Err("Enter the unit, such as kg, bag or tuber".into()); }
    if p.wholesale_kobo.is_none() && p.retail_kobo.is_none() && p.buyer_offer_kobo.is_none() { return Err("Enter at least one price".into()); }
    for v in [p.wholesale_kobo, p.retail_kobo, p.buyer_offer_kobo].into_iter().flatten() { if !(0..=100_000_000_000).contains(&v) { return Err("Check the prices".into()); } }
    for (v, n, l) in [(&p.product, 80, "Product"), (&p.market, 80, "Market"), (&p.state, 60, "State"), (&p.lga, 80, "LGA"), (&p.unit, 30, "Unit"), (&p.quantity_available, 80, "Quantity"), (&p.source, 120, "Source"), (&p.notes, 500, "Notes")] { check_len(v, n, l)?; }
    conn.execute("INSERT INTO market_prices(product,market,state,lga,price_date,unit,wholesale_kobo,retail_kobo,buyer_offer_kobo,quantity_available,source,notes,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![p.product.trim(), p.market.trim(), p.state.trim(), p.lga.trim(), p.price_date, p.unit.trim(), p.wholesale_kobo, p.retail_kobo, p.buyer_offer_kobo, p.quantity_available.trim(), p.source.trim(), p.notes.trim(), who]).map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn add_market_price(state: tauri::State<'_, AppState>, session_token: String, input: MarketPrice) -> Result<i64, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    add_price_tx(&*state.db()?, &input, &me.display_name())
}

#[tauri::command]
pub fn delete_market_price(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    state.db()?.execute("DELETE FROM market_prices WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Serialize, Debug)]
pub struct PriceSummary { pub product: String, pub unit: String, pub market: String, pub observations: i64, pub latest_date: String, pub latest_wholesale_kobo: Option<i64>, pub latest_retail_kobo: Option<i64>, pub min_kobo: Option<i64>, pub max_kobo: Option<i64>, pub demo_only: bool }

/// Latest observation and range for each product, unit and market, so markets can be compared.
pub fn price_summary_tx(conn: &Connection) -> Result<Vec<PriceSummary>, String> {
    rows(conn, "SELECT m.product,m.unit,m.market,COUNT(*),MAX(m.price_date),
        (SELECT wholesale_kobo FROM market_prices x WHERE lower(x.product)=lower(m.product) AND x.unit=m.unit AND x.market=m.market ORDER BY x.price_date DESC, x.id DESC LIMIT 1),
        (SELECT retail_kobo FROM market_prices x WHERE lower(x.product)=lower(m.product) AND x.unit=m.unit AND x.market=m.market ORDER BY x.price_date DESC, x.id DESC LIMIT 1),
        MIN(COALESCE(m.wholesale_kobo,m.retail_kobo,m.buyer_offer_kobo)),MAX(COALESCE(m.retail_kobo,m.wholesale_kobo,m.buyer_offer_kobo)),MIN(m.is_demo)
        FROM market_prices m GROUP BY lower(m.product),m.unit,m.market ORDER BY m.product, m.market", [], |r| Ok(PriceSummary {
        product: r.get(0)?, unit: r.get(1)?, market: r.get(2)?, observations: r.get(3)?, latest_date: r.get(4)?, latest_wholesale_kobo: r.get(5)?, latest_retail_kobo: r.get(6)?, min_kobo: r.get(7)?, max_kobo: r.get(8)?, demo_only: r.get::<_, i64>(9)? == 1,
    }))
}

#[tauri::command]
pub fn market_price_summary(state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<PriceSummary>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    price_summary_tx(&*state.db()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weather_outlooks_and_prices() {
        let conn = crate::db::test_conn();
        assert!(add_weather_tx(&conn, &Weather { record_date: "2026-05-01".into(), ..Default::default() }, "W").is_err());
        assert!(add_weather_tx(&conn, &Weather { record_date: "2026-05-01".into(), temp_min_c: Some(30.0), temp_max_c: Some(20.0), ..Default::default() }, "W").is_err());
        add_weather_tx(&conn, &Weather { record_date: "2026-05-01".into(), rainfall_mm: Some(12.5), ..Default::default() }, "W").unwrap();
        let o = Outlook { year: 2026, state: "Akwa Ibom".into(), onset_date: "2026-03-10".into(), cessation_date: "2026-11-30".into(), source: "NiMet SCP 2026".into(), ..Default::default() };
        save_outlook_tx(&conn, &o).unwrap();
        save_outlook_tx(&conn, &Outlook { onset_date: "2026-03-15".into(), ..o.clone() }).unwrap();
        let all = list_outlooks_tx(&conn).unwrap();
        assert_eq!(all.len(), 1, "same year and place is replaced");
        assert_eq!(all[0].onset_date, "2026-03-15");
        let p = MarketPrice { product: "Garri".into(), market: "Itam".into(), price_date: "2026-05-01".into(), unit: "kg".into(), wholesale_kobo: Some(90_000), ..Default::default() };
        add_price_tx(&conn, &p, "W").unwrap();
        add_price_tx(&conn, &MarketPrice { price_date: "2026-05-08".into(), wholesale_kobo: Some(100_000), ..p.clone() }, "W").unwrap();
        assert!(add_price_tx(&conn, &MarketPrice { wholesale_kobo: None, ..p }, "W").is_err());
        let s = price_summary_tx(&conn).unwrap();
        assert_eq!((s[0].observations, s[0].latest_wholesale_kobo, s[0].min_kobo), (2, Some(100_000), Some(90_000)));
    }
}
