//! Farm-wide finance and production reports: profit and loss, cash flow,
//! profit by activity, balances, the farm finance summary for loan or grant
//! applications, and livestock, crop and processing reports.
//!
//! Rules that keep the figures honest (shown to the user with every report):
//! - Money is whole kobo; every report states its date range.
//! - Voided sales are excluded; returns and refunds are taken off.
//! - Credit sales count as sales, not as cash received.
//! - Purchases go into stock; they become costs only when the stock is sold
//!   (cost of goods sold) or used on the farm (production costs). Buying feed
//!   is therefore never counted twice.
//! - Processing costs carried into stock with the outputs are not counted as
//!   production costs again; they come back as cost of goods sold when sold.
//! - Estimated value of produce or animals not yet sold is never income.
//! - Demo records are left out unless asked for.

use crate::{auth::{self, Role}, farms::{check_len, rows, valid_date}, AppState};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

fn sum(conn: &Connection, sql: &str, from: &str, to: &str) -> Result<i64, String> {
    conn.query_row(sql, params![from, to], |r| r.get::<_, Option<i64>>(0)).map(|v| v.unwrap_or(0)).map_err(|e| e.to_string())
}

#[derive(Serialize, Debug, Default, Clone)]
pub struct Line { pub label: String, pub amount_kobo: i64 }

#[derive(Serialize, Debug, Default)]
pub struct ProfitLoss {
    pub from: String, pub to: String, pub include_demo: bool,
    pub revenue: Vec<Line>, pub revenue_kobo: i64,
    pub cost_of_sales_kobo: i64,
    pub production: Vec<Line>, pub production_kobo: i64,
    pub capitalised_to_stock_kobo: i64,
    pub gross_profit_kobo: i64,
    pub expenses: Vec<Line>, pub expenses_kobo: i64,
    pub net_profit_kobo: i64,
    pub cash_in: Vec<Line>, pub cash_in_kobo: i64,
    pub cash_out: Vec<Line>, pub cash_out_kobo: i64,
    pub net_cash_kobo: i64,
    pub owed_by_customers_kobo: i64, pub owed_to_suppliers_kobo: i64, pub inventory_value_kobo: i64,
    pub rules: Vec<String>,
}

