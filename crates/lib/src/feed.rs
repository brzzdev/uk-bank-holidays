use chrono::NaiveDate;
use serde::Deserialize;

use crate::{Division, Error};

/// The gov.uk bank holidays feed, `https://www.gov.uk/bank-holidays.json`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Feed {
    pub england_and_wales: DivisionFeed,
    pub northern_ireland: DivisionFeed,
    pub scotland: DivisionFeed,
}

#[derive(Debug, Deserialize)]
pub struct DivisionFeed {
    pub events: Vec<Event>,
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
        let feed: Self = serde_json::from_slice(bytes).map_err(Error::Feed)?;
        match Division::ALL
            .into_iter()
            .find(|&division| feed.events(division).is_empty())
        {
            Some(division) => Err(Error::EmptyDivision(division)),
            None => Ok(feed),
        }
    }

    #[must_use]
    pub fn events(&self, division: Division) -> &[Event] {
        let feed = match division {
            Division::EnglandAndWales => &self.england_and_wales,
            Division::NorthernIreland => &self.northern_ireland,
            Division::Scotland => &self.scotland,
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
