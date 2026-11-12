//! Test-only: writes the real JSON the farm commands return for the demo farm,
//! so the UI can be smoke-tested in a browser without the desktop shell.
//! Run with: FB_FIXTURES=/tmp/fixtures.json cargo test --lib ui_fixtures -- --ignored

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    #[test]
    #[ignore]
    fn dump_ui_fixtures() {
        let mut base = crate::db::test_conn();
        base.execute("INSERT INTO users(username,full_name,password_hash,role) VALUES('owner','Ada Owner','x','owner')", []).unwrap();
        let conn = base.transaction().unwrap();
        crate::demo::load_tx(&conn).unwrap();
        crate::db::set_setting(&conn, "onboarding_done", "1").unwrap();
        crate::db::set_setting(&conn, "business_name", "FarmBuddy Demo Farm").unwrap();
        let v = |x: Result<Value, serde_json::Error>| x.unwrap();
        let flock: i64 = conn.query_row("SELECT id FROM flocks WHERE code='BR-DEMO'", [], |r| r.get(0)).unwrap();
        let cycle: i64 = conn.query_row("SELECT id FROM crop_cycles LIMIT 1", [], |r| r.get(0)).unwrap();
        let batch: i64 = conn.query_row("SELECT id FROM processing_batches LIMIT 1", [], |r| r.get(0)).unwrap();
        let palm: i64 = conn.query_row("SELECT id FROM palm_runs LIMIT 1", [], |r| r.get(0)).unwrap();
        let article: i64 = conn.query_row("SELECT id FROM knowledge_articles WHERE slug='vaccination-records'", [], |r| r.get(0)).unwrap();
        let ids: Vec<i64> = crate::farms::rows(&conn, "SELECT id FROM flocks ORDER BY id", [], |r| r.get(0)).unwrap();
        let flocks: Vec<_> = ids.iter().map(|id| crate::flocks::summary(&conn, *id).unwrap()).collect();
        let goats: i64 = conn.query_row("SELECT id FROM flocks WHERE code='GT-DEMO'", [], |r| r.get(0)).unwrap();
        let pond: i64 = conn.query_row("SELECT id FROM fish_ponds LIMIT 1", [], |r| r.get(0)).unwrap();
        let store: i64 = conn.query_row("SELECT id FROM warehouses WHERE name LIKE 'Main store%'", [], |r| r.get(0)).unwrap();
        let year: i64 = crate::farms::today_str()[..4].parse().unwrap();
        // Overview with the money cards a manager sees.
        let mut overview = crate::farms::overview_tx(&conn).unwrap();
        overview.money = Some(crate::farms::overview_money(&conn).unwrap());
        let out = json!({
            "get_settings": crate::db::all_settings(&conn).unwrap(),
            "farm_overview": v(serde_json::to_value(&overview)),
            "list_farms": v(serde_json::to_value(crate::farms::list_farms_tx(&conn, true).unwrap())),
            "list_flocks": v(serde_json::to_value(&flocks)),
            "get_flock": v(serde_json::to_value(crate::flocks::flock_detail(&conn, flock).unwrap())),
            "get_flock_by_id": v(serde_json::to_value(ids.iter().map(|id| (id.to_string(), crate::flocks::flock_detail(&conn, *id).unwrap())).collect::<std::collections::HashMap<_, _>>())),
            "list_vaccinations": v(serde_json::to_value(crate::health::list_vacc_tx(&conn, None, false).unwrap())),
            "list_health_records": v(serde_json::to_value(crate::health::list_health_tx(&conn, None).unwrap())),
            "list_plots": v(serde_json::to_value(crate::crops::list_plots_tx(&conn, true).unwrap())),
            "list_crop_cycles": v(serde_json::to_value(vec![crate::crops::cycle_summary(&conn, cycle).unwrap()])),
            "get_crop_cycle": v(serde_json::to_value(crate::crops::cycle_detail(&conn, cycle).unwrap())),
            "process_products": v(serde_json::to_value(crate::palm_oil::product_options(&conn).unwrap())),
            "list_palm_runs": v(serde_json::to_value(vec![crate::palm_oil::run_detail(&conn, palm).unwrap().run])),
            "record_run_sale": serde_json::json!({"sale_id": 1, "receipt_id": 1, "receipt_no": "RCP-2611-00001", "run_finished": false}),
            "get_palm_run": v(serde_json::to_value(crate::palm_oil::run_detail(&conn, palm).unwrap())),
            "list_processing_batches": v(serde_json::to_value(vec![crate::processing::batch_detail(&conn, batch).unwrap().batch])),
            "get_processing_batch": v(serde_json::to_value(crate::processing::batch_detail(&conn, batch).unwrap())),
            "get_recommendations": v(serde_json::to_value(crate::advisory::recommendations(&conn).unwrap())),
            "list_reminders": v(serde_json::to_value(crate::advisory::open_reminders(&conn, 365).unwrap())),
            "list_articles": v(serde_json::to_value(crate::knowledge::search(&conn, "", "").unwrap())),
            "get_article_id": article,
            "list_feed_guides": v(serde_json::to_value(crate::feed::list_guides_tx(&conn).unwrap())),
            "get_batch_feed": v(serde_json::to_value(crate::feed::batch_feed(&conn, flock, true).unwrap())),
            "compare_batch_feed": v(serde_json::to_value(crate::feed::compare_tx(&conn, None).unwrap())),
            "list_formulations": v(serde_json::to_value(crate::feed::list_formulations_tx(&conn).unwrap())),
            "calculate_formulation": v(serde_json::to_value(crate::feed::calculate(&[crate::feed::FormulaItem { ingredient: "Maize".into(), quantity_kg: 50.0, unit_cost_kobo: 45000, protein_pct: Some(9.0), energy_kcal_kg: Some(3300.0) }]).unwrap())),
            "list_warehouses": v(serde_json::to_value(crate::storage::list_warehouses_tx(&conn, true).unwrap())),
            "location_stock": v(serde_json::to_value(crate::storage::location_stock_tx(&conn, store).unwrap())),
            "storage_history": v(serde_json::to_value(crate::storage::storage_history_tx(&conn, store).unwrap())),
            "list_weather": v(serde_json::to_value(crate::weather_market::list_weather_tx(&conn, None, None).unwrap())),
            "rainfall_by_month": v(serde_json::to_value(crate::weather_market::rainfall_tx(&conn, year).unwrap())),
            "list_outlooks": v(serde_json::to_value(crate::weather_market::list_outlooks_tx(&conn).unwrap())),
            "list_market_prices": v(serde_json::to_value(crate::weather_market::list_prices_tx(&conn, "").unwrap())),
            "market_price_summary": v(serde_json::to_value(crate::weather_market::price_summary_tx(&conn).unwrap())),
            "farm_profit_loss": v(serde_json::to_value(crate::finance::profit_loss(&conn, "2000-01-01", "2100-01-01", true).unwrap())),
            "profit_by_activity": v(serde_json::to_value(crate::finance::by_activity(&conn, "2000-01-01", "2100-01-01", true).unwrap())),
            "get_production_report": v(serde_json::to_value(crate::finance::production_report(&conn, "2000-01-01", "2100-01-01", true).unwrap())),
            "list_finance_items": v(serde_json::to_value(crate::finance::list_finance_items_tx(&conn).unwrap())),
            "farm_finance_summary": v(serde_json::to_value(crate::finance::finance_summary_tx(&conn, "2000-01-01", "2100-01-01").unwrap())),
            "list_animals": v(serde_json::to_value(crate::livestock_ext::list_animals_tx(&conn, Some(goats), true).unwrap())),
            "list_breeding": v(serde_json::to_value(crate::livestock_ext::list_breeding_tx(&conn, Some(goats)).unwrap())),
            "list_ponds": v(serde_json::to_value(crate::livestock_ext::list_ponds_tx(&conn, true).unwrap())),
            "list_water_records": v(serde_json::to_value(crate::livestock_ext::list_water_tx(&conn, pond).unwrap())),
            "plot_history": v(serde_json::to_value(vec![crate::crops::cycle_summary(&conn, cycle).unwrap()])),
            "list_fields": v(serde_json::to_value(crate::crops_ext::list_fields_tx(&conn).unwrap())),
            "list_varieties": v(serde_json::to_value(crate::crops_ext::list_varieties_tx(&conn).unwrap())),
            "list_crop_health": v(serde_json::to_value(crate::crops_ext::list_crop_health_tx(&conn, None).unwrap())),
            "crop_calendar": v(serde_json::to_value(crate::crops_ext::calendar_tx(&conn, "2000-01-01", "2100-01-01").unwrap())),
            "list_protocols": v(serde_json::to_value(crate::health::list_protocols_tx(&conn).unwrap())),
            "list_rules": v(serde_json::to_value(crate::advisory::rules_tx(&conn).unwrap())),
            "list_advisory_flows": v(serde_json::to_value(crate::advisory_flows::list_advisory_flows())),
            "advisory_questions": v(serde_json::to_value(crate::advisory_flows::visible("poultry", &serde_json::Map::new()).unwrap())),
            "advisory_history": [],
            "list_audit": v(serde_json::to_value(crate::audit::list_tx(&conn, "", 100).unwrap())),
            "list_farm_templates": v(serde_json::to_value(crate::farm_templates::list_farm_templates())),
            "my_farms": v(serde_json::to_value(crate::farms::list_farms_tx(&conn, false).unwrap())),
            "list_content_versions": [],
            "list_backup_files": [],
            "setup_flow": v(serde_json::to_value(crate::onboarding::flow(&serde_json::from_value(json!({"business_type": "poultry", "manage": ["poultry", "fish"], "poultry_type": "broiler"})).unwrap()))),
            "onboarding_flow": v(serde_json::to_value(crate::onboarding::flow(&serde_json::from_value(json!({"manage": ["poultry", "cassava", "processing"], "processing_types": ["palm_oil"]})).unwrap()))),
        });
        let path = std::env::var("FB_FIXTURES").unwrap_or_else(|_| "/tmp/fb-fixtures.json".into());
        std::fs::write(&path, serde_json::to_string_pretty(&out).unwrap()).unwrap();
    }

    /// Seeds a database file with the demo farm, for training and screenshots.
    /// Used by `npm run seed:demo`. FB_DB = database path; FB_OWNER_PASSWORD
    /// creates an owner `owner` when the file has no users yet.
    #[test]
    #[ignore]
    fn seed_demo_db() {
        let path = std::path::PathBuf::from(std::env::var("FB_DB").expect("FB_DB is required"));
        if let Some(dir) = path.parent() { std::fs::create_dir_all(dir).unwrap(); }
        let mut conn = crate::db::open(&path).unwrap();
        let users: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0)).unwrap();
        if users == 0 {
            let pw = std::env::var("FB_OWNER_PASSWORD").expect("This database has no users: set FB_OWNER_PASSWORD to create the owner");
            assert!(pw.len() >= 8, "FB_OWNER_PASSWORD must be at least 8 characters");
            conn.execute("INSERT INTO users(username,full_name,password_hash,role) VALUES('owner','Farm Owner',?1,'owner')", [crate::auth::hash_password(&pw).unwrap()]).unwrap();
        }
        let tx = conn.transaction().unwrap();
        if crate::db::setting(&tx, "demo_loaded").unwrap_or_default() == "1" { println!("Demo data is already loaded in {}", path.display()); return; }
        crate::demo::load_tx(&tx).unwrap();
        tx.commit().unwrap();
        println!("Demo farm loaded into {}", path.display());
    }

}
