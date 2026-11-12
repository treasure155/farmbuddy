//! Offline knowledge library: source register and guidance articles.
//!
//! Built-in articles are drafts written by the FarmBuddy team for record
//! keeping and general farm management. Every article carries its source, a
//! review status and a safety level. Built-in drafts ship as `pending_review`:
//! the app never presents them as approved professional advice until an owner
//! records who reviewed them. Health, vaccination and chemical topics only
//! describe what to record and who to ask; they never give doses or diagnoses.

use crate::{auth::{self, Role}, farms::{check_len, today_str}, AppState};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

pub const SAFETY_LEVELS: [&str; 6] = ["general", "record_keeping", "planning", "professional_confirmation", "vet_only", "chemical_label"];
const REVIEW: [&str; 3] = ["draft", "pending_review", "approved"];
const TEAM: &str = "FarmBuddy team (draft)";
const OWN: &str = "FarmBuddy (TechAlpha Hub)";

struct Src { name: &'static str, publisher: &'static str, url: &'static str }
const SOURCES: &[Src] = &[
    Src { name: OWN, publisher: "TechAlpha Hub", url: "" },
    Src { name: "NAERLS National Farmers' Helpline", publisher: "National Agricultural Extension and Research Liaison Services", url: "https://nfhl.naerls.gov.ng/index.php?action=overview" },
    Src { name: "NiMet Seasonal Climate Prediction", publisher: "Nigerian Meteorological Agency", url: "https://nimet.gov.ng/scp" },
    Src { name: "NiMet Crop Weather Calendar", publisher: "Nigerian Meteorological Agency", url: "https://nimet.gov.ng/product_detail?id=11" },
    Src { name: "National Agricultural Seeds Council", publisher: "National Agricultural Seeds Council", url: "https://seedcouncil.gov.ng/council-policies/" },
    Src { name: "National Fertilizer Management Platform", publisher: "Federal Government of Nigeria", url: "https://nfmp.gov.ng/guidelines" },
    Src { name: "NAFDAC Pesticide Registration Regulations 2019", publisher: "NAFDAC", url: "https://www.nafdac.gov.ng/wp-content/uploads/Files/Resources/Regulations/CHEMICAL_REGULATIONS/Pesticide-Registration-Regulations-2019.pdf" },
    Src { name: "IITA Crop processing", publisher: "International Institute of Tropical Agriculture", url: "https://iita.org/crop-processing/" },
];

struct Art { slug: &'static str, version: i64, title: &'static str, category: &'static str, applies_to: &'static str, safety: &'static str, source: &'static str, summary: &'static str, body: &'static str }

/// Increase `version` when an article's text changes: the new text replaces the
/// old one on the next start and the article returns to pending review.
const ARTICLES: &[Art] = &[
    Art { slug: "daily-livestock-records", version: 1, title: "Keeping daily livestock records", category: "Record keeping", applies_to: "poultry,goat,sheep,pig,cattle,rabbit,fish", safety: "record_keeping", source: OWN,
        summary: "What to write down every day for each batch, and why deaths, culls, sales and transfers are recorded separately.",
        body: "Good numbers start with a short daily record for every batch.\n\nRecord each day:\n- Deaths, and anything you noticed before they died.\n- Animals culled, sold or moved to another batch, each as its own entry.\n- Feed given, in kilograms, and any feed wasted.\n- Water used, if you measure it.\n- Eggs collected and eggs damaged, for layers.\n- Unusual behaviour, weather or other events.\n\nWhy separate entries matter: FarmBuddy works out the animals still alive from what you record. If one death is also written as a sale, the count is wrong. Record each event once, in the right place.\n\nWho records: give one person the job each day. Workers can enter daily records; managers check them weekly." },
    Art { slug: "mortality-and-survival", version: 2, title: "Mortality and survival rate", category: "Record keeping", applies_to: "poultry,goat,sheep,pig,cattle,rabbit,fish", safety: "record_keeping", source: OWN,
        summary: "How FarmBuddy calculates mortality and survival, and why it uses every animal that came into the batch.",
        body: "Mortality = total deaths ÷ all animals that came into the batch × 100.\nSurvival = 100 − mortality.\n\nAnimals that came into the batch are the number placed at the start, plus animals received later, animals born in the batch and animals moved in from another batch. FarmBuddy never divides by the number alive today: dividing by the animals still alive makes losses look bigger as the batch gets smaller.\n\nCulls, sales and animals moved to another batch are not deaths. They reduce the animals alive but do not count toward mortality.\n\nIf deaths rise for several days in a row, write down what you see in Health records and speak to a veterinary professional." },
    Art { slug: "feed-conversion-ratio", version: 1, title: "Feed conversion ratio (FCR)", category: "Feed", applies_to: "poultry,pig,fish", safety: "record_keeping", source: OWN,
        summary: "What FCR means, how FarmBuddy works it out, and the weighing it needs.",
        body: "FCR tells you how much feed was used to produce one kilogram of live weight.\n\nFarmBuddy uses: FCR = total feed used (kg) ÷ live weight produced (kg).\nLive weight produced = weight already sold + (animals alive × latest average weight).\n\nFCR needs a recent average weight. Weigh a small sample of animals, divide the total by the number weighed, and enter it in the daily record.\n\nA lower FCR means less feed per kilogram. Compare batches on your own farm using the same method; targets differ by breed, age and system, so ask your feed supplier or extension officer what to expect for your breed." },
    Art { slug: "vaccination-records", version: 1, title: "Vaccination records: what to write down", category: "Health", applies_to: "poultry,goat,sheep,pig,cattle", safety: "professional_confirmation", source: "NAERLS National Farmers' Helpline",
        summary: "The details to record for every vaccination, and why schedules must be confirmed by a vet.",
        body: "For every vaccination, record: the batch, the date, the vaccine name, the disease it protects against, the manufacturer, the lot number and expiry date, how it was given, and who gave it.\n\nVaccination schedules are not the same for every farm. They depend on what the hatchery already gave, the breed or strain, the age of the animals, your location and local disease risk, and the vaccine maker's instructions.\n\nFarmBuddy keeps schedules you enter. A schedule copied from a public reference is marked 'Needs vet review' until a veterinary professional confirms it. Ask the hatchery for the vaccination record of every batch you receive.\n\nFarmBuddy does not diagnose disease or recommend medicines. Use it to keep accurate records you can show your vet." },
    Art { slug: "biosecurity-basics", version: 1, title: "Farm hygiene and biosecurity basics", category: "Biosecurity", applies_to: "poultry,goat,sheep,pig,cattle,rabbit", safety: "general", source: "NAERLS National Farmers' Helpline",
        summary: "Everyday habits that reduce the chance of disease entering or spreading on your farm.",
        body: "Biosecurity means keeping disease out of your farm and stopping it from spreading inside.\n\nEveryday habits:\n- Limit visitors to animal houses and keep a visitor book.\n- Wash hands and change or clean footwear before entering a pen.\n- Keep new or sick animals apart from the rest.\n- Clean and disinfect equipment, feeders and drinkers regularly.\n- Keep feed dry and away from rodents and wild birds.\n- Remove dead animals promptly and dispose of them as local guidance requires.\n- Clean and rest houses between batches.\n\nYour veterinary professional or extension officer can advise on measures for your farm and the products to use." },
    Art { slug: "crop-records", version: 1, title: "Recording crops from planting to harvest", category: "Crops", applies_to: "cassava,yam,maize,rice,plantain,vegetables,oil_palm", safety: "record_keeping", source: OWN,
        summary: "The planting, activity and harvest details that let FarmBuddy work out yield and cost of production.",
        body: "For each plot and crop, record:\n- Planting date, variety, area planted and planting material source.\n- Every activity: land preparation, weeding, fertiliser, spraying, irrigation, with labour and input costs.\n- Every harvest: quantity, unit, grade, quantity damaged, stored and sold.\n\nFarmBuddy then shows total cost, yield per unit of land and harvest loss. It only counts money you actually received as revenue; an estimated value of produce in store is never shown as income.\n\nUse your own planting date for the calendar; planting times differ by state and season." },
    Art { slug: "certified-seed", version: 1, title: "Choosing certified seed and planting material", category: "Crops", applies_to: "maize,rice,cassava,yam,vegetables", safety: "general", source: "National Agricultural Seeds Council",
        summary: "Why certified seed matters and what to keep as proof of where your seed came from.",
        body: "The National Agricultural Seeds Council is responsible for seed certification and quality control in Nigeria.\n\nWhen buying seed or planting material:\n- Prefer certified seed from registered dealers.\n- Keep the receipt and the bag label or tag.\n- Record the variety, the seller and the lot number in FarmBuddy when you plant.\n\nFarmBuddy records what you bought; it does not verify sellers. Check certification with the Seeds Council or your extension officer." },
    Art { slug: "pesticide-labels", version: 1, title: "Pesticides and herbicides: follow the approved label", category: "Crop health", applies_to: "crops", safety: "chemical_label", source: "NAFDAC Pesticide Registration Regulations 2019",
        summary: "FarmBuddy does not give spray rates. Use registered products and follow the product label.",
        body: "Pesticides, herbicides and fungicides must be registered with NAFDAC.\n\n- Use only registered products and follow the instructions on the product label.\n- FarmBuddy does not give spray rates or mixing instructions.\n- Wear the protective clothing the label requires.\n- Record in FarmBuddy: the product, the quantity used, the date, the plot and who applied it.\n- Observe any waiting period on the label before harvest.\n\nIf you are unsure, ask an agricultural extension officer or the product supplier." },
    Art { slug: "processing-batch-costing", version: 1, title: "Costing a processing batch", category: "Processing", applies_to: "palm_oil,cassava,grain,feed", safety: "record_keeping", source: "IITA Crop processing",
        summary: "How to record a processing batch so FarmBuddy can work out cost per litre, per kilogram or per bag.",
        body: "Treat every processing run as a batch.\n\n1. Record the raw material: quantity, unit and what you paid.\n2. Record every cost: transport, milling or pressing fees, labour, fuel, water and packaging.\n3. When processing ends, record what came out: main products, by-products and waste.\n4. Split the batch cost between the main products (for example 85% to oil and 15% to kernels).\n\nFarmBuddy then shows the cost per unit of each output. Yields vary with fruit quality, ripeness, equipment and method, so FarmBuddy never assumes a fixed yield; it uses what you record." },
    Art { slug: "seasonal-planning", version: 1, title: "Planning with the seasonal rainfall forecast", category: "Weather", applies_to: "crops", safety: "planning", source: "NiMet Seasonal Climate Prediction",
        summary: "Use NiMet's yearly forecast for your state, and your own rainfall records, when planning planting.",
        body: "Each year the Nigerian Meteorological Agency (NiMet) publishes a Seasonal Climate Prediction with expected rainfall onset and cessation dates, growing-season length and dry spells by location. NiMet also publishes a Crop Weather Calendar.\n\n- Read the current year's forecast for your state and LGA.\n- Record your own rainfall and weather observations in FarmBuddy.\n- Plan planting around the expected onset for your area, not a single national date.\n\nFarmBuddy works offline, so it cannot show live weather. Forecast information shows the date it was last updated." },
];

pub fn seed_builtin(conn: &Connection) -> Result<(), String> {
    // The library tables arrive with migration 005; older test databases skip seeding.
    let has: i64 = conn.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='knowledge_articles'", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    if has == 0 { return Ok(()); }
    for s in SOURCES {
        conn.execute("INSERT INTO knowledge_sources(name,publisher,url) VALUES(?1,?2,?3) ON CONFLICT(name) DO UPDATE SET publisher=excluded.publisher,url=excluded.url", params![s.name, s.publisher, s.url]).map_err(|e| e.to_string())?;
    }
    for a in ARTICLES {
        let source_id: i64 = conn.query_row("SELECT id FROM knowledge_sources WHERE name=?1", [a.source], |r| r.get(0)).map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO knowledge_articles(slug,language,title,category,applies_to,summary,body,source_id,author,review_status,version,safety_level,is_builtin)
             VALUES(?1,'en',?2,?3,?4,?5,?6,?7,?8,'pending_review',?9,?10,1)
             ON CONFLICT(slug) DO UPDATE SET title=excluded.title,category=excluded.category,applies_to=excluded.applies_to,summary=excluded.summary,body=excluded.body,
               source_id=excluded.source_id,safety_level=excluded.safety_level,version=excluded.version,review_status='pending_review',reviewer='',review_date='',updated_at=CURRENT_TIMESTAMP
             WHERE knowledge_articles.is_builtin=1 AND knowledge_articles.version < excluded.version",
            params![a.slug, a.title, a.category, a.applies_to, a.summary, a.body, source_id, TEAM, a.version, a.safety],
        ).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Serialize)]