pub fn profit_loss(conn: &Connection, from: &str, to: &str, include_demo: bool) -> Result<ProfitLoss, String> {
    if !valid_date(from) || !valid_date(to) || to < from { return Err("Choose a valid date range".into()); }
    let d = if include_demo { "1=1" } else { "is_demo=0" };
    let fd = if include_demo { "1=1" } else { "f.is_demo=0" };
    let cd = if include_demo { "1=1" } else { "c.is_demo=0" };
    let bd = if include_demo { "1=1" } else { "b.is_demo=0" };
    let rd = if include_demo { "1=1" } else { "r.is_demo=0" };
    let pd = if include_demo { "1=1" } else { "p.is_demo=0" };
    let mut p = ProfitLoss { from: from.into(), to: to.into(), include_demo, ..Default::default() };

    // ---- revenue (by sale date)
    let till = sum(conn, &format!("SELECT SUM(r.total_kobo) FROM receipts r WHERE r.payment_status<>'void' AND {rd} AND date(r.created_at) BETWEEN ?1 AND ?2"), from, to)?
        - sum(conn, &format!("SELECT SUM(t.value_kobo) FROM returns t JOIN receipts r ON r.id=t.receipt_id WHERE {rd} AND date(t.created_at) BETWEEN ?1 AND ?2"), from, to)?;
    let animals = sum(conn, &format!("SELECT SUM(s.amount_kobo) FROM flock_sales s JOIN flocks f ON f.id=s.flock_id WHERE {fd} AND s.sale_date BETWEEN ?1 AND ?2"), from, to)?;
    let eggs = sum(conn, &format!("SELECT SUM(e.amount_kobo) FROM egg_movements e JOIN flocks f ON f.id=e.flock_id WHERE {fd} AND e.kind='sold' AND e.move_date BETWEEN ?1 AND ?2"), from, to)?;
    let harvest = sum(conn, &format!("SELECT SUM(h.sale_amount_kobo) FROM harvest_records h JOIN crop_cycles c ON c.id=h.cycle_id WHERE {cd} AND h.harvest_date BETWEEN ?1 AND ?2"), from, to)?;
    let processed = sum(conn, &format!("SELECT SUM(o.sale_amount_kobo) FROM processing_outputs o JOIN processing_batches b ON b.id=o.batch_id WHERE {bd} AND o.entry_date BETWEEN ?1 AND ?2"), from, to)?;
    let rund = if include_demo { "1=1" } else { "x.is_demo=0" };
    // Sales from runs make receipts (counted above); only sales kept before 1.0.5 have none.
    let run_sales = sum(conn, &format!("SELECT SUM(s.amount_kobo) FROM run_sales s JOIN palm_runs x ON x.id=s.run_id WHERE {rund} AND s.receipt_id IS NULL AND s.sale_date BETWEEN ?1 AND ?2"), from, to)?;
    let run_costs = sum(conn, &format!("SELECT SUM(l.amount_kobo) FROM palm_run_lines l JOIN palm_runs x ON x.id=l.run_id WHERE {rund} AND l.line_type IN ('fruit','cost') AND l.entry_date BETWEEN ?1 AND ?2"), from, to)?
        + crate::poultry::period_costs(conn, from, to, include_demo)?
        + sum(conn, &format!("SELECT SUM(s.transport_kobo+s.other_kobo) FROM run_sales s JOIN palm_runs x ON x.id=s.run_id LEFT JOIN receipts r ON r.id=s.receipt_id WHERE {rund} AND (r.id IS NULL OR r.payment_status<>'void') AND s.sale_date BETWEEN ?1 AND ?2"), from, to)?;
    p.revenue = vec![
        Line { label: "Produce sales on receipts (after returns)".into(), amount_kobo: till },
        Line { label: "Animals sold from batches".into(), amount_kobo: animals },
        Line { label: "Eggs sold".into(), amount_kobo: eggs },
        Line { label: "Crops sold at harvest".into(), amount_kobo: harvest },
        Line { label: "Processed products sold from batches".into(), amount_kobo: processed },
        Line { label: "Run sales recorded before receipts (1.0.4 and earlier)".into(), amount_kobo: run_sales },
    ];
    p.revenue_kobo = p.revenue.iter().map(|l| l.amount_kobo).sum();

    // ---- cost of goods sold through the till
    p.cost_of_sales_kobo = sum(conn, &format!("SELECT CAST(SUM(ROUND(i.unit_cost_kobo*i.quantity)) AS INTEGER) FROM receipt_items i JOIN receipts r ON r.id=i.receipt_id WHERE r.payment_status<>'void' AND {rd} AND date(r.created_at) BETWEEN ?1 AND ?2"), from, to)?
        - sum(conn, &format!("SELECT SUM(x.cost_kobo) FROM return_items x JOIN returns t ON t.id=x.return_id JOIN receipts r ON r.id=t.receipt_id WHERE {rd} AND date(t.created_at) BETWEEN ?1 AND ?2"), from, to)?;

    // ---- production costs (when the cost was incurred)
    let stock_purchase = sum(conn, &format!("SELECT SUM(f.chick_cost_kobo) FROM flocks f WHERE {fd} AND f.placed_date BETWEEN ?1 AND ?2"), from, to)?
        + sum(conn, &format!("SELECT SUM(x.cost_kobo) FROM flock_receipts x JOIN flocks f ON f.id=x.flock_id WHERE {fd} AND x.receipt_date BETWEEN ?1 AND ?2"), from, to)?;
    let livestock = sum(conn, &format!("SELECT SUM(x.amount_kobo) FROM flock_costs x JOIN flocks f ON f.id=x.flock_id WHERE {fd} AND x.cost_date BETWEEN ?1 AND ?2"), from, to)?;
    let crops = sum(conn, &format!("SELECT SUM(c.planting_cost_kobo) FROM crop_cycles c WHERE {cd} AND c.planting_date BETWEEN ?1 AND ?2"), from, to)?
        + sum(conn, &format!("SELECT SUM(a.labour_kobo+a.other_cost_kobo+a.input_cost_kobo) FROM crop_activities a JOIN crop_cycles c ON c.id=a.cycle_id WHERE {cd} AND a.activity_date BETWEEN ?1 AND ?2"), from, to)?
        + sum(conn, &format!("SELECT SUM(h.labour_kobo+h.transport_kobo+h.other_cost_kobo) FROM harvest_records h JOIN crop_cycles c ON c.id=h.cycle_id WHERE {cd} AND h.harvest_date BETWEEN ?1 AND ?2"), from, to)?
        + sum(conn, &format!("SELECT SUM(x.cost_kobo) FROM crop_health_records x JOIN crop_cycles c ON c.id=x.cycle_id WHERE {cd} AND x.observed_date BETWEEN ?1 AND ?2"), from, to)?;
    let processing = sum(conn, &format!("SELECT SUM(i.amount_kobo) FROM processing_inputs i JOIN processing_batches b ON b.id=i.batch_id WHERE {bd} AND i.entry_date BETWEEN ?1 AND ?2"), from, to)?;
    let storage = sum(conn, "SELECT SUM(m.cost_kobo) FROM storage_movements m JOIN warehouses w ON w.id=m.warehouse_id WHERE m.move_date BETWEEN ?1 AND ?2", from, to)?;
    p.production = vec![
        Line { label: "Animals bought (placed and received)".into(), amount_kobo: stock_purchase },
        Line { label: "Livestock running costs (feed, vaccines, labour…)".into(), amount_kobo: livestock },
        Line { label: "Crop costs (planting, field work, crop health, harvest)".into(), amount_kobo: crops },
        Line { label: "Processing costs (raw materials and processing)".into(), amount_kobo: processing },
        Line { label: "Runs and livestock batches: buying, feed used, health, workers, transport and other costs".into(), amount_kobo: run_costs },
        Line { label: "Storage costs".into(), amount_kobo: storage },
    ];
    // Cost carried into stock by processing batches closed in the period: it returns as cost of goods sold.
    let batches: Vec<i64> = rows(conn, &format!("SELECT b.id FROM processing_batches b WHERE {bd} AND b.status='closed' AND b.end_date BETWEEN ?1 AND ?2"), params![from, to], |r| r.get(0))?;
    for id in batches {
        let det = crate::processing::batch_detail(conn, id)?;
        p.capitalised_to_stock_kobo += det.outputs.iter().filter(|o| o.added_to_stock && o.output_type == "main").map(|o| ((o.unit_cost_kobo as f64) * (o.quantity - o.sold_qty)).round() as i64).sum::<i64>();
    }
    p.capitalised_to_stock_kobo += crate::palm_oil::carried_to_stock(conn, from, to, include_demo)?;
    p.production_kobo = p.production.iter().map(|l| l.amount_kobo).sum::<i64>() - p.capitalised_to_stock_kobo;
    p.gross_profit_kobo = p.revenue_kobo - p.cost_of_sales_kobo - p.production_kobo;

    // ---- operating expenses
    p.expenses = rows(conn, &format!("SELECT category,SUM(amount_kobo) FROM expenses WHERE {d} AND expense_date BETWEEN ?1 AND ?2 GROUP BY category ORDER BY 2 DESC"), params![from, to], |r| Ok(Line { label: r.get(0)?, amount_kobo: r.get(1)? }))?;
    p.expenses_kobo = p.expenses.iter().map(|l| l.amount_kobo).sum();
    p.net_profit_kobo = p.gross_profit_kobo - p.expenses_kobo;

    // ---- cash flow (when money actually moved)
    let till_cash = sum(conn, &format!("SELECT SUM(x.amount_kobo) FROM receipt_payments x JOIN receipts r ON r.id=x.receipt_id WHERE r.payment_status<>'void' AND {rd} AND date(x.created_at) BETWEEN ?1 AND ?2"), from, to)?
        - sum(conn, &format!("SELECT SUM(t.refund_paid_kobo) FROM returns t JOIN receipts r ON r.id=t.receipt_id WHERE {rd} AND date(t.created_at) BETWEEN ?1 AND ?2"), from, to)?;
    p.cash_in = vec![
        Line { label: "Payments received for produce sales (less refunds)".into(), amount_kobo: till_cash },
        Line { label: "Direct farm sales (animals, eggs, harvest, processed)".into(), amount_kobo: animals + eggs + harvest + processed + run_sales },
    ];
    p.cash_in_kobo = p.cash_in.iter().map(|l| l.amount_kobo).sum();
    // Paid at the time of purchase (amount paid less later payments recorded against it), plus later payments made in the period.
    let supplier_cash = sum(conn, &format!("SELECT SUM(p.amount_paid_kobo-(SELECT COALESCE(SUM(x.amount_kobo),0) FROM supplier_payments x WHERE x.purchase_id=p.id)) FROM purchases p WHERE {pd} AND date(p.created_at) BETWEEN ?1 AND ?2"), from, to)?
        + sum(conn, "SELECT SUM(x.amount_kobo) FROM supplier_payments x WHERE date(x.created_at) BETWEEN ?1 AND ?2", from, to)?;
    // Costs paid directly (not taken from stock, which was paid for as a purchase).
    let direct = stock_purchase
        + sum(conn, &format!("SELECT SUM(x.amount_kobo) FROM flock_costs x JOIN flocks f ON f.id=x.flock_id WHERE {fd} AND x.product_id IS NULL AND x.cost_date BETWEEN ?1 AND ?2"), from, to)?
        + sum(conn, &format!("SELECT SUM(c.planting_cost_kobo) FROM crop_cycles c WHERE {cd} AND c.planting_date BETWEEN ?1 AND ?2"), from, to)?
        + sum(conn, &format!("SELECT SUM(a.labour_kobo+a.other_cost_kobo) FROM crop_activities a JOIN crop_cycles c ON c.id=a.cycle_id WHERE {cd} AND a.activity_date BETWEEN ?1 AND ?2"), from, to)?
        + sum(conn, &format!("SELECT SUM(h.labour_kobo+h.transport_kobo+h.other_cost_kobo) FROM harvest_records h JOIN crop_cycles c ON c.id=h.cycle_id WHERE {cd} AND h.harvest_date BETWEEN ?1 AND ?2"), from, to)?
        + sum(conn, &format!("SELECT SUM(x.cost_kobo) FROM crop_health_records x JOIN crop_cycles c ON c.id=x.cycle_id WHERE {cd} AND x.observed_date BETWEEN ?1 AND ?2"), from, to)?
        + sum(conn, &format!("SELECT SUM(i.amount_kobo) FROM processing_inputs i JOIN processing_batches b ON b.id=i.batch_id WHERE {bd} AND i.product_id IS NULL AND i.entry_date BETWEEN ?1 AND ?2"), from, to)?
        + storage
        + run_costs;
    p.cash_out = vec![
        Line { label: "Paid to suppliers (purchases and later payments)".into(), amount_kobo: supplier_cash },
        Line { label: "Farm costs paid directly".into(), amount_kobo: direct },
        Line { label: "Expenses".into(), amount_kobo: p.expenses_kobo },
    ];
    p.cash_out_kobo = p.cash_out.iter().map(|l| l.amount_kobo).sum();
    p.net_cash_kobo = p.cash_in_kobo - p.cash_out_kobo;

    // ---- balances today
    p.owed_by_customers_kobo = conn.query_row(&format!("SELECT COALESCE(SUM({}),0) FROM receipts r WHERE r.payment_status IN ('partial','unpaid') AND {rd}", crate::sales::OWED_SQL), [], |r| r.get(0)).map_err(|e| e.to_string())?;
    p.owed_to_suppliers_kobo = conn.query_row(&format!("SELECT COALESCE(SUM(MAX(0,total_kobo-amount_paid_kobo)),0) FROM purchases WHERE {d}"), [], |r| r.get(0)).map_err(|e| e.to_string())?;
    p.inventory_value_kobo = inventory_value(conn, include_demo)?;
    p.rules = vec![
        format!("Period: {from} to {to}. Money is in whole kobo and shown in naira."),
        "Voided sales are left out; returns and refunds are taken off.".into(),
        "Credit sales count as sales on the sale date but as cash only when paid.".into(),
        "Purchases go into stock. Stock becomes a cost when it is sold (cost of goods sold) or used on the farm (production costs), so nothing is counted twice.".into(),
        "Processing costs carried into stock with finished products are taken out of production costs; they return as cost of goods sold when those products are sold.".into(),
        "Harvests and animals not yet sold are not income. Produce put into stock keeps the item's cost price.".into(),
        if include_demo { "Demo records are included.".into() } else { "Demo records are left out.".into() },
    ];
    Ok(p)
}

