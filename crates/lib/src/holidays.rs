use std::collections::{BTreeMap, HashMap};
use std::ops::RangeInclusive;

use chrono::{Datelike, NaiveDate};

use crate::predict::predict;
use crate::{DatedFeed, Division, Event, Feed};

/// The `notes` the feed gives a substitute day.
const SUBSTITUTE_DAY_NOTE: &str = "Substitute day";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BankHoliday {
    pub date: NaiveDate,
    /// gov.uk's name, ending " (substitute day)" on a substitute day.
    pub name: String,
    pub predicted: bool,
}

impl BankHoliday {
    pub(crate) fn published(event: &Event) -> Self {
        let name = if event.notes == SUBSTITUTE_DAY_NOTE {
            substitute_name(&event.title)
        } else {
            event.title.clone()
        };
        Self {
            date: event.date,
            name,
            predicted: false,
        }
    }
}

pub(crate) fn substitute_name(title: &str) -> String {
    format!("{title} (substitute day)")
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

/// How many years past the later of the published data's last year and the current year
/// predictions run.
const PREDICTED_YEARS_AHEAD: i32 = 2;

/// Merges the snapshot with the cache: the cache's copy of a year wins and the snapshot fills the
/// years it lacks. A cache fetched before the snapshot was captured is ignored. Each division then
/// gets predicted bank holidays from the year after its own last published year, through a last
/// year shared by every division.
#[must_use]
pub fn merge(snapshot: &DatedFeed, cache: Option<&DatedFeed>, current_year: i32) -> Holidays {
    let cache = cache.filter(|cache| cache.as_of >= snapshot.as_of);
    let mut divisions: HashMap<_, _> = Division::ALL
        .into_iter()
        .map(|division| {
            let mut years = by_year(&snapshot.feed, division);
            if let Some(cache) = cache {
                years.extend(by_year(&cache.feed, division));
            }
            (division, years)
        })
        .collect();
    let last_predicted = divisions
        .values()
        .filter_map(|years| years.keys().next_back().copied())
        .fold(current_year, i32::max)
        + PREDICTED_YEARS_AHEAD;
    for (&division, years) in &mut divisions {
        let Some(&last) = years.keys().next_back() else {
            continue;
        };
        for year in last + 1..=last_predicted {
            years.insert(year, predict(year, division));
        }
    }
    Holidays { divisions }
}

pub(crate) fn by_year(feed: &Feed, division: Division) -> BTreeMap<i32, Vec<BankHoliday>> {
    let mut years: BTreeMap<i32, Vec<BankHoliday>> = BTreeMap::new();
    for event in feed.events(division) {
        years
            .entry(event.date.year())
            .or_default()
            .push(BankHoliday::published(event));
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

    /// A feed with one Christmas Day per division, in the year given for it.
    fn feed_ending(last_years: [(&str, i32); 3]) -> DatedFeed {
        let json: serde_json::Map<_, _> = last_years
            .into_iter()
            .map(|(division, year)| {
                let event = serde_json::json!(
                    {"title": "Christmas Day", "date": format!("{year}-12-25"), "notes": "", "bunting": true}
                );
                (division.to_owned(), serde_json::json!({ "events": [event] }))
            })
            .collect();
        DatedFeed {
            as_of: date("2025-06-01"),
            feed: Feed::parse(serde_json::Value::Object(json).to_string().as_bytes())
                .expect("test feeds are valid"),
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
        let holidays = merge(&snapshot, Some(&cache), 2025);
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
    fn a_cache_fetched_on_the_day_the_snapshot_was_captured_is_used() {
        let snapshot = dated_feed("2025-06-01", &[("2026-12-25", "Christmas Day")]);
        let cache = dated_feed("2025-06-01", &[("2026-12-28", "Boxing Day")]);
        let holidays = merge(&snapshot, Some(&cache), 2025);
        assert_eq!(names(&holidays, 2026), [(date("2026-12-28"), "Boxing Day")]);
    }

    #[test]
    fn a_cache_fetched_before_the_snapshot_was_captured_is_ignored() {
        let snapshot = dated_feed("2025-06-01", &[("2026-12-25", "Christmas Day")]);
        let cache = dated_feed("2025-05-31", &[("2026-12-28", "Boxing Day")]);
        let holidays = merge(&snapshot, Some(&cache), 2025);
        assert_eq!(
            names(&holidays, 2026),
            [(date("2026-12-25"), "Christmas Day")]
        );
    }

    #[test]
    fn predictions_run_to_two_years_after_the_data_when_the_data_is_current() {
        let snapshot = dated_feed("2025-06-01", &[("2026-12-25", "Christmas Day")]);
        let holidays = merge(&snapshot, None, 2025);
        for division in Division::ALL {
            assert_eq!(holidays.years(division), 2026..=2028, "{division:?}");
        }
    }

    #[test]
    fn predictions_run_to_two_years_after_the_current_year_when_the_data_ends_before_it() {
        let snapshot = dated_feed("2025-06-01", &[("2026-12-25", "Christmas Day")]);
        let holidays = merge(&snapshot, None, 2028);
        for division in Division::ALL {
            assert_eq!(holidays.years(division), 2026..=2030, "{division:?}");
        }
        assert_eq!(
            holidays
                .year(Division::EnglandAndWales, 2030)
                .and_then(<[_]>::first),
            Some(&BankHoliday {
                date: date("2030-01-01"),
                name: "New Year’s Day".to_owned(),
                predicted: true,
            }),
        );
    }

    #[test]
    fn each_division_predicts_from_the_year_after_its_own_last_published_year() {
        let snapshot = feed_ending([
            ("england-and-wales", 2026),
            ("northern-ireland", 2026),
            ("scotland", 2025),
        ]);
        let holidays = merge(&snapshot, None, 2025);
        let first_of_2026 = |division| {
            let holiday = &holidays
                .year(division, 2026)
                .expect("2026 has bank holidays")[0];
            (holiday.name.as_str(), holiday.predicted)
        };
        assert_eq!(
            first_of_2026(Division::EnglandAndWales),
            ("Christmas Day", false)
        );
        assert_eq!(first_of_2026(Division::Scotland), ("New Year’s Day", true));
        assert_eq!(holidays.years(Division::Scotland), 2025..=2028);
    }

    #[test]
    fn the_next_bank_holiday_on_its_own_day_is_zero_days_away() {
        let holidays = merge(
            &dated_feed("2025-06-01", &[("2025-12-25", "Christmas Day")]),
            None,
            2025,
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
            2025,
        );
        let next = holidays
            .next(Division::EnglandAndWales, date("2025-12-27"))
            .expect("New Year’s Day follows");
        assert_eq!((next.holiday.date, next.days), (date("2026-01-01"), 5));
    }

    #[test]
    fn there_is_no_next_bank_holiday_after_the_last_predicted_one() {
        let holidays = merge(
            &dated_feed("2025-06-01", &[("2025-12-25", "Christmas Day")]),
            None,
            2025,
        );
        assert_eq!(
            holidays.next(Division::EnglandAndWales, date("2027-12-29")),
            None
        );
    }
}