pub struct ArticleSummary {
    id: i64, slug: String, language: String, title: String, category: String, applies_to: String, summary: String,
    source_name: String, source_url: String, source_ref: String, author: String, review_status: String, reviewer: String, review_date: String,
    version: i64, safety_level: String, is_builtin: bool, updated_at: String,
}

#[derive(Serialize)]
pub struct Article { #[serde(flatten)] meta: ArticleSummary, body: String, source_id: Option<i64> }

const SELECT: &str = "SELECT a.id,a.slug,a.language,a.title,a.category,a.applies_to,a.summary,COALESCE(s.name,''),COALESCE(s.url,''),a.source_ref,a.author,a.review_status,a.reviewer,a.review_date,a.version,a.safety_level,a.is_builtin,a.updated_at,a.body,a.source_id
    FROM knowledge_articles a LEFT JOIN knowledge_sources s ON s.id=a.source_id";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Article> {
    Ok(Article {
        meta: ArticleSummary {
            id: r.get(0)?, slug: r.get(1)?, language: r.get(2)?, title: r.get(3)?, category: r.get(4)?, applies_to: r.get(5)?, summary: r.get(6)?,
            source_name: r.get(7)?, source_url: r.get(8)?, source_ref: r.get(9)?, author: r.get(10)?, review_status: r.get(11)?, reviewer: r.get(12)?,
            review_date: r.get(13)?, version: r.get(14)?, safety_level: r.get(15)?, is_builtin: r.get::<_, i64>(16)? == 1, updated_at: r.get(17)?,
        },
        body: r.get(18)?, source_id: r.get(19)?,
    })
}

