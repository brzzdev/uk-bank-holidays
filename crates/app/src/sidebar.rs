use bank_holidays::{BankHoliday, Calendar, Next};
use chrono::{Datelike as _, NaiveDate, TimeDelta};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::{ActiveTheme, IconName, StyledExt as _, Theme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Context, Div, Hsla, InteractiveElement as _, IntoElement, ParentElement,
    StatefulInteractiveElement as _, Styled, div, px,
};

use crate::selection::Move;
use crate::{BankHolidays, FULL_DATE, SHORT_DATE, divider};

const DATE_COLUMN_WIDTH: f32 = 52.;
const DOT_SIZE: f32 = 7.;
/// "25 Dec 2025"
const SHORT_DATE_WITH_YEAR: &str = "%-d %b %Y";
const SIDEBAR_WIDTH: f32 = 270.;
/// The age past which the data counts as gov.uk being unreachable.
const UNREACHABLE_AFTER: TimeDelta = TimeDelta::days(30);
const YEAR_WIDTH: f32 = 40.;

/// The one mark a row of the list carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Dot {
    Bridge,
    FourDayWeekend,
    Predicted,
}

/// How a row of the list is drawn.
struct RowLook {
    dot: Option<Dot>,
    past: bool,
    selected: bool,
}

impl BankHolidays {
    pub(crate) fn sidebar(
        &self,
        calendar: &Calendar,
        today: NaiveDate,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.selection.date;
        let holidays = self
            .holidays
            .year(self.division, selected.year())
            .unwrap_or_default();
        v_flex()
            .w(px(SIDEBAR_WIDTH))
            .flex_none()
            .h_full()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(divider(cx.theme()))
            .children(calendar.next(today).map(|next| next_holiday(&next, cx)))
            .child(self.year_stepper(calendar, cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_flex()
                            .id("holidays")
                            .track_scroll(&self.holiday_list)
                            .size_full()
                            .p_1p5()
                            .gap_px()
                            .overflow_y_scroll()
                            .children(holidays.iter().enumerate().map(|(index, holiday)| {
                                let look = RowLook {
                                    dot: dot(calendar, holiday),
                                    past: holiday.date < today,
                                    selected: holiday.date == selected,
                                };
                                row(holiday, &look, cx)
                                    .id(("holiday", index))
                                    .on_click(Self::select_on_click(holiday.date, cx))
                            })),
                    )
                    .vertical_scrollbar(&self.holiday_list),
            )
            .children(unreachable_note(self.holidays.as_of(), today).map(|note| {
                div()
                    .p_3()
                    .border_t_1()
                    .border_color(divider(cx.theme()))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(note)
            }))
    }

    fn year_stepper(&self, calendar: &Calendar, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .justify_center()
            .gap_1()
            .p_1p5()
            .border_b_1()
            .border_color(divider(cx.theme()))
            .child(self.step_button(
                "previous-year",
                IconName::ChevronLeft,
                Move::Years(-1),
                calendar,
                cx,
            ))
            .child(
                div()
                    .min_w(px(YEAR_WIDTH))
                    .text_center()
                    .font_semibold()
                    .child(self.selection.date.year().to_string()),
            )
            .child(self.step_button(
                "next-year",
                IconName::ChevronRight,
                Move::Years(1),
                calendar,
                cx,
            ))
    }
}

fn unreachable_note(as_of: NaiveDate, today: NaiveDate) -> Option<String> {
    if today - as_of <= UNREACHABLE_AFTER {
        return None;
    }
    let format = if as_of.year() == today.year() {
        SHORT_DATE
    } else {
        SHORT_DATE_WITH_YEAR
    };
    Some(format!(
        "Couldn't reach gov.uk since {}",
        as_of.format(format)
    ))
}

fn dot(calendar: &Calendar, holiday: &BankHoliday) -> Option<Dot> {
    if holiday.predicted {
        return Some(Dot::Predicted);
    }
    if calendar.four_day_weekend(holiday.date).is_some() {
        return Some(Dot::FourDayWeekend);
    }
    if calendar.bridges_joined(holiday.date).next().is_some() {
        return Some(Dot::Bridge);
    }
    None
}

