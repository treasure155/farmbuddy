//! Knowledge content packages. The FarmBuddy content team publishes updates
//! (articles, sources, vaccination schedule templates, feed guides and seasonal
//! outlooks) as one JSON file that the owner imports from Settings, without
//! reinstalling the app.
//!
//! File format:
//! ```json
//! { "payload": "<the package JSON as a string>", "signature": "<base64 Ed25519 signature of payload>" }
//! ```
//! An unsigned package may also give `payload` as a plain object. When the
//! owner switches on "Only accept signed content", unsigned or wrongly signed
//! files are rejected. Rules that protect the farmer's own records:
//! - the version must be newer than the last import of the same package;
//! - articles update only package-managed copies, never ones written on the farm;
//! - schedules, guides and outlooks are added, never written over;
//! - unsigned content always arrives as "pending review".

use crate::{audit, auth::{self, Role}, db, farms::{rows, valid_date}, AppState};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

const PUBLIC_KEY_B64: &str = include_str!("../content_public_key.txt");
const MAX_BYTES: usize = 5 * 1024 * 1024;

fn public_key() -> Option<VerifyingKey> {
    use base64::Engine;
    let raw = PUBLIC_KEY_B64.trim();
    if raw.is_empty() { return None; }
    let bytes = base64::engine::general_purpose::STANDARD.decode(raw).ok()?;
    VerifyingKey::from_bytes(&bytes.try_into().ok()?).ok()
}

#[derive(Deserialize)]
struct Envelope { payload: serde_json::Value, #[serde(default)] signature: String }

#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Package {
    pub package_id: String,
    pub version: i64,
    pub title: String,
    pub published_date: String,
    pub sources: Vec<SourceIn>,
    pub articles: Vec<ArticleIn>,
    pub protocols: Vec<ProtocolIn>,
    pub feed_guides: Vec<GuideIn>,
    pub outlooks: Vec<OutlookIn>,
}
#[derive(Deserialize, Default)] #[serde(default)]
pub struct SourceIn { name: String, publisher: String, url: String }
#[derive(Deserialize, Default)] #[serde(default)]
pub struct ArticleIn { slug: String, language: String, title: String, category: String, applies_to: String, summary: String, body: String, source: String, source_ref: String, author: String, reviewer: String, review_date: String, review_status: String, safety_level: String, version: i64 }
#[derive(Deserialize, Default)] #[serde(default)]
pub struct ProtocolIn { name: String, species: String, source_text: String, reviewer: String, review_date: String, review_status: String, items: Vec<ProtocolItemIn> }
#[derive(Deserialize, Default)] #[serde(default)]
pub struct ProtocolItemIn { day_offset: i64, vaccine_name: String, disease: String, route: String, dose: String, notes: String }
#[derive(Deserialize, Default)] #[serde(default)]
pub struct GuideIn { species: String, age_from_day: i64, age_to_day: i64, grams_per_animal_day: f64, source: String, reviewer: String, review_status: String, notes: String }
#[derive(Deserialize, Default)] #[serde(default)]
pub struct OutlookIn { year: i64, state: String, lga: String, onset_date: String, cessation_date: String, season_days: Option<i64>, dry_spell_notes: String, notes: String, source: String, published_date: String }

#[derive(Serialize, Debug, Default, PartialEq)]
pub struct ImportResult { pub package_id: String, pub version: i64, pub signed: bool, pub sources: usize, pub articles_added: usize, pub articles_updated: usize, pub articles_skipped: usize, pub protocols: usize, pub feed_guides: usize, pub outlooks: usize }