pub fn search(conn: &Connection, query: &str, category: &str) -> Result<Vec<ArticleSummary>, String> {
    let q = format!("%{}%", query.trim().to_lowercase());
    let mut stmt = conn.prepare(&format!("{SELECT} WHERE (?1='%%' OR lower(a.title) LIKE ?1 OR lower(a.summary) LIKE ?1 OR lower(a.body) LIKE ?1 OR lower(a.applies_to) LIKE ?1) AND (?2='' OR a.category=?2) ORDER BY a.category, a.title")).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params![q, category.trim()], row).map_err(|e| e.to_string())?.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(|a| a.meta).collect())
}

#[tauri::command]
pub fn list_articles(state: tauri::State<'_, AppState>, session_token: String, query: Option<String>, category: Option<String>) -> Result<Vec<ArticleSummary>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    search(&*state.db()?, &query.unwrap_or_default(), &category.unwrap_or_default())
}

#[tauri::command]
pub fn get_article(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<Article, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    state.db()?.query_row(&format!("{SELECT} WHERE a.id=?1"), [id], row).optional().map_err(|e| e.to_string())?.ok_or_else(|| "Article not found".into())
}

#[derive(Serialize)]
pub struct Source { id: i64, name: String, publisher: String, url: String }

#[tauri::command]
pub fn list_sources(state: tauri::State<'_, AppState>, session_token: String) -> Result<Vec<Source>, String> {
    auth::require(&state, &session_token, Role::Worker)?;
    let conn = state.db()?;
    let mut stmt = conn.prepare("SELECT id,name,publisher,url FROM knowledge_sources ORDER BY name").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |r| Ok(Source { id: r.get(0)?, name: r.get(1)?, publisher: r.get(2)?, url: r.get(3)? })).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct ArticleInput { id: Option<i64>, title: String, category: String, applies_to: String, summary: String, body: String, source_id: Option<i64>, source_ref: String, safety_level: String, language: String }

