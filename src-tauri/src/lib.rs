#![cfg_attr(test, recursion_limit = "512")]
//! FarmBuddy backend: offline agricultural management and advisory. Farm
//! production (livestock, crops, processing), inventory, sales and receipts,
//! with role-based staff accounts. All data lives in one local SQLite database;
//! the UI talks to it only through the commands below.

mod advisory;
mod advisory_flows;
mod audit;
mod crops_ext;
mod feed;
mod finance;
mod livestock_ext;
mod storage;
mod weather_market;
mod apk_update;
mod auth;
mod backup;
mod content;
mod crops;
mod db;
mod demo;
mod email;
mod expenses;
mod farm_access;
mod farm_templates;
mod farms;
mod flocks;
mod health;
mod inventory;
mod knowledge;
mod license;
mod onboarding;
mod palm_oil;
mod processing;
mod purchases;
mod recovery;
mod reports;
mod reset;
mod sales;
mod settings;
mod shifts;
mod telemetry;
mod templates;
mod ui_fixtures;

use rusqlite::Connection;
use std::{collections::HashMap, sync::{Mutex, MutexGuard}, time::Instant};
use tauri::Manager;

pub struct AppState {
    conn: Mutex<Connection>,
    pub sessions: Mutex<HashMap<String, auth::Session>>,
    pub login_failures: Mutex<HashMap<String, (u32, Instant)>>,
    pub recovery_failures: Mutex<(u32, Instant)>,
    pub license: Mutex<Option<license::LicenseState>>,
}

impl AppState {
    pub fn db(&self) -> Result<MutexGuard<'_, Connection>, String> {
        self.conn.lock().map_err(|_| "Database lock was poisoned. Restart the app.".to_string())
    }
}

/// Opens the operating system print dialog for the current window. Used instead
/// of `window.print()`, which some platform webviews ignore.
#[cfg(desktop)]
#[tauri::command]
fn print_window(window: tauri::WebviewWindow) -> Result<(), String> {
    window.print().map_err(|e| format!("Could not open the print dialog: {e}"))
}

