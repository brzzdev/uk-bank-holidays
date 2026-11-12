use chrono::NaiveDate;

use crate::{DatedFeed, Feed};

const CAPTURED: &str = include_str!("../data/captured");
const FEED: &[u8] = include_bytes!("../data/bank-holidays.json");

/// The copy of the feed built into the binary.
///
/// # Panics
///
/// Never: the committed feed and capture date always parse.
#[must_use]
pub fn snapshot() -> DatedFeed {
    DatedFeed {
        as_of: CAPTURED
            .trim()
            .parse::<NaiveDate>()
            .expect("the committed capture date always parses"),
        feed: Feed::parse(FEED).expect("the committed feed always parses"),
    }
}
