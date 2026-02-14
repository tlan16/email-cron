extern crate imap;
extern crate native_tls;
use chrono::{DateTime, Utc};
use dotenv::dotenv;
use std::env;

fn main() {
    dotenv().ok();
    match fetch_inbox_top() {
        Ok(_) => println!("Successfully processed inbox"),
        Err(e) => eprintln!("Error: {}", e),
    }
}

fn fetch_inbox_top() -> Result<(), Box<dyn std::error::Error>> {
    let domain = env::var("APP_IMAP_HOST")
        .map_err(|_| "APP_IMAP_HOST environment variable not set")?;
    let tls = native_tls::TlsConnector::builder().build()?;

    let port = env::var("APP_IMAP_PORT")
        .map_err(|_| "APP_IMAP_PORT environment variable not set")?
        .parse::<u16>()?;

    // Create address string for connection
    let addr = format!("{}:{}", domain, port);
    let tcp = std::net::TcpStream::connect(&addr)?;
    let tls_stream = tls.connect(&domain, tcp)?;
    let client = imap::Client::new(tls_stream);

    let username = env::var("APP_IMAP_USERNAME")
        .map_err(|_| "APP_IMAP_USERNAME environment variable not set")?;
    let password = env::var("APP_IMAP_PASSWORD")
        .map_err(|_| "APP_IMAP_PASSWORD environment variable not set")?;

    let mut imap_session = client.login(username, password)
        .map_err(|e| format!("Login failed: {}", e.0))?;

    let inbox = imap_session.select("[Gmail]/All Mail")?;
    let total_message_count = inbox.exists;
    let number_of_messages_to_fetch = env::var("APP_MAX_EMAIL_TO_FETCH")
        .map_err(|_| "APP_MAX_EMAIL_TO_FETCH environment variable not set")?
        .parse::<u32>()?;
    let sequence_set = format!(
        "{:}:{}",
        total_message_count - number_of_messages_to_fetch,
        total_message_count
    );

    let mailboxes = imap_session.list(Option::from(""), Option::from("*"))?;
    for mailbox_name in mailboxes.iter() {
        println!("Discovered mailbox: {}", mailbox_name.name());
    }

    let messages = imap_session.fetch(sequence_set, "(ENVELOPE UID)")?;
    let mut to_delete = Vec::new();
    for message in messages.iter() {
        let envelope = message.envelope();
        if let (Some(envelope), Some(uid)) = (envelope, message.uid) {
            let from = envelope.from.as_ref();
            let date = envelope.date.as_ref();
            let subject = envelope.subject.as_ref();
            if let (Some(from), Some(date_bytes), Some(subject_bytes)) = (from, date, subject) {
                if let Ok(date_str) = std::str::from_utf8(date_bytes) {
                    if let Ok(parsed_date) = DateTime::parse_from_rfc2822(date_str) {
                        let current_time = Utc::now();
                        let duration = current_time.signed_duration_since(parsed_date);
                        let subject = rfc2047_decoder::decode(subject_bytes).unwrap_or(
                            std::str::from_utf8(subject_bytes).unwrap().to_string()
                        );

                        for from_item in from.iter() {
                            let mailbox = from_item.mailbox.as_ref();
                            let host = from_item.host.as_ref();
                            if let (Some(mailbox_bytes), Some(host_bytes)) = (mailbox, host) {
                                if let (Ok(mailbox), Ok(host)) = (
                                    std::str::from_utf8(mailbox_bytes),
                                    std::str::from_utf8(host_bytes),
                                ) {
                                    let address = format!("{mailbox}@{host}");
                                    let days = duration.num_days();
                                    println!(
                                        "Received from {} since {} days ago. Subject is {}",
                                        address,
                                        duration.num_days(),
                                        subject
                                    );

                                    let should_delete = (address == "noreply@ozbargain.com.au"
                                        || address == "crew@morningbrew.com")
                                        && days > 5;
                                    if should_delete {
                                        to_delete.push((uid, address, subject.clone()));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    for (uid, address, subject) in to_delete {
        imap_session.uid_mv(uid.to_string(), "[Gmail]/Bin")?;
        println!("Deleted email from {} with subject {}", address, subject);
    }
    imap_session.expunge()?;
    Ok(())
}
