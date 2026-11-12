//! Receipt email over SMTP: Gmail, Outlook / Microsoft 365, Yahoo, Zoho or any
//! custom server. Sends a styled HTML receipt with a plain-text alternative.
//! SMTP runs on a blocking worker thread so the window never freezes.

use crate::{auth::{self, Role}, db, sales::{self, valid_email, ReceiptDetail}, AppState};
use base64::Engine;
use lettre::{
    message::{header::ContentType, Attachment, Mailbox, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    Message, SmtpTransport, Transport,
};
use rusqlite::{params, Connection};
use std::time::Duration;

#[derive(Clone)]
struct SmtpConfig {
    provider: String,
    host: String,
    port: u16,
    security: String,
    username: String,
    password: String,
    from_name: String,
    from_email: String,
    reply_to: String,
    bcc_self: bool,
}

struct Brand {
    name: String,
    address: String,
    phone: String,
    email: String,
    logo: String,
    primary: String,
    secondary: String,
    currency: String,
    title: String,
    footer: String,
    tax_label: String,
    label_customer: String,
}

fn load_config(conn: &Connection) -> Result<SmtpConfig, String> {
    let s = |k: &str| db::setting(conn, k);
    let provider = s("smtp_provider")?;
    let host = s("smtp_host")?.trim().to_string();
    let port: u16 = s("smtp_port")?.trim().parse().map_err(|_| "Set a valid SMTP port in Settings → Email".to_string())?;
    let username = s("smtp_username")?.trim().to_string();
    let mut password = s("smtp_password")?;
    // Google shows App Passwords in four groups ("abcd efgh ijkl mnop"); spaces are not part of it.
    if provider == "gmail" { password.retain(|c| !c.is_whitespace()); }
    let mut from_email = s("smtp_from_email")?.trim().to_string();
    if from_email.is_empty() { from_email = username.clone(); }
    if host.is_empty() || username.is_empty() || password.is_empty() || from_email.is_empty() {
        return Err("Email is not set up yet. Open Settings → Email and enter your server, username and password.".into());
    }
    let security = s("smtp_security")?;
    let local = host == "localhost" || host == "127.0.0.1";
    if security == "none" && !local {
        return Err("For your safety, passwords are only sent over an encrypted connection. Choose STARTTLS or SSL/TLS.".into());
    }
    Ok(SmtpConfig { provider, host, port, security, username, password, from_name: s("smtp_from_name")?.trim().to_string(), from_email, reply_to: s("smtp_reply_to")?.trim().to_string(), bcc_self: db::setting_bool(conn, "email_bcc_self")? })
}

fn load_brand(conn: &Connection) -> Result<Brand, String> {
    let s = |k: &str| db::setting(conn, k);
    let color = |k: &str, d: &str| -> Result<String, String> { let v = s(k)?; Ok(if crate::settings::is_hex_color(&v) { v } else { d.to_string() }) };
    Ok(Brand {
        name: s("business_name")?, address: s("business_address")?, phone: s("business_phone")?, email: s("business_email")?, logo: s("business_logo")?,
        primary: color("theme_primary", "#17845A")?, secondary: color("theme_secondary", "#0F2E24")?, currency: s("currency_symbol")?,
        title: s("receipt_title")?, footer: s("receipt_footer")?, tax_label: s("tax_label")?, label_customer: s("label_customer")?,
    })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

fn decode_logo(data_url: &str) -> Option<(String, Vec<u8>)> {
    let rest = data_url.strip_prefix("data:")?;
    let (mime, b64) = rest.split_once(";base64,")?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    Some((mime.to_string(), bytes))
}

fn friendly_error(cfg: &SmtpConfig, err: &str) -> String {
    let e = err.to_lowercase();
    if e.contains("535") || e.contains("534") || e.contains("username and password not accepted") || e.contains("authentication") {
        return match cfg.provider.as_str() {
            "gmail" => "Gmail did not accept the sign-in. Use a 16-character App Password, not your normal Gmail password: turn on 2-Step Verification, then create one at myaccount.google.com/apppasswords.".into(),
            "outlook" => "Outlook did not accept the sign-in. Check the password, and make sure SMTP AUTH is enabled for this mailbox (Microsoft 365 admins can switch it off).".into(),
            "yahoo" => "Yahoo did not accept the sign-in. Generate an app password in Yahoo Account Security and use it here.".into(),
            "zoho" => "Zoho did not accept the sign-in. If you use two-factor sign-in, create an application-specific password in Zoho.".into(),
            _ => format!("The mail server rejected the username or password. ({err})"),
        };
    }
    if e.contains("timed out") || e.contains("connection refused") || e.contains("dns") || e.contains("failed to lookup") || e.contains("network") || e.contains("connection") {
        return format!("Could not reach {}:{}. Check your internet connection and the server and port in Settings → Email. ({err})", cfg.host, cfg.port);
    }
    if e.contains("553") || e.contains("550") || e.contains("sender") {
        return format!("The server refused the sender address. Send from the same address you sign in with, or an alias your provider allows. ({err})");
    }
    format!("Email could not be sent: {err}")
}

fn send_blocking(cfg: &SmtpConfig, message: &Message) -> Result<(), String> {
    let builder = match cfg.security.as_str() {
        "ssl" => SmtpTransport::relay(&cfg.host),
        "starttls" => SmtpTransport::starttls_relay(&cfg.host),
        _ => Ok(SmtpTransport::builder_dangerous(&cfg.host)),
    }
    .map_err(|e| format!("SMTP server setting is not valid: {e}"))?;
    let mailer = builder
        .port(cfg.port)
        .credentials(Credentials::new(cfg.username.clone(), cfg.password.clone()))
        .timeout(Some(Duration::from_secs(30)))
        .build();
    mailer.send(message).map(|_| ()).map_err(|e| friendly_error(cfg, &e.to_string()))
}

fn envelope(cfg: &SmtpConfig, brand: &Brand, to: &str, subject: String) -> Result<lettre::message::MessageBuilder, String> {
    let display = if cfg.from_name.is_empty() { brand.name.clone() } else { cfg.from_name.clone() };
    let from = Mailbox::new(Some(display), cfg.from_email.parse().map_err(|e| format!("Sender email is not valid: {e}"))?);
    let to: Mailbox = to.parse().map_err(|e| format!("Recipient email is not valid: {e}"))?;
    let mut b = Message::builder().from(from.clone()).to(to).subject(subject);
    if !cfg.reply_to.is_empty() {
        b = b.reply_to(cfg.reply_to.parse().map_err(|e| format!("Reply-to email is not valid: {e}"))?);
    } else if !brand.email.is_empty() && valid_email(&brand.email) {
        if let Ok(m) = brand.email.parse() { b = b.reply_to(m); }
    }
    if cfg.bcc_self { b = b.bcc(from); }
    Ok(b)
}

fn body_with_logo(brand: &Brand, html: String, text: String) -> MultiPart {
    let html_part = SinglePart::builder().header(ContentType::TEXT_HTML).body(html);
    let text_part = SinglePart::builder().header(ContentType::TEXT_PLAIN).body(text);
    match decode_logo(&brand.logo).and_then(|(mime, bytes)| ContentType::parse(&mime).ok().map(|ct| (ct, bytes))) {
        Some((ct, bytes)) => MultiPart::alternative()
            .singlepart(text_part)
            .multipart(MultiPart::related().singlepart(html_part).singlepart(Attachment::new_inline("logo".into()).body(bytes, ct))),
        None => MultiPart::alternative().singlepart(text_part).singlepart(html_part),
    }
}

fn receipt_html(b: &Brand, r: &ReceiptDetail) -> String {
    let m = |k: i64| esc(&db::format_money(&b.currency, k));
    let logo = if decode_logo(&b.logo).is_some() { r#"<img src="cid:logo" alt="" width="56" height="56" style="display:block;border-radius:8px;object-fit:contain;margin-bottom:10px">"#.to_string() } else { String::new() };
    let mut rows = String::new();
    for i in &r.items {
        rows.push_str(&format!(
            r#"<tr><td style="padding:10px 0;border-bottom:1px solid #E6EAF0;font-size:14px;color:#1B2433">{}<div style="font-size:12px;color:#6B7585">{} {} × {}</div></td><td align="right" style="padding:10px 0;border-bottom:1px solid #E6EAF0;font-size:14px;color:#1B2433;white-space:nowrap">{}</td></tr>"#,
            esc(&i.product_name), esc(&db::format_qty(i.quantity)), esc(&i.unit), m(i.unit_price_kobo), m(i.line_total_kobo)
        ));
    }
    let line = |label: &str, v: String, strong: bool| {
        let w = if strong { "700;font-size:17px" } else { "400;font-size:14px" };
        format!(r#"<tr><td style="padding:4px 0;color:#4A5568;font-weight:{w}">{}</td><td align="right" style="padding:4px 0;color:#1B2433;font-weight:{w}">{v}</td></tr>"#, esc(label))
    };
    let mut totals = line("Subtotal", m(r.subtotal_kobo), false);
    if r.discount_kobo > 0 { totals.push_str(&line("Discount", format!("−{}", m(r.discount_kobo)), false)); }
    if r.tax_kobo > 0 { totals.push_str(&line(&b.tax_label, m(r.tax_kobo), false)); }
    totals.push_str(&line("Total", m(r.total_kobo), true));
    for p in &r.payments { totals.push_str(&line(&format!("Paid by {}", p.method), m(p.amount_kobo), false)); }
    if r.change_kobo > 0 { totals.push_str(&line("Change", m(r.change_kobo), false)); }
    if r.balance_kobo > 0 { totals.push_str(&format!(r#"<tr><td style="padding:6px 0;color:#B42318;font-weight:700">Balance due</td><td align="right" style="padding:6px 0;color:#B42318;font-weight:700">{}</td></tr>"#, m(r.balance_kobo))); }
    let void = if r.payment_status == "void" { format!(r#"<p style="margin:0 0 16px;padding:10px 12px;background:#FEE4E2;color:#B42318;border-radius:6px;font-weight:700">This receipt was voided: {}</p>"#, esc(&r.void_reason)) } else { String::new() };
    let collection = if !r.collection_date.is_empty() { format!("<br>Collection date: {}", esc(&r.collection_date)) } else { String::new() };
    let customer = if r.customer_name.is_empty() { String::new() } else { format!("<br>{}: {}", esc(&b.label_customer), esc(&r.customer_name)) };
    let contact = [b.address.as_str(), b.phone.as_str(), b.email.as_str()].iter().filter(|s| !s.is_empty()).map(|s| esc(s)).collect::<Vec<_>>().join("<br>");
    format!(
        r#"<!doctype html><html><body style="margin:0;padding:0;background:#EEF1F5;font-family:'Libre Franklin',Arial,Helvetica,sans-serif">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="background:#EEF1F5;padding:24px 12px"><tr><td align="center">
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="max-width:520px;background:#FFFFFF;border-radius:10px;overflow:hidden;border:1px solid #DCE2EA">
<tr><td style="background:{sec};padding:22px 28px;border-bottom:4px solid {pri}">{logo}<div style="font-size:20px;font-weight:700;color:#FFFFFF">{name}</div><div style="font-size:12px;line-height:1.6;color:#D6E0EE;margin-top:4px">{contact}</div></td></tr>
<tr><td style="padding:24px 28px">{void}
<div style="font-size:13px;color:{pri};font-weight:700;margin-bottom:4px">{title}</div>
<div style="font-size:22px;font-weight:700;color:#1B2433">{no}</div>
<div style="font-size:13px;color:#6B7585;line-height:1.7;margin:6px 0 18px">{date}<br>Served by {cashier}{customer}{collection}</div>
<table role="presentation" width="100%" cellpadding="0" cellspacing="0">{rows}</table>
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" style="margin-top:14px">{totals}</table>
{note}
<p style="margin:22px 0 0;font-size:13px;line-height:1.6;color:#4A5568">{footer}</p>
</td></tr></table>
<p style="font-size:11px;color:#8A94A6;margin:14px 0 0">Sent from {name}</p>
</td></tr></table></body></html>"#,
        sec = b.secondary, pri = b.primary, logo = logo, name = esc(&b.name), contact = contact, void = void, title = esc(&b.title), no = esc(&r.receipt_no),
        date = esc(&r.created_at), cashier = esc(&r.cashier_name), customer = customer, collection = collection, rows = rows, totals = totals,
        note = if r.note.is_empty() { String::new() } else { format!(r#"<p style="margin:16px 0 0;font-size:13px;color:#4A5568"><b>Note:</b> {}</p>"#, esc(&r.note)) },
        footer = esc(&b.footer),
    )
}

fn receipt_text(b: &Brand, r: &ReceiptDetail) -> String {
    let m = |k: i64| db::format_money(&b.currency, k);
    let mut t = format!("{}\n{}\n\n{} {}\nDate: {}\nServed by: {}\n", b.name, b.address, b.title, r.receipt_no, r.created_at, r.cashier_name);
    if !r.customer_name.is_empty() { t.push_str(&format!("{}: {}\n", b.label_customer, r.customer_name)); }
    if !r.collection_date.is_empty() { t.push_str(&format!("Collection date: {}\n", r.collection_date)); }
    if r.payment_status == "void" { t.push_str(&format!("VOID: {}\n", r.void_reason)); }
    t.push('\n');
    for i in &r.items { t.push_str(&format!("{}\n  {} {} x {} = {}\n", i.product_name, db::format_qty(i.quantity), i.unit, m(i.unit_price_kobo), m(i.line_total_kobo))); }
    t.push_str(&format!("\nSubtotal: {}\n", m(r.subtotal_kobo)));
    if r.discount_kobo > 0 { t.push_str(&format!("Discount: -{}\n", m(r.discount_kobo))); }
    if r.tax_kobo > 0 { t.push_str(&format!("{}: {}\n", b.tax_label, m(r.tax_kobo))); }
    t.push_str(&format!("TOTAL: {}\n", m(r.total_kobo)));
    for p in &r.payments { t.push_str(&format!("Paid by {}: {}\n", p.method, m(p.amount_kobo))); }
    if r.change_kobo > 0 { t.push_str(&format!("Change: {}\n", m(r.change_kobo))); }
    if r.balance_kobo > 0 { t.push_str(&format!("Balance due: {}\n", m(r.balance_kobo))); }
    t.push_str(&format!("\n{}\n", b.footer));
    t
}

#[tauri::command]
pub async fn send_receipt_email(state: tauri::State<'_, AppState>, session_token: String, receipt_id: i64, recipient: String) -> Result<String, String> {
    let me = auth::require_seller(&state, &session_token, Role::Cashier)?;
    let recipient = recipient.trim().to_string();
    if !valid_email(&recipient) { return Err("Enter a valid email address".into()); }
    let (cfg, brand, receipt) = {
        let conn = state.db()?;
        if me.role < Role::Manager {
            let cashier: Option<i64> = conn.query_row("SELECT cashier_id FROM receipts WHERE id=?1", [receipt_id], |r| r.get(0)).map_err(|_| "Receipt not found".to_string())?;
            if cashier != Some(me.user_id) { return Err("You can only email receipts you issued".into()); }
        }
        (load_config(&conn)?, load_brand(&conn)?, sales::load_receipt(&conn, receipt_id)?)
    };
    let subject = format!("{} {} from {}", brand.title, receipt.receipt_no, brand.name);
    let message = envelope(&cfg, &brand, &recipient, subject)?
        .multipart(body_with_logo(&brand, receipt_html(&brand, &receipt), receipt_text(&brand, &receipt)))
        .map_err(|e| e.to_string())?;
    let cfg_worker = cfg.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || send_blocking(&cfg_worker, &message)).await.map_err(|e| e.to_string())?;
    {
        let conn = state.db()?;
        let (status, msg) = match &outcome { Ok(_) => ("sent", "Accepted by mail server".to_string()), Err(e) => ("failed", e.clone()) };
        let _ = conn.execute("INSERT INTO email_logs(receipt_id,recipient,status,message) VALUES(?1,?2,?3,?4)", params![receipt_id, recipient, status, msg]);
        if outcome.is_ok() {
            let _ = conn.execute("UPDATE receipts SET customer_email=?1 WHERE id=?2 AND customer_email=''", params![recipient, receipt_id]);
        }
    }
    outcome.map(|_| format!("Receipt sent to {recipient}"))
}

#[tauri::command]
pub async fn send_test_email(state: tauri::State<'_, AppState>, session_token: String, recipient: String) -> Result<String, String> {
    auth::require(&state, &session_token, Role::Owner)?;
    let recipient = recipient.trim().to_string();
    if !valid_email(&recipient) { return Err("Enter a valid email address to receive the test".into()); }
    let (cfg, brand) = { let conn = state.db()?; (load_config(&conn)?, load_brand(&conn)?) };
    let html = format!(
        r#"<div style="font-family:Arial,Helvetica,sans-serif;max-width:480px;margin:24px auto;border:1px solid #DCE2EA;border-radius:10px;overflow:hidden"><div style="background:{};padding:18px 24px;border-bottom:4px solid {};color:#fff;font-size:18px;font-weight:700">{}</div><div style="padding:22px 24px;color:#1B2433;font-size:14px;line-height:1.6">Your email settings work. Receipts will be sent from <b>{}</b> through {}:{}.</div></div>"#,
        brand.secondary, brand.primary, esc(&brand.name), esc(&cfg.from_email), esc(&cfg.host), cfg.port
    );
    let text = format!("Your email settings work. Receipts will be sent from {} through {}:{}.", cfg.from_email, cfg.host, cfg.port);
    let message = envelope(&cfg, &brand, &recipient, format!("Test email from {}", brand.name))?
        .multipart(body_with_logo(&brand, html, text))
        .map_err(|e| e.to_string())?;
    let cfg_worker = cfg.clone();
    tauri::async_runtime::spawn_blocking(move || send_blocking(&cfg_worker, &message)).await.map_err(|e| e.to_string())??;
    Ok(format!("Test email sent to {recipient}. Check the inbox (and spam folder)."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gmail_app_password_spaces_are_removed_and_plaintext_is_refused() {
        let conn = db::test_conn();
        for (k, v) in [("smtp_username", "shop@gmail.com"), ("smtp_password", "abcd efgh ijkl mnop")] { db::set_setting(&conn, k, v).unwrap(); }
        let cfg = load_config(&conn).unwrap();
        assert_eq!(cfg.password, "abcdefghijklmnop");
        assert_eq!(cfg.from_email, "shop@gmail.com");
        db::set_setting(&conn, "smtp_security", "none").unwrap();
        assert!(load_config(&conn).is_err());
    }

    #[test]
    fn html_escapes_user_content() {
        assert_eq!(esc("<b>&\""), "&lt;b&gt;&amp;&quot;");
    }
}
