mod division;
mod error;
mod feed;
mod holidays;
mod snapshot;

pub use division::Division;
pub use error::Error;
pub use feed::{DatedFeed, DivisionFeed, Event, Feed};
pub use holidays::{BankHoliday, Holidays, Next, merge};
pub use snapshot::snapshot;

#[cfg(test)]
mod tests {
    use chrono::Datelike;

    use super::*;

    #[test]
    fn the_committed_snapshot_has_every_division_and_a_capture_date() {
        let snapshot = snapshot();
        for division in Division::ALL {
            assert!(!snapshot.feed.events(division).is_empty());
        }
        assert!(snapshot.as_of.year() >= 2026);
    }
}
