use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate, Weekday};

use crate::predict::predict;
use crate::{BankHoliday, Division, Holidays};

/// The most working days a bridge books.
const MAX_DAYS_BOOKED: usize = 8;
/// The fewest days off in a four-day weekend.
const MIN_FOUR_DAY_WEEKEND_DAYS: usize = 4;
/// The fewest bank holidays in a four-day weekend.
const MIN_FOUR_DAY_WEEKEND_HOLIDAYS: usize = 2;

/// A run of consecutive days, both ends included.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    pub first: NaiveDate,
    pub last: NaiveDate,
}

impl Span {
    pub(crate) fn contains(self, day: NaiveDate) -> bool {
        (self.first..=self.last).contains(&day)
    }

    #[must_use]
    pub fn day_count(self) -> usize {
        self.days().count()
    }

    pub(crate) fn days(self) -> impl Iterator<Item = NaiveDate> {
        self.first
            .iter_days()
            .take_while(move |&day| day <= self.last)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bridge {
    pub booked: Span,
    pub days_booked: usize,
    pub off: Span,
    /// Whether either stretch it joins holds a predicted bank holiday.
    pub predicted: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Day {
    BankHoliday,
    /// A working day booked in a bridge.
    Bridge,
    /// A day of a four-day weekend that isn't itself a bank holiday.
    FourDayWeekend,
    Ordinary,
    PredictedBankHoliday,
}

/// The next bank holiday on or after a day.
#[derive(Debug, Eq, PartialEq)]
pub struct Next<'a> {
    pub days: i64,
    pub holiday: &'a BankHoliday,
    pub starts_four_day_weekend: bool,
}

/// A run of days off, as long as weekends and bank holidays allow, holding at least one bank
/// holiday.
struct Stretch<'a> {
    holidays: Vec<&'a BankHoliday>,
    span: Span,
}

/// One division's bank holidays, four-day weekends, and bridges across all its years.
pub struct Calendar<'a> {
    /// Keyed by the last bank holiday before the bridge.
    bridges: BTreeMap<NaiveDate, Bridge>,
    division: Division,
    four_day_weekends: Vec<Span>,
    holidays: BTreeMap<NaiveDate, &'a BankHoliday>,
}

impl<'a> Calendar<'a> {
    #[must_use]
    pub fn new(holidays: &'a Holidays, division: Division) -> Self {
        let following_year = predict(holidays.years(division).end() + 1, division);
        let holidays: BTreeMap<_, _> = holidays
            .years_of(division)
            .values()
            .flatten()
            .map(|holiday| (holiday.date, holiday))
            .collect();
        // The year after the last is predicted so the last year's bridges have an end.
        let through_following_year: BTreeMap<_, _> = holidays
            .iter()
            .map(|(&date, &holiday)| (date, holiday))
            .chain(following_year.iter().map(|holiday| (holiday.date, holiday)))
            .collect();
        let stretches = stretches(&through_following_year);
        let in_range = |stretch: &Stretch| {
            stretch
                .holidays
                .iter()
                .any(|holiday| holidays.contains_key(&holiday.date))
        };
        let four_day_weekends = stretches
            .iter()
            .filter(|stretch| in_range(stretch) && stretch.is_four_day_weekend())
            .map(|stretch| stretch.span)
            .collect();
        let bridges = stretches
            .array_windows()
            .filter(|[before, _]| in_range(before))
            .filter_map(|[before, after]| {
                let booked = Span {
                    first: before.span.last.succ_opt()?,
                    last: after.span.first.pred_opt()?,
                };
                let days_booked = booked.days().filter(|&day| !is_weekend(day)).count();
                if days_booked > MAX_DAYS_BOOKED {
                    return None;
                }
                let bridge = Bridge {
                    booked,
                    days_booked,
                    off: Span {
                        first: before.span.first,
                        last: after.span.last,
                    },
                    predicted: before
                        .holidays
                        .iter()
                        .chain(&after.holidays)
                        .any(|holiday| holiday.predicted),
                };
                Some((before.holidays.last()?.date, bridge))
            })
            .collect();
        Self {
            bridges,
            division,
            four_day_weekends,
            holidays,
        }
    }

