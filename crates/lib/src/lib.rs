mod cache;
mod calendar;
mod division;
mod error;
mod feed;
mod holidays;
mod predict;
mod snapshot;

pub use cache::{BUNDLE_ID, Loaded, load};
pub use calendar::{Bridge, Calendar, Day, Next, Span};
pub use division::Division;
pub use error::Error;
pub use feed::{DatedFeed, Event, Feed};
pub use holidays::{BankHoliday, Holidays, merge};
pub use snapshot::snapshot;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_committed_snapshot_and_its_capture_date_parse() {
        let _ = snapshot();
    }
}