/// Checks the file and its signature. Returns the package and whether it was signed.
pub fn parse(text: &str, require_signature: bool, key: Option<&VerifyingKey>) -> Result<(Package, bool), String> {
    if text.len() > MAX_BYTES { return Err("This file is too large to be a content package".into()); }
    let env: Envelope = serde_json::from_str(text).map_err(|_| "This is not a FarmBuddy content package (the file is not valid JSON in the expected shape)".to_string())?;
    let (payload, signed) = match &env.payload {
        serde_json::Value::String(p) if !env.signature.trim().is_empty() => {
            use base64::Engine;
            let key = key.ok_or("This app has no content verification key, so signed packages cannot be checked")?;
            let sig = base64::engine::general_purpose::STANDARD.decode(env.signature.trim()).map_err(|_| "The package signature is not readable")?;
            let sig = Signature::from_slice(&sig).map_err(|_| "The package signature is not readable")?;
            key.verify(p.as_bytes(), &sig).map_err(|_| "The package signature does not match. The file may have been changed after it was published.")?;
            (p.clone(), true)
        }
        serde_json::Value::String(p) => (p.clone(), false),
        v @ serde_json::Value::Object(_) => (v.to_string(), false),
        _ => return Err("The package has no content".into()),
    };
    if require_signature && !signed { return Err("Only signed content packages are accepted on this device (Settings → Content).".into()); }
    let pkg: Package = serde_json::from_str(&payload).map_err(|e| format!("The package content is malformed: {e}"))?;
    if pkg.package_id.trim().is_empty() || pkg.version < 1 { return Err("The package has no id or version".into()); }
    if !pkg.published_date.is_empty() && !valid_date(&pkg.published_date) { return Err("The package published date is not a valid date".into()); }
    for a in &pkg.articles {
        if a.slug.trim().is_empty() || a.title.trim().is_empty() || a.body.trim().is_empty() { return Err(format!("Article \"{}\" is missing its slug, title or text", a.slug)); }
        if a.source.trim().is_empty() { return Err(format!("Article \"{}\" has no source", a.slug)); }
    }
    for p in &pkg.protocols { if p.name.trim().is_empty() || p.species.trim().is_empty() || p.source_text.trim().is_empty() || p.items.is_empty() { return Err(format!("Schedule \"{}\" needs a species, a source and at least one vaccination", p.name)); } }
    for g in &pkg.feed_guides { if g.source.trim().is_empty() || g.grams_per_animal_day <= 0.0 || g.age_to_day < g.age_from_day { return Err(format!("A feed guide for {} is incomplete", g.species)); } }
    for o in &pkg.outlooks { if o.source.trim().is_empty() || o.state.trim().is_empty() || o.year < 2000 { return Err("A seasonal outlook is missing its year, state or source".into()); } }
    Ok((pkg, signed))
}

