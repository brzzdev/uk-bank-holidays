use chrono::{Datelike, Days, NaiveDate, Weekday};

use crate::{BankHoliday, Division};

const BATTLE_OF_THE_BOYNE: &str = "Battle of the Boyne (Orangemen’s Day)";
const BOXING_DAY: &str = "Boxing Day";
const CHRISTMAS_DAY: &str = "Christmas Day";
const EARLY_MAY: &str = "Early May bank holiday";
const EASTER_MONDAY: &str = "Easter Monday";
const GOOD_FRIDAY: &str = "Good Friday";
const NEW_YEARS_DAY: &str = "New Year’s Day";
const SECOND_JANUARY: &str = "2nd January";
const SPRING: &str = "Spring bank holiday";
const ST_ANDREWS_DAY: &str = "St Andrew’s Day";
const ST_PATRICKS_DAY: &str = "St Patrick’s Day";
const SUMMER: &str = "Summer bank holiday";

/// How a pair of fixed days starting on a Sunday is named; the dates are Monday and Tuesday either
/// way.
#[derive(Clone, Copy)]
enum SundayStart {
    /// Both are substitutes, named in date order: Scotland's New Year pair, as gov.uk gave 2023.
    InDateOrder,
    /// The second day keeps its own Monday and the first moves to Tuesday: Christmas, as in 2022.
    SecondKeepsItsDay,
}

/// The bank holidays the usual rules give a division in a year, in date order.
pub(crate) fn predict(year: i32, division: Division) -> Vec<BankHoliday> {
    let easter = easter_sunday(year);
    let mut holidays = Vec::new();
    if division == Division::Scotland {
        holidays.extend(pair(
            date(year, 1, 1),
            [NEW_YEARS_DAY, SECOND_JANUARY],
            SundayStart::InDateOrder,
        ));
    } else {
        holidays.push(single(date(year, 1, 1), NEW_YEARS_DAY));
    }
    if division == Division::NorthernIreland {
        holidays.push(single(date(year, 3, 17), ST_PATRICKS_DAY));
    }
    holidays.push(BankHoliday::predicted(easter - Days::new(2), GOOD_FRIDAY));
    if division != Division::Scotland {
        holidays.push(BankHoliday::predicted(easter + Days::new(1), EASTER_MONDAY));
    }
    holidays.push(BankHoliday::predicted(first_monday(year, 5), EARLY_MAY));
    holidays.push(BankHoliday::predicted(
        monday_on_or_before(date(year, 5, 31)),
        SPRING,
    ));
    if division == Division::NorthernIreland {
        holidays.push(single(date(year, 7, 12), BATTLE_OF_THE_BOYNE));
    }
    let summer = if division == Division::Scotland {
        first_monday(year, 8)
    } else {
        monday_on_or_before(date(year, 8, 31))
    };
    holidays.push(BankHoliday::predicted(summer, SUMMER));
    if division == Division::Scotland {
        holidays.push(single(date(year, 11, 30), ST_ANDREWS_DAY));
    }
    holidays.extend(pair(
        date(year, 12, 25),
        [CHRISTMAS_DAY, BOXING_DAY],
        SundayStart::SecondKeepsItsDay,
    ));
    holidays.sort_by_key(|holiday| holiday.date);
    holidays
}

/// Easter Sunday by the anonymous Gregorian computus (Meeus/Jones/Butcher).
fn easter_sunday(year: i32) -> NaiveDate {
    let golden_number = year % 19;
    let century = year / 100;
    let year_of_century = year % 100;
    let solar_correction = century / 4;
    let solar_remainder = century % 4;
    let lunar_correction = (century - (century + 8) / 25 + 1) / 3;
    let paschal_full_moon =
        (19 * golden_number + century - solar_correction - lunar_correction + 15) % 30;
    let days_to_sunday = (32 + 2 * solar_remainder + 2 * (year_of_century / 4)
        - paschal_full_moon
        - year_of_century % 4)
        % 7;
    let late_correction = (golden_number + 11 * paschal_full_moon + 22 * days_to_sunday) / 451;
    let offset = paschal_full_moon + days_to_sunday - 7 * late_correction + 114;
    date(
        year,
        (offset / 31)
            .try_into()
            .expect("the computus gives March or April"),
        (offset % 31 + 1)
            .try_into()
            .expect("the computus gives a day of the month"),
    )
}