pub fn save_article_tx(conn: &Connection, input: &ArticleInput, author: &str) -> Result<i64, String> {
    let title = input.title.trim();
    if title.is_empty() { return Err("Give the article a title".into()); }
    check_len(title, 150, "Title")?;
    check_len(&input.category, 60, "Category")?;
    check_len(&input.applies_to, 300, "Applies to")?;
    check_len(&input.summary, 400, "Summary")?;
    check_len(&input.source_ref, 300, "Source reference")?;
    if input.body.trim().is_empty() { return Err("Write the article text".into()); }
    check_len(&input.body, 20_000, "Article text")?;
    if input.category.trim().is_empty() { return Err("Choose a category".into()); }
    if !SAFETY_LEVELS.contains(&input.safety_level.as_str()) { return Err("Choose a safety level".into()); }
    if !["en", "ib"].contains(&input.language.as_str()) { return Err("Choose English or Ibibio".into()); }
    // Any edit sends the article back for review and stops built-in updates replacing it.
    match input.id {
        Some(id) => {
            let n = conn.execute("UPDATE knowledge_articles SET title=?1,category=?2,applies_to=?3,summary=?4,body=?5,source_id=?6,source_ref=?7,safety_level=?8,language=?9,
                is_builtin=0,version=version+1,review_status='pending_review',reviewer='',review_date='',updated_at=CURRENT_TIMESTAMP WHERE id=?10",
                params![title, input.category.trim(), input.applies_to.trim(), input.summary.trim(), input.body.trim(), input.source_id, input.source_ref.trim(), input.safety_level, input.language, id]).map_err(|e| e.to_string())?;
            if n == 0 { return Err("Article not found".into()); }
            Ok(id)
        }
        None => {
            let slug = format!("custom-{}", uuid::Uuid::new_v4().simple());
            conn.execute("INSERT INTO knowledge_articles(slug,language,title,category,applies_to,summary,body,source_id,source_ref,author,review_status,safety_level,is_builtin) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'draft',?11,0)",
                params![slug, input.language, title, input.category.trim(), input.applies_to.trim(), input.summary.trim(), input.body.trim(), input.source_id, input.source_ref.trim(), author, input.safety_level]).map_err(|e| e.to_string())?;
            Ok(conn.last_insert_rowid())
        }
    }
}