    /// The bridge that books a day.
    fn bridge_booked_on(&self, date: NaiveDate) -> Option<&Bridge> {
        if is_weekend(date) {
            return None;
        }
        self.bridges
            .values()
            .find(|bridge| bridge.booked.contains(date))
    }

    /// The bridge that follows a bank holiday, when it's the last bank holiday before one.
    #[must_use]
    pub fn bridge_after(&self, date: NaiveDate) -> Option<&Bridge> {
        self.bridges.get(&date)
    }

    #[must_use]
    pub fn day(&self, date: NaiveDate) -> Day {
        if let Some(holiday) = self.holidays.get(&date) {
            return if holiday.predicted {
                Day::PredictedBankHoliday
            } else {
                Day::BankHoliday
            };
        }
        if self.four_day_weekend(date).is_some() {
            return Day::FourDayWeekend;
        }
        if self.bridge_booked_on(date).is_some() {
            return Day::Bridge;
        }
        Day::Ordinary
    }

    #[must_use]
    pub fn division(&self) -> Division {
        self.division
    }

    /// The four-day weekend a day is part of.
    #[must_use]
    pub fn four_day_weekend(&self, date: NaiveDate) -> Option<Span> {
        self.four_day_weekends
            .iter()
            .copied()
            .find(|weekend| weekend.contains(date))
    }

    /// Whether a bank holiday is in either stretch of days off a bridge joins.
    #[must_use]
    pub fn joins_bridge(&self, date: NaiveDate) -> bool {
        self.bridges
            .values()
            .any(|bridge| bridge.off.contains(date))
    }

    #[must_use]
    pub fn next(&self, from: NaiveDate) -> Option<Next<'a>> {
        let (&date, &holiday) = self.holidays.range(from..).next()?;
        Some(Next {
            days: (date - from).num_days(),
            holiday,
            starts_four_day_weekend: self.starts_four_day_weekend(date),
        })
    }

    /// Whether a bank holiday is the first bank holiday of a four-day weekend.
    fn starts_four_day_weekend(&self, date: NaiveDate) -> bool {
        self.four_day_weekend(date).is_some_and(|weekend| {
            self.holidays
                .range(weekend.first..=weekend.last)
                .next()
                .is_some_and(|(&first, _)| first == date)
        })
    }
}

impl Stretch<'_> {
    fn is_four_day_weekend(&self) -> bool {
        self.holidays.len() >= MIN_FOUR_DAY_WEEKEND_HOLIDAYS
            && self.span.day_count() >= MIN_FOUR_DAY_WEEKEND_DAYS
            && self.span.days().any(is_weekend)
    }
}

fn is_weekend(day: NaiveDate) -> bool {
    matches!(day.weekday(), Weekday::Sat | Weekday::Sun)
}

