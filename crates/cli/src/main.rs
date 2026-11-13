use std::env;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use bank_holidays::{BankHoliday, Division, Holidays, load, merge, snapshot};
use chrono::{Datelike, Local, NaiveDate};
use lexopt::prelude::*;

const HELP: &str = "\
Show UK bank holidays.

Usage: bank-holidays [OPTIONS] [COMMAND]

Commands:
  next         The next bank holiday and how many days away it is (the default)
  list [YEAR]  The year's bank holidays (the current year by default)

Options:
  -d, --division <DIVISION>  england-and-wales (or ew), scotland, or northern-ireland (or ni);
                             england-and-wales by default
      --json                 Print JSON instead of text
  -h, --help                 Print this help
  -V, --version              Print the version

Bank holiday data from GOV.UK (https://www.gov.uk/bank-holidays). Contains public sector
information licensed under the Open Government Licence v3.0
(https://www.nationalarchives.gov.uk/doc/open-government-licence/version/3/).
Predicted bank holidays are calculated by this tool and are not published by GOV.UK.
";
const NAME: &str = "bank-holidays";
const RUNTIME_FAILURE: u8 = 1;
const USAGE_ERROR: u8 = 2;

/// What a run prints and the code it exits with.
#[derive(Debug)]
struct Outcome {
    code: u8,
    stderr: String,
    stdout: String,
}

impl Outcome {
    fn success(stdout: String) -> Self {
        Self {
            code: 0,
            stderr: String::new(),
            stdout,
        }
    }

    fn failure(code: u8, message: impl std::fmt::Display) -> Self {
        Self {
            code,
            stderr: format!("{NAME}: {message}\n"),
            stdout: String::new(),
        }
    }
}

enum Command {
    Help,
    List(Option<i32>),
    Next,
    Version,
}

struct Options {
    command: Command,
    division: Division,
    format: Format,
}

#[derive(Clone, Copy)]
enum Format {
    Json,
    Text,
}

fn main() -> ExitCode {
    let now = Local::now();
    let today = now.date_naive();
    let holidays = match env::var_os("HOME") {
        Some(home) => load(Path::new(&home), now.into()).holidays,
        None => merge(&snapshot(), None),
    };
    let outcome = run(
        env::args_os().skip(1),
        today,
        &holidays.with_predictions(today.year()),
    );
    let mut code = outcome.code;
    let mut stderr = outcome.stderr;
    if let Err(error) = io::stdout().lock().write_all(outcome.stdout.as_bytes()) {
        stderr = format!("{NAME}: couldn't write to stdout: {error}\n{stderr}");
        code = RUNTIME_FAILURE;
    }
    // A failed write to stderr leaves nowhere to report it, and the exit code already says why.
    let _ = io::stderr().lock().write_all(stderr.as_bytes());
    ExitCode::from(code)
}

fn run(
    args: impl IntoIterator<Item: Into<OsString>>,
    today: NaiveDate,
    holidays: &Holidays,
) -> Outcome {
    let options = match parse(args) {
        Ok(options) => options,
        Err(error) => return Outcome::failure(USAGE_ERROR, error),
    };
    let division = options.division;
    match options.command {
        Command::Help => Outcome::success(HELP.to_owned()),
        Command::List(year) => {
            let year = year.unwrap_or(today.year());
            let Some(year_holidays) = holidays.year(division, year) else {
                let have = holidays.years(division);
                return Outcome::failure(
                    USAGE_ERROR,
                    format!(
                        "no bank holidays for {year} (have {}–{})",
                        have.start(),
                        have.end()
                    ),
                );
            };
            Outcome::success(match options.format {
                Format::Json => pretty(
                    &year_holidays
                        .iter()
                        .map(|holiday| to_json(holiday, division))
                        .collect(),
                ),
                Format::Text => year_holidays
                    .iter()
                    .fold(String::new(), |mut text, holiday| {
                        let _ = writeln!(
                            text,
                            "{}  {}{}",
                            holiday.date.format("%a %e %b"),
                            holiday.name,
                            predicted_marker(holiday)
                        );
                        text
                    }),
            })
        }
        Command::Next => {
            let Some(next) = holidays.next(division, today) else {
                return Outcome::failure(
                    RUNTIME_FAILURE,
                    format!("no bank holidays on or after {today}"),
                );
            };
            Outcome::success(match options.format {
                Format::Json => pretty(&to_json(next.holiday, division)),
                Format::Text => {
                    let countdown = match next.days {
                        0 => "today".to_owned(),
                        1 => "in 1 day".to_owned(),
                        days => format!("in {days} days"),
                    };
                    format!(
                        "{}{}, {}, {countdown}\n",
                        next.holiday.name,
                        predicted_marker(next.holiday),
                        next.holiday.date.format("%A %-d %B %Y"),
                    )
                }
            })
        }
        Command::Version => Outcome::success(format!("{NAME} {}\n", env!("CARGO_PKG_VERSION"))),
    }
}

fn parse(args: impl IntoIterator<Item: Into<OsString>>) -> Result<Options, lexopt::Error> {
    let mut command = None;
    let mut division = Division::EnglandAndWales;
    let mut format = Format::Text;
    let mut parser = lexopt::Parser::from_args(args);
    while let Some(arg) = parser.next()? {
        match arg {
            Short('d') | Long("division") => division = parser.value()?.parse()?,
            Short('h') | Long("help") => {
                command = Some(Command::Help);
                break;
            }
            Long("json") => format = Format::Json,
            Short('V') | Long("version") => {
                command = Some(Command::Version);
                break;
            }
            Value(value) if command.is_none() && value == "list" => {
                command = Some(Command::List(None));
            }
            Value(value) if command.is_none() && value == "next" => command = Some(Command::Next),
            Value(value) if matches!(command, Some(Command::List(None))) => {
                command = Some(Command::List(Some(value.parse()?)));
            }
            _ => return Err(arg.unexpected()),
        }
    }
    Ok(Options {
        command: command.unwrap_or(Command::Next),
        division,
        format,
    })
}

fn predicted_marker(holiday: &BankHoliday) -> &'static str {
    if holiday.predicted {
        " (predicted)"
    } else {
        ""
    }
}

fn to_json(holiday: &BankHoliday, division: Division) -> serde_json::Value {
    serde_json::json!({
        "date": holiday.date,
        "division": division.key(),
        "name": holiday.name,
        "predicted": holiday.predicted,
    })
}

fn pretty(value: &serde_json::Value) -> String {
    let mut json = serde_json::to_string_pretty(value).expect("JSON values always serialise");
    json.push('\n');
    json
}

#[cfg(test)]
mod tests {
    use super::*;

    use bank_holidays::{DatedFeed, Feed};

    /// Bank holidays published for 2025 and 2026, with Northern Ireland's own two in 2025, and
    /// predicted ones from 2027.
    fn fixture(current_year: i32) -> Holidays {
        fn event(date: &str, title: &str) -> serde_json::Value {
            serde_json::json!({"title": title, "date": date, "notes": "", "bunting": true})
        }
        let shared = [
            event("2025-12-25", "Christmas Day"),
            event("2025-12-26", "Boxing Day"),
            event("2026-01-01", "New Year’s Day"),
            event("2026-12-25", "Christmas Day"),
            event("2026-12-28", "Boxing Day"),
        ];
        let mut northern_ireland = vec![
            event("2025-03-17", "St Patrick’s Day"),
            event("2025-07-14", "Battle of the Boyne (Orangemen’s Day)"),
        ];
        northern_ireland.extend(shared.clone());
        let json = serde_json::json!({
            "england-and-wales": {"events": shared},
            "northern-ireland": {"events": northern_ireland},
            "scotland": {"events": shared},
        });
        let feed = Feed::parse(json.to_string().as_bytes()).expect("the fixture is a valid feed");
        let as_of = "2025-01-01".parse().expect("the fixture date is valid");
        merge(&DatedFeed { as_of, feed }, None).with_predictions(current_year)
    }

    fn run_on(today: &str, args: &[&str]) -> Outcome {
        let today = today.parse::<NaiveDate>().expect("test dates are valid");
        run(args.iter().copied(), today, &fixture(today.year()))
    }

    fn parsed(outcome: &Outcome) -> serde_json::Value {
        serde_json::from_str(&outcome.stdout).expect("--json prints JSON")
    }

    #[test]
    fn with_no_arguments_it_prints_the_next_england_and_wales_bank_holiday_and_its_countdown() {
        let outcome = run_on("2025-12-20", &[]);
        assert_eq!(outcome.code, 0);
        assert_eq!(
            outcome.stdout,
            "Christmas Day, Thursday 25 December 2025, in 5 days\n"
        );
        assert_eq!(run_on("2025-12-20", &["next"]).stdout, outcome.stdout);
    }

    #[test]
    fn a_bank_holiday_today_is_today() {
        assert_eq!(
            run_on("2025-12-25", &[]).stdout,
            "Christmas Day, Thursday 25 December 2025, today\n",
        );
    }

    #[test]
    fn northern_ireland_lists_its_own_bank_holidays_with_their_gov_uk_names() {
        let outcome = run_on("2025-01-01", &["-d", "ni", "list", "2025"]);
        assert_eq!(outcome.code, 0);
        assert_eq!(
            outcome.stdout,
            "Mon 17 Mar  St Patrick’s Day\n\
             Mon 14 Jul  Battle of the Boyne (Orangemen’s Day)\n\
             Thu 25 Dec  Christmas Day\n\
             Fri 26 Dec  Boxing Day\n",
        );
    }

    #[test]
    fn list_defaults_to_the_current_year() {
        let outcome = run_on("2026-06-01", &["list"]);
        assert!(outcome.stdout.starts_with("Thu  1 Jan  New Year’s Day\n"));
        assert!(outcome.stdout.ends_with("Mon 28 Dec  Boxing Day\n"));
    }

    #[test]
    fn list_json_is_an_array_of_date_name_and_division() {
        let outcome = run_on(
            "2025-01-01",
            &["list", "--json", "--division", "northern-ireland"],
        );
        let first = &parsed(&outcome)[0];
        assert_eq!(
            *first,
            serde_json::json!({
                "date": "2025-03-17",
                "division": "northern-ireland",
                "name": "St Patrick’s Day",
                "predicted": false,
            }),
        );
    }

    #[test]
    fn next_json_is_one_object() {
        let outcome = run_on("2025-12-20", &["next", "--json"]);
        assert_eq!(
            parsed(&outcome),
            serde_json::json!({
                "date": "2025-12-25",
                "division": "england-and-wales",
                "name": "Christmas Day",
                "predicted": false,
            }),
        );
    }

    #[test]
    fn a_year_outside_the_data_is_a_usage_error_naming_the_years_there_are() {
        let outcome = run_on("2025-01-01", &["list", "1999"]);
        assert_eq!(
            (
                outcome.code,
                outcome.stdout.as_str(),
                outcome.stderr.as_str()
            ),
            (
                2,
                "",
                "bank-holidays: no bank holidays for 1999 (have 2025–2028)\n"
            ),
        );
    }

    #[test]
    fn list_marks_each_predicted_bank_holiday() {
        let outcome = run_on("2025-01-01", &["list", "2027"]);
        assert_eq!(outcome.code, 0);
        assert_eq!(
            outcome.stdout,
            "Fri  1 Jan  New Year’s Day (predicted)\n\
             Fri 26 Mar  Good Friday (predicted)\n\
             Mon 29 Mar  Easter Monday (predicted)\n\
             Mon  3 May  Early May bank holiday (predicted)\n\
             Mon 31 May  Spring bank holiday (predicted)\n\
             Mon 30 Aug  Summer bank holiday (predicted)\n\
             Mon 27 Dec  Christmas Day (substitute day) (predicted)\n\
             Tue 28 Dec  Boxing Day (substitute day) (predicted)\n",
        );
    }

    #[test]
    fn list_json_says_whether_each_bank_holiday_is_predicted() {
        let predicted = |year| {
            parsed(&run_on("2025-01-01", &["list", year, "--json"]))
                .as_array()
                .expect("list prints an array")
                .iter()
                .map(|holiday| holiday["predicted"].clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(predicted("2026"), [false; 3]);
        assert_eq!(predicted("2027"), [true; 8]);
    }

    #[test]
    fn next_can_land_on_a_predicted_bank_holiday() {
        assert_eq!(
            run_on("2026-12-29", &[]).stdout,
            "New Year’s Day (predicted), Friday 1 January 2027, in 3 days\n",
        );
    }

    #[test]
    fn the_year_range_runs_two_years_past_the_current_year_when_it_is_past_the_data() {
        let outcome = run_on("2028-06-01", &["list", "2031"]);
        assert_eq!(
            outcome.stderr,
            "bank-holidays: no bank holidays for 2031 (have 2025–2030)\n"
        );
    }

    #[test]
    fn an_unknown_division_flag_command_or_year_is_a_usage_error() {
        for args in [
            &["-d", "wales"][..],
            &["--bogus"],
            &["list", "nineteen"],
            &["later"],
        ] {
            let outcome = run_on("2025-01-01", args);
            assert_eq!(outcome.code, USAGE_ERROR, "{args:?}");
            assert!(outcome.stdout.is_empty(), "{args:?}");
            assert!(outcome.stderr.starts_with("bank-holidays: "), "{args:?}");
        }
    }

    #[test]
    fn version_prints_the_name_and_version() {
        let outcome = run_on("2025-01-01", &["-V"]);
        assert_eq!(
            outcome.stdout,
            format!("bank-holidays {}\n", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn help_ends_with_the_gov_uk_attribution() {
        let outcome = run_on("2025-01-01", &["--help"]);
        assert_eq!(outcome.code, 0);
        assert!(outcome.stdout.ends_with(
            "https://www.nationalarchives.gov.uk/doc/open-government-licence/version/3/).\n\
             Predicted bank holidays are calculated by this tool and are not published by GOV.UK.\n"
        ));
    }

    #[test]
    fn a_day_after_the_last_predicted_bank_holiday_is_a_runtime_failure() {
        let today = "2028-12-29".parse().expect("test dates are valid");
        let outcome = run(std::iter::empty::<&str>(), today, &fixture(2025));
        assert_eq!(outcome.code, RUNTIME_FAILURE);
        assert!(outcome.stderr.starts_with("bank-holidays: "));
    }
}