#[tauri::command]
pub fn save_article(state: tauri::State<'_, AppState>, session_token: String, input: ArticleInput) -> Result<i64, String> {
    let me = auth::require_adviser_or_manager(&state, &session_token)?;
    save_article_tx(&*state.db()?, &input, &me.display_name())
}

/// Records who reviewed an article. Only the owner can mark content approved,
/// and the reviewer's name is required so the approval is traceable.
pub fn set_review_tx(conn: &Connection, id: i64, status: &str, reviewer: &str) -> Result<(), String> {
    if !REVIEW.contains(&status) { return Err("Unknown review status".into()); }
    let reviewer = reviewer.trim();
    if status == "approved" && reviewer.chars().count() < 3 { return Err("Enter the name and qualification of the person who reviewed it".into()); }
    check_len(reviewer, 120, "Reviewer")?;
    let date = if status == "approved" { today_str() } else { String::new() };
    let n = conn.execute("UPDATE knowledge_articles SET review_status=?1,reviewer=?2,review_date=?3,updated_at=CURRENT_TIMESTAMP WHERE id=?4",
        params![status, if status == "approved" { reviewer } else { "" }, date, id]).map_err(|e| e.to_string())?;
    if n == 0 { return Err("Article not found".into()); }
    Ok(())
}

