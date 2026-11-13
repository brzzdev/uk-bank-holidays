use bank_holidays::Calendar;
use chrono::{Datelike as _, NaiveDate};

use crate::FULL_DATE;

/// What the detail panel says about the selected day.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Detail {
    pub(crate) title: String,
    /// The date and how far it is from today.
    pub(crate) when: String,
    pub(crate) lines: Vec<String>,
}

pub(crate) fn detail(calendar: &Calendar, date: NaiveDate, today: NaiveDate) -> Detail {
    let full_date = date.format(FULL_DATE).to_string();
    let countdown = countdown(date, today);
    let when = format!("{full_date} · {countdown}");
    if let Some(holiday) = calendar.holiday(date) {
        let mut lines = Vec::new();
        if holiday.predicted {
            lines.push("Predicted".to_owned());
            lines.push(format!("gov.uk hasn't published {} yet", date.year()));
        }
        if let Some(weekend) = calendar.four_day_weekend(date) {
            lines.push(format!("Four-day weekend, {weekend}"));
        }
        lines.extend(calendar.bridges_joined(date).map(ToString::to_string));
        return Detail {
            title: holiday.name.clone(),
            when,
            lines,
        };
    }
    if let Some(bridge) = calendar.bridge_booked_on(date) {
        return Detail {
            title: "Bridge".to_owned(),
            when,
            lines: vec![bridge.to_string()],
        };
    }
    if let Some(weekend) = calendar.four_day_weekend(date) {
        return Detail {
            title: "Four-day weekend".to_owned(),
            when,
            lines: vec![weekend.to_string()],
        };
    }
    Detail {
        title: full_date,
        when: countdown,
        lines: calendar
            .next(date)
            .map(|next| {
                format!(
                    "Next bank holiday: {}, {}",
                    next.holiday.name,
                    next.holiday.date.format(FULL_DATE)
                )
            })
            .into_iter()
            .collect(),
    }
}

fn countdown(date: NaiveDate, today: NaiveDate) -> String {
    match (date - today).num_days() {
        -1 => "yesterday".to_owned(),
        0 => "today".to_owned(),
        1 => "tomorrow".to_owned(),
        days if days < 0 => format!("{} days ago", -days),
        days => format!("in {days} days"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use bank_holidays::{Division, Holidays, merge, snapshot};

    fn date(s: &str) -> NaiveDate {
        s.parse().expect("test dates are valid")
    }

    fn holidays() -> Holidays {
        merge(&snapshot(), None).with_predictions(2026)
    }

    fn detail_on(holidays: &Holidays, day: &str, today: &str) -> Detail {
        let calendar = Calendar::new(holidays, Division::EnglandAndWales);
        detail(&calendar, date(day), date(today))
    }

    #[test]
    fn a_bridge_day_describes_its_bridge() {
        assert_eq!(
            detail_on(&holidays(), "2025-12-29", "2025-12-20"),
            Detail {
                title: "Bridge".to_owned(),
                when: "Monday 29 December 2025 · in 9 days".to_owned(),
                lines: vec!["Book 3 days (29–31 Dec), get 8 off (25 Dec – 1 Jan)".to_owned()],
            },
        );
    }

    #[test]
    fn a_bank_holiday_gives_its_countdown_four_day_weekend_and_bridge() {
        assert_eq!(
            detail_on(&holidays(), "2025-12-26", "2025-12-25"),
            Detail {
                title: "Boxing Day".to_owned(),
                when: "Friday 26 December 2025 · tomorrow".to_owned(),
                lines: vec![
                    "Four-day weekend, 25–28 Dec".to_owned(),
                    "Book 3 days (29–31 Dec), get 8 off (25 Dec – 1 Jan)".to_owned(),
                ],
            },
        );
    }

    #[test]
    fn a_past_bank_holiday_says_how_long_ago_it_was() {
        assert_eq!(
            detail_on(&holidays(), "2025-08-25", "2025-09-04").when,
            "Monday 25 August 2025 · 10 days ago",
        );
    }

    #[test]
    fn a_predicted_bank_holiday_says_gov_uk_has_not_published_its_year() {
        let holidays = holidays();
        let year = *holidays.years(Division::EnglandAndWales).end();
        let new_year = format!("{year}-01-01");
        let detail = detail_on(&holidays, &new_year, "2026-06-01");
        assert_eq!(detail.title, "New Year’s Day");
        assert_eq!(
            detail.lines[..2],
            [
                "Predicted".to_owned(),
                format!("gov.uk hasn't published {year} yet"),
            ],
        );
    }

    #[test]
    fn a_four_day_weekend_day_describes_its_weekend() {
        assert_eq!(
            detail_on(&holidays(), "2025-04-19", "2025-04-19"),
            Detail {
                title: "Four-day weekend".to_owned(),
                when: "Saturday 19 April 2025 · today".to_owned(),
                lines: vec!["18–21 Apr".to_owned()],
            },
        );
    }

    #[test]
    fn an_ordinary_day_names_the_next_bank_holiday_after_it() {
        assert_eq!(
            detail_on(&holidays(), "2025-12-02", "2025-12-03"),
            Detail {
                title: "Tuesday 2 December 2025".to_owned(),
                when: "yesterday".to_owned(),
                lines: vec![
                    "Next bank holiday: Christmas Day, Thursday 25 December 2025".to_owned()
                ],
            },
        );
    }
}
