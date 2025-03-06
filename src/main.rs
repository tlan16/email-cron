extern crate imap;
extern crate native_tls;
use chrono::{DateTime, Utc};
use dotenv::dotenv;
use std::env;

fn main() {
    dotenv().ok();
    fetch_inbox_top();
}

fn fetch_inbox_top() {
    let domain = env::vars()
        .find(|(key, _)| key == "APP_IMAP_HOST")
        .unwrap()
        .1;
    let domain = domain.as_str();
    let tls = native_tls::TlsConnector::builder().build().unwrap();

    let port = env::vars()
        .find(|(key, _)| key == "APP_IMAP_PORT")
        .unwrap()
        .1;
    let port = port.parse::<u16>().unwrap();
    let client = imap::connect((domain, port), domain, &tls).unwrap();

    let mut imap_session = client
        .login(
            env::vars()
                .find(|(key, _)| key == "APP_IMAP_USERNAME")
                .unwrap()
                .1,
            env::vars()
                .find(|(key, _)| key == "APP_IMAP_PASSWORD")
                .unwrap()
                .1,
        )
        .map_err(|e| e.0)
        .unwrap();

    let inbox = imap_session.select("[Gmail]/All Mail").unwrap();
    let total_message_count = inbox.exists;
    let number_of_messages_to_fetch = env::vars()
        .find(|(key, _)| key == "APP_MAX_EMAIL_TO_FETCH")
        .unwrap()
        .1;
    let number_of_messages_to_fetch = number_of_messages_to_fetch.parse::<u32>().unwrap();
    let sequence_set = format!(
        "{:}:{}",
        total_message_count - number_of_messages_to_fetch,
        total_message_count
    );

    let mailboxes = imap_session
        .list(Option::from(""), Option::from("*"))
        .unwrap();
    for mailbox in mailboxes.iter() {
        println!("Discovered mailbox: {}", mailbox.name());
    }

    let messages = imap_session.fetch(sequence_set, "(ENVELOPE UID)").unwrap();
    for message in messages.iter() {
        let envelope = message.envelope();
        if let (Some(envelope), Some(uid)) = (envelope, message.uid) {
            let from = envelope.from.as_ref();
            let date = envelope.date.as_ref();
            let subject = envelope.subject.as_ref();
            if let (Some(from), Some(date_bytes), Some(subject_bytes)) = (from, date, subject) {
                let date_str = std::str::from_utf8(date_bytes).unwrap();
                let current_time = Utc::now();
                let parsed_date = DateTime::parse_from_rfc2822(date_str).unwrap();
                let duration = current_time.signed_duration_since(parsed_date);
                let subject = rfc2047_decoder::decode(subject_bytes).unwrap_or(
                    std::str::from_utf8(subject_bytes).unwrap().to_string()
                );

                for from_item in from.iter() {
                    let mailbox = from_item.mailbox.as_ref();
                    let host = from_item.host.as_ref();
                    if mailbox.is_some() && host.is_some() {
                        let mailbox = std::str::from_utf8(mailbox.unwrap()).unwrap();
                        let host = std::str::from_utf8(host.unwrap()).unwrap();
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
                            imap_session.uid_mv(uid.to_string(), "[Gmail]/Bin").unwrap();
                            println!("Deleted email from {} with subject {}", address, subject);
                        }
                    }
                }
            }
        }
    }
    imap_session.expunge().unwrap();
}
