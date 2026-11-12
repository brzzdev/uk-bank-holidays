use chrono::NaiveDate;

use crate::{DatedFeed, Feed};

const CAPTURED: &str = include_str!("../data/captured");
const FEED: &[u8] = include_bytes!("../data/bank-holidays.json");

/// The copy of the feed built into the binary.
///
/// # Panics
///
/// If the committed snapshot doesn't parse, which its test rules out.
#[must_use]
pub fn snapshot() -> DatedFeed {
    DatedFeed {
        as_of: CAPTURED
            .trim()
            .parse::<NaiveDate>()
            .expect("the snapshot test parses the committed capture date"),
        feed: Feed::parse(FEED).expect("the snapshot test parses the committed feed"),
    }
}
