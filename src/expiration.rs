use chrono::{NaiveDate, Utc};
use std::fs::File;
use std::io::Write;

const EXPIRATION_DATE: &str = "2025-06-23";

pub fn panic_if_expired() {
    let expiration_date = NaiveDate::parse_from_str(EXPIRATION_DATE, "%Y-%m-%d").unwrap();

    let now = Utc::now().date_naive();
    if now > expiration_date {
        let message =
            format!("The software has expired. Please contact the developer for an update: Javier Guallart <javier.guallart@dnv.com>");

        let mut log = File::create("manual_data_cleaner_expired.log").unwrap();
        let _ = writeln!(log, "{}", message);
        panic!("{}", message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chrono() {
        panic_if_expired();
    }
}