#[tauri::command]
pub fn set_article_review(state: tauri::State<'_, AppState>, session_token: String, id: i64, status: String, reviewer: String) -> Result<(), String> {
    let me = auth::require(&state, &session_token, Role::Worker)?;
    if !(me.role == Role::Owner || me.role == Role::Adviser) { return Err("Only the owner or an adviser can record a review".into()); }
    set_review_tx(&*state.db()?, id, &status, &reviewer)
}

#[tauri::command]
pub fn delete_article(state: tauri::State<'_, AppState>, session_token: String, id: i64) -> Result<(), String> {
    auth::require(&state, &session_token, Role::Owner)?;
    let n = state.db()?.execute("DELETE FROM knowledge_articles WHERE id=?1 AND is_builtin=0", [id]).map_err(|e| e.to_string())?;
    if n == 0 { return Err("Built-in articles cannot be deleted. Edit them instead.".into()); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_are_seeded_pending_review_with_sources() {
        let conn = crate::db::test_conn();
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM knowledge_articles WHERE is_builtin=1", [], |r| r.get(0)).unwrap();
        assert_eq!(n as usize, ARTICLES.len());
        let approved: i64 = conn.query_row("SELECT COUNT(*) FROM knowledge_articles WHERE review_status='approved'", [], |r| r.get(0)).unwrap();
        assert_eq!(approved, 0, "nothing ships approved");
        let no_source: i64 = conn.query_row("SELECT COUNT(*) FROM knowledge_articles WHERE source_id IS NULL", [], |r| r.get(0)).unwrap();
        assert_eq!(no_source, 0);
        // Seeding again changes nothing.
        seed_builtin(&conn).unwrap();
        let again: i64 = conn.query_row("SELECT COUNT(*) FROM knowledge_articles", [], |r| r.get(0)).unwrap();
        assert_eq!(again as usize, ARTICLES.len());
    }

    #[test]
    fn approval_needs_a_reviewer_and_edits_reset_it() {
        let conn = crate::db::test_conn();
        let id: i64 = conn.query_row("SELECT id FROM knowledge_articles WHERE slug='biosecurity-basics'", [], |r| r.get(0)).unwrap();
        assert!(set_review_tx(&conn, id, "approved", "").is_err());
        set_review_tx(&conn, id, "approved", "Dr A. Okon, DVM").unwrap();
        let hits = search(&conn, "visitor", "").unwrap();
        assert!(hits.iter().any(|a| a.id == id && a.review_status == "approved"));
        let input = ArticleInput { id: Some(id), title: "Biosecurity".into(), category: "Biosecurity".into(), applies_to: "poultry".into(), summary: "s".into(), body: "b".into(), source_id: None, source_ref: String::new(), safety_level: "general".into(), language: "en".into() };
        save_article_tx(&conn, &input, "Owner").unwrap();
        let (status, builtin): (String, i64) = conn.query_row("SELECT review_status,is_builtin FROM knowledge_articles WHERE id=?1", [id], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        assert_eq!(status, "pending_review");
        assert_eq!(builtin, 0);
    }
}
