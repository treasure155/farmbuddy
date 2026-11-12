//! Livestock health observations, treatments as prescribed, and vaccination
//! events with the statuses Upcoming, Due, Overdue, Completed, Skipped and
//! Needs vet review.
//!
//! FarmBuddy never diagnoses or prescribes. It records what the farmer saw and
//! what a veterinary professional advised. A vaccination planned from a public
//! reference stays "Needs vet review" until someone records that a vet (or the
//! vaccine label / hatchery) confirmed it.

use crate::{auth::{self, Role}, farms::{check_len, check_opt_date, parse_date, rows, today, valid_date}, AppState};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

// ------------------------------------------------------------------ health records

#[derive(Serialize)]
pub struct HealthRecord {
    id: i64, flock_id: i64, flock_code: String, observed_date: String, issue: String, symptoms: String, affected: i64, action_taken: String,
    vet_consulted: String, treatment: String, dosage_as_prescribed: String, route: String, start_date: String, end_date: String,
    withdrawal_days: i64, withdrawal_until: String, follow_up_date: String, notes: String, user_name: String,
}

pub fn list_health_tx(conn: &Connection, flock_id: Option<i64>) -> Result<Vec<HealthRecord>, String> {
    rows(conn, "SELECT h.id,h.flock_id,f.code,h.observed_date,h.issue,h.symptoms,h.affected,h.action_taken,h.vet_consulted,h.treatment,h.dosage_as_prescribed,h.route,h.start_date,h.end_date,h.withdrawal_days,h.follow_up_date,h.notes,h.user_name
        FROM health_records h JOIN flocks f ON f.id=h.flock_id WHERE ?1 IS NULL OR h.flock_id=?1 ORDER BY h.observed_date DESC, h.id DESC LIMIT 500", [flock_id], |r| {
        let end: String = r.get(13)?;
        let wd: i64 = r.get(14)?;
        // Withdrawal ends `wd` days after the last treatment day, as the prescriber or label states.
        let until = parse_date(&end).filter(|_| wd > 0).map(|d| (d + chrono::Duration::days(wd)).format("%Y-%m-%d").to_string()).unwrap_or_default();
        Ok(HealthRecord {
            id: r.get(0)?, flock_id: r.get(1)?, flock_code: r.get(2)?, observed_date: r.get(3)?, issue: r.get(4)?, symptoms: r.get(5)?, affected: r.get(6)?, action_taken: r.get(7)?,
            vet_consulted: r.get(8)?, treatment: r.get(9)?, dosage_as_prescribed: r.get(10)?, route: r.get(11)?, start_date: r.get(12)?, end_date: end,
            withdrawal_days: wd, withdrawal_until: until, follow_up_date: r.get(15)?, notes: r.get(16)?, user_name: r.get(17)?,
        })
    })
}

#[tauri::command]
pub fn list_health_records(state: tauri::State<'_, AppState>, session_token: String, flock_id: Option<i64>) -> Result<Vec<HealthRecord>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    list_health_tx(&*state.db()?, flock_id)
}

#[derive(Deserialize, Default)]
pub struct HealthInput {
    pub id: Option<i64>, pub flock_id: i64, pub observed_date: String, pub issue: String, #[serde(default)] pub symptoms: String, #[serde(default)] pub affected: i64,
    #[serde(default)] pub action_taken: String, #[serde(default)] pub vet_consulted: String, #[serde(default)] pub treatment: String, #[serde(default)] pub dosage_as_prescribed: String,
    #[serde(default)] pub route: String, #[serde(default)] pub start_date: String, #[serde(default)] pub end_date: String, #[serde(default)] pub withdrawal_days: i64,
    #[serde(default)] pub follow_up_date: String, #[serde(default)] pub notes: String,
}

