//! Optional demonstration data: a demo farm with the specification's 500-bird
//! broiler batch, a cassava plot, the 100-bunch palm milling batch, and
//! fictional suppliers and buyers. Every record is flagged `is_demo`, labelled
//! "Demo" in its name, never mixed with real sales or stock, and removed in one
//! step. Prices are illustrative, not current market prices.

use crate::{auth::{self, Role}, db, farms::today, AppState};
use rusqlite::{Connection, Transaction};

fn ago(days: i64) -> String { (today() - chrono::Duration::days(days)).format("%Y-%m-%d").to_string() }
fn ahead(days: i64) -> String { (today() + chrono::Duration::days(days)).format("%Y-%m-%d").to_string() }

const SUPPLIERS: [(&str, &str, &str); 5] = [
    ("Akwa Farm Inputs (Demo)", "Fertiliser, seeds, farm tools", "Uyo, Akwa Ibom"),
    ("South-South Feeds (Demo)", "Poultry feed, fish feed", "Ikot Ekpene, Akwa Ibom"),
    ("Green Harvest (Demo)", "Cassava planting materials", "Abak, Akwa Ibom"),
    ("Palm Produce (Demo)", "Palm fruit bunches", "Eket, Akwa Ibom"),
    ("Farm Health Supplies (Demo)", "Veterinary products", "Uyo, Akwa Ibom"),
];
const BUYERS: [(&str, &str); 5] = [
    ("Uyo Produce Buyer (Demo)", "Produce wholesaler: cassava, yam, plantain"),
    ("Akwa Palm Oil Buyer (Demo)", "Processor / wholesaler: palm oil, kernels"),
    ("South-South Food Market (Demo)", "Retailer: garri, vegetables, grains"),
    ("Fresh Fish Buyer (Demo)", "Fish wholesaler: catfish, tilapia"),
    ("Poultry Meat Buyer (Demo)", "Meat retailer: broilers and processed poultry"),
];
const NOTE: &str = "Fictional demonstration record. Not a real business.";