pub fn import_tx(conn: &Connection, pkg: &Package, signed: bool, user: &str) -> Result<ImportResult, String> {
    let last: Option<i64> = conn.query_row("SELECT MAX(version) FROM content_versions WHERE package_id=?1", [&pkg.package_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    if let Some(v) = last { if pkg.version <= v { return Err(format!("Version {} of this package (or a newer one, {v}) is already installed", pkg.version)); } }
    let status_for = |s: &str, approved: &str, pending: &str| -> String { if signed && s == "approved" { approved.into() } else { pending.into() } };
    let mut r = ImportResult { package_id: pkg.package_id.clone(), version: pkg.version, signed, ..Default::default() };
    for s in &pkg.sources {
        if s.name.trim().is_empty() { continue; }
        conn.execute("INSERT INTO knowledge_sources(name,publisher,url) VALUES(?1,?2,?3) ON CONFLICT(name) DO UPDATE SET publisher=excluded.publisher,url=excluded.url", params![s.name.trim(), s.publisher.trim(), s.url.trim()]).map_err(|e| e.to_string())?;
        r.sources += 1;
    }
    for a in &pkg.articles {
        conn.execute("INSERT OR IGNORE INTO knowledge_sources(name) VALUES(?1)", [a.source.trim()]).map_err(|e| e.to_string())?;
        let source_id: i64 = conn.query_row("SELECT id FROM knowledge_sources WHERE name=?1", [a.source.trim()], |x| x.get(0)).map_err(|e| e.to_string())?;
        let lang = if a.language == "ib" { "ib" } else { "en" };
        let safety = if ["general", "record_keeping", "planning", "professional_confirmation", "vet_only", "chemical_label"].contains(&a.safety_level.as_str()) { a.safety_level.as_str() } else { "general" };
        let status = status_for(&a.review_status, "approved", "pending_review");
        let (reviewer, rdate) = if status == "approved" { (a.reviewer.trim(), a.review_date.trim()) } else { ("", "") };
        let version = a.version.max(1);
        let existing: Option<(i64, i64)> = conn.query_row("SELECT is_builtin,version FROM knowledge_articles WHERE slug=?1", [a.slug.trim()], |x| Ok((x.get(0)?, x.get(1)?))).optional().map_err(|e| e.to_string())?;
        match existing {
            None => {
                conn.execute("INSERT INTO knowledge_articles(slug,language,title,category,applies_to,summary,body,source_id,source_ref,author,review_status,reviewer,review_date,version,safety_level,is_builtin) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,1)",
                    params![a.slug.trim(), lang, a.title.trim(), a.category.trim(), a.applies_to.trim(), a.summary.trim(), a.body.trim(), source_id, a.source_ref.trim(), a.author.trim(), status, reviewer, rdate, version, safety]).map_err(|e| e.to_string())?;
                r.articles_added += 1;
            }
            Some((1, v)) if version > v => {
                conn.execute("UPDATE knowledge_articles SET language=?2,title=?3,category=?4,applies_to=?5,summary=?6,body=?7,source_id=?8,source_ref=?9,author=?10,review_status=?11,reviewer=?12,review_date=?13,version=?14,safety_level=?15,updated_at=CURRENT_TIMESTAMP WHERE slug=?1",
                    params![a.slug.trim(), lang, a.title.trim(), a.category.trim(), a.applies_to.trim(), a.summary.trim(), a.body.trim(), source_id, a.source_ref.trim(), a.author.trim(), status, reviewer, rdate, version, safety]).map_err(|e| e.to_string())?;
                r.articles_updated += 1;
            }
            // Written on this farm, or already at this version: left alone.
            Some(_) => r.articles_skipped += 1,
        }
    }
    for p in &pkg.protocols {
        let exists: i64 = conn.query_row("SELECT COUNT(*) FROM vaccination_protocols WHERE name=?1 AND species=?2", params![p.name.trim(), p.species.trim()], |x| x.get(0)).map_err(|e| e.to_string())?;
        if exists > 0 { continue; }
        let status = status_for(&p.review_status, "approved", "pending");
        conn.execute("INSERT INTO vaccination_protocols(name,species,source_text,review_status,reviewer,review_date,notes) VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![p.name.trim(), p.species.trim(), p.source_text.trim(), status, if status == "approved" { p.reviewer.trim() } else { "" }, if status == "approved" { p.review_date.trim() } else { "" }, format!("Imported from {} v{}", pkg.package_id, pkg.version)]).map_err(|e| e.to_string())?;
        let pid = conn.last_insert_rowid();
        for i in &p.items {
            if i.vaccine_name.trim().is_empty() || i.day_offset < 0 { return Err(format!("Schedule \"{}\" has an incomplete vaccination", p.name)); }
            conn.execute("INSERT INTO vaccination_protocol_items(protocol_id,day_offset,vaccine_name,disease,route,dose,notes) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![pid, i.day_offset, i.vaccine_name.trim(), i.disease.trim(), i.route.trim(), i.dose.trim(), i.notes.trim()]).map_err(|e| e.to_string())?;
        }
        r.protocols += 1;
    }
    for g in &pkg.feed_guides {
        let exists: i64 = conn.query_row("SELECT COUNT(*) FROM feed_guides WHERE species=?1 AND age_from_day=?2 AND age_to_day=?3 AND source=?4", params![g.species, g.age_from_day, g.age_to_day, g.source], |x| x.get(0)).map_err(|e| e.to_string())?;
        if exists > 0 { continue; }
        let status = status_for(&g.review_status, "approved", "pending");
        conn.execute("INSERT INTO feed_guides(species,age_from_day,age_to_day,grams_per_animal_day,source,review_status,reviewer,notes) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![g.species.trim(), g.age_from_day, g.age_to_day, g.grams_per_animal_day, g.source.trim(), status, if status == "approved" { g.reviewer.trim() } else { "" }, g.notes.trim()]).map_err(|e| e.to_string())?;
        r.feed_guides += 1;
    }
    for o in &pkg.outlooks {
        let n = conn.execute("INSERT OR IGNORE INTO climate_outlooks(year,state,lga,onset_date,cessation_date,season_days,dry_spell_notes,notes,source,published_date) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![o.year, o.state.trim(), o.lga.trim(), o.onset_date.trim(), o.cessation_date.trim(), o.season_days, o.dry_spell_notes.trim(), o.notes.trim(), o.source.trim(), o.published_date.trim()]).map_err(|e| e.to_string())?;
        r.outlooks += n;
    }
    let summary = format!("{} sources, {} new and {} updated articles ({} kept as they are), {} schedules, {} feed guides, {} outlooks", r.sources, r.articles_added, r.articles_updated, r.articles_skipped, r.protocols, r.feed_guides, r.outlooks);
    conn.execute("INSERT INTO content_versions(package_id,version,title,published_date,signed,summary,user_name) VALUES(?1,?2,?3,?4,?5,?6,?7)",
        params![pkg.package_id, pkg.version, pkg.title, pkg.published_date, signed as i64, summary, user]).map_err(|e| e.to_string())?;
    db::set_setting(conn, "last_content_update", &format!("{} v{} ({})", pkg.package_id, pkg.version, crate::farms::today_str()))?;
    audit::log(conn, user, "content_imported", "content", None, &format!("{} v{}{}: {summary}", pkg.package_id, pkg.version, if signed { " signed" } else { " unsigned" }));
    Ok(r)
}

#[tauri::command]
pub fn import_content_package(state: tauri::State<'_, AppState>, session_token: String, path: String) -> Result<ImportResult, String> {
    let me = auth::require(&state, &session_token, Role::Owner)?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Could not read the file: {e}"))?;
    let mut conn = state.db()?;
    let require = db::setting_bool(&conn, "content_require_signature")?;
    let (pkg, signed) = parse(&text, require, public_key().as_ref())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let r = import_tx(&tx, &pkg, signed, &me.display_name())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(r)
}

#[derive(Serialize)]
pub struct ContentVersion { id: i64, package_id: String, version: i64, title: String, published_date: String, signed: bool, summary: String, imported_at: String, user_name: String }

#[tauri::command]
pub fn list_content_versions(state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<ContentVersion>, String> {
    auth::require(&state, &session_token, Role::Manager)?;
    rows(&*state.db()?, "SELECT id,package_id,version,title,published_date,signed,summary,imported_at,user_name FROM content_versions ORDER BY id DESC", [], |r| Ok(ContentVersion {
        id: r.get(0)?, package_id: r.get(1)?, version: r.get(2)?, title: r.get(3)?, published_date: r.get(4)?, signed: r.get::<_, i64>(5)? == 1, summary: r.get(6)?, imported_at: r.get(7)?, user_name: r.get(8)?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use serde_json::json;

    fn pkg(version: i64) -> String {
        json!({ "package_id": "fb-core", "version": version, "title": "Core", "published_date": "2026-10-01",
            "sources": [{ "name": "Test source", "publisher": "Test" }],
            "articles": [{ "slug": "new-article", "title": "New", "category": "Poultry", "body": "Body", "source": "Test source", "review_status": "approved", "reviewer": "Dr A", "version": version },
                         { "slug": "farm-own", "title": "Edited", "category": "Poultry", "body": "Changed", "source": "Test source", "version": 9 }],
            "protocols": [{ "name": "Broiler schedule", "species": "broiler", "source_text": "Test", "items": [{ "day_offset": 7, "vaccine_name": "Vaccine A" }] }],
            "feed_guides": [{ "species": "broiler", "age_from_day": 0, "age_to_day": 7, "grams_per_animal_day": 20.0, "source": "Bag label" }],
            "outlooks": [{ "year": 2026, "state": "Akwa Ibom", "source": "NiMet SCP 2026" }] }).to_string()
    }

    #[test]
    fn signed_import_respects_farm_records_and_versions() {
        let conn = crate::db::test_conn();
        conn.execute("INSERT INTO knowledge_articles(slug,title,category,body,is_builtin) VALUES('farm-own','Mine','Poultry','My text',0)", []).unwrap();
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let vk = sk.verifying_key();
        let p = pkg(1);
        use base64::Engine;
        let sig = base64::engine::general_purpose::STANDARD.encode(sk.sign(p.as_bytes()).to_bytes());
        let file = json!({ "payload": p, "signature": sig }).to_string();
        let (pk, signed) = parse(&file, true, Some(&vk)).unwrap();
        assert!(signed);
        let r = import_tx(&conn, &pk, signed, "Owner").unwrap();
        assert_eq!((r.articles_added, r.articles_skipped, r.protocols, r.feed_guides, r.outlooks), (1, 1, 1, 1, 1));
        let own: String = conn.query_row("SELECT body FROM knowledge_articles WHERE slug='farm-own'", [], |x| x.get(0)).unwrap();
        assert_eq!(own, "My text");
        let st: String = conn.query_row("SELECT review_status FROM knowledge_articles WHERE slug='new-article'", [], |x| x.get(0)).unwrap();
        assert_eq!(st, "approved");
        assert!(import_tx(&conn, &pk, signed, "Owner").is_err(), "same version twice");
        // Tampered payload fails; unsigned rejected when required; unsigned arrives pending.
        let bad = json!({ "payload": p.replace("Body", "Other"), "signature": sig }).to_string();
        assert!(parse(&bad, false, Some(&vk)).is_err());
        let plain = json!({ "payload": serde_json::from_str::<serde_json::Value>(&pkg(2)).unwrap() }).to_string();
        assert!(parse(&plain, true, Some(&vk)).is_err());
        let (p2, s2) = parse(&plain, false, Some(&vk)).unwrap();
        import_tx(&conn, &p2, s2, "Owner").unwrap();
        let st: String = conn.query_row("SELECT review_status FROM knowledge_articles WHERE slug='new-article'", [], |x| x.get(0)).unwrap();
        assert_eq!(st, "pending_review");
        assert!(parse("not json", false, None).is_err());
    }

    #[test]
    fn shipped_public_key_is_valid() { assert!(public_key().is_some()); }

    /// `FB_PKG=path cargo test --lib verify_package_file -- --ignored`: checks a signed file against the shipped key.
    #[test]
    #[ignore]
    fn verify_package_file() {
        let text = std::fs::read_to_string(std::env::var("FB_PKG").unwrap()).unwrap();
        let (p, signed) = parse(&text, true, public_key().as_ref()).unwrap();
        assert!(signed);
        let conn = crate::db::test_conn();
        let r = import_tx(&conn, &p, signed, "Test").unwrap();
        println!("{:?}", r);
    }

}