pub fn inventory_value(conn: &Connection, include_demo: bool) -> Result<i64, String> {
    conn.query_row(&format!("SELECT CAST(COALESCE(SUM(ROUND(MAX(0,stock_qty)*cost_price_kobo)),0) AS INTEGER) FROM products WHERE active=1 AND track_stock=1 {}", if include_demo { "" } else { "AND is_demo=0" }), [], |r| r.get(0)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn farm_profit_loss(state: tauri::State<'_, AppState>, session_token: String, from: String, to: String, include_demo: Option<bool>) -> Result<ProfitLoss, String> {
    auth::require_money(&state, &session_token)?;
    profit_loss(&*state.db()?, &from, &to, include_demo.unwrap_or(false))
}

// ------------------------------------------------------------------ profit by activity and production reports

#[derive(Serialize, Debug)]
pub struct ActivityResult { pub kind: String, pub id: i64, pub name: String, pub detail: String, pub status: String, pub started: String, pub revenue_kobo: i64, pub cost_kobo: i64, pub profit_kobo: i64, pub unit_cost: String, pub is_demo: bool }

/// Every livestock batch, crop and processing batch started in the period, with its lifetime figures.
pub fn by_activity(conn: &Connection, from: &str, to: &str, include_demo: bool) -> Result<Vec<ActivityResult>, String> {
    let mut out = Vec::new();
    let flocks: Vec<i64> = rows(conn, "SELECT id FROM flocks WHERE placed_date BETWEEN ?1 AND ?2 AND (?3=1 OR is_demo=0) ORDER BY placed_date", params![from, to, include_demo as i64], |r| r.get(0))?;
    for id in flocks {
        let f = crate::flocks::summary(conn, id)?;
        out.push(ActivityResult { kind: "livestock".into(), id, name: f.code.clone(), detail: format!("{} · {} alive of {}", f.species, f.alive, f.total_in), status: f.status.clone(), started: f.placed_date.clone(),
            revenue_kobo: f.revenue_kobo, cost_kobo: f.costs_kobo, profit_kobo: f.profit_kobo, unit_cost: format!("{} per animal", crate::db::format_money("₦", f.cost_per_bird_kobo)), is_demo: f.is_demo });
    }
    let cycles: Vec<i64> = rows(conn, "SELECT id FROM crop_cycles WHERE planting_date BETWEEN ?1 AND ?2 AND (?3=1 OR is_demo=0) ORDER BY planting_date", params![from, to, include_demo as i64], |r| r.get(0))?;
    for id in cycles {
        let c = crate::crops::cycle_summary(conn, id)?;
        let health: i64 = conn.query_row("SELECT COALESCE(SUM(cost_kobo),0) FROM crop_health_records WHERE cycle_id=?1", [id], |r| r.get(0)).map_err(|e| e.to_string())?;
        let cost = c.total_cost_kobo + health;
        let unit = c.harvests.first().map(|h| if h.quantity > 0.0 { format!("{} per {}", crate::db::format_money("₦", (cost as f64 / h.quantity).round() as i64), h.unit) } else { String::new() }).unwrap_or_default();
        out.push(ActivityResult { kind: "crop".into(), id, name: format!("{} on {}", c.crop, c.plot_code), detail: c.harvests.first().map(|h| format!("{} {} harvested", crate::db::format_qty(h.quantity), h.unit)).unwrap_or_else(|| "Not harvested yet".into()),
            status: c.status.clone(), started: c.planting_date.clone(), revenue_kobo: c.revenue_kobo, cost_kobo: cost, profit_kobo: c.revenue_kobo - cost, unit_cost: unit, is_demo: c.is_demo });
    }
    let batches: Vec<i64> = rows(conn, "SELECT id FROM processing_batches WHERE start_date BETWEEN ?1 AND ?2 AND (?3=1 OR is_demo=0) ORDER BY start_date", params![from, to, include_demo as i64], |r| r.get(0))?;
    for id in batches {
        let d = crate::processing::batch_detail(conn, id)?;
        let unit = d.outputs.iter().filter(|o| o.output_type == "main" && o.unit_cost_kobo > 0).map(|o| format!("{} per {} of {}", crate::db::format_money("₦", o.unit_cost_kobo), o.unit, o.name)).collect::<Vec<_>>().join("; ");
        out.push(ActivityResult { kind: "processing".into(), id, name: d.batch.batch_no.clone(), detail: d.batch.process_type.replace('_', " "), status: d.batch.status.clone(), started: d.batch.start_date.clone(),
            revenue_kobo: d.batch.revenue_kobo, cost_kobo: d.batch.total_cost_kobo, profit_kobo: d.batch.profit_on_sold_kobo, unit_cost: unit, is_demo: d.batch.is_demo });
    }
    let runs: Vec<i64> = rows(conn, "SELECT id FROM palm_runs WHERE start_date BETWEEN ?1 AND ?2 AND (?3=1 OR is_demo=0) ORDER BY start_date", params![from, to, include_demo as i64], |r| r.get(0))?;
    for id in runs {
        let r = crate::palm_oil::run_detail(conn, id)?.run;
        let unit = r.cost_per_oil_unit_kobo.map(|c| format!("{} per {} of {}", crate::db::format_money("₦", c), r.oil_unit, r.main_label.to_lowercase())).unwrap_or_default();
        out.push(ActivityResult { kind: "processing".into(), id, name: r.run_no.clone(), detail: r.title.clone(), status: r.status.clone(), started: r.start_date.clone(),
            revenue_kobo: r.total_sales_kobo, cost_kobo: r.total_cost_kobo, profit_kobo: r.profit_kobo, unit_cost: unit, is_demo: r.is_demo });
    }
    Ok(out)
}

#[tauri::command]
pub fn profit_by_activity(state: tauri::State<'_, AppState>, session_token: String, from: String, to: String, include_demo: Option<bool>) -> Result<Vec<ActivityResult>, String> {
    auth::require_money(&state, &session_token)?;
    by_activity(&*state.db()?, &from, &to, include_demo.unwrap_or(false))
}

#[derive(Serialize, Debug, Default)]
pub struct ProductionReport {
    pub from: String, pub to: String,
    pub livestock: Vec<serde_json::Value>, pub crops: Vec<serde_json::Value>, pub processing: Vec<serde_json::Value>,
    pub vaccinations: Vec<serde_json::Value>, pub health: Vec<serde_json::Value>, pub weights: Vec<serde_json::Value>, pub feed: Vec<serde_json::Value>,
}

/// Livestock register with mortality, feed, weight, vaccination and health
/// history; crop register with activities, yield and loss; processing register
/// with inputs, outputs, yield and waste. Money is included only for managers.
pub fn production_report(conn: &Connection, from: &str, to: &str, with_money: bool) -> Result<ProductionReport, String> {
    if !valid_date(from) || !valid_date(to) || to < from { return Err("Choose a valid date range".into()); }
    let mut r = ProductionReport { from: from.into(), to: to.into(), ..Default::default() };
    let flocks: Vec<i64> = rows(conn, "SELECT id FROM flocks WHERE placed_date<=?2 AND (status='active' OR closed_date>=?1) ORDER BY placed_date", params![from, to], |x| x.get(0))?;
    for id in flocks {
        let f = crate::flocks::summary(conn, id)?;
        let deaths: i64 = conn.query_row("SELECT COALESCE(SUM(mortality),0) FROM flock_logs WHERE flock_id=?1 AND log_date BETWEEN ?2 AND ?3", params![id, from, to], |x| x.get(0)).map_err(|e| e.to_string())?;
        let feed: f64 = conn.query_row("SELECT COALESCE(SUM(feed_kg),0) FROM flock_logs WHERE flock_id=?1 AND log_date BETWEEN ?2 AND ?3", params![id, from, to], |x| x.get(0)).map_err(|e| e.to_string())?;
        let mut v = serde_json::json!({ "code": f.code, "species": f.species, "breed": f.breed, "placed_date": f.placed_date, "status": f.status, "total_in": f.total_in, "alive": f.alive, "dead": f.dead, "culled": f.culled, "sold": f.sold,
            "mortality_pct": f.mortality_pct, "deaths_in_period": deaths, "feed_in_period_kg": feed, "feed_total_kg": f.feed_kg, "fcr": f.fcr, "latest_weight_kg": f.latest_weight_kg, "eggs": f.eggs, "egg_stock": f.egg_stock, "is_demo": f.is_demo });
        if with_money { v["costs_kobo"] = f.costs_kobo.into(); v["revenue_kobo"] = f.revenue_kobo.into(); v["profit_kobo"] = f.profit_kobo.into(); }
        r.livestock.push(v);
        let w: Vec<serde_json::Value> = rows(conn, "SELECT log_date,avg_weight_kg FROM flock_logs WHERE flock_id=?1 AND avg_weight_kg>0 AND log_date BETWEEN ?2 AND ?3 ORDER BY log_date", params![id, from, to], |x| Ok(serde_json::json!({ "code": f.code, "date": x.get::<_, String>(0)?, "avg_weight_kg": x.get::<_, f64>(1)? })))?;
        r.weights.extend(w);
        let fb = crate::feed::batch_feed(conn, id, false)?;
        r.feed.push(serde_json::json!({ "code": f.code, "feed_kg": fb.feed_kg, "wasted_kg": fb.wasted_kg, "wastage_pct": fb.wastage_pct, "per_animal_kg": fb.feed_per_animal_kg, "fcr": fb.fcr }));
    }
    r.vaccinations = rows(conn, "SELECT f.code,v.vaccine_name,v.disease,v.due_date,v.status,v.administered_date,v.administered_by,v.source,v.reviewed_by FROM vaccination_events v JOIN flocks f ON f.id=v.flock_id WHERE (v.due_date BETWEEN ?1 AND ?2) OR (v.administered_date BETWEEN ?1 AND ?2) ORDER BY v.due_date",
        params![from, to], |x| Ok(serde_json::json!({ "code": x.get::<_, String>(0)?, "vaccine": x.get::<_, String>(1)?, "disease": x.get::<_, String>(2)?, "due": x.get::<_, String>(3)?, "status": x.get::<_, String>(4)?, "given": x.get::<_, String>(5)?, "by": x.get::<_, String>(6)?, "source": x.get::<_, String>(7)?, "confirmed_by": x.get::<_, String>(8)? })))?;
    r.health = rows(conn, "SELECT f.code,h.observed_date,h.issue,h.affected,h.action_taken,h.vet_consulted,h.treatment FROM health_records h JOIN flocks f ON f.id=h.flock_id WHERE h.observed_date BETWEEN ?1 AND ?2 ORDER BY h.observed_date",
        params![from, to], |x| Ok(serde_json::json!({ "code": x.get::<_, String>(0)?, "date": x.get::<_, String>(1)?, "issue": x.get::<_, String>(2)?, "affected": x.get::<_, i64>(3)?, "action": x.get::<_, String>(4)?, "vet": x.get::<_, String>(5)?, "treatment": x.get::<_, String>(6)? })))?;
    let cycles: Vec<i64> = rows(conn, "SELECT id FROM crop_cycles WHERE planting_date<=?2 AND (status='growing' OR closed_date>=?1) ORDER BY planting_date", params![from, to], |x| x.get(0))?;
    for id in cycles {
        let c = crate::crops::cycle_summary(conn, id)?;
        let mut v = serde_json::json!({ "crop": c.crop, "variety": c.variety, "plot": c.plot_code, "area": c.area, "area_unit": c.area_unit, "planted": c.planting_date, "status": c.status, "activities": c.activity_count,
            "harvests": c.harvests, "yield_per_area": c.yield_per_area, "loss_pct": c.loss_pct, "is_demo": c.is_demo });
        if with_money { v["cost_kobo"] = c.total_cost_kobo.into(); v["revenue_kobo"] = c.revenue_kobo.into(); v["margin_kobo"] = c.margin_kobo.into(); }
        r.crops.push(v);
    }
    let batches: Vec<i64> = rows(conn, "SELECT id FROM processing_batches WHERE start_date<=?2 AND (status='open' OR end_date>=?1) ORDER BY start_date", params![from, to], |x| x.get(0))?;
    for id in batches {
        let d = crate::processing::batch_detail(conn, id)?;
        let outputs: Vec<serde_json::Value> = d.outputs.iter().map(|o| serde_json::json!({ "name": o.name, "type": o.output_type, "quantity": o.quantity, "unit": o.unit, "unit_cost_kobo": if with_money { o.unit_cost_kobo } else { 0 } })).collect();
        let mut v = serde_json::json!({ "batch_no": d.batch.batch_no, "type": d.batch.process_type, "started": d.batch.start_date, "ended": d.batch.end_date, "status": d.batch.status, "raw_qty": d.batch.raw_qty, "raw_unit": d.batch.raw_unit,
            "outputs": outputs, "yield_pct": d.batch.yield_pct, "waste_pct": d.batch.waste_pct, "is_demo": d.batch.is_demo });
        if with_money { v["cost_kobo"] = d.batch.total_cost_kobo.into(); v["revenue_kobo"] = d.batch.revenue_kobo.into(); v["profit_on_sold_kobo"] = d.batch.profit_on_sold_kobo.into(); }
        r.processing.push(v);
    }
    Ok(r)
}

#[tauri::command]
pub fn get_production_report(state: tauri::State<'_, AppState>, session_token: String, from: String, to: String) -> Result<ProductionReport, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    production_report(&*state.db()?, &from, &to, me.role >= Role::Manager)
}

// ------------------------------------------------------------------ assets, liabilities, finance summary

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct FinanceItem { #[serde(default)] pub id: i64, pub kind: String, pub name: String, pub value_kobo: i64, pub as_of: String, #[serde(default)] pub notes: String }

pub fn list_finance_items_tx(conn: &Connection) -> Result<Vec<FinanceItem>, String> {
    rows(conn, "SELECT id,kind,name,value_kobo,as_of,notes FROM finance_items ORDER BY kind, name", [], |r| Ok(FinanceItem { id: r.get(0)?, kind: r.get(1)?, name: r.get(2)?, value_kobo: r.get(3)?, as_of: r.get(4)?, notes: r.get(5)? }))
}

#[tauri::command]
pub fn list_finance_items(state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<FinanceItem>, String> {
    auth::require_money(&state, &session_token)?;
    list_finance_items_tx(&*state.db()?)
}

#[tauri::command]
pub fn save_finance_item(state: tauri::State<'_, AppState>, session_token: String, input: FinanceItem) -> Result<i64, String> {
    auth::require(&state, &session_token, Role::Owner)?;
    let conn = state.db()?;
    if !["asset", "liability"].contains(&input.kind.as_str()) { return Err("Choose asset or liability".into()); }
    if input.name.trim().is_empty() { return Err("Describe the item".into()); }
    check_len(&input.name, 120, "Name")?;
    check_len(&input.notes, 500, "Notes")?;
    if input.value_kobo < 0 { return Err("Value cannot be negative".into()); }
    if !valid_date(&input.as_of) { return Err("Choose the date of this value".into()); }
    if input.id > 0 {
        conn.execute("UPDATE finance_items SET kind=?1,name=?2,value_kobo=?3,as_of=?4,notes=?5 WHERE id=?6", params![input.kind, input.name.trim(), input.value_kobo, input.as_of, input.notes.trim(), input.id]).map_err(|e| e.to_string())?;
        Ok(input.id)
    } else {
        conn.execute("INSERT INTO finance_items(kind,name,value_kobo,as_of,notes) VALUES(?1,?2,?3,?4,?5)", params![input.kind, input.name.trim(), input.value_kobo, input.as_of, input.notes.trim()]).map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }
}

#[tauri::command]
pub fn delete_finance_item(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Owner)?;
    state.db()?.execute("DELETE FROM finance_items WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Serialize, Debug)]
pub struct FinanceSummary {
    pub generated_at: String, pub business: serde_json::Value, pub farms: Vec<crate::farms::Farm>,
    pub profit_loss: ProfitLoss, pub activities: Vec<ActivityResult>,
    pub assets: Vec<FinanceItem>, pub liabilities: Vec<FinanceItem>, pub assets_kobo: i64, pub liabilities_kobo: i64,
    pub production: serde_json::Value, pub disclaimer: String,
}

/// Everything a farmer usually needs to organise for a loan or grant
/// application, from their own records. It is a summary, not an application.
pub fn finance_summary_tx(conn: &Connection, from: &str, to: &str) -> Result<FinanceSummary, String> {
    let s = crate::db::all_settings(conn)?;
    let items = list_finance_items_tx(conn)?;
    let (assets, liabilities): (Vec<_>, Vec<_>) = items.into_iter().partition(|i| i.kind == "asset");
    let o = crate::farms::overview_tx(conn)?;
    Ok(FinanceSummary {
        generated_at: crate::db::now_local(),
        business: serde_json::json!({ "name": s.get("business_name"), "address": s.get("business_address"), "phone": s.get("business_phone"), "email": s.get("business_email") }),
        farms: crate::farms::list_farms_tx(conn, false)?.into_iter().filter(|f| !f.is_demo).collect(),
        assets_kobo: assets.iter().map(|a| a.value_kobo).sum(), liabilities_kobo: liabilities.iter().map(|a| a.value_kobo).sum(),
        assets, liabilities,
        profit_loss: profit_loss(conn, from, to, false)?,
        activities: by_activity(conn, from, to, false)?,
        production: serde_json::json!({ "live_animals": o.live_animals, "active_batches": o.active_batches, "by_species": o.by_species, "active_plots": o.active_plots, "growing_crops": o.growing_cycles, "open_processing": o.open_processing }),
        disclaimer: "Prepared by the farm from its own FarmBuddy records. FarmBuddy does not approve loans, guarantee finance or provide credit.".into(),
    })
}

#[tauri::command]
pub fn farm_finance_summary(state: tauri::State<'_, AppState>, session_token: String, from: String, to: String) -> Result<FinanceSummary, String> {
    auth::require(&state, &session_token, Role::Owner)?;
    finance_summary_tx(&*state.db()?, &from, &to)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profit_and_loss_counts_each_naira_once() {
        let conn = crate::db::test_conn();
        // Feed bought (inventory, not an expense) then issued to a batch (production cost).
        conn.execute("INSERT INTO products(sku,name,kind,unit,cost_price_kobo,stock_qty,track_stock) VALUES('F','Feed','material','kg',50_000,100,1)", []).unwrap();
        conn.execute("INSERT INTO purchases(purchase_no,total_kobo,amount_paid_kobo,created_at) VALUES('P1',5_000_000,5_000_000,'2026-05-01 10:00:00')", []).unwrap();
        conn.execute("INSERT INTO flocks(code,placed_date,chicks_placed,chick_cost_kobo) VALUES('B1','2026-05-01',100,10_000_000)", []).unwrap();
        conn.execute("INSERT INTO flock_costs(flock_id,cost_date,category,product_id,quantity,amount_kobo) VALUES(1,'2026-05-02','Feed',1,40,2_000_000)", []).unwrap();
        conn.execute("INSERT INTO flock_sales(flock_id,sale_date,birds,amount_kobo) VALUES(1,'2026-05-20',50,25_000_000)", []).unwrap();
        conn.execute("INSERT INTO expenses(expense_date,category,amount_kobo) VALUES('2026-05-10','Fuel',1_000_000)", []).unwrap();
        // A credit sale through the till: counted as a sale, not as cash.
        conn.execute("INSERT INTO receipts(receipt_no,subtotal_kobo,total_kobo,amount_paid_kobo,payment_status,customer_id,created_at) VALUES('R1',3_000_000,3_000_000,1_000_000,'partial',NULL,'2026-05-21 09:00:00')", []).unwrap();
        conn.execute("INSERT INTO receipt_items(receipt_id,product_name,product_kind,unit,quantity,unit_price_kobo,unit_cost_kobo,line_total_kobo) VALUES(1,'Eggs','product','crate',10,300_000,200_000,3_000_000)", []).unwrap();
        conn.execute("INSERT INTO receipt_payments(receipt_id,method,amount_kobo,created_at) VALUES(1,'Cash',1_000_000,'2026-05-21 09:00:00')", []).unwrap();
        // Demo records are left out by default.
        conn.execute("INSERT INTO flocks(code,placed_date,chicks_placed,chick_cost_kobo,is_demo) VALUES('DEMO','2026-05-01',500,99_000_000,1)", []).unwrap();
        let p = profit_loss(&conn, "2026-05-01", "2026-05-31", false).unwrap();
        assert_eq!(p.revenue_kobo, 28_000_000);
        assert_eq!(p.cost_of_sales_kobo, 2_000_000);
        assert_eq!(p.production_kobo, 12_000_000, "chicks + feed used; the feed purchase is not counted again");
        assert_eq!(p.gross_profit_kobo, 14_000_000);
        assert_eq!(p.net_profit_kobo, 13_000_000);
        assert_eq!(p.cash_in_kobo, 26_000_000, "only ₦10,000 of the credit sale was paid");
        assert_eq!(p.cash_out_kobo, 5_000_000 + 10_000_000 + 1_000_000, "supplier + chicks paid directly + expenses; feed from stock is not cash again");
        assert_eq!(p.owed_by_customers_kobo, 2_000_000);
        assert_eq!(p.inventory_value_kobo, 5_000_000);
        let with_demo = profit_loss(&conn, "2026-05-01", "2026-05-31", true).unwrap();
        assert_eq!(with_demo.production_kobo, 111_000_000);
    }

    #[test]
    fn processing_cost_carried_to_stock_is_not_double_counted() {
        let conn = crate::db::test_conn();
        conn.execute("INSERT INTO products(sku,name,kind,unit,stock_qty,track_stock) VALUES('PO','Palm oil','product','litre',0,1)", []).unwrap();
        let id = crate::processing::save_batch_tx(&conn, &crate::processing::BatchInput { batch_no: "PB".into(), process_type: "palm_oil".into(), start_date: "2026-06-01".into(), ..Default::default() }).unwrap();
        crate::processing::add_input_tx(&conn, &crate::processing::InputEntry { batch_id: id, entry_date: "2026-06-01".into(), kind: "raw".into(), name: "Bunches".into(), quantity: 100.0, unit: "bunch".into(), unit_cost_kobo: 200_000, ..Default::default() }, "M").unwrap();
        crate::processing::add_output_tx(&conn, &crate::processing::OutputEntry { batch_id: id, entry_date: "2026-06-02".into(), output_type: "main".into(), name: "Palm oil".into(), quantity: 100.0, unit: "litre".into(), cost_share_pct: 100.0, product_id: Some(1), ..Default::default() }, "M").unwrap();
        crate::processing::close_batch_tx(&conn, id, "2026-06-02", "M").unwrap();
        let p = profit_loss(&conn, "2026-06-01", "2026-06-30", false).unwrap();
        assert_eq!(p.capitalised_to_stock_kobo, 20_000_000);
        assert_eq!(p.production_kobo, 0, "the whole batch cost is now in stock");
        assert_eq!(p.inventory_value_kobo, 20_000_000);
        let acts = by_activity(&conn, "2026-06-01", "2026-06-30", false).unwrap();
        assert_eq!(acts.len(), 1);
        assert!(production_report(&conn, "2026-06-01", "2026-06-30", false).unwrap().processing[0].get("cost_kobo").is_none(), "no money for staff without access");
    }
}
