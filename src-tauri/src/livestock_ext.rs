//! More livestock records: animals received into a batch, transfers between
//! batches, egg stock (sold, used, lost), individually identified animals with
//! weights, breeding and births, fish ponds and water quality.
//!
//! Every count change is its own event in one transaction, so the animals
//! alive in a batch always add up and nothing is deducted twice.

use crate::{auth::{self, Role}, farms::{check_len, check_money, check_opt_date, rows, valid_date}, flocks::summary, AppState};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

fn active_batch(conn: &Connection, id: i64) -> Result<crate::flocks::FlockSummary, String> {
    let f = summary(conn, id)?;
    if f.status != "active" { return Err(format!("Batch {} is closed. Reopen it first.", f.code)); }
    Ok(f)
}

// ------------------------------------------------------------------ receipts and transfers

#[derive(Deserialize, Default)]
pub struct ReceiveInput { pub flock_id: i64, pub date: String, pub quantity: i64, #[serde(default)] pub cost_kobo: i64, #[serde(default)] pub source: String, #[serde(default)] pub notes: String }

pub fn receive_tx(conn: &Connection, i: &ReceiveInput, who: &str) -> Result<(), String> {
    active_batch(conn, i.flock_id)?;
    if !valid_date(&i.date) { return Err("Choose the date".into()); }
    if !(1..=1_000_000).contains(&i.quantity) { return Err("Enter how many animals arrived".into()); }
    check_money(i.cost_kobo, "Cost")?;
    check_len(&i.source, 120, "Source")?;
    check_len(&i.notes, 300, "Notes")?;
    conn.execute("INSERT INTO flock_receipts(flock_id,receipt_date,quantity,cost_kobo,source,notes,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![i.flock_id, i.date, i.quantity, i.cost_kobo, i.source.trim(), i.notes.trim(), who]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn receive_animals(state: tauri::State<'_, AppState>, session_token: String, input: ReceiveInput) -> Result<(), String> {
    let me = auth::require(&state, &session_token, Role::Manager)?;
    receive_tx(&*state.db()?, &input, &me.display_name())
}

#[derive(Deserialize, Default)]
pub struct TransferInput { pub from_flock_id: i64, pub to_flock_id: i64, pub date: String, pub quantity: i64, #[serde(default)] pub notes: String }

/// Moves animals between two active batches of the same kind. The total
/// across both batches does not change.
pub fn transfer_tx(conn: &Connection, i: &TransferInput, who: &str) -> Result<(), String> {
    if i.from_flock_id == i.to_flock_id { return Err("Choose a different batch to move them to".into()); }
    let from = active_batch(conn, i.from_flock_id)?;
    let to = active_batch(conn, i.to_flock_id)?;
    if from.group != to.group { return Err("Animals can only move between batches of the same kind".into()); }
    if !valid_date(&i.date) { return Err("Choose the date".into()); }
    if i.quantity <= 0 { return Err("Enter how many to move".into()); }
    if i.quantity > from.alive { return Err(format!("Only {} are alive in {}", from.alive, from.code)); }
    check_len(&i.notes, 300, "Notes")?;
    conn.execute("INSERT INTO flock_transfers(from_flock_id,to_flock_id,transfer_date,quantity,notes,user_name) VALUES(?1,?2,?3,?4,?5,?6)",
        params![i.from_flock_id, i.to_flock_id, i.date, i.quantity, i.notes.trim(), who]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn transfer_animals(state: tauri::State<'_, AppState>, session_token: String, input: TransferInput) -> Result<(), String> {
    let me = auth::require(&state, &session_token, Role::Manager)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    transfer_tx(&tx, &input, &me.display_name())?;
    tx.commit().map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ eggs

#[derive(Deserialize, Default)]
pub struct EggInput { pub flock_id: i64, pub date: String, pub kind: String, pub eggs: i64, #[serde(default)] pub amount_kobo: i64, #[serde(default)] pub buyer: String, #[serde(default)] pub notes: String }

/// Sold, used on the farm or lost. Never more than the eggs in stock.
pub fn eggs_tx(conn: &Connection, i: &EggInput, who: &str) -> Result<(), String> {
    let f = summary(conn, i.flock_id)?;
    if !valid_date(&i.date) { return Err("Choose the date".into()); }
    if !["sold", "used", "lost"].contains(&i.kind.as_str()) { return Err("Choose sold, used or lost".into()); }
    if i.eggs <= 0 { return Err("Enter the number of eggs".into()); }
    if i.eggs > f.egg_stock { return Err(format!("Only {} eggs are in stock for {}", f.egg_stock, f.code)); }
    check_money(i.amount_kobo, "Amount")?;
    if i.kind != "sold" && i.amount_kobo > 0 { return Err("Only sold eggs have an amount".into()); }
    check_len(&i.buyer, 120, "Buyer")?;
    check_len(&i.notes, 300, "Notes")?;
    conn.execute("INSERT INTO egg_movements(flock_id,move_date,kind,eggs,amount_kobo,buyer,notes,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![i.flock_id, i.date, i.kind, i.eggs, i.amount_kobo, i.buyer.trim(), i.notes.trim(), who]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn record_eggs(state: tauri::State<'_, AppState>, session_token: String, input: EggInput) -> Result<(), String> {
    let me = if input.kind == "sold" { auth::require_seller(&state, &session_token, Role::Cashier)? } else { auth::require(&state, &session_token, Role::Worker)? };
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    eggs_tx(&tx, &input, &me.display_name())?;
    tx.commit().map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ individual animals

#[derive(Serialize, Debug)]
pub struct Animal {
    id: i64, flock_id: i64, flock_code: String, tag: String, name: String, sex: String, breed: String, birth_date: String, acquired_date: String,
    dam_id: Option<i64>, dam_tag: String, sire_id: Option<i64>, sire_tag: String, status: String, status_date: String, notes: String,
    latest_weight_kg: f64, latest_weight_date: String, weights: Vec<(String, f64)>,
}

pub fn list_animals_tx(conn: &Connection, flock_id: Option<i64>, include_gone: bool) -> Result<Vec<Animal>, String> {
    let mut list = rows(conn, "SELECT a.id,a.flock_id,f.code,a.tag,a.name,a.sex,a.breed,a.birth_date,a.acquired_date,a.dam_id,COALESCE(d.tag,''),a.sire_id,COALESCE(s.tag,''),a.status,a.status_date,a.notes
        FROM livestock_animals a JOIN flocks f ON f.id=a.flock_id LEFT JOIN livestock_animals d ON d.id=a.dam_id LEFT JOIN livestock_animals s ON s.id=a.sire_id
        WHERE (?1 IS NULL OR a.flock_id=?1) AND (?2=1 OR a.status='active') ORDER BY a.status, a.tag LIMIT 2000", params![flock_id, include_gone as i64], |r| Ok(Animal {
        id: r.get(0)?, flock_id: r.get(1)?, flock_code: r.get(2)?, tag: r.get(3)?, name: r.get(4)?, sex: r.get(5)?, breed: r.get(6)?, birth_date: r.get(7)?, acquired_date: r.get(8)?,
        dam_id: r.get(9)?, dam_tag: r.get(10)?, sire_id: r.get(11)?, sire_tag: r.get(12)?, status: r.get(13)?, status_date: r.get(14)?, notes: r.get(15)?,
        latest_weight_kg: 0.0, latest_weight_date: String::new(), weights: Vec::new(),
    }))?;
    for a in list.iter_mut() {
        a.weights = rows(conn, "SELECT weigh_date,weight_kg FROM animal_weights WHERE animal_id=?1 ORDER BY weigh_date, id", [a.id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        if let Some((d, w)) = a.weights.last() { a.latest_weight_date = d.clone(); a.latest_weight_kg = *w; }
    }
    Ok(list)
}

#[tauri::command]
pub fn list_animals(state: tauri::State<'_, AppState>, session_token: String, flock_id: Option<i64>, include_gone: Option<bool>) -> Result<Vec<Animal>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    list_animals_tx(&*state.db()?, flock_id, include_gone.unwrap_or(false))
}

#[derive(Deserialize, Default)]
pub struct AnimalInput {
    pub id: Option<i64>, pub flock_id: i64, pub tag: String, #[serde(default)] pub name: String, #[serde(default)] pub sex: String, #[serde(default)] pub breed: String,
    #[serde(default)] pub birth_date: String, #[serde(default)] pub acquired_date: String, #[serde(default)] pub dam_id: Option<i64>, #[serde(default)] pub sire_id: Option<i64>, #[serde(default)] pub notes: String,
}

pub fn save_animal_tx(conn: &Connection, i: &AnimalInput) -> Result<i64, String> {
    let tag = i.tag.trim();
    if tag.is_empty() { return Err("Enter the animal's tag or ID".into()); }
    check_len(tag, 40, "Tag")?;
    check_len(&i.name, 60, "Name")?;
    check_len(&i.breed, 60, "Breed")?;
    check_len(&i.notes, 500, "Notes")?;
    check_opt_date(&i.birth_date, "Date of birth")?;
    check_opt_date(&i.acquired_date, "Date acquired")?;
    let sex = if i.sex.is_empty() { "unknown" } else { i.sex.as_str() };
    if !["female", "male", "unknown"].contains(&sex) { return Err("Choose female, male or unknown".into()); }
    let f = summary(conn, i.flock_id)?;
    if f.group == "fish" { return Err("Fish are tracked by batch, not one by one".into()); }
    if i.id.is_some() && (i.dam_id == i.id || i.sire_id == i.id) { return Err("An animal cannot be its own parent".into()); }
    let dup: Option<i64> = conn.query_row("SELECT id FROM livestock_animals WHERE tag=?1 AND id<>COALESCE(?2,-1)", params![tag, i.id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    if dup.is_some() { return Err("Another animal already has that tag".into()); }
    if i.id.is_none() {
        // The register can never list more live animals than the batch has.
        if f.animals_registered >= f.alive { return Err(format!("All {} animals alive in {} are already registered", f.alive, f.code)); }
    }
    match i.id {
        Some(id) => {
            let n = conn.execute("UPDATE livestock_animals SET flock_id=?1,tag=?2,name=?3,sex=?4,breed=?5,birth_date=?6,acquired_date=?7,dam_id=?8,sire_id=?9,notes=?10 WHERE id=?11",
                params![i.flock_id, tag, i.name.trim(), sex, i.breed.trim(), i.birth_date, i.acquired_date, i.dam_id, i.sire_id, i.notes.trim(), id]).map_err(|e| e.to_string())?;
            if n == 0 { return Err("Animal not found".into()); }
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO livestock_animals(flock_id,tag,name,sex,breed,birth_date,acquired_date,dam_id,sire_id,notes) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                params![i.flock_id, tag, i.name.trim(), sex, i.breed.trim(), i.birth_date, i.acquired_date, i.dam_id, i.sire_id, i.notes.trim()]).map_err(|e| e.to_string())?;
            Ok(conn.last_insert_rowid())
        }
    }
}

#[tauri::command]
pub fn save_animal(state: tauri::State<'_, AppState>, session_token: String, input: AnimalInput) -> Result<i64, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    save_animal_tx(&*state.db()?, &input)
}

/// Marks an animal sold, dead or culled and records the same event on its
/// batch, so the batch count and the register always agree.
pub fn animal_status_tx(conn: &Connection, id: i64, status: &str, date: &str, amount_kobo: i64, buyer: &str, who: &str) -> Result<(), String> {
    let (flock, tag, current): (i64, String, String) = conn.query_row("SELECT flock_id,tag,status FROM livestock_animals WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional().map_err(|e| e.to_string())?.ok_or("Animal not found")?;
    if current != "active" { return Err(format!("{tag} is already marked {current}")); }
    if !valid_date(date) { return Err("Choose the date".into()); }
    check_money(amount_kobo, "Amount")?;
    let f = active_batch(conn, flock)?;
    if f.alive < 1 { return Err("No animals are alive in this batch".into()); }
    match status {
        "dead" => conn.execute("INSERT INTO flock_logs(flock_id,log_date,mortality,notes,user_name) VALUES(?1,?2,1,?3,?4)", params![flock, date, format!("{tag} died"), who]),
        "culled" => conn.execute("INSERT INTO flock_logs(flock_id,log_date,culls,notes,user_name) VALUES(?1,?2,1,?3,?4)", params![flock, date, format!("{tag} culled"), who]),
        "sold" => conn.execute("INSERT INTO flock_sales(flock_id,sale_date,birds,amount_kobo,buyer,user_name) VALUES(?1,?2,1,?3,?4,?5)", params![flock, date, amount_kobo, buyer.trim(), who]),
        _ => return Err("Choose sold, dead or culled".into()),
    }.map_err(|e| e.to_string())?;
    conn.execute("UPDATE livestock_animals SET status=?1,status_date=?2 WHERE id=?3", params![status, date, id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_animal_status(state: tauri::State<'_, AppState>, session_token: String, id: i64, status: String, date: String, amount_kobo: Option<i64>, buyer: Option<String>) -> Result<(), String> {
    let me = auth::require(&state, &session_token, Role::Manager)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    animal_status_tx(&tx, id, &status, &date, amount_kobo.unwrap_or(0), &buyer.unwrap_or_default(), &me.display_name())?;
    crate::audit::log(&tx, &me.display_name(), "animal_status", "livestock_animal", Some(id), &status);
    tx.commit().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_animal_weight(state: tauri::State<'_, AppState>, session_token: String, animal_id: i64, date: String, weight_kg: f64) -> Result<(), String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    if !valid_date(&date) { return Err("Choose the date".into()); }
    if !(weight_kg.is_finite() && weight_kg > 0.0 && weight_kg < 5000.0) { return Err("Enter the weight in kg".into()); }
    let conn = state.db()?;
    let n = conn.execute("INSERT INTO animal_weights(animal_id,weigh_date,weight_kg,user_name) SELECT id,?2,?3,?4 FROM livestock_animals WHERE id=?1", params![animal_id, date, weight_kg, me.display_name()]).map_err(|e| e.to_string())?;
    if n == 0 { return Err("Animal not found".into()); }
    Ok(())
}

// ------------------------------------------------------------------ breeding and births

#[derive(Serialize, Debug)]
pub struct Breeding { id: i64, flock_id: i64, flock_code: String, dam_id: Option<i64>, dam_tag: String, sire_id: Option<i64>, sire_tag: String, sire_text: String, mating_date: String, method: String, expected_due: String, status: String, notes: String, births: i64 }

pub fn list_breeding_tx(conn: &Connection, flock_id: Option<i64>) -> Result<Vec<Breeding>, String> {
    rows(conn, "SELECT b.id,b.flock_id,f.code,b.dam_id,COALESCE(d.tag,''),b.sire_id,COALESCE(s.tag,''),b.sire_text,b.mating_date,b.method,b.expected_due,b.status,b.notes,
        (SELECT COALESCE(SUM(born_alive),0) FROM birth_records x WHERE x.breeding_id=b.id)
        FROM breeding_records b JOIN flocks f ON f.id=b.flock_id LEFT JOIN livestock_animals d ON d.id=b.dam_id LEFT JOIN livestock_animals s ON s.id=b.sire_id
        WHERE ?1 IS NULL OR b.flock_id=?1 ORDER BY CASE WHEN b.status IN ('mated','pregnant') THEN 0 ELSE 1 END, b.expected_due, b.id DESC LIMIT 500", [flock_id], |r| Ok(Breeding {
        id: r.get(0)?, flock_id: r.get(1)?, flock_code: r.get(2)?, dam_id: r.get(3)?, dam_tag: r.get(4)?, sire_id: r.get(5)?, sire_tag: r.get(6)?, sire_text: r.get(7)?,
        mating_date: r.get(8)?, method: r.get(9)?, expected_due: r.get(10)?, status: r.get(11)?, notes: r.get(12)?, births: r.get(13)?,
    }))
}

#[tauri::command]
pub fn list_breeding(state: tauri::State<'_, AppState>, session_token: String, flock_id: Option<i64>) -> Result<Vec<Breeding>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    list_breeding_tx(&*state.db()?, flock_id)
}

#[derive(Deserialize, Default)]
pub struct BreedingInput { pub id: Option<i64>, pub flock_id: i64, #[serde(default)] pub dam_id: Option<i64>, #[serde(default)] pub sire_id: Option<i64>, #[serde(default)] pub sire_text: String, pub mating_date: String, #[serde(default)] pub method: String, #[serde(default)] pub expected_due: String, #[serde(default)] pub status: String, #[serde(default)] pub notes: String }

pub fn save_breeding_tx(conn: &Connection, i: &BreedingInput, who: &str) -> Result<i64, String> {
    summary(conn, i.flock_id)?;
    if !valid_date(&i.mating_date) { return Err("Choose the mating or service date".into()); }
    check_opt_date(&i.expected_due, "Expected due date")?;
    if !i.expected_due.is_empty() && i.expected_due < i.mating_date { return Err("The due date is before the mating date".into()); }
    let method = if i.method.is_empty() { "natural" } else { i.method.as_str() };
    if !["natural", "artificial"].contains(&method) { return Err("Choose natural or artificial".into()); }
    let status = if i.status.is_empty() { "mated" } else { i.status.as_str() };
    if !["mated", "pregnant", "not_pregnant", "delivered", "lost"].contains(&status) { return Err("Unknown status".into()); }
    check_len(&i.sire_text, 80, "Sire")?;
    check_len(&i.notes, 500, "Notes")?;
    for (aid, sex) in [(i.dam_id, "female"), (i.sire_id, "male")] {
        if let Some(a) = aid {
            let s: String = conn.query_row("SELECT sex FROM livestock_animals WHERE id=?1", [a], |r| r.get(0)).optional().map_err(|e| e.to_string())?.ok_or("Animal not found")?;
            if s != sex && s != "unknown" { return Err(format!("The {} must be {sex}", if sex == "female" { "dam" } else { "sire" })); }
        }
    }
    match i.id {
        Some(id) => {
            let n = conn.execute("UPDATE breeding_records SET dam_id=?1,sire_id=?2,sire_text=?3,mating_date=?4,method=?5,expected_due=?6,status=?7,notes=?8 WHERE id=?9",
                params![i.dam_id, i.sire_id, i.sire_text.trim(), i.mating_date, method, i.expected_due, status, i.notes.trim(), id]).map_err(|e| e.to_string())?;
            if n == 0 { return Err("Record not found".into()); }
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO breeding_records(flock_id,dam_id,sire_id,sire_text,mating_date,method,expected_due,status,notes,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                params![i.flock_id, i.dam_id, i.sire_id, i.sire_text.trim(), i.mating_date, method, i.expected_due, status, i.notes.trim(), who]).map_err(|e| e.to_string())?;
            Ok(conn.last_insert_rowid())
        }
    }
}

#[tauri::command]
pub fn save_breeding(state: tauri::State<'_, AppState>, session_token: String, input: BreedingInput) -> Result<i64, String> {
    let me = auth::require_adviser_or_manager(&state, &session_token)?;
    save_breeding_tx(&*state.db()?, &input, &me.display_name())
}

#[derive(Deserialize, Default)]
pub struct BirthInput {
    pub flock_id: i64, #[serde(default)] pub breeding_id: Option<i64>, #[serde(default)] pub dam_id: Option<i64>, pub birth_date: String, pub born_alive: i64, #[serde(default)] pub stillborn: i64, #[serde(default)] pub notes: String,
    /// Register each live offspring in the animal register with tags PREFIX-1, PREFIX-2…
    #[serde(default)] pub tag_prefix: String,
}

pub fn birth_tx(conn: &Connection, i: &BirthInput, who: &str) -> Result<Vec<i64>, String> {
    let f = active_batch(conn, i.flock_id)?;
    if f.group == "fish" { return Err("Use Receive animals for fish fingerlings".into()); }
    if !valid_date(&i.birth_date) { return Err("Choose the date of birth".into()); }
    if i.born_alive < 0 || i.stillborn < 0 || i.born_alive + i.stillborn == 0 { return Err("Enter how many were born".into()); }
    if i.born_alive > 1000 { return Err("Check the number born".into()); }
    check_len(&i.notes, 500, "Notes")?;
    check_len(&i.tag_prefix, 30, "Tag prefix")?;
    let mut sire: Option<i64> = None;
    if let Some(b) = i.breeding_id {
        let (fl, s): (i64, Option<i64>) = conn.query_row("SELECT flock_id,sire_id FROM breeding_records WHERE id=?1", [b], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(|e| e.to_string())?.ok_or("Breeding record not found")?;
        if fl != i.flock_id { return Err("That breeding record belongs to another batch".into()); }
        sire = s;
        conn.execute("UPDATE breeding_records SET status='delivered' WHERE id=?1", [b]).map_err(|e| e.to_string())?;
    }
    conn.execute("INSERT INTO birth_records(flock_id,breeding_id,dam_id,birth_date,born_alive,stillborn,notes,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![i.flock_id, i.breeding_id, i.dam_id, i.birth_date, i.born_alive, i.stillborn, i.notes.trim(), who]).map_err(|e| e.to_string())?;
    let mut ids = Vec::new();
    let prefix = i.tag_prefix.trim();
    if !prefix.is_empty() {
        let mut n = 1;
        while (ids.len() as i64) < i.born_alive {
            let tag = format!("{prefix}-{n}");
            n += 1;
            let taken: Option<i64> = conn.query_row("SELECT id FROM livestock_animals WHERE tag=?1", [&tag], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
            if taken.is_some() { continue; }
            conn.execute("INSERT INTO livestock_animals(flock_id,tag,birth_date,dam_id,sire_id,breed) VALUES(?1,?2,?3,?4,?5,?6)", params![i.flock_id, tag, i.birth_date, i.dam_id, sire, f.breed]).map_err(|e| e.to_string())?;
            ids.push(conn.last_insert_rowid());
        }
    }
    Ok(ids)
}

#[tauri::command]
pub fn record_birth(state: tauri::State<'_, AppState>, session_token: String, input: BirthInput) -> Result<usize, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let ids = birth_tx(&tx, &input, &me.display_name())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(ids.len())
}

// ------------------------------------------------------------------ fish ponds and water quality

#[derive(Serialize, Debug)]
pub struct Pond { id: i64, farm_id: Option<i64>, code: String, pond_type: String, size: f64, size_unit: String, notes: String, status: String, is_demo: bool, stocked: i64, batches: Vec<String>, last_water_date: String }

pub fn list_ponds_tx(conn: &Connection, include_archived: bool) -> Result<Vec<Pond>, String> {
    let mut ps = rows(conn, "SELECT id,farm_id,code,pond_type,size,size_unit,notes,status,is_demo,COALESCE((SELECT MAX(record_date) FROM water_quality_records w WHERE w.pond_id=p.id),'') FROM fish_ponds p WHERE ?1=1 OR status='active' ORDER BY status, code",
        [include_archived as i64], |r| Ok(Pond { id: r.get(0)?, farm_id: r.get(1)?, code: r.get(2)?, pond_type: r.get(3)?, size: r.get(4)?, size_unit: r.get(5)?, notes: r.get(6)?, status: r.get(7)?, is_demo: r.get::<_, i64>(8)? == 1, stocked: 0, batches: vec![], last_water_date: r.get(9)? }))?;
    for p in ps.iter_mut() {
        let ids: Vec<i64> = rows(conn, "SELECT id FROM flocks WHERE pond_id=?1 AND status='active'", [p.id], |r| r.get(0))?;
        for id in ids { let f = summary(conn, id)?; p.stocked += f.alive; p.batches.push(f.code); }
    }
    Ok(ps)
}

#[tauri::command]
pub fn list_ponds(state: tauri::State<'_, AppState>, session_token: String, include_archived: Option<bool>) -> Result<Vec<Pond>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    list_ponds_tx(&*state.db()?, include_archived.unwrap_or(false))
}

#[derive(Deserialize, Default)]
pub struct PondInput { pub id: Option<i64>, pub farm_id: Option<i64>, pub code: String, #[serde(default)] pub pond_type: String, #[serde(default)] pub size: f64, #[serde(default)] pub size_unit: String, #[serde(default)] pub notes: String }

pub fn save_pond_tx(conn: &Connection, i: &PondInput) -> Result<i64, String> {
    let code = i.code.trim();
    if code.is_empty() { return Err("Give the pond a code, such as POND-1".into()); }
    check_len(code, 30, "Pond code")?;
    check_len(&i.pond_type, 60, "Pond type")?;
    check_len(&i.notes, 500, "Notes")?;
    crate::farms::check_qty(i.size, "Size")?;
    let unit = if i.size_unit.trim().is_empty() { "square_metre" } else { i.size_unit.trim() };
    check_len(unit, 30, "Unit")?;
    let dup: Option<i64> = conn.query_row("SELECT id FROM fish_ponds WHERE code=?1 AND id<>COALESCE(?2,-1)", params![code, i.id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    if dup.is_some() { return Err("Another pond already uses that code".into()); }
    match i.id {
        Some(id) => { conn.execute("UPDATE fish_ponds SET farm_id=?1,code=?2,pond_type=?3,size=?4,size_unit=?5,notes=?6 WHERE id=?7", params![i.farm_id, code, i.pond_type.trim(), i.size, unit, i.notes.trim(), id]).map_err(|e| e.to_string())?; Ok(id) }
        None => { conn.execute("INSERT INTO fish_ponds(farm_id,code,pond_type,size,size_unit,notes) VALUES(?1,?2,?3,?4,?5,?6)", params![i.farm_id, code, i.pond_type.trim(), i.size, unit, i.notes.trim()]).map_err(|e| e.to_string())?; Ok(conn.last_insert_rowid()) }
    }
}

#[tauri::command]
pub fn save_pond(state: tauri::State<'_, AppState>, session_token: String, input: PondInput) -> Result<i64, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    save_pond_tx(&*state.db()?, &input)
}

#[tauri::command]
pub fn archive_pond(state: tauri::State<'_, AppState>, session_token: String, id: i64, restore: Option<bool>) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let conn = state.db()?;
    if !restore.unwrap_or(false) {
        let stocked: i64 = conn.query_row("SELECT COUNT(*) FROM flocks WHERE pond_id=?1 AND status='active'", [id], |r| r.get(0)).map_err(|e| e.to_string())?;
        if stocked > 0 { return Err("Close or move the fish in this pond first".into()); }
    }
    conn.execute("UPDATE fish_ponds SET status=?1 WHERE id=?2", params![if restore.unwrap_or(false) { "active" } else { "archived" }, id]).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct WaterRecord { #[serde(default)] pub id: i64, pub pond_id: i64, pub record_date: String, pub temp_c: Option<f64>, pub ph: Option<f64>, pub dissolved_oxygen: Option<f64>, pub ammonia: Option<f64>, #[serde(default)] pub notes: String, #[serde(default)] pub user_name: String }

pub fn list_water_tx(conn: &Connection, pond_id: i64) -> Result<Vec<WaterRecord>, String> {
    rows(conn, "SELECT id,pond_id,record_date,temp_c,ph,dissolved_oxygen,ammonia,notes,user_name FROM water_quality_records WHERE pond_id=?1 ORDER BY record_date DESC, id DESC LIMIT 500", [pond_id], |r| Ok(WaterRecord {
        id: r.get(0)?, pond_id: r.get(1)?, record_date: r.get(2)?, temp_c: r.get(3)?, ph: r.get(4)?, dissolved_oxygen: r.get(5)?, ammonia: r.get(6)?, notes: r.get(7)?, user_name: r.get(8)?,
    }))
}

#[tauri::command]
pub fn list_water_records(state: tauri::State<'_, AppState>, session_token: String, pond_id: i64) -> Result<Vec<WaterRecord>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    list_water_tx(&*state.db()?, pond_id)
}

/// Stores only what was measured. At least one reading is required, and each
/// must be physically possible; nothing is filled in or assumed.
pub fn add_water_tx(conn: &Connection, w: &WaterRecord, who: &str) -> Result<i64, String> {
    if !valid_date(&w.record_date) { return Err("Choose the date".into()); }
    let checks = [(w.temp_c, 0.0, 45.0, "Temperature"), (w.ph, 0.0, 14.0, "pH"), (w.dissolved_oxygen, 0.0, 30.0, "Dissolved oxygen"), (w.ammonia, 0.0, 50.0, "Ammonia")];
    if checks.iter().all(|(v, ..)| v.is_none()) && w.notes.trim().is_empty() { return Err("Enter at least one reading or a note".into()); }
    for (v, lo, hi, label) in checks { if let Some(x) = v { if !(x.is_finite() && x >= lo && x <= hi) { return Err(format!("{label} must be between {lo} and {hi}")); } } }
    check_len(&w.notes, 500, "Notes")?;
    let n = conn.execute("INSERT INTO water_quality_records(pond_id,record_date,temp_c,ph,dissolved_oxygen,ammonia,notes,user_name) SELECT id,?2,?3,?4,?5,?6,?7,?8 FROM fish_ponds WHERE id=?1",
        params![w.pond_id, w.record_date, w.temp_c, w.ph, w.dissolved_oxygen, w.ammonia, w.notes.trim(), who]).map_err(|e| e.to_string())?;
    if n == 0 { return Err("Pond not found".into()); }
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn add_water_record(state: tauri::State<'_, AppState>, session_token: String, input: WaterRecord) -> Result<i64, String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    add_water_tx(&*state.db()?, &input, &me.display_name())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch(conn: &Connection, code: &str, species: &str, n: i64) -> i64 {
        conn.execute("INSERT INTO flocks(code,placed_date,chicks_placed,species) VALUES(?1,'2026-01-01',?2,?3)", params![code, n, species]).unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn transfers_receipts_and_births_keep_counts_honest() {
        let conn = crate::db::test_conn();
        let a = batch(&conn, "GT-1", "goat", 10);
        let b = batch(&conn, "GT-2", "goat", 5);
        let fish = batch(&conn, "CF-1", "catfish", 100);
        transfer_tx(&conn, &TransferInput { from_flock_id: a, to_flock_id: b, date: "2026-02-01".into(), quantity: 3, ..Default::default() }, "M").unwrap();
        assert!(transfer_tx(&conn, &TransferInput { from_flock_id: a, to_flock_id: b, date: "2026-02-01".into(), quantity: 8, ..Default::default() }, "M").is_err(), "only 7 left");
        assert!(transfer_tx(&conn, &TransferInput { from_flock_id: a, to_flock_id: fish, date: "2026-02-01".into(), quantity: 1, ..Default::default() }, "M").is_err(), "different kind");
        receive_tx(&conn, &ReceiveInput { flock_id: b, date: "2026-02-02".into(), quantity: 2, cost_kobo: 6_000_000, ..Default::default() }, "M").unwrap();
        birth_tx(&conn, &BirthInput { flock_id: b, birth_date: "2026-03-01".into(), born_alive: 2, stillborn: 1, tag_prefix: "KID".into(), ..Default::default() }, "W").unwrap();
        let (fa, fb) = (summary(&conn, a).unwrap(), summary(&conn, b).unwrap());
        assert_eq!(fa.alive + fb.alive, 10 + 5 + 2 + 2, "total only grows by arrivals and births");
        assert_eq!(fb.alive, 12);
        assert_eq!(fb.total_in, 12);
        assert_eq!(fb.costs_kobo, 6_000_000);
        assert_eq!(fb.animals_registered, 2);
        assert!(birth_tx(&conn, &BirthInput { flock_id: fish, birth_date: "2026-03-01".into(), born_alive: 5, ..Default::default() }, "W").is_err());
    }

    #[test]
    fn egg_stock_and_revenue() {
        let conn = crate::db::test_conn();
        let l = batch(&conn, "LY-1", "layer", 100);
        conn.execute("INSERT INTO flock_logs(flock_id,log_date,eggs_collected,eggs_damaged) VALUES(?1,'2026-02-01',90,5)", [l]).unwrap();
        eggs_tx(&conn, &EggInput { flock_id: l, date: "2026-02-01".into(), kind: "sold".into(), eggs: 60, amount_kobo: 300_000, ..Default::default() }, "S").unwrap();
        eggs_tx(&conn, &EggInput { flock_id: l, date: "2026-02-01".into(), kind: "used".into(), eggs: 5, ..Default::default() }, "S").unwrap();
        assert!(eggs_tx(&conn, &EggInput { flock_id: l, date: "2026-02-01".into(), kind: "sold".into(), eggs: 21, ..Default::default() }, "S").is_err(), "only 20 left");
        let f = summary(&conn, l).unwrap();
        assert_eq!(f.egg_stock, 20);
        assert_eq!(f.revenue_kobo, 300_000);
        assert!((f.laying_pct - 90.0).abs() < 1e-9);
    }

    #[test]
    fn animal_register_matches_batch() {
        let conn = crate::db::test_conn();
        let p = batch(&conn, "PG-1", "pig", 2);
        let sow = save_animal_tx(&conn, &AnimalInput { flock_id: p, tag: "SOW-1".into(), sex: "female".into(), ..Default::default() }).unwrap();
        let boar = save_animal_tx(&conn, &AnimalInput { flock_id: p, tag: "BOAR-1".into(), sex: "male".into(), ..Default::default() }).unwrap();
        assert!(save_animal_tx(&conn, &AnimalInput { flock_id: p, tag: "X".into(), ..Default::default() }).is_err(), "only 2 alive");
        assert!(save_breeding_tx(&conn, &BreedingInput { flock_id: p, dam_id: Some(boar), mating_date: "2026-03-01".into(), ..Default::default() }, "M").is_err(), "dam must be female");
        let br = save_breeding_tx(&conn, &BreedingInput { flock_id: p, dam_id: Some(sow), sire_id: Some(boar), mating_date: "2026-03-01".into(), expected_due: "2026-06-23".into(), ..Default::default() }, "M").unwrap();
        birth_tx(&conn, &BirthInput { flock_id: p, breeding_id: Some(br), dam_id: Some(sow), birth_date: "2026-06-24".into(), born_alive: 8, tag_prefix: "P26".into(), ..Default::default() }, "W").unwrap();
        let litter = list_animals_tx(&conn, Some(p), false).unwrap();
        assert_eq!(litter.len(), 10);
        assert!(litter.iter().filter(|a| a.tag.starts_with("P26")).all(|a| a.dam_id == Some(sow) && a.sire_id == Some(boar)));
        assert_eq!(list_breeding_tx(&conn, Some(p)).unwrap()[0].status, "delivered");
        animal_status_tx(&conn, boar, "sold", "2026-07-01", 25_000_000, "Buyer", "M").unwrap();
        let f = summary(&conn, p).unwrap();
        assert_eq!((f.alive, f.sold, f.revenue_kobo), (9, 1, 25_000_000));
        assert!(animal_status_tx(&conn, boar, "dead", "2026-07-02", 0, "", "M").is_err());
    }

    #[test]
    fn water_readings_are_never_invented() {
        let conn = crate::db::test_conn();
        let pond = save_pond_tx(&conn, &PondInput { code: "POND-1".into(), size: 50.0, ..Default::default() }).unwrap();
        assert!(add_water_tx(&conn, &WaterRecord { pond_id: pond, record_date: "2026-02-01".into(), ..Default::default() }, "W").is_err());
        assert!(add_water_tx(&conn, &WaterRecord { pond_id: pond, record_date: "2026-02-01".into(), ph: Some(15.0), ..Default::default() }, "W").is_err());
        add_water_tx(&conn, &WaterRecord { pond_id: pond, record_date: "2026-02-01".into(), ph: Some(7.2), ..Default::default() }, "W").unwrap();
        let w = list_water_tx(&conn, pond).unwrap();
        assert_eq!((w[0].ph, w[0].temp_c), (Some(7.2), None));
    }
}