pub fn save_health_tx(conn: &Connection, i: &HealthInput, who: &str) -> Result<i64, String> {
    if !valid_date(&i.observed_date) { return Err("Choose the date you noticed it".into()); }
    if i.issue.trim().is_empty() { return Err("Describe what you noticed".into()); }
    check_len(&i.issue, 150, "Issue")?;
    for (v, n, l) in [(&i.symptoms, 1000, "Symptoms"), (&i.action_taken, 500, "Action taken"), (&i.vet_consulted, 120, "Vet consulted"), (&i.treatment, 150, "Treatment"),
        (&i.dosage_as_prescribed, 200, "Dosage"), (&i.route, 60, "Route"), (&i.notes, 1000, "Notes")] { check_len(v, n, l)?; }
    for (d, l) in [(&i.start_date, "Start date"), (&i.end_date, "End date"), (&i.follow_up_date, "Follow-up date")] { check_opt_date(d, l)?; }
    if i.affected < 0 || i.withdrawal_days < 0 || i.withdrawal_days > 365 { return Err("Check the numbers entered".into()); }
    if !i.start_date.is_empty() && !i.end_date.is_empty() && i.end_date < i.start_date { return Err("The end date is before the start date".into()); }
    // A dose may only be written down when a vet or the product label gave it.
    if !i.dosage_as_prescribed.trim().is_empty() && i.vet_consulted.trim().is_empty() && i.treatment.trim().is_empty() { return Err("Enter the product and who prescribed it before recording a dose".into()); }
    let alive: Option<i64> = conn.query_row("SELECT id FROM flocks WHERE id=?1", [i.flock_id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    if alive.is_none() { return Err("Batch not found".into()); }
    let p = params![i.flock_id, i.observed_date, i.issue.trim(), i.symptoms.trim(), i.affected, i.action_taken.trim(), i.vet_consulted.trim(), i.treatment.trim(), i.dosage_as_prescribed.trim(), i.route.trim(), i.start_date, i.end_date, i.withdrawal_days, i.follow_up_date, i.notes.trim(), who];
    match i.id {
        Some(id) => {
            let n = conn.execute("UPDATE health_records SET flock_id=?1,observed_date=?2,issue=?3,symptoms=?4,affected=?5,action_taken=?6,vet_consulted=?7,treatment=?8,dosage_as_prescribed=?9,route=?10,start_date=?11,end_date=?12,withdrawal_days=?13,follow_up_date=?14,notes=?15,user_name=?16 WHERE id=?17",
                params![i.flock_id, i.observed_date, i.issue.trim(), i.symptoms.trim(), i.affected, i.action_taken.trim(), i.vet_consulted.trim(), i.treatment.trim(), i.dosage_as_prescribed.trim(), i.route.trim(), i.start_date, i.end_date, i.withdrawal_days, i.follow_up_date, i.notes.trim(), who, id]).map_err(|e| e.to_string())?;
            if n == 0 { return Err("Record not found".into()); }
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO health_records(flock_id,observed_date,issue,symptoms,affected,action_taken,vet_consulted,treatment,dosage_as_prescribed,route,start_date,end_date,withdrawal_days,follow_up_date,notes,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)", p).map_err(|e| e.to_string())?;
            Ok(conn.last_insert_rowid())
        }
    }
}

#[tauri::command]
pub fn save_health_record(state: tauri::State<'_, AppState>, session_token: String, input: HealthInput) -> Result<i64, String> {
    let me = auth::require(&state, &session_token, if input.id.is_some() { Role::Manager } else { Role::Cashier })?;
    save_health_tx(&*state.db()?, &input, &me.display_name())
}

#[tauri::command]
pub fn delete_health_record(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let n = state.db()?.execute("DELETE FROM health_records WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    if n == 0 { return Err("Record not found".into()); }
    Ok(())
}

// ------------------------------------------------------------------ vaccinations

/// Days before the due date when a planned vaccination shows as "Due".
pub const DUE_WINDOW_DAYS: i64 = 3;

/// upcoming | due | overdue | completed | skipped | needs_review
pub fn vaccination_status(status: &str, source: &str, due_date: &str, today: chrono::NaiveDate) -> &'static str {
    match status {
        "completed" => "completed",
        "skipped" => "skipped",
        _ if source == "pending" => "needs_review",
        _ => match parse_date(due_date) {
            Some(d) if d < today => "overdue",
            Some(d) if (d - today).num_days() <= DUE_WINDOW_DAYS => "due",
            _ => "upcoming",
        },
    }
}

#[derive(Serialize)]
pub struct Vaccination {
    pub id: i64, pub flock_id: i64, pub flock_code: String, pub species: String, pub due_date: String, pub vaccine_name: String, pub disease: String, pub manufacturer: String, pub lot_no: String,
    pub vaccine_expiry: String, pub dose: String, pub route: String, pub source: String, pub reviewed_by: String, pub status: String, pub display_status: String,
    pub administered_date: String, pub administered_by: String, pub skip_reason: String, pub notes: String, pub user_name: String,
}

pub fn list_vacc_tx(conn: &Connection, flock_id: Option<i64>, open_only: bool) -> Result<Vec<Vaccination>, String> {
    let t = today();
    rows(conn, "SELECT v.id,v.flock_id,f.code,f.species,v.due_date,v.vaccine_name,v.disease,v.manufacturer,v.lot_no,v.vaccine_expiry,v.dose,v.route,v.source,v.reviewed_by,v.status,v.administered_date,v.administered_by,v.skip_reason,v.notes,v.user_name
        FROM vaccination_events v JOIN flocks f ON f.id=v.flock_id WHERE (?1 IS NULL OR v.flock_id=?1) AND (?2=0 OR (v.status='planned' AND f.status='active'))
        ORDER BY CASE v.status WHEN 'planned' THEN 0 ELSE 1 END, v.due_date, v.id LIMIT 500", params![flock_id, open_only as i64], |r| {
        let status: String = r.get(14)?;
        let source: String = r.get(12)?;
        let due: String = r.get(4)?;
        let display = vaccination_status(&status, &source, &due, t).to_string();
        Ok(Vaccination {
            id: r.get(0)?, flock_id: r.get(1)?, flock_code: r.get(2)?, species: r.get(3)?, due_date: due, vaccine_name: r.get(5)?, disease: r.get(6)?, manufacturer: r.get(7)?, lot_no: r.get(8)?,
            vaccine_expiry: r.get(9)?, dose: r.get(10)?, route: r.get(11)?, source, reviewed_by: r.get(13)?, status, display_status: display,
            administered_date: r.get(15)?, administered_by: r.get(16)?, skip_reason: r.get(17)?, notes: r.get(18)?, user_name: r.get(19)?,
        })
    })
}

#[tauri::command]
pub fn list_vaccinations(state: tauri::State<'_, AppState>, session_token: String, flock_id: Option<i64>, open_only: Option<bool>) -> Result<Vec<Vaccination>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    list_vacc_tx(&*state.db()?, flock_id, open_only.unwrap_or(false))
}

#[derive(Default, Debug)]
pub struct VaccCounts { pub due: i64, pub overdue: i64, pub needs_review: i64 }

pub fn vaccination_counts(conn: &Connection) -> Result<VaccCounts, String> {
    let mut c = VaccCounts::default();
    for v in list_vacc_tx(conn, None, true)? {
        match v.display_status.as_str() { "due" => c.due += 1, "overdue" => c.overdue += 1, "needs_review" => c.needs_review += 1, _ => {} }
    }
    Ok(c)
}

#[derive(Deserialize, Default)]
pub struct VaccinationInput {
    pub id: Option<i64>, pub flock_id: i64, pub due_date: String, pub vaccine_name: String, #[serde(default)] pub disease: String, #[serde(default)] pub manufacturer: String,
    #[serde(default)] pub lot_no: String, #[serde(default)] pub vaccine_expiry: String, #[serde(default)] pub dose: String, #[serde(default)] pub route: String,
    pub source: String, #[serde(default)] pub reviewed_by: String, #[serde(default)] pub notes: String,
}

pub fn save_vacc_tx(conn: &Connection, i: &VaccinationInput, who: &str) -> Result<i64, String> {
    if !valid_date(&i.due_date) { return Err("Choose the planned date".into()); }
    if i.vaccine_name.trim().is_empty() { return Err("Enter the vaccine name".into()); }
    if !["vet", "label", "hatchery", "pending"].contains(&i.source.as_str()) { return Err("Say where this schedule came from".into()); }
    if i.source != "pending" && i.reviewed_by.trim().is_empty() { return Err("Enter the vet, hatchery or label that gave this schedule".into()); }
    for (v, n, l) in [(&i.vaccine_name, 120, "Vaccine name"), (&i.disease, 120, "Disease"), (&i.manufacturer, 120, "Manufacturer"), (&i.lot_no, 60, "Lot number"),
        (&i.dose, 120, "Dose"), (&i.route, 60, "Route"), (&i.reviewed_by, 120, "Confirmed by"), (&i.notes, 1000, "Notes")] { check_len(v, n, l)?; }
    check_opt_date(&i.vaccine_expiry, "Vaccine expiry")?;
    let exists: Option<i64> = conn.query_row("SELECT id FROM flocks WHERE id=?1", [i.flock_id], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
    if exists.is_none() { return Err("Batch not found".into()); }
    match i.id {
        Some(id) => {
            let n = conn.execute("UPDATE vaccination_events SET flock_id=?1,due_date=?2,vaccine_name=?3,disease=?4,manufacturer=?5,lot_no=?6,vaccine_expiry=?7,dose=?8,route=?9,source=?10,reviewed_by=?11,notes=?12 WHERE id=?13 AND status='planned'",
                params![i.flock_id, i.due_date, i.vaccine_name.trim(), i.disease.trim(), i.manufacturer.trim(), i.lot_no.trim(), i.vaccine_expiry, i.dose.trim(), i.route.trim(), i.source, i.reviewed_by.trim(), i.notes.trim(), id]).map_err(|e| e.to_string())?;
            if n == 0 { return Err("Only planned vaccinations can be edited".into()); }
            Ok(id)
        }
        None => {
            conn.execute("INSERT INTO vaccination_events(flock_id,due_date,vaccine_name,disease,manufacturer,lot_no,vaccine_expiry,dose,route,source,reviewed_by,notes,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                params![i.flock_id, i.due_date, i.vaccine_name.trim(), i.disease.trim(), i.manufacturer.trim(), i.lot_no.trim(), i.vaccine_expiry, i.dose.trim(), i.route.trim(), i.source, i.reviewed_by.trim(), i.notes.trim(), who]).map_err(|e| e.to_string())?;
            Ok(conn.last_insert_rowid())
        }
    }
}

#[tauri::command]
pub fn save_vaccination(state: tauri::State<'_, AppState>, session_token: String, input: VaccinationInput) -> Result<i64, String> {
    let me = auth::require_adviser_or_manager(&state, &session_token)?;
    save_vacc_tx(&*state.db()?, &input, &me.display_name())
}

pub fn complete_vacc_tx(conn: &Connection, id: i64, date: &str, by: &str, lot_no: &str) -> Result<(), String> {
    if !valid_date(date) { return Err("Choose the date it was given".into()); }
    if by.trim().is_empty() { return Err("Enter who gave the vaccine".into()); }
    check_len(by, 120, "Given by")?;
    check_len(lot_no, 60, "Lot number")?;
    let n = conn.execute("UPDATE vaccination_events SET status='completed',administered_date=?1,administered_by=?2,lot_no=CASE WHEN ?3<>'' THEN ?3 ELSE lot_no END WHERE id=?4 AND status='planned'",
        params![date, by.trim(), lot_no.trim(), id]).map_err(|e| e.to_string())?;
    if n == 0 { return Err("This vaccination is not planned any more".into()); }
    Ok(())
}

#[tauri::command]
pub fn complete_vaccination(state: tauri::State<'_, AppState>, session_token: String, id: i64, date: String, given_by: String, lot_no: Option<String>) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Worker)?;
    complete_vacc_tx(&*state.db()?, id, &date, &given_by, &lot_no.unwrap_or_default())
}

#[tauri::command]
pub fn skip_vaccination(state: tauri::State<'_, AppState>, session_token: String, id: i64, reason: String) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    if reason.trim().chars().count() < 3 { return Err("Give a reason for skipping".into()); }
    check_len(&reason, 300, "Reason")?;
    let n = state.db()?.execute("UPDATE vaccination_events SET status='skipped',skip_reason=?1 WHERE id=?2 AND status='planned'", params![reason.trim(), id]).map_err(|e| e.to_string())?;
    if n == 0 { return Err("This vaccination is not planned any more".into()); }
    Ok(())
}

#[tauri::command]
pub fn delete_vaccination(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    let n = state.db()?.execute("DELETE FROM vaccination_events WHERE id=?1 AND status='planned'", [id]).map_err(|e| e.to_string())?;
    if n == 0 { return Err("Completed or skipped vaccinations stay in the history".into()); }
    Ok(())
}

// ------------------------------------------------------------------ vaccination schedules (protocols)

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ProtocolItem { pub day_offset: i64, pub vaccine_name: String, #[serde(default)] pub disease: String, #[serde(default)] pub route: String, #[serde(default)] pub dose: String, #[serde(default)] pub notes: String }

#[derive(Serialize, Debug)]
pub struct Protocol { pub id: i64, pub name: String, pub species: String, pub source_text: String, pub review_status: String, pub reviewer: String, pub review_date: String, pub notes: String, pub items: Vec<ProtocolItem> }

pub fn list_protocols_tx(conn: &Connection) -> Result<Vec<Protocol>, String> {
    let mut ps = rows(conn, "SELECT id,name,species,source_text,review_status,reviewer,review_date,notes FROM vaccination_protocols ORDER BY species, name", [], |r| Ok(Protocol {
        id: r.get(0)?, name: r.get(1)?, species: r.get(2)?, source_text: r.get(3)?, review_status: r.get(4)?, reviewer: r.get(5)?, review_date: r.get(6)?, notes: r.get(7)?, items: vec![],
    }))?;
    for p in ps.iter_mut() {
        p.items = rows(conn, "SELECT day_offset,vaccine_name,disease,route,dose,notes FROM vaccination_protocol_items WHERE protocol_id=?1 ORDER BY day_offset, id", [p.id], |r| Ok(ProtocolItem {
            day_offset: r.get(0)?, vaccine_name: r.get(1)?, disease: r.get(2)?, route: r.get(3)?, dose: r.get(4)?, notes: r.get(5)?,
        }))?;
    }
    Ok(ps)
}

#[tauri::command]
pub fn list_protocols(state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<Protocol>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    list_protocols_tx(&*state.db()?)
}

#[derive(Deserialize, Default)]
pub struct ProtocolInput { pub id: Option<i64>, pub name: String, pub species: String, pub source_text: String, #[serde(default)] pub notes: String, pub items: Vec<ProtocolItem> }

/// Saving a schedule always returns it to "pending" review: a schedule is
/// never treated as correct just because it exists.
pub fn save_protocol_tx(conn: &Connection, p: &ProtocolInput) -> Result<i64, String> {
    if p.name.trim().is_empty() { return Err("Name the schedule".into()); }
    check_len(&p.name, 100, "Name")?;
    if crate::flocks::species_group(&p.species).is_none() { return Err("Choose the type of livestock".into()); }
    if p.source_text.trim().chars().count() < 3 { return Err("Say where the schedule comes from (vet, vaccine maker, hatchery, NAERLS…)".into()); }
    check_len(&p.source_text, 300, "Source")?;
    check_len(&p.notes, 1000, "Notes")?;
    if p.items.is_empty() { return Err("Add at least one vaccination".into()); }
    for i in &p.items {
        if i.vaccine_name.trim().is_empty() { return Err("Name every vaccine".into()); }
        if !(0..=3650).contains(&i.day_offset) { return Err("Days must be between 0 and 3650".into()); }
        for (v, n) in [(&i.vaccine_name, 120), (&i.disease, 120), (&i.route, 60), (&i.dose, 120), (&i.notes, 300)] { check_len(v, n, "Field")?; }
    }
    let id = match p.id {
        Some(id) => {
            let n = conn.execute("UPDATE vaccination_protocols SET name=?1,species=?2,source_text=?3,notes=?4,review_status='pending',reviewer='',review_date='' WHERE id=?5",
                params![p.name.trim(), p.species, p.source_text.trim(), p.notes.trim(), id]).map_err(|e| e.to_string())?;
            if n == 0 { return Err("Schedule not found".into()); }
            conn.execute("DELETE FROM vaccination_protocol_items WHERE protocol_id=?1", [id]).map_err(|e| e.to_string())?;
            id
        }
        None => { conn.execute("INSERT INTO vaccination_protocols(name,species,source_text,notes) VALUES(?1,?2,?3,?4)", params![p.name.trim(), p.species, p.source_text.trim(), p.notes.trim()]).map_err(|e| e.to_string())?; conn.last_insert_rowid() }
    };
    for i in &p.items {
        conn.execute("INSERT INTO vaccination_protocol_items(protocol_id,day_offset,vaccine_name,disease,route,dose,notes) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![id, i.day_offset, i.vaccine_name.trim(), i.disease.trim(), i.route.trim(), i.dose.trim(), i.notes.trim()]).map_err(|e| e.to_string())?;
    }
    Ok(id)
}

#[tauri::command]
pub fn save_protocol(state: tauri::State<'_, AppState>, session_token: String, input: ProtocolInput) -> Result<i64, String> {
    let me = auth::require_adviser_or_manager(&state, &session_token)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let id = save_protocol_tx(&tx, &input)?;
    crate::audit::log(&tx, &me.display_name(), "protocol_saved", "vaccination_protocol", Some(id), input.name.trim());
    tx.commit().map_err(|e| e.to_string())?;
    Ok(id)
}

/// Owner or adviser records the veterinary review of a schedule.
#[tauri::command]
pub fn review_protocol(state: tauri::State<'_, AppState>, session_token: String, id: i64, approved: bool, reviewer: String) -> Result<(), String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    if !(me.role == Role::Owner || me.role == Role::Adviser) { return Err("Only the owner or an adviser can record a review".into()); }
    if approved && reviewer.trim().chars().count() < 3 { return Err("Enter the name and qualification of the vet who reviewed it".into()); }
    check_len(&reviewer, 120, "Reviewer")?;
    let conn = state.db()?;
    let n = conn.execute("UPDATE vaccination_protocols SET review_status=?1,reviewer=?2,review_date=?3 WHERE id=?4",
        params![if approved { "approved" } else { "pending" }, if approved { reviewer.trim() } else { "" }, if approved { crate::farms::today_str() } else { String::new() }, id]).map_err(|e| e.to_string())?;
    if n == 0 { return Err("Schedule not found".into()); }
    crate::audit::log(&conn, &me.display_name(), if approved { "protocol_approved" } else { "protocol_unapproved" }, "vaccination_protocol", Some(id), reviewer.trim());
    Ok(())
}

#[tauri::command]
pub fn delete_protocol(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Manager)?;
    state.db()?.execute("DELETE FROM vaccination_protocols WHERE id=?1", [id]).map_err(|e| e.to_string())?;
    Ok(())
}

/// Plans every vaccination of a schedule on a batch, dated from the day the
/// batch was placed. A reviewed schedule gives confirmed plans; anything else
/// is planned as "needs vet review". Vaccinations already planned with the same
/// name and date are not duplicated.
pub fn apply_protocol_tx(conn: &Connection, protocol_id: i64, flock_id: i64, who: &str) -> Result<usize, String> {
    let p = list_protocols_tx(conn)?.into_iter().find(|p| p.id == protocol_id).ok_or("Schedule not found")?;
    let f = crate::flocks::summary(conn, flock_id)?;
    if f.species != p.species { return Err(format!("This schedule is for {}, but {} is {}", p.species, f.code, f.species)); }
    let placed = parse_date(&f.placed_date).ok_or("The batch has no valid placement date")?;
    let (source, by) = if p.review_status == "approved" { ("vet", p.reviewer.clone()) } else { ("pending", String::new()) };
    let mut added = 0;
    for i in &p.items {
        let due = (placed + chrono::Duration::days(i.day_offset)).format("%Y-%m-%d").to_string();
        let exists: Option<i64> = conn.query_row("SELECT id FROM vaccination_events WHERE flock_id=?1 AND due_date=?2 AND lower(vaccine_name)=lower(?3)", params![flock_id, due, i.vaccine_name], |r| r.get(0)).optional().map_err(|e| e.to_string())?;
        if exists.is_some() { continue; }
        conn.execute("INSERT INTO vaccination_events(flock_id,due_date,vaccine_name,disease,route,dose,source,reviewed_by,notes,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![flock_id, due, i.vaccine_name, i.disease, i.route, i.dose, source, by, format!("From schedule: {}", p.name), who]).map_err(|e| e.to_string())?;
        added += 1;
    }
    Ok(added)
}

#[tauri::command]
pub fn apply_protocol(state: tauri::State<'_, AppState>, session_token: String, protocol_id: i64, flock_id: i64) -> Result<usize, String> {
    let me = auth::require_adviser_or_manager(&state, &session_token)?;
    let mut conn = state.db()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let n = apply_protocol_tx(&tx, protocol_id, flock_id, &me.display_name())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn d(s: &str) -> NaiveDate { NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap() }

    #[test]
    fn status_rules() {
        let t = d("2026-05-10");
        assert_eq!(vaccination_status("planned", "vet", "2026-05-20", t), "upcoming");
        assert_eq!(vaccination_status("planned", "vet", "2026-05-13", t), "due");
        assert_eq!(vaccination_status("planned", "label", "2026-05-10", t), "due");
        assert_eq!(vaccination_status("planned", "vet", "2026-05-09", t), "overdue");
        assert_eq!(vaccination_status("planned", "pending", "2026-05-01", t), "needs_review");
        assert_eq!(vaccination_status("completed", "pending", "2026-05-01", t), "completed");
        assert_eq!(vaccination_status("skipped", "vet", "2026-05-01", t), "skipped");
    }

    #[test]
    fn vaccination_lifecycle() {
        let conn = crate::db::test_conn();
        conn.execute("INSERT INTO flocks(code,placed_date,chicks_placed) VALUES('B1','2026-01-01',100)", []).unwrap();
        let base = VaccinationInput { flock_id: 1, due_date: "2026-01-08".into(), vaccine_name: "Example vaccine".into(), source: "vet".into(), ..Default::default() };
        assert!(save_vacc_tx(&conn, &base, "M").is_err(), "a confirmed source needs a name");
        let id = save_vacc_tx(&conn, &VaccinationInput { reviewed_by: "Dr Etim".into(), ..base }, "M").unwrap();
        let pending = save_vacc_tx(&conn, &VaccinationInput { flock_id: 1, due_date: "2026-01-20".into(), vaccine_name: "From a leaflet".into(), source: "pending".into(), ..Default::default() }, "M").unwrap();
        let c = vaccination_counts(&conn).unwrap();
        assert_eq!(c.needs_review, 1);
        complete_vacc_tx(&conn, id, "2026-01-08", "Worker", "LOT9").unwrap();
        assert!(complete_vacc_tx(&conn, id, "2026-01-08", "Worker", "").is_err(), "cannot complete twice");
        let all = list_vacc_tx(&conn, Some(1), false).unwrap();
        assert_eq!(all.iter().find(|v| v.id == id).unwrap().display_status, "completed");
        assert_eq!(all.iter().find(|v| v.id == pending).unwrap().display_status, "needs_review");
    }

    #[test]
    fn health_record_rules() {
        let conn = crate::db::test_conn();
        conn.execute("INSERT INTO flocks(code,placed_date,chicks_placed) VALUES('B1','2026-01-01',100)", []).unwrap();
        let mut h = HealthInput { flock_id: 1, observed_date: "2026-01-05".into(), issue: "Birds huddling".into(), affected: 10, ..Default::default() };
        save_health_tx(&conn, &h, "W").unwrap();
        h.dosage_as_prescribed = "as written".into();
        assert!(save_health_tx(&conn, &h, "W").is_err(), "no dose without a product or prescriber");
        h.vet_consulted = "Dr Etim".into();
        h.treatment = "Product X".into();
        h.end_date = "2026-01-09".into();
        h.start_date = "2026-01-05".into();
        h.withdrawal_days = 5;
        save_health_tx(&conn, &h, "W").unwrap();
        let list = list_health_tx(&conn, Some(1)).unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|r| r.withdrawal_until == "2026-01-14"));
    }

    #[test]
    fn protocols_plan_from_placement_and_respect_review() {
        let conn = crate::db::test_conn();
        conn.execute("INSERT INTO flocks(code,placed_date,chicks_placed,species) VALUES('B1','2026-01-01',100,'broiler')", []).unwrap();
        let p = ProtocolInput { id: None, name: "Farm schedule".into(), species: "broiler".into(), source_text: "Hatchery leaflet".into(), notes: String::new(),
            items: vec![ProtocolItem { day_offset: 7, vaccine_name: "Vaccine A".into(), ..Default::default() }, ProtocolItem { day_offset: 14, vaccine_name: "Vaccine B".into(), ..Default::default() }] };
        let id = save_protocol_tx(&conn, &p).unwrap();
        assert_eq!(apply_protocol_tx(&conn, id, 1, "M").unwrap(), 2);
        assert_eq!(apply_protocol_tx(&conn, id, 1, "M").unwrap(), 0, "not duplicated");
        let v = list_vacc_tx(&conn, Some(1), false).unwrap();
        assert_eq!(v[0].due_date, "2026-01-08");
        assert!(v.iter().all(|x| x.display_status == "needs_review"));
        conn.execute("UPDATE vaccination_protocols SET review_status='approved',reviewer='Dr Etim' WHERE id=?1", [id]).unwrap();
        conn.execute("INSERT INTO flocks(code,placed_date,chicks_placed,species) VALUES('B2','2026-01-01',100,'broiler')", []).unwrap();
        apply_protocol_tx(&conn, id, 2, "M").unwrap();
        assert!(list_vacc_tx(&conn, Some(2), false).unwrap().iter().all(|x| x.source == "vet" && x.reviewed_by == "Dr Etim"));
        // Editing a reviewed schedule sends it back for review.
        save_protocol_tx(&conn, &ProtocolInput { id: Some(id), ..p }).unwrap();
        assert_eq!(list_protocols_tx(&conn).unwrap()[0].review_status, "pending");
    }
}