fn stretches<'a>(holidays: &BTreeMap<NaiveDate, &'a BankHoliday>) -> Vec<Stretch<'a>> {
    let is_off = |day: NaiveDate| is_weekend(day) || holidays.contains_key(&day);
    let mut stretches: Vec<Stretch<'a>> = Vec::new();
    for &date in holidays.keys() {
        if stretches
            .last()
            .is_some_and(|stretch| stretch.span.last >= date)
        {
            continue;
        }
        let mut first = date;
        while let Some(day) = first.pred_opt()
            && is_off(day)
        {
            first = day;
        }
        let mut last = date;
        while let Some(day) = last.succ_opt()
            && is_off(day)
        {
            last = day;
        }
        stretches.push(Stretch {
            holidays: holidays
                .range(first..=last)
                .map(|(_, &holiday)| holiday)
                .collect(),
            span: Span { first, last },
        });
    }
    stretches
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DatedFeed, Feed, merge, snapshot};

    fn date(s: &str) -> NaiveDate {
        s.parse().expect("test dates are valid")
    }

    fn span(first: &str, last: &str) -> Span {
        Span {
            first: date(first),
            last: date(last),
        }
    }

    /// Published bank holidays on these dates in every division.
    fn holidays(dates: &[&str]) -> Holidays {
        let events: Vec<_> = dates
            .iter()
            .map(|date| {
                serde_json::json!({"title": "Bank holiday", "date": date, "notes": "", "bunting": true})
            })
            .collect();
        let json: serde_json::Map<_, _> = Division::ALL
            .into_iter()
            .map(|division| {
                (
                    division.key().to_owned(),
                    serde_json::json!({ "events": events }),
                )
            })
            .collect();
        let feed = Feed::parse(serde_json::Value::Object(json).to_string().as_bytes())
            .expect("test feeds are valid");
        merge(
            &DatedFeed {
                as_of: date("2000-01-01"),
                feed,
            },
            None,
        )
    }

    fn bridge(booked: (&str, &str), days_booked: usize, off: (&str, &str)) -> Bridge {
        Bridge {
            booked: span(booked.0, booked.1),
            days_booked,
            off: span(off.0, off.1),
            predicted: false,
        }
    }

    fn bridges(holidays: &Holidays, division: Division) -> Vec<Bridge> {
        Calendar::new(holidays, division)
            .bridges
            .into_values()
            .collect()
    }

    #[test]
    fn bank_holidays_on_both_sides_of_a_bridge_join_it() {
        let holidays = holidays(&["2025-05-05", "2025-12-25", "2025-12-26", "2026-01-01"]);
        let calendar = Calendar::new(&holidays, Division::EnglandAndWales);
        assert!(calendar.joins_bridge(date("2025-12-25")));
        assert!(calendar.joins_bridge(date("2026-01-01")));
        assert!(!calendar.joins_bridge(date("2025-05-05")));
    }

    #[test]
    fn christmas_2025_is_bridged_to_new_year_by_booking_three_days() {
        let holidays = holidays(&["2025-12-25", "2025-12-26", "2026-01-01"]);
        assert_eq!(
            bridges(&holidays, Division::EnglandAndWales),
            [bridge(
                ("2025-12-29", "2025-12-31"),
                3,
                ("2025-12-25", "2026-01-01")
            )],
        );
    }

    #[test]
    fn an_easter_monday_on_24_to_26_april_is_bridged_to_early_may_by_four_days() {
        for (good_friday, easter_monday, early_may, booked) in [
            (
                "2000-04-21",
                "2000-04-24",
                "2000-05-01",
                ("2000-04-25", "2000-04-28"),
            ),
            (
                "2095-04-22",
                "2095-04-25",
                "2095-05-02",
                ("2095-04-26", "2095-04-29"),
            ),
            (
                "2038-04-23",
                "2038-04-26",
                "2038-05-03",
                ("2038-04-27", "2038-04-30"),
            ),
        ] {
            let holidays = holidays(&[good_friday, easter_monday, early_may]);
            assert_eq!(
                bridges(&holidays, Division::EnglandAndWales),
                [bridge(booked, 4, (good_friday, early_may))],
                "{good_friday}",
            );
        }
    }

    #[test]
    fn the_2023_coronation_is_bridged_to_early_may_by_four_days() {
        let holidays = holidays(&["2023-05-01", "2023-05-08", "2023-05-29"]);
        assert_eq!(
            bridges(&holidays, Division::EnglandAndWales),
            [bridge(
                ("2023-05-02", "2023-05-05"),
                4,
                ("2023-04-29", "2023-05-08")
            )],
        );
    }

    #[test]
    fn st_patricks_day_2016_is_bridged_to_good_friday_by_five_days_across_a_weekend() {
        let holidays = holidays(&["2016-03-17", "2016-03-25", "2016-03-28"]);
        assert_eq!(
            bridges(&holidays, Division::NorthernIreland),
            [bridge(
                ("2016-03-18", "2016-03-24"),
                5,
                ("2016-03-17", "2016-03-28")
            )],
        );
    }

    #[test]
    fn eight_working_days_is_the_longest_bridge() {
        let holidays = holidays(&["2025-01-01", "2025-01-14"]);
        assert_eq!(
            bridges(&holidays, Division::EnglandAndWales),
            [bridge(
                ("2025-01-02", "2025-01-13"),
                8,
                ("2025-01-01", "2025-01-14")
            )],
        );
    }

    #[test]
    fn the_usual_nine_days_from_easter_to_early_may_is_no_bridge() {
        let holidays = holidays(&["2025-04-18", "2025-04-21", "2025-05-05"]);
        assert_eq!(bridges(&holidays, Division::EnglandAndWales), []);
    }

    #[test]
    fn the_ten_days_from_ve_day_2020_to_late_may_is_no_bridge() {
        let holidays = holidays(&["2020-05-08", "2020-05-25"]);
        assert_eq!(bridges(&holidays, Division::EnglandAndWales), []);
    }

    /// Every day of 2025 that isn't ordinary in the committed snapshot, by what it is.
    fn days_of_2025(division: Division) -> BTreeMap<NaiveDate, Day> {
        let holidays = merge(&snapshot(), None);
        let calendar = Calendar::new(&holidays, division);
        span("2025-01-01", "2025-12-31")
            .days()
            .map(|day| (day, calendar.day(day)))
            .filter(|&(_, kind)| kind != Day::Ordinary)
            .collect()
    }

    fn expected(days: &[(Day, &[&str])]) -> BTreeMap<NaiveDate, Day> {
        days.iter()
            .flat_map(|&(kind, dates)| dates.iter().map(move |&day| (date(day), kind)))
            .collect()
    }

    #[test]
    fn england_and_wales_2025_has_easter_and_christmas_four_day_weekends_and_a_new_year_bridge() {
        assert_eq!(
            days_of_2025(Division::EnglandAndWales),
            expected(&[
                (
                    Day::BankHoliday,
                    &[
                        "2025-01-01",
                        "2025-04-18",
                        "2025-04-21",
                        "2025-05-05",
                        "2025-05-26",
                        "2025-08-25",
                        "2025-12-25",
                        "2025-12-26",
                    ],
                ),
                (Day::Bridge, &["2025-12-29", "2025-12-30", "2025-12-31"]),
                (
                    Day::FourDayWeekend,
                    &["2025-04-19", "2025-04-20", "2025-12-27", "2025-12-28"],
                ),
            ]),
        );
    }

    #[test]
    fn scotland_2025_has_only_a_christmas_four_day_weekend_and_a_new_year_bridge() {
        assert_eq!(
            days_of_2025(Division::Scotland),
            expected(&[
                (
                    Day::BankHoliday,
                    &[
                        "2025-01-01",
                        "2025-01-02",
                        "2025-04-18",
                        "2025-05-05",
                        "2025-05-26",
                        "2025-08-04",
                        "2025-12-01",
                        "2025-12-25",
                        "2025-12-26",
                    ],
                ),
                (Day::Bridge, &["2025-12-29", "2025-12-30", "2025-12-31"]),
                (Day::FourDayWeekend, &["2025-12-27", "2025-12-28"]),
            ]),
        );
    }

    #[test]
    fn northern_ireland_2025_adds_its_own_bank_holidays_to_england_and_wales_days() {
        assert_eq!(
            days_of_2025(Division::NorthernIreland),
            expected(&[
                (
                    Day::BankHoliday,
                    &[
                        "2025-01-01",
                        "2025-03-17",
                        "2025-04-18",
                        "2025-04-21",
                        "2025-05-05",
                        "2025-05-26",
                        "2025-07-14",
                        "2025-08-25",
                        "2025-12-25",
                        "2025-12-26",
                    ],
                ),
                (Day::Bridge, &["2025-12-29", "2025-12-30", "2025-12-31"]),
                (
                    Day::FourDayWeekend,
                    &["2025-04-19", "2025-04-20", "2025-12-27", "2025-12-28"],
                ),
            ]),
        );
    }

    #[test]
    fn a_predicted_bank_holiday_is_marked_predicted() {
        let holidays = holidays(&["2025-12-25"]).with_predictions(2025);
        let calendar = Calendar::new(&holidays, Division::EnglandAndWales);
        assert_eq!(calendar.day(date("2026-01-01")), Day::PredictedBankHoliday);
    }

    #[test]
    fn the_weekend_inside_a_bridge_is_ordinary() {
        let holidays = holidays(&["2016-03-17", "2016-03-25", "2016-03-28"]);
        let calendar = Calendar::new(&holidays, Division::EnglandAndWales);
        assert_eq!(calendar.day(date("2016-03-19")), Day::Ordinary);
        assert_eq!(calendar.day(date("2016-03-21")), Day::Bridge);
    }

    #[test]
    fn a_bridge_follows_the_last_bank_holiday_before_it() {
        let holidays = holidays(&["2025-12-25", "2025-12-26", "2026-01-01"]);
        let calendar = Calendar::new(&holidays, Division::EnglandAndWales);
        assert_eq!(calendar.bridge_after(date("2025-12-25")), None);
        assert_eq!(
            calendar
                .bridge_after(date("2025-12-26"))
                .map(|bridge| bridge.booked),
            Some(span("2025-12-29", "2025-12-31")),
        );
        assert_eq!(calendar.bridge_after(date("2026-01-01")), None);
    }

    fn next(holidays: &Holidays, from: &str) -> Option<(NaiveDate, i64, bool)> {
        Calendar::new(holidays, Division::EnglandAndWales)
            .next(date(from))
            .map(|next| (next.holiday.date, next.days, next.starts_four_day_weekend))
    }

    #[test]
    fn the_next_bank_holiday_on_its_own_day_is_zero_days_away() {
        let holidays = holidays(&["2025-12-25"]);
        assert_eq!(
            next(&holidays, "2025-12-25"),
            Some((date("2025-12-25"), 0, false))
        );
    }

    #[test]
    fn the_next_bank_holiday_after_the_last_of_a_year_is_in_the_next_year() {
        let holidays = holidays(&["2025-12-26", "2026-01-01"]);
        assert_eq!(
            next(&holidays, "2025-12-27"),
            Some((date("2026-01-01"), 5, false))
        );
    }

    #[test]
    fn there_is_no_next_bank_holiday_after_the_last_one_in_the_data() {
        let holidays = holidays(&["2025-12-25"]);
        assert_eq!(next(&holidays, "2025-12-26"), None);
    }

    #[test]
    fn only_the_first_bank_holiday_of_a_four_day_weekend_starts_it() {
        let holidays = holidays(&["2022-12-26", "2022-12-27", "2025-07-14"]);
        let starts = |from| next(&holidays, from).is_some_and(|(_, _, starts)| starts);
        assert!(starts("2022-12-26"));
        assert!(!starts("2022-12-27"));
        assert!(!starts("2025-07-14"));
    }

    #[test]
    fn the_last_year_is_bridged_into_the_predicted_year_after_it() {
        let holidays = holidays(&["2025-12-25", "2025-12-26"]);
        let calendar = Calendar::new(&holidays, Division::EnglandAndWales);
        assert_eq!(
            calendar.bridge_after(date("2025-12-26")).cloned(),
            Some(Bridge {
                predicted: true,
                ..bridge(
                    ("2025-12-29", "2025-12-31"),
                    3,
                    ("2025-12-25", "2026-01-01")
                )
            }),
        );
        assert_eq!(calendar.next(date("2025-12-27")), None);
    }

    #[test]
    fn the_predicted_year_after_the_data_has_no_four_day_weekends_or_bridges_of_its_own() {
        let holidays = holidays(&["2010-12-27", "2010-12-28"]);
        let calendar = Calendar::new(&holidays, Division::EnglandAndWales);
        assert_eq!(calendar.day(date("2011-04-23")), Day::Ordinary);
        assert_eq!(calendar.day(date("2011-04-26")), Day::Ordinary);
    }
}