/// A dashed dot takes the row's text colour.
fn dot_mark(dot: Option<Dot>, text: Hsla, theme: &Theme) -> Div {
    let mark = div().flex_none().size(px(DOT_SIZE)).rounded_full();
    match dot {
        None => mark,
        Some(Dot::Bridge) => mark.bg(theme.warning),
        Some(Dot::FourDayWeekend) => mark.bg(theme.red),
        Some(Dot::Predicted) => mark.border_1().border_dashed().border_color(text),
    }
}

fn next_holiday(next: &Next, cx: &App) -> Div {
    let theme = cx.theme();
    let caption = match next.days {
        0 => "today",
        1 => "day to go",
        _ => "days to go",
    };
    h_flex()
        .gap_2()
        .p_3()
        .border_b_1()
        .border_color(divider(theme))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(div().text_xs().font_semibold().child("Next bank holiday"))
                .child(
                    div()
                        .text_base()
                        .font_semibold()
                        .text_color(theme.foreground)
                        .child(next.holiday.name.clone()),
                )
                .child(next.holiday.date.format(FULL_DATE).to_string())
                .children(next_marks(next)),
        )
        .child(
            v_flex()
                .flex_none()
                .items_end()
                .child(
                    div()
                        .text_3xl()
                        .font_bold()
                        .text_color(theme.red)
                        .child(next.days.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(caption),
                ),
        )
}

fn next_marks(next: &Next) -> Option<&'static str> {
    match (next.starts_four_day_weekend, next.holiday.predicted) {
        (false, false) => None,
        (false, true) => Some("Predicted"),
        (true, false) => Some("Four-day weekend"),
        (true, true) => Some("Four-day weekend · Predicted"),
    }
}

fn row(holiday: &BankHoliday, look: &RowLook, cx: &App) -> Div {
    let theme = cx.theme();
    let colour = if look.past {
        theme.muted_foreground
    } else {
        theme.foreground
    };
    h_flex()
        .gap_2()
        .px_2()
        .py_1p5()
        .rounded_md()
        .text_sm()
        .text_color(colour)
        .when(look.selected, |row| row.bg(theme.sidebar_accent))
        .child(
            div()
                .w(px(DATE_COLUMN_WIDTH))
                .flex_none()
                .text_color(theme.muted_foreground)
                .child(holiday.date.format(SHORT_DATE).to_string()),
        )
        .child(div().flex_1().min_w_0().child(holiday.name.clone()))
        .child(dot_mark(look.dot, colour, theme))
}

#[cfg(test)]
mod tests {
    use super::*;

    use bank_holidays::{Division, merge, snapshot};

    fn date(s: &str) -> NaiveDate {
        s.parse().expect("test dates are valid")
    }

    #[test]
    fn the_unreachable_note_shows_once_the_data_is_more_than_30_days_old() {
        let as_of = date("2026-10-01");
        assert_eq!(unreachable_note(as_of, date("2026-10-31")), None);
        assert_eq!(
            unreachable_note(as_of, date("2026-11-01")).as_deref(),
            Some("Couldn't reach gov.uk since 1 Oct"),
        );
    }

    #[test]
    fn the_unreachable_note_names_the_year_once_it_differs_from_this_one() {
        assert_eq!(
            unreachable_note(date("2025-10-01"), date("2026-01-15")).as_deref(),
            Some("Couldn't reach gov.uk since 1 Oct 2025"),
        );
    }

    #[test]
    fn a_row_is_dotted_for_a_prediction_before_a_four_day_weekend_before_a_bridge() {
        let holidays = merge(&snapshot(), None).with_predictions(2026);
        let division = Division::EnglandAndWales;
        let calendar = Calendar::new(&holidays, division);
        let year = |year| holidays.year(division, year).unwrap_or_default();
        let dot_on = |day: &str| {
            let date = date(day);
            let holiday = year(date.year())
                .iter()
                .find(|holiday| holiday.date == date)
                .expect("each test date is a bank holiday");
            dot(&calendar, holiday)
        };
        assert_eq!(dot_on("2025-05-05"), None);
        assert_eq!(dot_on("2025-12-25"), Some(Dot::FourDayWeekend));
        assert_eq!(dot_on("2026-01-01"), Some(Dot::Bridge));
        assert!(
            year(*holidays.years(division).end())
                .iter()
                .all(|holiday| dot(&calendar, holiday) == Some(Dot::Predicted))
        );
    }
}