/// A single fixed day, moved to the next Monday when it falls on a weekend.
fn single(day: NaiveDate, title: &str) -> BankHoliday {
    match day.weekday() {
        Weekday::Sat | Weekday::Sun => {
            let monday = day + Days::new(u64::from(Weekday::Mon.days_since(day.weekday())));
            BankHoliday::predicted_substitute(monday, title)
        }
        _ => BankHoliday::predicted(day, title),
    }
}

/// Two consecutive fixed days, each moved off a weekend to the next weekday not already taken.
fn pair(
    first: NaiveDate,
    [first_title, second_title]: [&str; 2],
    sunday: SundayStart,
) -> [BankHoliday; 2] {
    let day = |offset| first + Days::new(offset);
    match (first.weekday(), sunday) {
        (Weekday::Fri, _) => [
            BankHoliday::predicted(first, first_title),
            BankHoliday::predicted_substitute(day(3), second_title),
        ],
        (Weekday::Sat, _) => [
            BankHoliday::predicted_substitute(day(2), first_title),
            BankHoliday::predicted_substitute(day(3), second_title),
        ],
        (Weekday::Sun, SundayStart::InDateOrder) => [
            BankHoliday::predicted_substitute(day(1), first_title),
            BankHoliday::predicted_substitute(day(2), second_title),
        ],
        (Weekday::Sun, SundayStart::SecondKeepsItsDay) => [
            BankHoliday::predicted(day(1), second_title),
            BankHoliday::predicted_substitute(day(2), first_title),
        ],
        _ => [
            BankHoliday::predicted(first, first_title),
            BankHoliday::predicted(day(1), second_title),
        ],
    }
}

fn first_monday(year: i32, month: u32) -> NaiveDate {
    NaiveDate::from_weekday_of_month_opt(year, month, Weekday::Mon, 1)
        .expect("every month has a first Monday")
}

fn monday_on_or_before(day: NaiveDate) -> NaiveDate {
    day - Days::new(u64::from(day.weekday().num_days_from_monday()))
}

fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).expect("the rules name real dates")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::holidays::by_year;
    use crate::snapshot;

    /// Dates the feed has, or lacks, only because a proclamation moved or added a day.
    const PROCLAIMED_ONE_OFFS: [(&str, &[Division]); 8] = [
        ("2020-05-04", &Division::ALL),        // early May moved to VE Day…
        ("2020-05-08", &Division::ALL),        // …here
        ("2022-05-30", &Division::ALL),        // spring moved for the Platinum Jubilee…
        ("2022-06-02", &Division::ALL),        // …here
        ("2022-06-03", &Division::ALL),        // Platinum Jubilee
        ("2022-09-19", &Division::ALL),        // State Funeral of Queen Elizabeth II
        ("2023-05-08", &Division::ALL),        // coronation of King Charles III
        ("2026-06-15", &[Division::Scotland]), // World Cup
    ];

    #[test]
    fn the_rules_reproduce_every_published_bank_holiday_but_the_proclaimed_one_offs() {
        let one_offs: Vec<(NaiveDate, &[Division])> = PROCLAIMED_ONE_OFFS
            .iter()
            .map(|&(date, divisions)| (date.parse().expect("one-off dates are valid"), divisions))
            .collect();
        let feed = snapshot().feed;
        for division in Division::ALL {
            for (year, published) in by_year(&feed, division) {
                let as_set = |holidays: Vec<BankHoliday>| -> BTreeSet<_> {
                    holidays
                        .into_iter()
                        .map(|holiday| (holiday.date, holiday.name))
                        .collect()
                };
                let published = as_set(published);
                let predicted = as_set(predict(year, division));
                let differences: Vec<_> = published
                    .symmetric_difference(&predicted)
                    .filter(|(date, _)| {
                        !one_offs.iter().any(|(one_off, divisions)| {
                            one_off == date && divisions.contains(&division)
                        })
                    })
                    .collect();
                assert!(
                    differences.is_empty(),
                    "{division:?} {year}: {differences:?}"
                );
            }
        }
    }
}
