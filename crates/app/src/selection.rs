use bank_holidays::Calendar;
use chrono::{Datelike as _, Months, NaiveDate, TimeDelta};

use crate::first_of_month;

const MONTHS_IN_YEAR: i32 = 12;

/// A change to the selected day.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Move {
    By(TimeDelta),
    /// Keeps the day of the month a run of month moves began on, clamped to shorter months.
    Months(i32),
    NextBankHoliday,
    PreviousBankHoliday,
    To(NaiveDate),
    /// Keeps the date, 29 Feb becoming 28 Feb.
    Years(i32),
}

#[derive(Clone, Copy)]
pub(crate) struct Selection {
    pub(crate) date: NaiveDate,
    month_day: Option<u32>,
}

impl Selection {
    pub(crate) fn new(date: NaiveDate) -> Self {
        Self {
            date,
            month_day: None,
        }
    }

    /// Where a move lands, while it stays in the calendar's years.
    pub(crate) fn moved(self, step: Move, calendar: &Calendar) -> Option<Self> {
        let (date, month_day) = match step {
            Move::By(delta) => (self.date.checked_add_signed(delta)?, None),
            Move::Months(months) => {
                let day = self.month_day.unwrap_or(self.date.day());
                let first = add_months(first_of_month(self.date), months)?;
                let date = first.with_day(day.min(first.num_days_in_month().into()))?;
                (date, Some(day))
            }
            Move::NextBankHoliday => (calendar.holiday_after(self.date)?.date, None),
            Move::PreviousBankHoliday => (calendar.holiday_before(self.date)?.date, None),
            Move::To(date) => (date, None),
            Move::Years(years) => {
                let date = add_months(self.date, years.checked_mul(MONTHS_IN_YEAR)?)?;
                (date, None)
            }
        };
        calendar
            .years()
            .contains(&date.year())
            .then_some(Self { date, month_day })
    }
}

/// Clamps the day to the end of a shorter month.
fn add_months(date: NaiveDate, months: i32) -> Option<NaiveDate> {
    let step = Months::new(months.unsigned_abs());
    if months < 0 {
        date.checked_sub_months(step)
    } else {
        date.checked_add_months(step)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use bank_holidays::{Division, Holidays, merge, snapshot};
    use chrono::Days;

    fn date(s: &str) -> NaiveDate {
        s.parse().expect("test dates are valid")
    }

    fn holidays() -> Holidays {
        merge(&snapshot(), None).with_predictions(2026)
    }

    /// The dates a run of moves lands on from `from`, stopping at the first that can't be made.
    fn walk(holidays: &Holidays, from: NaiveDate, steps: &[Move]) -> Vec<NaiveDate> {
        let calendar = Calendar::new(holidays, Division::EnglandAndWales);
        let mut selection = Selection::new(from);
        steps
            .iter()
            .map_while(|&step| {
                selection = selection.moved(step, &calendar)?;
                Some(selection.date)
            })
            .collect()
    }

    #[test]
    fn day_and_week_moves_cross_month_and_year_ends() {
        assert_eq!(
            walk(
                &holidays(),
                date("2025-12-31"),
                &[
                    Move::By(TimeDelta::days(1)),
                    Move::By(TimeDelta::weeks(-1)),
                    Move::By(TimeDelta::weeks(1)),
                    Move::By(TimeDelta::days(-1)),
                ],
            ),
            [
                date("2026-01-01"),
                date("2025-12-25"),
                date("2026-01-01"),
                date("2025-12-31"),
            ],
        );
    }

    #[test]
    fn month_moves_keep_the_day_they_started_on_clamped_to_shorter_months() {
        assert_eq!(
            walk(
                &holidays(),
                date("2026-01-31"),
                &[Move::Months(1), Move::Months(1), Move::Months(-2)],
            ),
            [date("2026-02-28"), date("2026-03-31"), date("2026-01-31")],
        );
    }

    #[test]
    fn any_other_move_forgets_the_day_month_moves_started_on() {
        assert_eq!(
            walk(
                &holidays(),
                date("2026-01-31"),
                &[
                    Move::Months(1),
                    Move::To(date("2026-02-28")),
                    Move::Months(1),
                ],
            ),
            [date("2026-02-28"), date("2026-02-28"), date("2026-03-28")],
        );
    }

    #[test]
    fn year_moves_keep_the_date_with_29_february_becoming_28_february() {
        assert_eq!(
            walk(
                &holidays(),
                date("2024-02-29"),
                &[Move::Years(1), Move::Years(-1)],
            ),
            [date("2025-02-28"), date("2024-02-28")],
        );
    }

    #[test]
    fn bank_holiday_moves_jump_across_months_and_years() {
        assert_eq!(
            walk(
                &holidays(),
                date("2025-12-25"),
                &[
                    Move::NextBankHoliday,
                    Move::NextBankHoliday,
                    Move::PreviousBankHoliday,
                    Move::PreviousBankHoliday,
                    Move::PreviousBankHoliday,
                ],
            ),
            [
                date("2025-12-26"),
                date("2026-01-01"),
                date("2025-12-26"),
                date("2025-12-25"),
                date("2025-08-25"),
            ],
        );
    }

    #[test]
    fn the_next_bank_holiday_after_a_years_last_is_new_years_day_even_when_predicted() {
        let holidays = holidays();
        let last = *holidays.years(Division::EnglandAndWales).end();
        let year = |year| {
            holidays
                .year(Division::EnglandAndWales, year)
                .expect("every available year has bank holidays")
        };
        let from = year(last - 1)
            .last()
            .expect("a year has bank holidays")
            .date;
        let new_years_day = year(last).first().expect("a year has bank holidays").date;
        assert_eq!(
            walk(&holidays, from, &[Move::NextBankHoliday]),
            [new_years_day],
        );
    }

    #[test]
    fn no_move_leaves_the_available_years() {
        let holidays = holidays();
        let years = holidays.years(Division::EnglandAndWales);
        let first = NaiveDate::from_ymd_opt(*years.start(), 1, 1).expect("1 Jan exists");
        let last = NaiveDate::from_ymd_opt(*years.end(), 12, 31).expect("31 Dec exists");
        for step in [
            Move::By(TimeDelta::days(-1)),
            Move::Months(-1),
            Move::PreviousBankHoliday,
            Move::To(first - Days::new(1)),
            Move::Years(-1),
        ] {
            assert_eq!(walk(&holidays, first, &[step]), [], "{step:?}");
        }
        for step in [
            Move::By(TimeDelta::days(1)),
            Move::Months(1),
            Move::NextBankHoliday,
            Move::To(last + Days::new(1)),
            Move::Years(1),
        ] {
            assert_eq!(walk(&holidays, last, &[step]), [], "{step:?}");
        }
    }
}
