use chrono::NaiveDate;
use serde::Deserialize;

use crate::{Division, Error};

/// The gov.uk bank holidays feed, `https://www.gov.uk/bank-holidays.json`, built only by
/// [`Feed::parse`].
#[derive(Debug)]
pub struct Feed(Divisions);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Divisions {
    england_and_wales: DivisionFeed,
    northern_ireland: DivisionFeed,
    scotland: DivisionFeed,
}

#[derive(Debug, Deserialize)]
struct DivisionFeed {
    events: Vec<Event>,
}

#[derive(Debug, Deserialize)]
pub struct Event {
    pub bunting: bool,
    pub date: NaiveDate,
    /// `"Substitute day"` on a substitute day; any other string is accepted.
    pub notes: String,
    pub title: String,
}

impl Feed {
    /// Parses a feed, rejecting one with a division that has no events.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let feed = Self(serde_json::from_slice(bytes).map_err(Error::Feed)?);
        let empty = Division::ALL
            .into_iter()
            .find(|&division| feed.events(division).is_empty());
        if let Some(division) = empty {
            return Err(Error::EmptyDivision(division));
        }
        Ok(feed)
    }

    #[must_use]
    pub fn events(&self, division: Division) -> &[Event] {
        let divisions = &self.0;
        let feed = match division {
            Division::EnglandAndWales => &divisions.england_and_wales,
            Division::NorthernIreland => &divisions.northern_ireland,
            Division::Scotland => &divisions.scotland,
        };
        &feed.events
    }
}

/// A copy of the feed and the date it was taken: a snapshot's capture date or a cache's fetch date.
#[derive(Debug)]
pub struct DatedFeed {
    pub as_of: NaiveDate,
    pub feed: Feed,
}