/// `WebviewWindow::print` does not exist on Android/iOS. Returning an error
/// makes the UI fall back to `window.print()`, which the mobile webview supports.
#[cfg(mobile)]
#[tauri::command]
fn print_window() -> Result<(), String> {
    Err("Native print dialog is not available on mobile".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init());
    #[cfg(desktop)]
    {
        builder = builder
            .plugin(tauri_plugin_process::init())
            .plugin(tauri_plugin_updater::Builder::new().build());
    }
    #[cfg(mobile)]
    {
        builder = builder.plugin(tauri_plugin_barcode_scanner::init());
    }
    builder
        .setup(|app| {
            let path = db::db_path(app.handle()).map_err(std::io::Error::other)?;
            let conn = db::open(&path).map_err(std::io::Error::other)?;
            let lic = license::compute(app.handle(), &conn);
            app.manage(AppState {
                conn: Mutex::new(conn),
                sessions: Mutex::new(HashMap::new()),
                login_failures: Mutex::new(HashMap::new()),
                recovery_failures: Mutex::new((0, Instant::now())),
                license: Mutex::new(Some(lic)),
            });
            backup::start_auto(app.handle().clone());
            telemetry::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // public
            auth::setup_status, auth::setup_owner, auth::setup_farm, onboarding::setup_flow, onboarding::setup_check_step, auth::login, apk_update::check_apk_update, settings::branding, templates::list_templates,
            recovery::recovery_options, recovery::recover_account,
            license::license_status, license::activate_license,
            // security (owner)
            recovery::security_status, recovery::regenerate_recovery_key, recovery::save_security_questions,
            // account
            auth::logout, auth::change_own_password, auth::list_users, auth::save_user,
            // settings
            settings::get_settings, settings::save_settings, templates::apply_business_template,
            // inventory
            inventory::list_products, inventory::save_product, inventory::remove_product, inventory::adjust_stock, inventory::list_movements,
            inventory::list_categories, inventory::save_category, inventory::delete_category,
            inventory::list_payment_methods, inventory::save_payment_method,
            inventory::list_batches, inventory::get_recipe, inventory::save_recipe, inventory::generate_barcodes,
            // sales
            sales::create_receipt, sales::list_receipts, sales::get_receipt, sales::record_payment, sales::void_receipt, sales::update_fulfilment,
            sales::create_return, sales::hold_sale, sales::list_held_sales, sales::delete_held_sale,
            sales::list_customers, sales::save_customer, sales::delete_customer, sales::customer_statement,
            // shifts & expenses
            shifts::current_shift, shifts::open_shift, shifts::close_shift, shifts::list_shifts, shifts::shift_report,
            expenses::list_expenses, expenses::save_expense, expenses::delete_expense,
            // suppliers & purchases
            purchases::list_suppliers, purchases::save_supplier, purchases::delete_supplier,
            purchases::create_purchase, purchases::list_purchases, purchases::get_purchase, purchases::record_supplier_payment,
            // farms, onboarding and demo data
            farms::list_farms, farms::save_farm, farms::archive_farm, farms::farm_overview,
            onboarding::onboarding_flow, onboarding::onboarding_check_step, onboarding::complete_onboarding,
            demo::load_demo_data, demo::clear_demo_data,
            // livestock batches (poultry, livestock, fish), health and vaccination
            flocks::list_flocks, flocks::get_flock, flocks::save_flock, flocks::close_flock, flocks::add_flock_entry, flocks::delete_flock_entry, flocks::delete_flock,
            health::list_health_records, health::save_health_record, health::delete_health_record,
            health::list_vaccinations, health::save_vaccination, health::complete_vaccination, health::skip_vaccination, health::delete_vaccination,
            // crops
            crops::list_plots, crops::save_plot, crops::archive_plot, crops::list_crop_cycles, crops::get_crop_cycle, crops::save_crop_cycle, crops::close_crop_cycle,
            crops::add_crop_activity, crops::add_harvest, crops::delete_crop_entry,
            // processing
            palm_oil::process_products, palm_oil::list_palm_runs, palm_oil::get_palm_run, palm_oil::save_process_run, palm_oil::record_run_sale, palm_oil::delete_palm_run,
            processing::list_processing_batches, processing::get_processing_batch, processing::save_processing_batch, processing::set_processing_stage,
            processing::add_processing_input, processing::add_processing_output, processing::delete_processing_entry, processing::close_processing_batch,
            // advisory, knowledge library and reminders
            advisory::get_recommendations, advisory::list_reminders, advisory::save_reminder, advisory::set_reminder_status, advisory::delete_reminder,
            knowledge::list_articles, knowledge::get_article, knowledge::list_sources, knowledge::save_article, knowledge::set_article_review, knowledge::delete_article,
            advisory::list_rules, advisory::save_rule,
            advisory_flows::advisory_questions, advisory_flows::run_advisory, advisory_flows::advisory_history,
            health::list_protocols, health::save_protocol, health::review_protocol, health::delete_protocol, health::apply_protocol,
            // livestock extensions: animals, breeding, transfers, eggs, ponds and water
            livestock_ext::receive_animals, livestock_ext::transfer_animals, livestock_ext::record_eggs,
            livestock_ext::list_animals, livestock_ext::save_animal, livestock_ext::set_animal_status, livestock_ext::add_animal_weight,
            livestock_ext::list_breeding, livestock_ext::save_breeding, livestock_ext::record_birth,
            livestock_ext::list_ponds, livestock_ext::save_pond, livestock_ext::archive_pond, livestock_ext::list_water_records, livestock_ext::add_water_record,
            // feed
            feed::list_feed_guides, feed::save_feed_guide, feed::delete_feed_guide, feed::get_batch_feed, feed::compare_batch_feed,
            feed::calculate_formulation, feed::list_formulations, feed::save_formulation, feed::delete_formulation,
            // crops extensions
            crops_ext::list_fields, crops_ext::save_field, crops_ext::set_plot_field, crops_ext::plot_history,
            crops_ext::list_varieties, crops_ext::save_variety, crops_ext::delete_variety,
            crops_ext::list_crop_health, crops_ext::save_crop_health, crops_ext::delete_crop_health, crops_ext::crop_calendar,
            // storage
            storage::list_warehouses, storage::save_warehouse, storage::archive_warehouse, storage::location_stock, storage::storage_action, storage::storage_history,
            // weather and market
            weather_market::list_weather, weather_market::add_weather, weather_market::delete_weather, weather_market::rainfall_by_month,
            weather_market::list_outlooks, weather_market::save_outlook, weather_market::delete_outlook,
            weather_market::list_market_prices, weather_market::add_market_price, weather_market::delete_market_price, weather_market::market_price_summary,
            // farm finance
            finance::farm_profit_loss, finance::profit_by_activity, finance::get_production_report,
            finance::list_finance_items, finance::save_finance_item, finance::delete_finance_item, finance::farm_finance_summary,
            content::import_content_package, content::list_content_versions,
            farm_templates::list_farm_templates, farm_templates::apply_farm_template, advisory_flows::list_advisory_flows,
            audit::list_audit, farm_access::get_user_farms, farm_access::set_user_farms, farm_access::my_farms,
            // reports, files & backups
            reports::dashboard, reports::sales_report, reports::save_export, reports::save_pdf_export,
            backup::backup_status, backup::save_backup_settings, backup::backup_now, backup::list_backup_files, backup::check_backup_file, backup::restore_backup, reset::reset_business_data,
            // email
            email::send_receipt_email, email::send_test_email,
            // printing
            print_window,
        ])
        .run(tauri::generate_context!())
        .expect("error while running FarmBuddy");
}
