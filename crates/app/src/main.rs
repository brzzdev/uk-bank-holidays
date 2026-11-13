mod detail;
mod grid;
mod sidebar;

use std::process;

use bank_holidays::{Calendar, Division, Holidays, merge, snapshot};
use chrono::{Datelike, Local, Months, NaiveDate};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{
    ActiveTheme, Disableable as _, IconName, Sizable as _, Theme, TitleBar, h_flex, v_flex,
};
use gpui_kit::{
    App, AppContext, Bounds, ClickEvent, Context, IntoElement, ParentElement, QuitMode, Render,
    Styled, Window, WindowBounds, WindowOptions, application, assets, div, init, open_window, px,
    size,
};

/// The division tabs in order, with their labels.
const DIVISIONS: [(Division, &str); 3] = [
    (Division::EnglandAndWales, "England and Wales"),
    (Division::Scotland, "Scotland"),
    (Division::NorthernIreland, "Northern Ireland"),
];
/// "Thursday 25 December 2025"
const FULL_DATE: &str = "%A %-d %B %Y";
const WINDOW_HEIGHT: f32 = 600.;
const WINDOW_WIDTH: f32 = 880.;

struct BankHolidays {
    division: Division,
    holidays: Holidays,
    /// The first day of the month the grid shows.
    month: NaiveDate,
    selected: NaiveDate,
}

impl BankHolidays {
    fn new(today: NaiveDate, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Theme::sync_system_appearance(Some(window), cx);
        cx.observe_window_appearance(window, |_, window, cx| {
            Theme::sync_system_appearance(Some(window), cx);
        })
        .detach();
        Self {
            division: Division::EnglandAndWales,
            holidays: merge(&snapshot(), None).with_predictions(today.year()),
            month: first_of_month(today),
            selected: today,
        }
    }

    fn in_range(&self, date: NaiveDate) -> bool {
        self.holidays.years(self.division).contains(&date.year())
    }

    /// Selects a day in the available year range, and shows its month.
    fn select(&mut self, date: NaiveDate) {
        if !self.in_range(date) {
            return;
        }
        self.selected = date;
        self.month = first_of_month(date);
    }

    fn select_on_click(
        date: NaiveDate,
        cx: &mut Context<Self>,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        cx.listener(move |this, _, _, cx| {
            this.select(date);
            cx.notify();
        })
    }

    /// Moves the shown month, greyed when the move leaves the available year range.
    fn step_button(
        &self,
        id: &'static str,
        icon: IconName,
        months: i32,
        cx: &mut Context<Self>,
    ) -> Button {
        Button::new(id)
            .ghost()
            .small()
            .tab_stop(false)
            .icon(icon)
            .disabled(self.stepped_month(months).is_none())
            .on_click(cx.listener(move |this, _, _, cx| {
                if let Some(month) = this.stepped_month(months) {
                    this.month = month;
                    cx.notify();
                }
            }))
    }

    /// The month a step lands on, while it's in the available year range.
    fn stepped_month(&self, months: i32) -> Option<NaiveDate> {
        let step = Months::new(months.unsigned_abs());
        let month = if months < 0 {
            self.month.checked_sub_months(step)
        } else {
            self.month.checked_add_months(step)
        }?;
        self.in_range(month).then_some(month)
    }

    fn title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = DIVISIONS
            .iter()
            .position(|&(division, _)| division == self.division)
            .expect("every division has a tab");
        TitleBar::new().child(div()).child(
            TabBar::new("divisions")
                .segmented()
                .mr_2()
                .selected_index(selected)
                .on_click(cx.listener(|this, &index: &usize, _, cx| {
                    this.division = DIVISIONS[index].0;
                    cx.notify();
                }))
                .children(DIVISIONS.map(|(_, label)| Tab::new().label(label))),
        )
    }
}

impl Render for BankHolidays {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let today = Local::now().date_naive();
        let calendar = Calendar::new(&self.holidays, self.division);
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.title_bar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar(&calendar, today, cx))
                    .child(self.month_pane(&calendar, today, cx)),
            )
    }
}

fn main() {
    application()
        .with_assets(assets::Assets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx| {
            init(cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)),
                    cx,
                ))),
                ..TitleBar::window_options()
            };
            let today = Local::now().date_naive();
            if let Err(error) = open_window(options, cx, |window, cx| {
                cx.new(|cx| BankHolidays::new(today, window, cx))
            }) {
                eprintln!("bank-holidays-app: couldn't open a window: {error}");
                process::exit(1);
            }
        });
}

fn first_of_month(date: NaiveDate) -> NaiveDate {
    date.with_day(1).expect("every month has a first day")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(s: &str) -> NaiveDate {
        s.parse().expect("test dates are valid")
    }

    fn app(selected: &str) -> BankHolidays {
        let selected = date(selected);
        BankHolidays {
            division: Division::EnglandAndWales,
            holidays: merge(&snapshot(), None).with_predictions(2026),
            month: first_of_month(selected),
            selected,
        }
    }

    #[test]
    fn selecting_a_day_shows_its_month() {
        let mut app = app("2025-12-29");
        app.select(date("2026-01-01"));
        assert_eq!(
            (app.selected, app.month),
            (date("2026-01-01"), date("2026-01-01"))
        );
    }

    #[test]
    fn a_day_outside_the_available_years_cannot_be_selected() {
        let mut app = app("2025-12-29");
        let first_year = *app.holidays.years(app.division).start();
        app.select(NaiveDate::from_ymd_opt(first_year - 1, 12, 31).expect("31 Dec exists"));
        assert_eq!(
            (app.selected, app.month),
            (date("2025-12-29"), date("2025-12-01"))
        );
    }

    #[test]
    fn stepping_months_and_years_stops_at_the_ends_of_the_available_range() {
        let mut app = app("2026-01-01");
        let years = app.holidays.years(app.division);
        let (first, last) = (*years.start(), *years.end());
        let month = |year, month| NaiveDate::from_ymd_opt(year, month, 1);
        app.month = month(first, 1).expect("January exists");
        assert_eq!(app.stepped_month(-1), None);
        assert_eq!(app.stepped_month(1), month(first, 2));
        assert_eq!(app.stepped_month(12), month(first + 1, 1));
        app.month = month(last, 12).expect("December exists");
        assert_eq!(app.stepped_month(1), None);
        assert_eq!(app.stepped_month(-12), month(last - 1, 12));
    }
}