pub fn load_tx(conn: &Transaction<'_>) -> Result<(), String> {
    if db::setting_bool(conn, "demo_loaded")? { return Err("Demo data is already loaded".into()); }
    let ex = |sql: &str, p: &[&dyn rusqlite::ToSql]| conn.execute(sql, p).map_err(|e| e.to_string());
    ex("INSERT INTO farms(name,state,lga,community,activities,notes,is_demo) VALUES('FarmBuddy Demo Farm','Akwa Ibom','Uyo','Uyo','poultry,cassava,processing',?1,1)", &[&NOTE])?;
    let farm = conn.last_insert_rowid();

    // The 500-bird broiler batch (day 21), with daily records adding up to 12 deaths and 3 culls.
    ex("INSERT INTO flocks(code,breed,house,placed_date,chicks_placed,chick_cost_kobo,species,purpose,source,farm_id,notes,is_demo) VALUES('BR-DEMO','Demo strain','Pen 1',?1,500,30000000,'broiler','Meat','Demo hatchery',?2,?3,1)", &[&ago(21), &farm, &"Illustrative demo batch. Figures are examples, not targets."])?;
    let flock = conn.last_insert_rowid();
    let logs: [(i64, i64, i64, f64, f64); 8] = [(20, 3, 0, 10.0, 0.0), (17, 2, 1, 18.0, 0.0), (14, 2, 0, 30.0, 0.38), (10, 1, 1, 42.0, 0.0), (7, 2, 0, 52.0, 0.52), (3, 1, 1, 62.0, 0.64), (2, 0, 0, 65.0, 0.0), (1, 1, 0, 67.0, 0.70)];
    for (d, dead, culls, feed, w) in logs {
        ex("INSERT INTO flock_logs(flock_id,log_date,mortality,culls,feed_kg,avg_weight_kg,notes,user_name) VALUES(?1,?2,?3,?4,?5,?6,'Demo record','Demo')", &[&flock, &ago(d), &dead, &culls, &feed, &w])?;
    }
    ex("INSERT INTO flock_costs(flock_id,cost_date,category,description,amount_kobo,user_name) VALUES(?1,?2,'Feed','Demo feed purchase',45000000,'Demo')", &[&flock, &ago(20)])?;
    ex("INSERT INTO vaccination_events(flock_id,due_date,vaccine_name,disease,source,notes,user_name) VALUES(?1,?2,'Example vaccine (demo)','Example','pending','Demo entry from a general reference. A vet must confirm any real schedule.','Demo')", &[&flock, &ahead(2)])?;

    // A cassava plot planted two months ago.
    ex("INSERT INTO farm_plots(farm_id,code,name,area,area_unit,location,is_demo) VALUES(?1,'DEMO-A','Demo cassava plot',1,'hectare','Behind the pens',1)", &[&farm])?;
    let plot = conn.last_insert_rowid();
    ex("INSERT INTO crop_cycles(plot_id,crop,variety,planting_date,quantity_planted,planting_unit,material_source,planting_cost_kobo,method,expected_harvest_from,expected_harvest_to,purpose,notes,is_demo)
        VALUES(?1,'Cassava','Demo variety',?2,1000,'stem bundle','Green Harvest (Demo)',4000000,'Ridges',?3,?4,'Sale and garri',?5,1)", &[&plot, &ago(60), &ahead(240), &ahead(300), &NOTE])?;
    let cycle = conn.last_insert_rowid();
    ex("INSERT INTO crop_activities(cycle_id,activity_date,activity_type,description,labour_kobo,user_name) VALUES(?1,?2,'land_preparation','Clearing and ridging (demo)',3000000,'Demo')", &[&cycle, &ago(62)])?;
    ex("INSERT INTO crop_activities(cycle_id,activity_date,activity_type,description,labour_kobo,user_name) VALUES(?1,?2,'weeding','First weeding (demo)',1500000,'Demo')", &[&cycle, &ago(30)])?;

    // The specification's palm milling batch: 100 bunches × ₦2,000, milling ₦500 per bunch, transport ₦10,000, labour ₦5,000.
    ex("INSERT INTO processing_batches(batch_no,process_type,title,farm_id,start_date,stage,notes,is_demo) VALUES('PB-DEMO','palm_oil','Palm fruit milling (demo)',?1,?2,'Pressing','Illustrative figures from the FarmBuddy specification. Not a fixed market rate or yield.',1)", &[&farm, &ago(1)])?;
    let batch = conn.last_insert_rowid();
    ex("INSERT INTO processing_inputs(batch_id,entry_date,kind,name,quantity,unit,unit_cost_kobo,amount_kobo,supplier,user_name) VALUES(?1,?2,'raw','Palm fruit bunches',100,'bunch',200000,20000000,'Palm Produce (Demo)','Demo')", &[&batch, &ago(1)])?;
    ex("INSERT INTO processing_inputs(batch_id,entry_date,kind,name,quantity,unit,unit_cost_kobo,amount_kobo,user_name) VALUES(?1,?2,'cost','Milling fee',100,'bunch',50000,5000000,'Demo')", &[&batch, &ago(1)])?;
    ex("INSERT INTO processing_inputs(batch_id,entry_date,kind,name,amount_kobo,user_name) VALUES(?1,?2,'cost','Transport',1000000,'Demo')", &[&batch, &ago(1)])?;
    ex("INSERT INTO processing_inputs(batch_id,entry_date,kind,name,amount_kobo,user_name) VALUES(?1,?2,'cost','Labour',500000,'Demo')", &[&batch, &ago(1)])?;

    // A palm oil run from bought fruit, with two sales (illustrative demo figures, not market prices or yields).
    use crate::palm_oil::{Measure, NamedAmount, RunForm, SaleForm};
    let m = |u: &str, per: f64, price: i64, stock: bool| Measure { unit: u.into(), per_stock_unit: per, price_kobo: price, is_stock_unit: stock };
    let run = crate::palm_oil::save_form_tx(conn, &RunForm {
        product: "palm_oil".into(), start_date: ago(6), farm_id: Some(farm), source: "bought".into(), raw_unit: "basin".into(), bought_qty: 20.0, raw_price_kobo: 350_000,
        transport_kobo: 800_000, mill_name: "Community mill (demo)".into(), mill_fee_kobo: 1_000_000,
        workers: vec![NamedAmount { name: "Worker A (demo)".into(), amount_kobo: 400_000 }, NamedAmount { name: "Worker B (demo)".into(), amount_kobo: 400_000 }],
        expenses: vec![NamedAmount { name: "Kegs (demo)".into(), amount_kobo: 300_000 }],
        made_qty: 4.0, made_unit: "25-litre keg".into(), second_qty: 1.0, second_unit: "bag".into(),
        measures: vec![m("25-litre keg", 1.0, 6_000_000, true), m("litre", 25.0, 250_000, false)],
        second_measures: vec![m("bag", 1.0, 2_000_000, true), m("cup", 10.0, 200_000, false)],
        notes: "Illustrative demo figures. Not market prices or yields.".into(), ..Default::default() }, "Demo")?;
    ex("UPDATE palm_runs SET is_demo=1, run_no='PO-DEMO' WHERE id=?1", &[&run])?;
    let owner: Option<i64> = conn.query_row("SELECT id FROM users WHERE role='owner' AND active=1 ORDER BY id LIMIT 1", [], |r| r.get(0)).ok();
    if let Some(owner) = owner {
        let me = auth::Session { user_id: owner, username: "demo".into(), full_name: "Demo".into(), role: Role::Owner, last_seen: std::time::Instant::now() };
        for (part, unit, q, price, transport) in [("main", "25-litre keg", 3.0, 5_500_000, 500_000), ("second", "cup", 10.0, 180_000, 0)] {
            let r = crate::palm_oil::record_sale_tx(conn, &SaleForm { run_id: run, part: part.into(), sale_date: ago(2), unit: unit.into(), quantity: q, price_kobo: price,
                buyer: "Akwa Palm Oil Buyer (Demo)".into(), method: "Cash".into(), paid_kobo: (q * price as f64) as i64, transport_kobo: transport, ..Default::default() }, &me)?;
            ex("UPDATE receipts SET is_demo=1 WHERE id=?1", &[&r.receipt_id])?;
        }
    }

    for (name, supplies, place) in SUPPLIERS {
        ex("INSERT INTO suppliers(name,address,notes,is_demo) VALUES(?1,?2,?3,1)", &[&name, &place, &format!("{supplies}. {NOTE}")])?;
    }
    for (name, kind) in BUYERS {
        ex("INSERT INTO customers(name,notes,is_demo) VALUES(?1,?2,1)", &[&name, &format!("{kind}. {NOTE}")])?;
    }
    ex("INSERT INTO farm_reminders(title,due_date,related_kind,related_id,source,notes,is_demo,user_name) VALUES('Weigh a sample of BR-DEMO',?1,'livestock',?2,'manual','Demo reminder',1,'Demo')", &[&ahead(1), &flock])?;
    load_more(conn, farm, cycle)?;
    db::set_setting(conn, "demo_loaded", "1")
}

/// Demo products (₦ prices here are illustrative only).
const PRODUCTS: [(&str, &str, &str, &str, i64, i64); 6] = [
    ("DEMO-FEED", "Broiler finisher feed 25kg (Demo)", "material", "bag", 0, 1_400_000),
    ("DEMO-EGGS", "Eggs, crate of 30 (Demo)", "product", "crate", 550_000, 0),
    ("DEMO-GARRI", "Garri, 50kg bag (Demo)", "product", "bag", 4_500_000, 3_600_000),
    ("DEMO-MAIZE", "Dry maize grain, 100kg bag (Demo)", "product", "bag", 6_000_000, 0),
    ("DEMO-OIL", "Palm oil, 25 litres (Demo)", "product", "jerrycan", 5_500_000, 0),
    ("DEMO-FISH", "Live catfish per kg (Demo)", "product", "kg", 300_000, 0),
];

fn pid(conn: &Connection, sku: &str) -> Result<i64, String> {
    conn.query_row("SELECT id FROM products WHERE sku=?1", [sku], |r| r.get(0)).map_err(|e| e.to_string())
}

/// The rest of the demonstration: commerce (a purchase on credit, a sale with
/// a balance), storage with a transfer and a moisture check, layer eggs, goats
/// with tags and breeding, a catfish pond with water readings, a maize harvest
/// into store, weather and illustrative market prices.
fn load_more(conn: &Transaction<'_>, farm: i64, cassava_cycle: i64) -> Result<(), String> {
    let ex = |sql: &str, p: &[&dyn rusqlite::ToSql]| conn.execute(sql, p).map_err(|e| e.to_string());
    let owner: i64 = conn.query_row("SELECT id FROM users WHERE role='owner' AND active=1 ORDER BY id LIMIT 1", [], |r| r.get(0)).map_err(|_| "Create the owner account before loading demo data".to_string())?;
    let demo = auth::Session { user_id: owner, username: "demo".into(), full_name: "Demo".into(), role: Role::Owner, last_seen: std::time::Instant::now() };
    for (sku, name, kind, unit, price, cost) in PRODUCTS {
        ex("INSERT INTO products(sku,name,kind,unit,selling_price_kobo,cost_price_kobo,track_stock,stock_qty,reorder_level,is_demo) VALUES(?1,?2,?3,?4,?5,?6,1,0,?7,1)",
            &[&sku, &name, &kind, &unit, &price, &cost, &(if kind == "material" { 5.0 } else { 0.0 })])?;
    }
    // A purchase of feed and garri, part paid: ₦30,000 still owed to the supplier.
    let supplier: i64 = conn.query_row("SELECT id FROM suppliers WHERE name='South-South Feeds (Demo)'", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    let p = crate::purchases::create_purchase_tx(conn, &demo, crate::purchases::PurchaseInput {
        supplier_id: Some(supplier), supplier_name: None, invoice_ref: "DEMO-INV-1".into(), note: "Demo purchase".into(),
        lines: vec![
            crate::purchases::PurchaseLineInput { product_id: pid(conn, "DEMO-FEED")?, quantity: 20.0, unit_cost_kobo: 1_400_000, as_pack: false, batch_no: String::new(), expiry_date: String::new() },
            crate::purchases::PurchaseLineInput { product_id: pid(conn, "DEMO-GARRI")?, quantity: 10.0, unit_cost_kobo: 3_600_000, as_pack: false, batch_no: String::new(), expiry_date: String::new() },
        ],
        amount_paid_kobo: 61_000_000, payment_method: "Bank transfer".into(),
    })?;
    ex("UPDATE purchases SET is_demo=1 WHERE id=?1", &[&p.id()])?;

    // Storage: a store and a cold room; garri put away, part moved, one bag spoiled; a moisture check on maize.
    let store = crate::storage::save_warehouse_tx(conn, &crate::storage::WarehouseInput { id: None, farm_id: Some(farm), name: "Main store (Demo)".into(), kind: "store".into(), notes: NOTE.into() })?;
    let shop = crate::storage::save_warehouse_tx(conn, &crate::storage::WarehouseInput { id: None, farm_id: Some(farm), name: "Roadside shop (Demo)".into(), kind: "retail_shop".into(), notes: NOTE.into() })?;
    ex("UPDATE warehouses SET is_demo=1 WHERE id IN (?1,?2)", &[&store, &shop])?;
    let act = |w: i64, sku: &str, kind: &str, d: i64, q: f64, to: Option<i64>, moisture: Option<f64>, note: &str| -> Result<(), String> {
        crate::storage::action_tx(conn, &crate::storage::StorageAction { warehouse_id: w, product_id: pid(conn, sku)?, kind: kind.into(), date: ago(d), quantity: q, to_warehouse_id: to, grade: String::new(), moisture_pct: moisture, cost_kobo: 0, note: note.into() }, "Demo")
    };
    act(store, "DEMO-GARRI", "put_away", 6, 10.0, None, None, "Delivered by supplier")?;
    act(store, "DEMO-FEED", "put_away", 6, 20.0, None, None, "Delivered by supplier")?;
    act(store, "DEMO-GARRI", "transfer", 4, 4.0, Some(shop), None, "Stock for the roadside shop")?;
    act(store, "DEMO-GARRI", "spoilage", 2, 1.0, None, None, "Bag got wet in the rain (demo)")?;

    // Layers: 300 birds, two weeks of egg collection; eggs sold, used and lost.
    ex("INSERT INTO flocks(code,breed,house,placed_date,chicks_placed,species,purpose,source,farm_id,notes,is_demo) VALUES('LY-DEMO','Demo layer strain','Pen 2',?1,300,'layer','Eggs','Demo hatchery',?2,?3,1)", &[&ago(180), &farm, &NOTE])?;
    let layers = conn.last_insert_rowid();
    for d in 1..=14i64 {
        let eggs = 240 + (d * 7) % 15;
        ex("INSERT INTO flock_logs(flock_id,log_date,mortality,feed_kg,eggs_collected,eggs_damaged,water_l,notes,user_name) VALUES(?1,?2,?3,34,?4,?5,70,'Demo record','Demo')", &[&layers, &ago(d), &(if d == 9 { 1 } else { 0 }), &eggs, &(eggs % 4)])?;
    }
    for (d, kind, eggs, amount, buyer) in [(10, "sold", 1500, 27_500_000i64, "South-South Food Market (Demo)"), (5, "sold", 900, 16_500_000, "South-South Food Market (Demo)"), (3, "used", 30, 0, ""), (2, "lost", 12, 0, "")] {
        crate::livestock_ext::eggs_tx(conn, &crate::livestock_ext::EggInput { flock_id: layers, date: ago(d), kind: kind.into(), eggs, amount_kobo: amount, buyer: buyer.into(), notes: "Demo".into() }, "Demo")?;
    }

    // Goats: a tagged herd, a mating, a birth with two kids, and two bucks moved to their own pen.
    ex("INSERT INTO flocks(code,breed,house,placed_date,chicks_placed,species,purpose,farm_id,notes,is_demo) VALUES('GT-DEMO','West African Dwarf (demo)','Goat house',?1,10,'goat','Meat and breeding',?2,?3,1)", &[&ago(400), &farm, &NOTE])?;
    let goats = conn.last_insert_rowid();
    ex("INSERT INTO flocks(code,breed,house,placed_date,chicks_placed,species,purpose,farm_id,notes,is_demo) VALUES('GT-DEMO-B','West African Dwarf (demo)','Buck pen',?1,1,'goat','Breeding bucks',?2,?3,1)", &[&ago(30), &farm, &NOTE])?;
    let bucks = conn.last_insert_rowid();
    let mut ids = Vec::new();
    for (i, (sex, born)) in [("female", 700), ("female", 650), ("female", 600), ("male", 720), ("male", 500)].into_iter().enumerate() {
        ids.push(crate::livestock_ext::save_animal_tx(conn, &crate::livestock_ext::AnimalInput { id: None, flock_id: goats, tag: format!("GT-D{:02}", i + 1), name: String::new(), sex: sex.into(), breed: "West African Dwarf".into(), birth_date: ago(born), acquired_date: ago(400), dam_id: None, sire_id: None, notes: "Demo".into() })?);
    }
    let br = crate::livestock_ext::save_breeding_tx(conn, &crate::livestock_ext::BreedingInput { id: None, flock_id: goats, dam_id: Some(ids[0]), sire_id: Some(ids[3]), sire_text: String::new(), mating_date: ago(160), method: "natural".into(), expected_due: ago(10), status: "pregnant".into(), notes: "Demo".into() }, "Demo")?;
    crate::livestock_ext::birth_tx(conn, &crate::livestock_ext::BirthInput { flock_id: goats, breeding_id: Some(br), dam_id: Some(ids[0]), birth_date: ago(8), born_alive: 2, stillborn: 0, notes: "Twins (demo)".into(), tag_prefix: "GT-K".into() }, "Demo")?;
    crate::livestock_ext::save_breeding_tx(conn, &crate::livestock_ext::BreedingInput { id: None, flock_id: goats, dam_id: Some(ids[1]), sire_id: Some(ids[3]), sire_text: String::new(), mating_date: ago(60), method: "natural".into(), expected_due: ahead(90), status: "mated".into(), notes: "Demo".into() }, "Demo")?;
    crate::livestock_ext::transfer_tx(conn, &crate::livestock_ext::TransferInput { from_flock_id: goats, to_flock_id: bucks, date: ago(30), quantity: 2, notes: "Bucks moved to their own pen (demo)".into() }, "Demo")?;
    for (d, w) in [(90, 18.5), (30, 21.0)] {
        ex("INSERT INTO animal_weights(animal_id,weigh_date,weight_kg) VALUES(?1,?2,?3)", &[&ids[0], &ago(d), &w])?;
    }

    // Catfish in a demo pond, with water readings (one reading left unmeasured on purpose).
    let pond = crate::livestock_ext::save_pond_tx(conn, &crate::livestock_ext::PondInput { id: None, farm_id: Some(farm), code: "POND-DEMO".into(), pond_type: "Concrete".into(), size: 40.0, size_unit: "square_metre".into(), notes: NOTE.into() })?;
    ex("UPDATE fish_ponds SET is_demo=1 WHERE id=?1", &[&pond])?;
    ex("INSERT INTO flocks(code,breed,placed_date,chicks_placed,chick_cost_kobo,species,purpose,source,farm_id,pond_id,notes,is_demo) VALUES('FS-DEMO','Catfish juveniles (demo)',?1,1000,5000000,'catfish','Table fish','Demo hatchery',?2,?3,?4,1)", &[&ago(70), &farm, &pond, &NOTE])?;
    let fish = conn.last_insert_rowid();
    for (d, dead, feed, w) in [(60, 6, 4.0, 0.0), (45, 3, 6.0, 0.12), (30, 2, 9.0, 0.25), (15, 1, 12.0, 0.42), (2, 0, 15.0, 0.55)] {
        ex("INSERT INTO flock_logs(flock_id,log_date,mortality,feed_kg,avg_weight_kg,notes,user_name) VALUES(?1,?2,?3,?4,?5,'Demo record','Demo')", &[&fish, &ago(d), &dead, &feed, &w])?;
    }
    for (d, t, ph, o2) in [(14, Some(28.5), Some(7.1), Some(5.2)), (7, Some(29.0), Some(6.9), None), (1, Some(28.0), Some(7.0), Some(5.0))] {
        crate::livestock_ext::add_water_tx(conn, &crate::livestock_ext::WaterRecord { id: 0, pond_id: pond, record_date: ago(d), temp_c: t, ph, dissolved_oxygen: o2, ammonia: None, notes: "Demo reading".into(), user_name: String::new() }, "Demo")?;
    }

    // Maize harvested from a second demo plot and stored.
    ex("INSERT INTO farm_plots(farm_id,code,name,area,area_unit,location,is_demo) VALUES(?1,'DEMO-B','Demo maize plot',0.5,'hectare','By the road',1)", &[&farm])?;
    let plot_b = conn.last_insert_rowid();
    ex("INSERT INTO crop_cycles(plot_id,crop,variety,planting_date,quantity_planted,planting_unit,planting_cost_kobo,method,expected_harvest_from,expected_harvest_to,purpose,notes,is_demo) VALUES(?1,'Maize','Demo variety',?2,12,'kg',2400000,'Rows',?3,?4,'Sale',?5,1)", &[&plot_b, &ago(110), &ago(20), &ahead(5), &NOTE])?;
    let maize = conn.last_insert_rowid();
    crate::crops::add_harvest_tx(conn, &crate::crops::HarvestInput { cycle_id: maize, harvest_date: ago(12), quantity: 18.0, unit: "bag".into(), grade: "Good".into(), damaged_qty: 1.0, product_id: Some(pid(conn, "DEMO-MAIZE")?), stored_qty: 15.0, sold_qty: 2.0, sale_amount_kobo: 12_000_000, buyer: "Uyo Produce Buyer (Demo)".into(), labour_kobo: 1_500_000, transport_kobo: 500_000, other_cost_kobo: 0, notes: "Demo harvest".into() }, "Demo")?;
    act(store, "DEMO-MAIZE", "put_away", 11, 15.0, None, None, "Dried and bagged")?;
    act(store, "DEMO-MAIZE", "inspection", 4, 0.0, None, Some(13.5), "Moisture check (demo)")?;
    ex("INSERT INTO crop_health_records(cycle_id,observed_date,kind,observation,affected_area,action_taken,user_name) VALUES(?1,?2,'pest','Some leaves with holes on the lower ridges (demo). To be shown to the extension officer.',0.05,'None yet','Demo')", &[&cassava_cycle, &ago(5)])?;

    // A sale with a balance owed: ₦55,000 of eggs and garri, ₦40,000 paid now.
    let buyer: i64 = conn.query_row("SELECT id FROM customers WHERE name='South-South Food Market (Demo)'", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    let shift_setting = db::setting(conn, "require_shift")?;
    db::set_setting(conn, "require_shift", "0")?;
    ex("UPDATE products SET stock_qty=stock_qty+10 WHERE sku='DEMO-EGGS'", &[])?;
    ex("INSERT INTO stock_movements(product_id,movement_type,quantity_delta,note,user_name) VALUES(?1,'opening',10,'Eggs from LY-DEMO (demo)','Demo')", &[&pid(conn, "DEMO-EGGS")?])?;
    let r = crate::sales::create_receipt_tx(conn, crate::sales::CreateReceiptInput {
        customer_id: Some(buyer), customer_name: None, customer_phone: None, customer_email: None, discount_kobo: 0, note: Some("Demo sale on part credit".into()), collection_date: None,
        items: vec![crate::sales::SaleItemInput { product_id: pid(conn, "DEMO-EGGS")?, quantity: 2.0, unit_price_kobo: None, sell_as: None }, crate::sales::SaleItemInput { product_id: pid(conn, "DEMO-GARRI")?, quantity: 1.0, unit_price_kobo: None, sell_as: None }],
        payments: vec![crate::sales::PaymentInput { method: "Cash".into(), amount_kobo: 4_000_000, reference: None }],
    }, &demo);
    db::set_setting(conn, "require_shift", &shift_setting)?;
    ex("UPDATE receipts SET is_demo=1 WHERE id=?1", &[&r?.id])?;
    act(shop, "DEMO-GARRI", "issue", 0, 1.0, None, None, "Sold at the shop (demo sale)")?;
    ex("INSERT INTO expenses(expense_date,category,description,amount_kobo,method,user_name,is_demo) VALUES(?1,'Utilities','Generator fuel (demo)',1500000,'Cash','Demo',1)", &[&ago(3)])?;

    // Weather and illustrative market prices (not current prices).
    for (d, rain, lo, hi) in [(6, Some(12.0), 23.0, 31.0), (5, Some(0.0), 24.0, 32.0), (4, Some(35.5), 22.5, 29.0), (3, None, 23.0, 30.5), (2, Some(4.0), 23.0, 31.0), (1, Some(0.0), 24.0, 32.5)] {
        crate::weather_market::add_weather_tx(conn, &crate::weather_market::Weather { id: 0, farm_id: Some(farm), record_date: ago(d), rainfall_mm: rain, temp_min_c: Some(lo), temp_max_c: Some(hi), conditions: String::new(), notes: "Demo reading".into(), user_name: String::new() }, "Demo")?;
    }
    for (product, unit, d, w, rt) in [("Garri (white)", "50kg bag", 30, 4_200_000i64, 4_600_000i64), ("Garri (white)", "50kg bag", 7, 4_300_000, 4_700_000), ("Palm oil", "25 litres", 20, 5_000_000, 5_600_000), ("Palm oil", "25 litres", 3, 5_200_000, 5_800_000), ("Eggs", "crate of 30", 10, 520_000, 570_000), ("Maize", "100kg bag", 6, 5_600_000, 6_200_000)] {
        crate::weather_market::add_price_tx(conn, &crate::weather_market::MarketPrice { id: 0, product: product.into(), market: "Itam Market (demo)".into(), state: "Akwa Ibom".into(), lga: "Itu".into(), price_date: ago(d), unit: unit.into(), wholesale_kobo: Some(w), retail_kobo: Some(rt), buyer_offer_kobo: None, quantity_available: String::new(), source: "Demo: illustrative, not a current market price".into(), notes: String::new(), is_demo: true, user_name: String::new() }, "Demo")?;
    }
    Ok(())
}

#[derive(serde::Serialize, Default, Debug)]
pub struct ClearResult { pub kept_suppliers: i64, pub kept_customers: i64 }

/// Removes every demo record. Demo suppliers or buyers that have since been
/// used in a real purchase or sale are kept, so no real record loses its link.
pub fn clear_tx(conn: &Connection) -> Result<ClearResult, String> {
    let ex = |sql: &str| conn.execute(sql, []).map_err(|e| e.to_string());
    let demo_farms = "(SELECT id FROM farms WHERE is_demo=1)";
    // Sales and purchases first, then the stock they moved, then the products.
    // Demo run sales give their stock back before the receipts go (the run then takes it all out).
    ex("UPDATE products SET stock_qty=stock_qty-COALESCE((SELECT SUM(m.quantity_delta) FROM stock_movements m JOIN run_sales s ON s.receipt_id=m.receipt_id JOIN receipts r ON r.id=s.receipt_id WHERE r.is_demo=1 AND m.movement_type='sale' AND m.product_id=products.id),0)")?;
    ex("DELETE FROM run_sales WHERE run_id IN (SELECT id FROM palm_runs WHERE is_demo=1)")?;
    ex("DELETE FROM receipt_payments WHERE receipt_id IN (SELECT id FROM receipts WHERE is_demo=1)")?;
    ex("DELETE FROM return_items WHERE return_id IN (SELECT id FROM returns WHERE receipt_id IN (SELECT id FROM receipts WHERE is_demo=1))")?;
    ex("DELETE FROM returns WHERE receipt_id IN (SELECT id FROM receipts WHERE is_demo=1)")?;
    ex("DELETE FROM receipt_items WHERE receipt_id IN (SELECT id FROM receipts WHERE is_demo=1)")?;
    ex("DELETE FROM stock_movements WHERE receipt_id IN (SELECT id FROM receipts WHERE is_demo=1)")?;
    ex("DELETE FROM receipts WHERE is_demo=1")?;
    ex("DELETE FROM supplier_payments WHERE purchase_id IN (SELECT id FROM purchases WHERE is_demo=1)")?;
    ex("DELETE FROM purchase_items WHERE purchase_id IN (SELECT id FROM purchases WHERE is_demo=1)")?;
    ex("DELETE FROM purchases WHERE is_demo=1")?;
    ex("DELETE FROM expenses WHERE is_demo=1")?;
    ex("DELETE FROM warehouses WHERE is_demo=1")?;
    ex("DELETE FROM market_prices WHERE is_demo=1")?;
    ex(&format!("DELETE FROM weather_records WHERE farm_id IN {demo_farms}"))?;
    ex("DELETE FROM flocks WHERE is_demo=1")?;
    ex("DELETE FROM fish_ponds WHERE is_demo=1")?;
    // Demo products are removed unless a real sale or purchase has used them since.
    let used = "(SELECT product_id FROM receipt_items WHERE product_id IS NOT NULL UNION SELECT product_id FROM purchase_items)";
    for t in ["stock_movements", "stock_batches", "stock_locations", "storage_movements"] {
        ex(&format!("DELETE FROM {t} WHERE product_id IN (SELECT id FROM products WHERE is_demo=1 AND id NOT IN {used})"))?;
    }
    ex(&format!("DELETE FROM products WHERE is_demo=1 AND id NOT IN {used}"))?;
    ex("DELETE FROM crop_cycles WHERE is_demo=1")?;
    ex("DELETE FROM farm_plots WHERE is_demo=1 AND id NOT IN (SELECT plot_id FROM crop_cycles)")?;
    ex("DELETE FROM processing_batches WHERE is_demo=1")?;
    let demo_runs: Vec<i64> = { let mut st = conn.prepare("SELECT id FROM palm_runs WHERE is_demo=1").map_err(|e| e.to_string())?; let v = st.query_map([], |r| r.get(0)).map_err(|e| e.to_string())?.collect::<Result<Vec<i64>, _>>().map_err(|e| e.to_string())?; v };
    for id in demo_runs { crate::palm_oil::undo_stock(conn, id, "Demo")?; }
    ex("DELETE FROM palm_runs WHERE is_demo=1")?;
    ex("DELETE FROM farm_reminders WHERE is_demo=1")?;
    ex("DELETE FROM suppliers WHERE is_demo=1 AND id NOT IN (SELECT supplier_id FROM purchases WHERE supplier_id IS NOT NULL)")?;
    ex("DELETE FROM customers WHERE is_demo=1 AND id NOT IN (SELECT customer_id FROM receipts WHERE customer_id IS NOT NULL)")?;
    ex("DELETE FROM farms WHERE is_demo=1 AND id NOT IN (SELECT farm_id FROM flocks WHERE farm_id IS NOT NULL UNION SELECT farm_id FROM farm_plots WHERE farm_id IS NOT NULL UNION SELECT farm_id FROM processing_batches WHERE farm_id IS NOT NULL)")?;
    let res = ClearResult {
        kept_suppliers: conn.query_row("SELECT COUNT(*) FROM suppliers WHERE is_demo=1", [], |r| r.get(0)).map_err(|e| e.to_string())?,
        kept_customers: conn.query_row("SELECT COUNT(*) FROM customers WHERE is_demo=1", [], |r| r.get(0)).map_err(|e| e.to_string())?,
    };
    db::set_setting(conn, "demo_loaded", "0")?;
    Ok(res)
}

#[tauri::command]
pub fn load_demo_data(state: tauri::State<'_, AppState>, session_token: String) -> Result<(), String> {
    let me = auth::require(&state, &session_token, Role::Owner)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    load_tx(&tx)?;
    crate::audit::log(&tx, &me.display_name(), "demo_loaded", "demo", None, "");
    tx.commit().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_demo_data(state: tauri::State<'_, AppState>, session_token: String) -> Result<ClearResult, String> {
    let me = auth::require(&state, &session_token, Role::Owner)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let r = clear_tx(&tx)?;
    crate::audit::log(&tx, &me.display_name(), "demo_cleared", "demo", None, "");
    tx.commit().map_err(|e| e.to_string())?;
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_loads_matches_spec_and_clears_completely() {
        let mut base = crate::db::test_conn();
        base.execute("INSERT INTO users(username,password_hash,role) VALUES('boss','x','owner')", []).unwrap();
        let conn = base.transaction().unwrap();
        // A real batch must survive clearing.
        conn.execute("INSERT INTO flocks(code,placed_date,chicks_placed) VALUES('REAL-1','2026-01-01',50)", []).unwrap();
        load_tx(&conn).unwrap();
        assert!(load_tx(&conn).is_err(), "loading twice is refused");
        let id: i64 = conn.query_row("SELECT id FROM flocks WHERE code='BR-DEMO'", [], |r| r.get(0)).unwrap();
        let f = crate::flocks::summary(&conn, id).unwrap();
        assert_eq!((f.chicks_placed, f.dead, f.culled, f.alive), (500, 12, 3, 485));
        assert!((f.mortality_pct - 2.4).abs() < 1e-9);
        let pb: i64 = conn.query_row("SELECT id FROM processing_batches WHERE batch_no='PB-DEMO'", [], |r| r.get(0)).unwrap();
        assert_eq!(crate::processing::batch_detail(&conn, pb).unwrap().batch.total_cost_kobo, 26_500_000);
        let o = crate::farms::overview_tx(&conn).unwrap();
        assert!(o.demo_loaded);
        assert_eq!(o.vaccinations_need_review, 1);
        let pl = crate::finance::profit_loss(&conn, "2000-01-01", "2100-01-01", true).unwrap();
        assert!(pl.owed_by_customers_kobo > 0 && pl.owed_to_suppliers_kobo == 3_000_000, "{} {}", pl.owed_by_customers_kobo, pl.owed_to_suppliers_kobo);
        let demo_only = crate::finance::profit_loss(&conn, "2000-01-01", "2100-01-01", false).unwrap();
        assert_eq!(demo_only.revenue_kobo, 0, "demo money is excluded by default");
        let n = |sql: &str| conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
        assert_eq!(n("SELECT COUNT(*) FROM livestock_animals"), 7);
        assert_eq!(n("SELECT COUNT(*) FROM flock_transfers"), 1);
        assert!(n("SELECT COUNT(*) FROM storage_movements WHERE kind='transfer_in'") == 1);
        assert_eq!(n("SELECT COUNT(*) FROM egg_movements"), 4);
        crate::finance::by_activity(&conn, "2000-01-01", "2100-01-01", true).unwrap();
        clear_tx(&conn).unwrap();
        for t in ["products", "receipts", "purchases", "expenses", "warehouses", "market_prices", "fish_ponds"] {
            assert_eq!(n(&format!("SELECT COUNT(*) FROM {t} WHERE is_demo=1")), 0, "{t}");
        }
        for t in ["receipt_items", "livestock_animals", "egg_movements", "storage_movements", "weather_records", "water_quality_records"] {
            assert_eq!(n(&format!("SELECT COUNT(*) FROM {t}")), 0, "{t}");
        }
        // The extended demo: credit sale, purchase owed, storage, eggs, goats, pond, harvest, prices.
        load_tx(&conn).unwrap();
        clear_tx(&conn).unwrap();
        for t in ["flocks", "crop_cycles", "farm_plots", "processing_batches", "suppliers", "customers", "farms", "farm_reminders"] {
            let n: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {t} WHERE is_demo=1"), [], |r| r.get(0)).unwrap();
            assert_eq!(n, 0, "{t}");
        }
        let real: i64 = conn.query_row("SELECT COUNT(*) FROM flocks", [], |r| r.get(0)).unwrap();
        assert_eq!(real, 1);
        let fk: i64 = conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0)).unwrap();
        assert_eq!(fk, 0);
    }
}
