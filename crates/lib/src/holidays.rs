use std::collections::{BTreeMap, HashMap};
use std::ops::RangeInclusive;

use chrono::{Datelike, NaiveDate};

use crate::{DatedFeed, Division};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BankHoliday {
    pub date: NaiveDate,
    pub name: String,
}

/// The next bank holiday on or after a day, and how many days away it is.
#[derive(Debug, Eq, PartialEq)]
pub struct Next<'a> {
    pub days: i64,
    pub holiday: &'a BankHoliday,
}

/// Each division's bank holidays, by year.
#[derive(Debug)]
pub struct Holidays {
    divisions: HashMap<Division, BTreeMap<i32, Vec<BankHoliday>>>,
}

/// Merges the snapshot with the cache: the cache's copy of a year wins and the snapshot fills the
/// years it lacks. A cache fetched before the snapshot was captured is ignored.
#[must_use]
pub fn merge(snapshot: &DatedFeed, cache: Option<&DatedFeed>) -> Holidays {
    let cache = cache.filter(|cache| cache.as_of >= snapshot.as_of);
    let divisions = Division::ALL
        .into_iter()
        .map(|division| {
            let mut years = by_year(snapshot, division);
            if let Some(cache) = cache {
                years.extend(by_year(cache, division));
            }
            (division, years)
        })
        .collect();
    Holidays { divisions }
}

fn by_year(feed: &DatedFeed, division: Division) -> BTreeMap<i32, Vec<BankHoliday>> {
    let mut years: BTreeMap<i32, Vec<BankHoliday>> = BTreeMap::new();
    for event in feed.feed.events(division) {
        years
            .entry(event.date.year())
            .or_default()
            .push(BankHoliday {
                date: event.date,
                name: event.title.clone(),
            });
    }
    for holidays in years.values_mut() {
        holidays.sort_by_key(|holiday| holiday.date);
    }
    years
}

impl Holidays {
    #[must_use]
    pub fn next(&self, division: Division, from: NaiveDate) -> Option<Next<'_>> {
        let holiday = self
            .years_of(division)
            .range(from.year()..)
            .flat_map(|(_, holidays)| holidays)
            .find(|holiday| holiday.date >= from)?;
        Some(Next {
            days: (holiday.date - from).num_days(),
            holiday,
        })
    }

    #[must_use]
    pub fn year(&self, division: Division, year: i32) -> Option<&[BankHoliday]> {
        self.years_of(division).get(&year).map(Vec::as_slice)
    }

    /// The first and last years the division has bank holidays for.
    ///
    /// # Panics
    ///
    /// Never: the snapshot gives every division at least one year.
    #[must_use]
    pub fn years(&self, division: Division) -> RangeInclusive<i32> {
        let years = self.years_of(division);
        let (Some(&first), Some(&last)) = (years.keys().next(), years.keys().next_back()) else {
            panic!("the snapshot gives every division at least one year");
        };
        first..=last
    }

    fn years_of(&self, division: Division) -> &BTreeMap<i32, Vec<BankHoliday>> {
        &self.divisions[&division]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Feed;

    fn date(s: &str) -> NaiveDate {
        s.parse().expect("test dates are valid")
    }

    /// A feed where every division has the same events, each `(date, title)`.
    fn dated_feed(as_of: &str, events: &[(&str, &str)]) -> DatedFeed {
        let events: Vec<_> = events
            .iter()
            .map(|(date, title)| {
                serde_json::json!({"title": title, "date": date, "notes": "", "bunting": true})
            })
            .collect();
        let division = serde_json::json!({ "events": events });
        let json = serde_json::json!({
            "england-and-wales": division,
            "northern-ireland": division,
            "scotland": division,
        });
        DatedFeed {
            as_of: date(as_of),
            feed: Feed::parse(json.to_string().as_bytes()).expect("test feeds are valid"),
        }
    }

    fn names(holidays: &Holidays, year: i32) -> Vec<(NaiveDate, &str)> {
        holidays
            .year(Division::EnglandAndWales, year)
            .unwrap_or_default()
            .iter()
            .map(|holiday| (holiday.date, holiday.name.as_str()))
            .collect()
    }

    #[test]
    fn the_cache_copy_of_a_year_replaces_the_snapshot_copy_and_the_snapshot_fills_the_rest() {
        let snapshot = dated_feed(
            "2025-06-01",
            &[
                ("2025-12-25", "Christmas Day"),
                ("2026-12-25", "Christmas Day"),
            ],
        );
        let cache = dated_feed(
            "2025-07-01",
            &[
                ("2026-12-25", "Christmas Day"),
                ("2026-12-28", "Boxing Day"),
            ],
        );
        let holidays = merge(&snapshot, Some(&cache));
        assert_eq!(
            names(&holidays, 2025),
            [(date("2025-12-25"), "Christmas Day")]
        );
        assert_eq!(
            names(&holidays, 2026),
            [
                (date("2026-12-25"), "Christmas Day"),
                (date("2026-12-28"), "Boxing Day")
            ],
        );
    }

    #[test]
    fn a_cache_fetched_before_the_snapshot_was_captured_is_ignored() {
        let snapshot = dated_feed("2025-06-01", &[("2026-12-25", "Christmas Day")]);
        let cache = dated_feed("2025-05-31", &[("2026-12-28", "Boxing Day")]);
        let holidays = merge(&snapshot, Some(&cache));
        assert_eq!(
            names(&holidays, 2026),
            [(date("2026-12-25"), "Christmas Day")]
        );
    }

    #[test]
    fn the_next_bank_holiday_on_its_own_day_is_zero_days_away() {
        let holidays = merge(
            &dated_feed("2025-06-01", &[("2025-12-25", "Christmas Day")]),
            None,
        );
        let next = holidays
            .next(Division::EnglandAndWales, date("2025-12-25"))
            .expect("Christmas is on or after itself");
        assert_eq!(
            (next.holiday.name.as_str(), next.days),
            ("Christmas Day", 0)
        );
    }

    #[test]
    fn the_next_bank_holiday_after_the_last_of_a_year_is_in_the_next_year() {
        let holidays = merge(
            &dated_feed(
                "2025-06-01",
                &[
                    ("2025-12-26", "Boxing Day"),
                    ("2026-01-01", "New Year’s Day"),
                ],
            ),
            None,
        );
        let next = holidays
            .next(Division::EnglandAndWales, date("2025-12-27"))
            .expect("New Year’s Day follows");
        assert_eq!((next.holiday.date, next.days), (date("2026-01-01"), 5));
    }

    #[test]
    fn there_is_no_next_bank_holiday_after_the_last_one_in_the_data() {
        let holidays = merge(
            &dated_feed("2025-06-01", &[("2025-12-25", "Christmas Day")]),
            None,
        );
        assert_eq!(
            holidays.next(Division::EnglandAndWales, date("2025-12-26")),
            None
        );
    }
}
